/* FreeRDP's proxy with a plugin from the build, for tests: freerdp-proxy
 * itself only loads plugins from FreeRDP's own folder.
 *
 *   test-proxy CONFIG.ini PLUGIN.so
 *
 * Copyright 2026 William Daugherty
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

#include <dlfcn.h>
#include <stdio.h>

#include <freerdp/server/proxy/proxy_config.h>
#include <freerdp/server/proxy/proxy_server.h>

int main(int argc, char** argv)
{
	if (argc != 3)
	{
		fprintf(stderr, "usage: test-proxy CONFIG.ini PLUGIN.so\n");
		return 2;
	}
	void* library = dlopen(argv[2], RTLD_NOW);
	proxyModuleEntryPoint entry = library ? (proxyModuleEntryPoint)dlsym(library, "proxy_module_entry_point") : NULL;
	proxyConfig* config = pf_server_config_load_file(argv[1]);
	proxyServer* server = config ? pf_server_new(config) : NULL;
	if (!entry || !server || !pf_server_add_module(server, entry, (void*)config) ||
	    !pf_server_start(server))
	{
		fprintf(stderr, "test-proxy: can't start (%s)\n", library ? "config" : dlerror());
		return 1;
	}
	BOOL ran = pf_server_run(server);
	pf_server_free(server);
	pf_server_config_free(config);
	return ran ? 0 : 1;
}
