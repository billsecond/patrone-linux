/* Patrone's sign-in check for FreeRDP's proxy (patrone-remote-desktop).
 *
 * The proxy accepts TLS connections without NLA, so the client sends the
 * account's username and password in its client info. Before the proxy
 * connects on, this hands them to patrone-remote-desktop's auth helper, which
 * checks them with PAM and gets the account's desktop ready. On success the
 * proxy connects to that account's own GNOME Remote Desktop on localhost,
 * signing in with the random secret the helper returns. The password goes
 * no further, and is cleared from the proxy's settings once checked.
 *
 * Copyright 2026 William Daugherty
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

#define _GNU_SOURCE
#include <errno.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/socket.h>
#include <sys/time.h>
#include <sys/un.h>
#include <unistd.h>

#include <freerdp/freerdp.h>
#include <freerdp/peer.h>
#include <freerdp/server/proxy/proxy_context.h>
#include <freerdp/server/proxy/proxy_modules_api.h>
#include <freerdp/settings.h>
#include <winpr/wlog.h>

#define TAG MODULE_TAG("patrone-login")

#ifndef AUTH_SOCKET
#define AUTH_SOCKET "/run/patrone-remote-desktop/auth.sock"
#endif

/* Starting a desktop session can take a while on a slow machine. */
#define AUTH_TIMEOUT_SECONDS 120
#define REPLY_MAX 256
#define SECRET_MAX 128

static const char plugin_name[] = "patrone-login";
static const char plugin_desc[] =
    "Checks Remote Desktop sign-ins with PAM and connects each account to its own desktop";

/* What a session signs in to GNOME Remote Desktop with */
typedef struct
{
	char* user;
	char secret[SECRET_MAX];
} Target;

static void target_free(Target* target)
{
	if (!target)
		return;
	free(target->user);
	explicit_bzero(target->secret, sizeof(target->secret));
	free(target);
}

static BOOL write_all(int fd, const char* data, size_t size)
{
	while (size > 0)
	{
		ssize_t written = write(fd, data, size);
		if (written < 0 && errno == EINTR)
			continue;
		if (written <= 0)
			return FALSE;
		data += written;
		size -= (size_t)written;
	}
	return TRUE;
}

/* Sends the username, password and client address, each ended by a NUL byte,
 * and reads the helper's one-line answer. */
static BOOL ask_auth_helper(const char* user, const char* password, const char* client,
                            char* reply, size_t size)
{
	struct sockaddr_un address = { .sun_family = AF_UNIX };
	struct timeval timeout = { .tv_sec = AUTH_TIMEOUT_SECONDS };
	size_t length = 0;
	BOOL ok = FALSE;

	int fd = socket(AF_UNIX, SOCK_STREAM | SOCK_CLOEXEC, 0);
	if (fd < 0)
		return FALSE;
	strncpy(address.sun_path, AUTH_SOCKET, sizeof(address.sun_path) - 1);
	if (connect(fd, (struct sockaddr*)&address, sizeof(address)) != 0)
	{
		WLog_ERR(TAG, "can't reach the sign-in helper at %s: %s", AUTH_SOCKET, strerror(errno));
		goto out;
	}
	setsockopt(fd, SOL_SOCKET, SO_RCVTIMEO, &timeout, sizeof(timeout));

	if (!write_all(fd, user, strlen(user) + 1) || !write_all(fd, password, strlen(password) + 1) ||
	    !write_all(fd, client, strlen(client) + 1))
		goto out;
	shutdown(fd, SHUT_WR);

	while (length + 1 < size)
	{
		ssize_t got = read(fd, reply + length, size - 1 - length);
		if (got < 0 && errno == EINTR)
			continue;
		if (got <= 0)
			break;
		length += (size_t)got;
		if (memchr(reply, '\n', length))
			break;
	}
	reply[length] = '\0';
	ok = length > 0;

out:
	close(fd);
	return ok;
}

/* Decides where the proxy connects: runs once per connection, after the
 * client's info (with its username and password) has arrived. Returning FALSE
 * ends the connection. */
static BOOL fetch_target(proxyPlugin* plugin, proxyData* pdata, void* param)
{
	proxyFetchTargetEventInfo* event = param;
	rdpSettings* front = pdata->ps->settings;
	rdpSettings* back = pdata->pc->settings;
	const char* client = pdata->ps->peer ? pdata->ps->peer->hostname : "";
	char reply[REPLY_MAX];
	unsigned int port = 0;
	Target* target = NULL;
	BOOL asked = FALSE;

	const char* user = freerdp_settings_get_string(front, FreeRDP_Username);
	const char* password = freerdp_settings_get_string(front, FreeRDP_Password);
	if (!user || !*user || !password || !*password)
	{
		/* Remote Desktop Connection sends them only when it asked for them on
		 * the client (see patrone-remote-desktop rdp-file). */
		if (user && *user)
			WLog_WARN(TAG, "refused a connection from %s that sent the username \"%s\" but no password",
			          client, user);
		else
			WLog_WARN(TAG, "refused a connection from %s that sent no username or password", client);
		freerdp_settings_set_string(front, FreeRDP_Password, NULL);
		return FALSE;
	}

	target = calloc(1, sizeof(*target));
	if (target)
		target->user = strdup(user);
	if (target && target->user)
		asked = ask_auth_helper(user, password, client, reply, sizeof(reply));

	/* Checked, or failed: either way the password isn't needed again. */
	freerdp_settings_set_string(front, FreeRDP_Password, NULL);

	if (!asked)
	{
		WLog_ERR(TAG, "couldn't check the sign-in for %s from %s", user, client);
		target_free(target);
		return FALSE;
	}
	if (sscanf(reply, "ok %u %127s", &port, target->secret) != 2 || port == 0 || port > 65535)
	{
		/* Only the first word: a garbled answer could hold the secret. */
		reply[strcspn(reply, " \n")] = '\0';
		WLog_WARN(TAG, "refused the sign-in for %s from %s (%s)", user, client,
		          strcmp(reply, "ok") == 0 ? "garbled answer from the sign-in helper" : reply);
		explicit_bzero(reply, sizeof(reply));
		target_free(target);
		return FALSE;
	}
	explicit_bzero(reply, sizeof(reply));

	if (!plugin->mgr->SetPluginData(plugin->mgr, plugin_name, pdata, target))
	{
		target_free(target);
		return FALSE;
	}
	event->fetch_method = PROXY_FETCH_TARGET_USE_CUSTOM_ADDR;
	event->target_address = strdup("127.0.0.1"); /* the proxy frees it */
	event->target_port = (UINT16)port;
	if (!event->target_address)
		return FALSE;
	WLog_INFO(TAG, "signed in %s from %s, to their desktop on port %u", user, client, port);

	return freerdp_settings_set_string(back, FreeRDP_Username, target->user) &&
	       freerdp_settings_set_string(back, FreeRDP_Password, target->secret) &&
	       freerdp_settings_set_string(back, FreeRDP_Domain, NULL);
}

/* Sets the secret again just before the proxy connects, in case the proxy
 * copied the client's own settings over it in between. */
static BOOL client_pre_connect(proxyPlugin* plugin, proxyData* pdata, void* param)
{
	rdpContext* context = param;
	Target* target = plugin->mgr->GetPluginData(plugin->mgr, plugin_name, pdata);
	if (!target)
		return FALSE;
	return freerdp_settings_set_string(context->settings, FreeRDP_Username, target->user) &&
	       freerdp_settings_set_string(context->settings, FreeRDP_Password, target->secret) &&
	       freerdp_settings_set_string(context->settings, FreeRDP_Domain, NULL);
}

static BOOL session_end(proxyPlugin* plugin, proxyData* pdata, void* param)
{
	WINPR_UNUSED(param);
	Target* target = plugin->mgr->GetPluginData(plugin->mgr, plugin_name, pdata);
	if (target)
	{
		BOOL cleared = plugin->mgr->SetPluginData(plugin->mgr, plugin_name, pdata, NULL);
		WINPR_UNUSED(cleared);
		target_free(target);
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
	plugin.ServerFetchTargetAddr = fetch_target;
	plugin.ClientPreConnect = client_pre_connect;
	plugin.ServerSessionEnd = session_end;
	plugin.userdata = userdata;
	return manager->RegisterPlugin(manager, &plugin);
}
