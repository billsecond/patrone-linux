/* Tests the patrone-login plugin's decisions without a proxy: it loads the
 * plugin, plays the proxy's part for one connection at a time, and talks to
 * tests/fake-auth.py in place of the sign-in helper.
 *
 *   test-plugin PLUGIN.so            the helper's answers (fake-auth.py running)
 *   test-plugin PLUGIN.so unreachable  no helper at all
 *
 * Copyright 2026 William Daugherty
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

#include <dlfcn.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include <freerdp/freerdp.h>
#include <freerdp/peer.h>
#include <freerdp/server/proxy/proxy_context.h>
#include <freerdp/server/proxy/proxy_modules_api.h>
#include <freerdp/settings.h>

static proxyPlugin plugin;
static void* plugin_data;
static int failures;

static BOOL register_plugin(proxyPluginsManager* manager, const proxyPlugin* registered)
{
	plugin = *registered;
	plugin.mgr = manager;
	return TRUE;
}

static BOOL set_data(proxyPluginsManager* manager, const char* name, proxyData* pdata, void* data)
{
	(void)manager, (void)name, (void)pdata;
	plugin_data = data;
	return TRUE;
}

static void* get_data(proxyPluginsManager* manager, const char* name, proxyData* pdata)
{
	(void)manager, (void)name, (void)pdata;
	return plugin_data;
}

typedef struct
{
	rdpContext front, back;
	freerdp_peer peer;
	proxyData pdata;
	proxyFetchTargetEventInfo event;
} Connection;

static Connection* connection(const char* user, const char* password)
{
	Connection* c = calloc(1, sizeof(*c));
	c->front.settings = freerdp_settings_new(FREERDP_SETTINGS_SERVER_MODE);
	c->back.settings = freerdp_settings_new(0);
	strcpy(c->peer.hostname, "192.0.2.7");
	c->front.peer = &c->peer;
	c->pdata.ps = &c->front;
	c->pdata.pc = &c->back;
	if (user)
		freerdp_settings_set_string(c->front.settings, FreeRDP_Username, user);
	if (password)
		freerdp_settings_set_string(c->front.settings, FreeRDP_Password, password);
	return c;
}

static void finish(Connection* c)
{
	BOOL ended = plugin.ServerSessionEnd(&plugin, &c->pdata, &c->peer);
	(void)ended;
	free(c->event.target_address);
	freerdp_settings_free(c->front.settings);
	freerdp_settings_free(c->back.settings);
	free(c);
}

static void expect(int condition, const char* test, const char* what)
{
	printf("%s %s: %s\n", condition ? "ok  " : "FAIL", test, what);
	if (!condition)
		failures++;
}

static int is_empty(const char* text)
{
	return !text || !*text;
}

/* A sign-in the plugin must refuse, leaving no password behind */
static void refused(const char* test, const char* user, const char* password)
{
	Connection* c = connection(user, password);
	BOOL accepted = plugin.ServerFetchTargetAddr(&plugin, &c->pdata, &c->event);
	expect(!accepted, test, "refused");
	expect(is_empty(freerdp_settings_get_string(c->front.settings, FreeRDP_Password)), test,
	       "password cleared");
	expect(is_empty(freerdp_settings_get_string(c->back.settings, FreeRDP_Password)), test,
	       "nothing sent on");
	finish(c);
}

static void accepted(void)
{
	const char* test = "right password";
	Connection* c = connection("alice", "right");
	BOOL ok = plugin.ServerFetchTargetAddr(&plugin, &c->pdata, &c->event);
	rdpSettings* back = c->back.settings;

	expect(ok, test, "accepted");
	expect(c->event.fetch_method == PROXY_FETCH_TARGET_USE_CUSTOM_ADDR, test, "custom target");
	expect(c->event.target_address && strcmp(c->event.target_address, "127.0.0.1") == 0, test,
	       "target is localhost");
	expect(c->event.target_port == 21000, test, "target port from the helper");
	expect(strcmp(freerdp_settings_get_string(back, FreeRDP_Username), "alice") == 0, test,
	       "signs on as the account");
	expect(strcmp(freerdp_settings_get_string(back, FreeRDP_Password), "0123abcd") == 0, test,
	       "signs on with the secret, not the password");
	expect(is_empty(freerdp_settings_get_string(back, FreeRDP_Domain)), test, "no domain");
	expect(is_empty(freerdp_settings_get_string(c->front.settings, FreeRDP_Password)), test,
	       "password cleared");

	/* As if the proxy copied the client's settings over the secret */
	freerdp_settings_set_string(back, FreeRDP_Password, "right");
	ok = plugin.ClientPreConnect(&plugin, &c->pdata, &c->back);
	expect(ok && strcmp(freerdp_settings_get_string(back, FreeRDP_Password), "0123abcd") == 0, test,
	       "secret set again before connecting");

	finish(c);
	expect(plugin_data == NULL, test, "forgotten when the session ends");
}

int main(int argc, char** argv)
{
	proxyPluginsManager manager = { 0 };
	manager.RegisterPlugin = register_plugin;
	manager.SetPluginData = set_data;
	manager.GetPluginData = get_data;

	if (argc < 2)
		return 2;
	void* library = dlopen(argv[1], RTLD_NOW);
	proxyModuleEntryPoint entry = library ? dlsym(library, "proxy_module_entry_point") : NULL;
	if (!entry || !entry(&manager, NULL))
	{
		fprintf(stderr, "can't load %s: %s\n", argv[1], dlerror());
		return 2;
	}

	if (argc > 2 && strcmp(argv[2], "unreachable") == 0)
		refused("helper unreachable", "alice", "right");
	else
	{
		refused("no credentials", NULL, NULL);
		refused("no password", "alice", NULL);
		refused("wrong password", "alice", "wrong");
		refused("signed in elsewhere", "alice", "busy");
		refused("garbled answer", "alice", "garbage");
		accepted();
	}
	return failures ? 1 : 0;
}
