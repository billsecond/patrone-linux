/* A Remote Desktop client for tests that connects as VMConnect's enhanced
 * session does, with no username or password, and types: each TEXT a couple
 * of seconds after the one before, with "\t" for Tab and "\n" for Enter, on a
 * US keyboard. It runs for SECONDS, or until the server hangs up, and then
 * says which, and how many times the server activated it. With
 * TEST_CLIENT_SHOTS=PREFIX it saves what it shows just before typing each
 * TEXT, and at the end, as PREFIX-N.bmp.
 *
 *   test-client HOST PORT SECONDS TEXT...
 *
 * Copyright 2026 William Daugherty
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

#include <freerdp/freerdp.h>
#include <freerdp/gdi/gdi.h>
#include <freerdp/input.h>
#include <freerdp/settings.h>
#include <winpr/image.h>
#include <winpr/synch.h>

/* Set 1 scancodes for a US keyboard's letters and digits */
static UINT8 scancode(char c)
{
	static const char letters[] = "qwertyuiop\0\0\0\0asdfghjkl\0\0\0\0\0zxcvbnm";
	static const char digits[] = "1234567890";
	const char* at;

	if (c == '\t')
		return 0x0f;
	if (c == '\n')
		return 0x1c;
	if (c >= 'a' && c <= 'z' && (at = memchr(letters, c, sizeof(letters))))
		return (UINT8)(0x10 + (at - letters));
	if (c >= '0' && c <= '9' && (at = strchr(digits, c)))
		return (UINT8)(0x02 + (at - digits));
	return 0;
}

static int activations;

static void on_activated(void* context, const ActivatedEventArgs* event)
{
	WINPR_UNUSED(context);
	WINPR_UNUSED(event);
	activations++;
}

static BOOL post_connect(freerdp* instance)
{
	return gdi_init(instance, PIXEL_FORMAT_BGRX32);
}

static void save_screen(rdpContext* context, int number)
{
	const char* prefix = getenv("TEST_CLIENT_SHOTS");
	char path[4096];
	rdpGdi* gdi = context->gdi;

	if (!prefix || !gdi || !gdi->primary_buffer)
		return;
	snprintf(path, sizeof(path), "%s-%d.bmp", prefix, number);
	if (winpr_bitmap_write(path, gdi->primary_buffer, (size_t)gdi->width, (size_t)gdi->height, 32) < 0)
		fprintf(stderr, "test-client: can't save %s\n", path);
}

static void post_disconnect(freerdp* instance)
{
	gdi_free(instance);
}

int main(int argc, char** argv)
{
	if (argc < 4)
	{
		fprintf(stderr, "usage: test-client HOST PORT SECONDS TEXT...\n");
		return 2;
	}
	freerdp* instance = freerdp_new();
	if (!instance || !freerdp_context_new(instance))
		return 1;
	rdpContext* context = instance->context;
	rdpSettings* settings = context->settings;
	instance->PostConnect = post_connect;
	instance->PostDisconnect = post_disconnect;

	if (!freerdp_settings_set_string(settings, FreeRDP_ServerHostname, argv[1]) ||
	    !freerdp_settings_set_uint32(settings, FreeRDP_ServerPort, (UINT32)atoi(argv[2])) ||
	    !freerdp_settings_set_bool(settings, FreeRDP_TlsSecurity, TRUE) ||
	    !freerdp_settings_set_bool(settings, FreeRDP_NlaSecurity, FALSE) ||
	    !freerdp_settings_set_bool(settings, FreeRDP_RdpSecurity, FALSE) ||
	    !freerdp_settings_set_bool(settings, FreeRDP_IgnoreCertificate, TRUE) ||
	    !freerdp_settings_set_uint32(settings, FreeRDP_DesktopWidth, 1024) ||
	    !freerdp_settings_set_uint32(settings, FreeRDP_DesktopHeight, 768) ||
	    !freerdp_settings_set_uint32(settings, FreeRDP_ColorDepth, 32) ||
	    !freerdp_settings_set_bool(settings, FreeRDP_SoftwareGdi, TRUE) ||
	    PubSub_SubscribeActivated(context->pubSub, on_activated) < 0)
		return 1;
	if (!freerdp_connect(instance))
	{
		fprintf(stderr, "test-client: can't connect: %s\n",
		        freerdp_get_last_error_string(freerdp_get_last_error(context)));
		return 1;
	}

	const time_t start = time(NULL), end = start + atoi(argv[3]);
	int next = 4;
	time_t typed = start;
	while (time(NULL) < end && !freerdp_shall_disconnect_context(context))
	{
		HANDLE handles[MAXIMUM_WAIT_OBJECTS];
		DWORD count = freerdp_get_event_handles(context, handles, MAXIMUM_WAIT_OBJECTS);
		if (count == 0 || WaitForMultipleObjects(count, handles, FALSE, 100) == WAIT_FAILED ||
		    !freerdp_check_event_handles(context))
			break;
		if (next < argc && time(NULL) - typed >= 2)
		{
			save_screen(context, next - 4);
			for (const char* c = argv[next]; *c; c++)
			{
				const UINT8 code = scancode(*c);
				if (!code ||
				    !freerdp_input_send_keyboard_event(context->input, KBD_FLAGS_DOWN, code) ||
				    !freerdp_input_send_keyboard_event(context->input, KBD_FLAGS_RELEASE, code))
				{
					fprintf(stderr, "test-client: can't type '%c'\n", *c);
					return 1;
				}
			}
			next++;
			typed = time(NULL);
		}
	}
	save_screen(context, next - 4);
	if (time(NULL) < end)
		printf("the server hung up after %ld seconds, activated %d times\n",
		       (long)(time(NULL) - start), activations);
	else
		printf("connected for %ld seconds, activated %d times\n", (long)(time(NULL) - start),
		       activations);
	freerdp_disconnect(instance);
	freerdp_context_free(instance);
	freerdp_free(instance);
	return 0;
}
