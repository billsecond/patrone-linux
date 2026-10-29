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
 * It sends sound as GNOME Remote Desktop does, with FreeRDP's sound server:
 * on the dynamic channel AUDIO_PLAYBACK_DVC first, and on the static channel
 * rdpsnd if the client answers that it can't open that one. Once the client
 * has said what it plays, it gets a second of silence, and the desktop says
 * on stdout each block the client confirms playing.
 *
 * Copyright 2026 William Daugherty
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

#include <netinet/in.h>
#include <stdio.h>
#include <string.h>
#include <sys/socket.h>
#include <unistd.h>

#include <freerdp/channels/channels.h>
#include <freerdp/channels/drdynvc.h>
#include <freerdp/channels/rdpsnd.h>
#include <freerdp/channels/wtsvc.h>
#include <freerdp/crypto/certificate.h>
#include <freerdp/crypto/privatekey.h>
#include <freerdp/freerdp.h>
#include <freerdp/peer.h>
#include <freerdp/server/rdpsnd.h>
#include <freerdp/settings.h>
#include <winpr/interlocked.h>
#include <winpr/ntlm.h>
#include <winpr/ssl.h>
#include <winpr/string.h>
#include <winpr/synch.h>
#include <winpr/sysinfo.h>
#include <winpr/wtsapi.h>

static const char* reached_file;

/* Sound, for the one connection at a time */
static RdpsndServerContext* sound;
static UINT32 sound_channel; /* the dynamic channel's id */
static volatile LONG sound_refused;

/* What the desktop sends: 44.1 kHz stereo, 16 bits */
static const AUDIO_FORMAT pcm = { .wFormatTag = WAVE_FORMAT_PCM,
	                              .nChannels = 2,
	                              .nSamplesPerSec = 44100,
	                              .nAvgBytesPerSec = 44100 * 4,
	                              .nBlockAlign = 4,
	                              .wBitsPerSample = 16 };

static BOOL sound_channel_assigned(RdpsndServerContext* context, UINT32 channel_id)
{
	WINPR_UNUSED(context);
	sound_channel = channel_id;
	return TRUE;
}

/* The client's answer to a dynamic channel. Like GNOME Remote Desktop, the
 * desktop falls back to the static channel later, from its own loop. */
static BOOL channel_created(void* userdata, UINT32 channel_id, INT32 status)
{
	WINPR_UNUSED(userdata);
	if (sound && sound->use_dynamic_virtual_channel && channel_id == sound_channel && status < 0)
	{
		printf("the client can't open %s; sound over %s instead\n", RDPSND_DVC_CHANNEL_NAME,
		       RDPSND_CHANNEL_NAME);
		(void)InterlockedExchange(&sound_refused, 1);
	}
	return TRUE;
}

/* The client has said which formats it plays */
static void sound_activated(RdpsndServerContext* context)
{
	static const BYTE silence[4410 * 4]; /* a tenth of a second */

	printf("the client takes sound over %s\n",
	       context->use_dynamic_virtual_channel ? RDPSND_DVC_CHANNEL_NAME : RDPSND_CHANNEL_NAME);
	for (UINT16 i = 0; i < context->num_client_formats; i++)
	{
		const AUDIO_FORMAT* format = &context->client_formats[i];
		if (format->wFormatTag != pcm.wFormatTag || format->nChannels != pcm.nChannels ||
		    format->nSamplesPerSec != pcm.nSamplesPerSec ||
		    format->wBitsPerSample != pcm.wBitsPerSample)
			continue;
		if (context->SelectFormat(context, i) != CHANNEL_RC_OK)
			break;
		for (int n = 0; n < 10; n++)
		{
			if (context->SendSamples(context, silence, 4410, (UINT16)GetTickCount64()) !=
			    CHANNEL_RC_OK)
				break;
		}
		return;
	}
	printf("the desktop couldn't send the client sound\n");
}

static UINT sound_confirmed(RdpsndServerContext* context, BYTE block, UINT16 timestamp)
{
	WINPR_UNUSED(context);
	WINPR_UNUSED(timestamp);
	printf("the client played sound block %u\n", block);
	return CHANNEL_RC_OK;
}

/* Once the client's dynamic channels are ready, as GNOME Remote Desktop starts
 * its channels */
static void start_sound(HANDLE vcm)
{
	sound = rdpsnd_server_context_new(vcm);
	if (!sound)
		return;
	sound->server_formats = audio_formats_new(1); /* the sound server frees them */
	if (!sound->server_formats)
		return;
	sound->server_formats[0] = pcm;
	sound->num_server_formats = 1;
	sound->src_format = &sound->server_formats[0];
	sound->use_dynamic_virtual_channel = TRUE;
	sound->ChannelIdAssigned = sound_channel_assigned;
	sound->Activated = sound_activated;
	sound->ConfirmBlock = sound_confirmed;
	if (sound->Initialize(sound, TRUE) != CHANNEL_RC_OK)
		printf("the desktop couldn't open %s\n", RDPSND_DVC_CHANNEL_NAME);
}

static void sound_over_static_channel(void)
{
	sound->Stop(sound);
	sound->use_dynamic_virtual_channel = FALSE;
	if (sound->Start(sound) != CHANNEL_RC_OK)
		printf("the desktop couldn't open %s\n", RDPSND_CHANNEL_NAME);
}

static void stop_sound(void)
{
	if (sound)
	{
		sound->Stop(sound);
		rdpsnd_server_context_free(sound);
	}
	sound = NULL;
	sound_channel = 0;
	(void)InterlockedExchange(&sound_refused, 0);
}

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
	HANDLE vcm = WTSOpenServerA((LPSTR)peer->context);

	if (vcm && vcm != INVALID_HANDLE_VALUE)
		WTSVirtualChannelManagerSetDVCCreationCallback(vcm, channel_created, NULL);
	if (vcm && vcm != INVALID_HANDLE_VALUE && private_key && cert &&
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
				DWORD count = peer->GetEventHandles(peer, handles, MAXIMUM_WAIT_OBJECTS - 1);
				if (count == 0)
					break;
				handles[count++] = WTSVirtualChannelManagerGetEventHandle(vcm);
				if (WaitForMultipleObjects(count, handles, FALSE, 1000) == WAIT_FAILED ||
				    !peer->CheckFileDescriptor(peer))
					break;
				/* As GNOME Remote Desktop does: not before the client has joined
				 * drdynvc, or the manager can't open it, and never tries again */
				if (!peer->connected ||
				    !WTSVirtualChannelManagerIsChannelJoined(vcm, DRDYNVC_SVC_CHANNEL_NAME))
					continue;
				if (!WTSVirtualChannelManagerCheckFileDescriptor(vcm))
					break;
				if (!sound && WTSVirtualChannelManagerGetDrdynvcState(vcm) == DRDYNVC_STATE_READY)
					start_sound(vcm);
				if (sound && InterlockedExchange(&sound_refused, 0))
					sound_over_static_channel();
			}
		}
		stop_sound();
		peer->Disconnect(peer);
	}
	if (vcm && vcm != INVALID_HANDLE_VALUE)
		WTSCloseServer(vcm);
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
	setvbuf(stdout, NULL, _IOLBF, 0);
	snprintf(sam, sizeof(sam), "%s.sam", argv[5]);
	int listener = socket(AF_INET, SOCK_STREAM, 0);
	if (!WTSRegisterWtsApiFunctionTable(FreeRDP_InitWtsApi()) ||
	    !winpr_InitializeSSL(WINPR_SSL_INIT_DEFAULT) || !write_sam(sam, argv[3], argv[4]) ||
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
