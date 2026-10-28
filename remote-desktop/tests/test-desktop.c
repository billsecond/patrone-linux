/* A stand-in for an account's GNOME Remote Desktop, for tests: it takes one
 * connection at a time on localhost, lets in one username and password with
 * NLA, as GNOME Remote Desktop does with the account's secret, and on each
 * activation writes the username it let in to a file.
 *
 *   test-desktop CERT KEY USER PASSWORD PORT-FILE REACHED-FILE
 *
 * It writes the port it listens on to PORT-FILE, and keeps its SAM file next
 * to it.
 *
 * Copyright 2026 William Daugherty
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

#include <netinet/in.h>
#include <stdio.h>
#include <string.h>
#include <sys/socket.h>
#include <unistd.h>

#include <freerdp/crypto/certificate.h>
#include <freerdp/crypto/privatekey.h>
#include <freerdp/freerdp.h>
#include <freerdp/peer.h>
#include <freerdp/settings.h>
#include <winpr/ntlm.h>
#include <winpr/ssl.h>
#include <winpr/string.h>
#include <winpr/synch.h>

static const char* reached_file;

static BOOL post_connect(freerdp_peer* peer)
{
	WINPR_UNUSED(peer);
	return TRUE;
}

static BOOL activate(freerdp_peer* peer)
{
	const char* user = freerdp_settings_get_string(peer->context->settings, FreeRDP_Username);
	FILE* file = fopen(reached_file, "w");
	if (!file)
		return FALSE;
	fprintf(file, "%s\n", user ? user : "");
	fclose(file);
	return TRUE;
}

/* The NT hash NLA checks, in a SAM file */
static BOOL write_sam(const char* path, const char* user, const char* password)
{
	size_t length = 0;
	BYTE hash[16];
	WCHAR* wide = ConvertUtf8ToWCharAlloc(password, &length);
	FILE* file = fopen(path, "w");

	if (!wide || !file || !NTOWFv1W(wide, (UINT32)(length * sizeof(WCHAR)), hash))
		return FALSE;
	fprintf(file, "%s:::", user);
	for (int i = 0; i < 16; i++)
		fprintf(file, "%02X", hash[i]);
	fprintf(file, ":\n");
	fclose(file);
	free(wide);
	return TRUE;
}

static void serve(int fd, const char* certificate, const char* key, const char* sam)
{
	freerdp_peer* peer = freerdp_peer_new(fd);
	if (!peer || !freerdp_peer_context_new(peer))
		return;
	rdpSettings* settings = peer->context->settings;
	rdpPrivateKey* private_key = freerdp_key_new_from_file_enc(key, NULL);
	rdpCertificate* cert = freerdp_certificate_new_from_file(certificate);

	if (private_key && cert &&
	    freerdp_settings_set_pointer_len(settings, FreeRDP_RdpServerRsaKey, private_key, 1) &&
	    freerdp_settings_set_pointer_len(settings, FreeRDP_RdpServerCertificate, cert, 1) &&
	    freerdp_settings_set_bool(settings, FreeRDP_NlaSecurity, TRUE) &&
	    freerdp_settings_set_bool(settings, FreeRDP_TlsSecurity, FALSE) &&
	    freerdp_settings_set_bool(settings, FreeRDP_RdpSecurity, FALSE) &&
	    freerdp_settings_set_string(settings, FreeRDP_NtlmSamFile, sam) &&
	    freerdp_settings_set_uint32(settings, FreeRDP_ColorDepth, 32))
	{
		peer->PostConnect = post_connect;
		peer->Activate = activate;
		if (peer->Initialize(peer))
		{
			for (;;)
			{
				HANDLE handles[MAXIMUM_WAIT_OBJECTS];
				DWORD count = peer->GetEventHandles(peer, handles, MAXIMUM_WAIT_OBJECTS);
				if (count == 0 || WaitForMultipleObjects(count, handles, FALSE, 1000) == WAIT_FAILED ||
				    !peer->CheckFileDescriptor(peer))
					break;
			}
		}
		peer->Disconnect(peer);
	}
	freerdp_peer_context_free(peer);
	freerdp_peer_free(peer);
}

int main(int argc, char** argv)
{
	char sam[4096];
	struct sockaddr_in address = { .sin_family = AF_INET, .sin_addr.s_addr = htonl(INADDR_LOOPBACK) };
	socklen_t size = sizeof(address);

	if (argc != 7)
	{
		fprintf(stderr, "usage: test-desktop CERT KEY USER PASSWORD PORT-FILE REACHED-FILE\n");
		return 2;
	}
	reached_file = argv[6];
	snprintf(sam, sizeof(sam), "%s.sam", argv[5]);
	int listener = socket(AF_INET, SOCK_STREAM, 0);
	if (!winpr_InitializeSSL(WINPR_SSL_INIT_DEFAULT) || !write_sam(sam, argv[3], argv[4]) ||
	    listener < 0 || bind(listener, (struct sockaddr*)&address, sizeof(address)) != 0 ||
	    listen(listener, 4) != 0 || getsockname(listener, (struct sockaddr*)&address, &size) != 0)
	{
		fprintf(stderr, "test-desktop: can't start\n");
		return 1;
	}
	FILE* port_file = fopen(argv[5], "w");
	if (!port_file)
		return 1;
	fprintf(port_file, "%u\n", ntohs(address.sin_port));
	fclose(port_file);

	for (;;)
	{
		int fd = accept(listener, NULL, NULL);
		if (fd >= 0)
			serve(fd, argv[1], argv[2], sam);
	}
}
