/* Patrone's sign-in for Hyper-V's enhanced session, for FreeRDP's proxy
 * (patrone-remote-desktop).
 *
 * An enhanced session is Remote Desktop over a Hyper-V socket, vsock port
 * 3389, where a proxy of its own listens (vmconnect.ini). Only the Hyper-V
 * host can connect there, and VMConnect brings no username or password: the
 * guest shows a login screen in its window, as Windows guests do. So this
 * plugin connects each new connection to Patrone's login screen (the greeter,
 * on a local socket) first. Once the greeter has had the password checked
 * with PAM, it sends a Server Redirection PDU with the account, the port of
 * the account's own GNOME Remote Desktop and the secret that opens it. The
 * proxy's own client takes that, VMConnect never sees it, and this plugin
 * points the client at that desktop on localhost instead, signing in with the
 * secret. VMConnect stays connected throughout; once the desktop is up, its
 * side of the proxy is brought up to date with it, as the proxy does after
 * any first connection.
 *
 * Copyright 2026 William Daugherty
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

#define _GNU_SOURCE
#include <linux/vm_sockets.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/socket.h>

#include <freerdp/freerdp.h>
#include <freerdp/peer.h>
#include <freerdp/server/proxy/proxy_config.h>
#include <freerdp/server/proxy/proxy_context.h>
#include <freerdp/server/proxy/proxy_modules_api.h>
#include <freerdp/settings.h>
#include <winpr/string.h>
#include <winpr/synch.h>
#include <winpr/wlog.h>

#define TAG MODULE_TAG("patrone-vmconnect")

#define DEFAULT_GREETER "/run/patrone-remote-desktop/greeter.sock"
#define USER_MAX 32
#define SECRET_MAX 128

static const char plugin_name[] = "patrone-vmconnect";
static const char plugin_desc[] =
    "Shows Hyper-V's enhanced session Patrone's login screen, then the account's own desktop";

/* One connection: first the login screen, then the account's desktop */
typedef struct
{
	BOOL at_desktop;
	char user[USER_MAX + 1];
} Session;

static Session* get_session(proxyPlugin* plugin, proxyData* pdata)
{
	return plugin->mgr->GetPluginData(plugin->mgr, plugin_name, pdata);
}

/* A yes-or-no setting of the plugin's own, from its section of the ini */
static BOOL config_true(proxyData* pdata, const char* key, BOOL otherwise)
{
	const char* value = pf_config_get(pdata->config, plugin_name, key);
	if (!value)
		return otherwise;
	return strcmp(value, "true") == 0;
}

/* The address the connection comes from. FreeRDP has handed the socket to its
 * transport by now (peer->sockfd is -1), so it's found through the
 * transport's event, which watches that socket. */
static BOOL peer_address(freerdp_peer* peer, struct sockaddr_storage* address)
{
	HANDLE events[MAXIMUM_WAIT_OBJECTS] = { 0 };
	const DWORD count = peer->GetEventHandles(peer, events, MAXIMUM_WAIT_OBJECTS);

	for (DWORD i = 0; i < count; i++)
	{
		socklen_t size = sizeof(*address);
		const int fd = GetEventFileDescriptor(events[i]);
		if (fd >= 0 && getpeername(fd, (struct sockaddr*)address, &size) == 0)
			return TRUE;
	}
	return FALSE;
}

/* Only the Hyper-V host may connect: VMConnect's connections come through it.
 * The host handles VMConnect's own security (TLS, and NLA with the host's
 * account), and the guest's side runs without it: FreeRDP's VMConnect mode.
 * Tests run over TCP on localhost (AllowLocal). */
static BOOL session_initialize(proxyPlugin* plugin, proxyData* pdata, void* param)
{
	WINPR_UNUSED(plugin);
	freerdp_peer* peer = param;
	struct sockaddr_storage address = { 0 };
	const BOOL found = peer_address(peer, &address);
	const struct sockaddr_vm* vsock = (const struct sockaddr_vm*)&address;

	if (found && address.ss_family == AF_VSOCK && vsock->svm_cid == VMADDR_CID_HOST)
	{
		WLog_INFO(TAG, "a connection from the Hyper-V host");
		return !config_true(pdata, "HostHandlesSecurity", TRUE) ||
		       freerdp_settings_set_bool(peer->context->settings, FreeRDP_VmConnectMode, TRUE);
	}
	if (config_true(pdata, "AllowLocal", FALSE))
		return TRUE;
	if (!found)
		WLog_WARN(TAG, "refused a connection whose address couldn't be read");
	else if (address.ss_family == AF_VSOCK)
		WLog_WARN(TAG, "refused a connection from vsock address %u, not the Hyper-V host",
		          vsock->svm_cid);
	else
		WLog_WARN(TAG, "refused a connection that didn't come over a Hyper-V socket");
	return FALSE;
}

/* Every connection starts at the login screen */
static BOOL fetch_target(proxyPlugin* plugin, proxyData* pdata, void* param)
{
	proxyFetchTargetEventInfo* event = param;
	const char* greeter = pf_config_get(pdata->config, plugin_name, "Greeter");

	Session* session = calloc(1, sizeof(*session));
	if (!session || !plugin->mgr->SetPluginData(plugin->mgr, plugin_name, pdata, session))
	{
		free(session);
		return FALSE;
	}
	event->fetch_method = PROXY_FETCH_TARGET_USE_CUSTOM_ADDR;
	event->target_address = strdup(greeter ? greeter : DEFAULT_GREETER); /* the proxy frees it */
	event->target_port = 3389; /* a local socket's path has no port; any will do */
	return event->target_address != NULL;
}

/* The login screen: TLS on its local socket, and no account to sign in with.
 * The proxy's client starts as a copy of VMConnect's side, VMConnect mode
 * and all, and that mode would have it negotiate a second time after NLA, as
 * with Hyper-V's host, which GNOME Remote Desktop doesn't: off. */
static BOOL client_pre_connect(proxyPlugin* plugin, proxyData* pdata, void* param)
{
	rdpContext* context = param;
	rdpSettings* settings = context->settings;
	Session* session = get_session(plugin, pdata);

	if (!session || session->at_desktop)
		return FALSE;
	return freerdp_settings_set_bool(settings, FreeRDP_VmConnectMode, FALSE) &&
	       freerdp_settings_set_bool(settings, FreeRDP_NlaSecurity, FALSE) &&
	       freerdp_settings_set_bool(settings, FreeRDP_TlsSecurity, TRUE) &&
	       freerdp_settings_set_bool(settings, FreeRDP_RdpSecurity, FALSE) &&
	       freerdp_settings_set_bool(settings, FreeRDP_ExtSecurity, FALSE) &&
	       freerdp_settings_set_string(settings, FreeRDP_Username, NULL) &&
	       freerdp_settings_set_string(settings, FreeRDP_Password, NULL) &&
	       freerdp_settings_set_string(settings, FreeRDP_Domain, NULL);
}

/* The connection whose client is on its way to the desktop. Each connection's
 * client has a thread of its own, which runs the redirection and then the
 * desktop's activation. */
static __thread proxyData* redirected;

/* Once the desktop is up, VMConnect's side takes its settings, as after the
 * proxy's first connection (pf_client_post_connect, which a redirection
 * doesn't run again): a reactivation, which a desktop resize sets off. */
static void on_desktop_activated(void* context, const ActivatedEventArgs* event)
{
	rdpContext* back = context;
	proxyData* pdata = redirected;
	WINPR_UNUSED(event);

	redirected = NULL;
	PubSub_UnsubscribeActivated(back->pubSub, on_desktop_activated);
	if (!pdata || pdata->pc != back || !pf_context_copy_settings(pdata->ps->settings, back->settings) ||
	    !pdata->ps->update->DesktopResize(pdata->ps))
		WLog_WARN(TAG, "couldn't bring VMConnect's side up to date with the desktop");
}

/* The login screen's redirection: the account, its desktop's port (as the
 * load-balancing info) and the secret (as the password). Anything else, or
 * a second redirection, ends the connection. */
static BOOL client_redirect(proxyPlugin* plugin, proxyData* pdata, void* param)
{
	rdpContext* context = param;
	rdpSettings* settings = context->settings;
	Session* session = get_session(plugin, pdata);
	char port_text[32] = "";
	char secret[SECRET_MAX] = "";
	unsigned int port = 0;
	BOOL ok = FALSE;

	if (!session || session->at_desktop)
	{
		WLog_WARN(TAG, "refused a redirection from the desktop itself");
		return FALSE;
	}

	const char* user = freerdp_settings_get_string(settings, FreeRDP_RedirectionUsername);
	const BYTE* where = freerdp_settings_get_pointer(settings, FreeRDP_LoadBalanceInfo);
	const UINT32 where_length = freerdp_settings_get_uint32(settings, FreeRDP_LoadBalanceInfoLength);
	const WCHAR* password = freerdp_settings_get_pointer(settings, FreeRDP_RedirectionPassword);
	const UINT32 password_length =
	    freerdp_settings_get_uint32(settings, FreeRDP_RedirectionPasswordLength);

	/* FreeRDP's server sends it as a routing token: "Cookie: msts=PORT\r\n" */
	if (where && where_length > 0 && where_length < sizeof(port_text))
	{
		memcpy(port_text, where, where_length);
		port_text[where_length] = '\0';
	}
	const char* port_start =
	    strncmp(port_text, "Cookie: msts=", 13) == 0 ? port_text + 13 : port_text;
	if (password && password_length >= sizeof(WCHAR))
	{
		char* text = ConvertWCharNToUtf8Alloc(password, password_length / sizeof(WCHAR), NULL);
		if (text && strlen(text) < sizeof(secret))
			snprintf(secret, sizeof(secret), "%s", text);
		if (text)
		{
			explicit_bzero(text, strlen(text));
			free(text);
		}
	}
	/* Each account's desktop is on port 20000 + its UID, from UID 1000 up */
	if (!user || !*user || strlen(user) > USER_MAX || sscanf(port_start, "%u", &port) != 1 ||
	    port < 21000 || port > 65535 || !secret[0])
	{
		WLog_WARN(TAG, "refused a redirection the login screen shouldn't have sent (%s)",
		          !user || !*user || strlen(user) > USER_MAX ? "the account"
		          : !secret[0]                               ? "the secret"
		                                                     : "the desktop's port");
		goto out;
	}
	snprintf(session->user, sizeof(session->user), "%s", user);

	ok = freerdp_settings_set_string(settings, FreeRDP_ServerHostname, "127.0.0.1") &&
	     freerdp_settings_set_uint32(settings, FreeRDP_ServerPort, port) &&
	     freerdp_settings_set_string(settings, FreeRDP_Username, session->user) &&
	     freerdp_settings_set_string(settings, FreeRDP_Password, secret) &&
	     freerdp_settings_set_string(settings, FreeRDP_Domain, NULL) &&
	     freerdp_settings_set_pointer_len(settings, FreeRDP_RedirectionPassword, NULL, 0) &&
	     freerdp_settings_set_pointer_len(settings, FreeRDP_LoadBalanceInfo, NULL, 0) &&
	     freerdp_settings_set_uint32(settings, FreeRDP_RedirectionFlags, 0) &&
	     freerdp_settings_set_bool(settings, FreeRDP_VmConnectMode, FALSE) &&
	     freerdp_settings_set_bool(settings, FreeRDP_NlaSecurity, TRUE) &&
	     freerdp_settings_set_bool(settings, FreeRDP_TlsSecurity, TRUE) &&
	     freerdp_settings_set_bool(settings, FreeRDP_RdpSecurity, FALSE) &&
	     freerdp_settings_set_bool(settings, FreeRDP_ExtSecurity, FALSE) &&
	     PubSub_SubscribeActivated(context->pubSub, on_desktop_activated) >= 0;
	if (ok)
	{
		redirected = pdata;
		session->at_desktop = TRUE;
		WLog_INFO(TAG, "signed in %s from the Hyper-V host, to their desktop on port %u",
		          session->user, port);
	}

out:
	explicit_bzero(secret, sizeof(secret));
	return ok;
}

static BOOL session_end(proxyPlugin* plugin, proxyData* pdata, void* param)
{
	WINPR_UNUSED(param);
	Session* session = get_session(plugin, pdata);
	if (session)
	{
		BOOL cleared = plugin->mgr->SetPluginData(plugin->mgr, plugin_name, pdata, NULL);
		WINPR_UNUSED(cleared);
		free(session);
	}
	return TRUE;
}

static BOOL unload(proxyPlugin* plugin)
{
	WINPR_UNUSED(plugin);
	return TRUE;
}

FREERDP_ENTRY_POINT(BOOL proxy_module_entry_point(proxyPluginsManager* manager, void* userdata))
{
	proxyPlugin plugin = { 0 };

	plugin.name = plugin_name;
	plugin.description = plugin_desc;
	plugin.PluginUnload = unload;
	plugin.ServerSessionInitialize = session_initialize;
	plugin.ServerFetchTargetAddr = fetch_target;
	plugin.ClientPreConnect = client_pre_connect;
	plugin.ClientRedirect = client_redirect;
	plugin.ServerSessionEnd = session_end;
	plugin.userdata = userdata;
	return manager->RegisterPlugin(manager, &plugin);
}
