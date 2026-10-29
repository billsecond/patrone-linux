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
#include <limits.h>
#include <linux/vm_sockets.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/socket.h>
#include <unistd.h>

#include <freerdp/channels/channels.h>
#include <freerdp/channels/disp.h>
#include <freerdp/channels/drdynvc.h>
#include <freerdp/channels/rdpsnd.h>
#include <freerdp/freerdp.h>
#include <freerdp/peer.h>
#include <freerdp/server/proxy/proxy_config.h>
#include <freerdp/server/proxy/proxy_context.h>
#include <freerdp/server/proxy/proxy_modules_api.h>
#include <freerdp/session.h>
#include <freerdp/settings.h>
#include <winpr/nt.h>
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
	BOOL told_signed_in;
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

/* VMConnect, like Remote Desktop Connection, counts the session as signed in
 * only once the server says so (a Save Session Info PDU, "logon"), and until
 * then keeps its window one size. Windows sends one after its own sign-in;
 * GNOME Remote Desktop never does, so the plugin does, for the account, once
 * VMConnect's side is active on the desktop. */
static void tell_signed_in(proxyData* pdata, Session* session)
{
	char host[HOST_NAME_MAX + 1] = "";
	logon_info info = { 0 };

	if (session->told_signed_in)
		return;
	session->told_signed_in = TRUE;
	if (gethostname(host, sizeof(host) - 1) != 0)
		host[0] = '\0';
	info.sessionId = 1;
	info.username = session->user;
	info.domain = host;
	/* Version 2, as Windows sends it: FreeRDP writes version 1's fixed-size
	 * fields short, and its own client refuses them */
	if (pdata->ps->update->SaveSessionInfo(pdata->ps, INFO_TYPE_LOGON_LONG, &info))
		WLog_INFO(TAG, "told VMConnect that %s is signed in", session->user);
	else
		WLog_WARN(TAG, "couldn't tell VMConnect that %s is signed in", session->user);
}

/* VMConnect's side active again: on the desktop, the last activation */
static BOOL peer_activate(proxyPlugin* plugin, proxyData* pdata, void* param)
{
	WINPR_UNUSED(param);
	Session* session = get_session(plugin, pdata);
	if (session && session->at_desktop)
		tell_signed_in(pdata, session);
	return TRUE;
}

/* The connection whose client is on its way to the desktop. Each connection's
 * client has a thread of its own, which runs the redirection and then the
 * desktop's activation. */
static __thread proxyData* redirected;
static __thread Session* redirected_session;

/* Once the desktop is up, VMConnect's side takes its settings, as after the
 * proxy's first connection (pf_client_post_connect, which a redirection
 * doesn't run again): a reactivation, which a desktop resize sets off. */
static void on_desktop_activated(void* context, const ActivatedEventArgs* event)
{
	rdpContext* back = context;
	proxyData* pdata = redirected;
	WINPR_UNUSED(event);

	Session* session = redirected_session;
	redirected = NULL;
	redirected_session = NULL;
	PubSub_UnsubscribeActivated(back->pubSub, on_desktop_activated);
	/* Without a reactivation, VMConnect's side is already active */
	if (pdata && !config_true(pdata, "ReactivateAtDesktop", TRUE))
	{
		if (session)
			tell_signed_in(pdata, session);
		return;
	}
	if (!pdata || pdata->pc != back || !pf_context_copy_settings(pdata->ps->settings, back->settings) ||
	    !pdata->ps->update->DesktopResize(pdata->ps))
		WLog_WARN(TAG, "couldn't bring VMConnect's side up to date with the desktop");
	else
		WLog_INFO(TAG, "VMConnect's side takes the desktop's settings");
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
		redirected_session = session;
		session->at_desktop = TRUE;
		WLog_INFO(TAG, "signed in %s from the Hyper-V host, to their desktop on port %u",
		          session->user, port);
	}

out:
	explicit_bzero(secret, sizeof(secret));
	return ok;
}

/* Answers the desktop that VMConnect couldn't open a dynamic channel, as
 * VMConnect itself answers: a Create Response PDU with STATUS_UNSUCCESSFUL,
 * on the desktop's side of drdynvc */
static BOOL refuse_channel(proxyData* pdata, UINT16 channel_id)
{
	freerdp* back = pdata->pc->instance;
	const UINT16 drdynvc = freerdp_channels_get_id_by_name(back, DRDYNVC_SVC_CHANNEL_NAME);
	const UINT32 status = (UINT32)STATUS_UNSUCCESSFUL;
	BYTE response[7];
	size_t length = 0;

	if (drdynvc == 0 || drdynvc == UINT16_MAX)
		return FALSE;
	/* The header byte says the command and how many bytes the channel's id
	 * takes: one or two */
	response[length++] = (CREATE_REQUEST_PDU << 4) | (channel_id > UINT8_MAX ? 0x01 : 0x00);
	response[length++] = channel_id & 0xff;
	if (channel_id > UINT8_MAX)
		response[length++] = channel_id >> 8;
	for (int i = 0; i < 4; i++)
		response[length++] = (status >> (8 * i)) & 0xff;
	return back->SendChannelData(back, drdynvc, response, length);
}

/* Each dynamic channel the desktop opens towards VMConnect, in the journal.
 *
 * GNOME Remote Desktop's sound (FreeRDP's sound server) sends its first
 * message, the formats it can send, right after asking for its channel,
 * without waiting for the answer. The proxy drops anything on a channel that
 * isn't open yet, so VMConnect never hears from it, and the desktop gives up
 * on sound ten seconds later. So the plugin answers the desktop itself that
 * the channel couldn't be opened, and the desktop sends its sound over the
 * older static channel, rdpsnd, instead, which the proxy passes on as it
 * comes. VMConnect never sees the request. */
static BOOL dynamic_channel_create(proxyPlugin* plugin, proxyData* pdata, void* param)
{
	const proxyChannelDataEventInfo* channel = param;
	WINPR_UNUSED(plugin);

	if (strcmp(channel->channel_name, RDPSND_DVC_CHANNEL_NAME) == 0 ||
	    strcmp(channel->channel_name, RDPSND_LOSSY_DVC_CHANNEL_NAME) == 0)
	{
		if (refuse_channel(pdata, channel->channel_id))
		{
			WLog_INFO(TAG, "the desktop opens the channel %s: told it to send sound over %s instead",
			          channel->channel_name, RDPSND_CHANNEL_NAME);
			return FALSE; /* the proxy drops the request */
		}
		WLog_WARN(TAG, "couldn't turn the desktop's sound to %s; VMConnect may get no sound",
		          RDPSND_CHANNEL_NAME);
	}
	WLog_INFO(TAG, "the desktop opens the channel %s", channel->channel_name);
	return TRUE;
}

/* Display control is how VMConnect resizes the desktop with its window: it's
 * watched, and passed on unchanged */
static BOOL watch_channel(proxyPlugin* plugin, proxyData* pdata, void* param)
{
	proxyChannelToInterceptData* channel = param;
	WINPR_UNUSED(plugin);
	WINPR_UNUSED(pdata);
	if (strcmp(channel->name, DISP_DVC_CHANNEL_NAME) == 0)
		channel->intercept = TRUE;
	return TRUE;
}

/* A display control message, inside a dynamic channel data PDU: its type,
 * and for a monitor layout, the first monitor's size */
static void log_display_control(wStream* packet, BOOL from_desktop)
{
	const BYTE* data = Stream_Buffer(packet);
	const size_t length = Stream_Length(packet);
	static const size_t id_sizes[] = { 1, 2, 4, 4 };

	if (length < 1)
		return;
	const BYTE command = data[0] >> 4;
	size_t offset = 1 + id_sizes[data[0] & 0x03];
	if (command == 0x02) /* DATA_FIRST: a length field before the data */
		offset += id_sizes[(data[0] >> 2) & 0x03];
	else if (command != 0x03) /* not DATA */
		return;
	if (offset + 8 > length)
		return;
	UINT32 type = 0;
	memcpy(&type, data + offset, sizeof(type));
	if (type == DISPLAY_CONTROL_PDU_TYPE_CAPS && from_desktop)
		WLog_INFO(TAG, "the desktop offers VMConnect display control");
	else if (type == DISPLAY_CONTROL_PDU_TYPE_MONITOR_LAYOUT && !from_desktop &&
	         offset + 16 + 20 <= length)
	{
		UINT32 monitors = 0, width = 0, height = 0;
		memcpy(&monitors, data + offset + 12, sizeof(monitors));
		memcpy(&width, data + offset + 16 + 12, sizeof(width));
		memcpy(&height, data + offset + 16 + 16, sizeof(height));
		WLog_INFO(TAG, "VMConnect asks for a desktop of %ux%u (%u monitors)", width, height,
		          monitors);
	}
}

static BOOL channel_data(proxyPlugin* plugin, proxyData* pdata, void* param)
{
	proxyDynChannelInterceptData* data = param;
	WINPR_UNUSED(plugin);
	WINPR_UNUSED(pdata);
	if (data->first)
		log_display_control(data->data, data->isBackData);
	data->rewritten = TRUE; /* sent on as it came */
	data->result = PF_CHANNEL_RESULT_PASS;
	return TRUE;
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
	plugin.ServerPeerActivate = peer_activate;
	plugin.DynamicChannelCreate = dynamic_channel_create;
	plugin.DynChannelToIntercept = watch_channel;
	plugin.DynChannelIntercept = channel_data;
	plugin.ServerSessionEnd = session_end;
	plugin.userdata = userdata;
	return manager->RegisterPlugin(manager, &plugin);
}
