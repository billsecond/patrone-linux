/* A Remote Desktop client for tests that connects as VMConnect's enhanced
 * session does, with no username or password, and types: each TEXT a couple
 * of seconds after the one before, with "\t" for Tab and "\n" for Enter, on a
 * US keyboard. It runs for SECONDS, or until the server hangs up, and then
 * says which, and how many times the server activated it. With
 * TEST_CLIENT_SHOTS=PREFIX it saves what it shows just before typing each
 * TEXT, and at the end, as PREFIX-N.bmp.
 *
 * With TEST_CLIENT_RESIZE=WIDTHxHEIGHT it also takes the graphics pipeline
 * and display control, as VMConnect does, and once it has typed everything
 * and the server offers display control, asks for that size, the way a
 * client asks when its window is resized; it says what it got.
 *
 * With TEST_CLIENT_SOUND=1 it takes sound, as VMConnect does, on both of
 * sound's channels (AUDIO_PLAYBACK_DVC and rdpsnd), into FreeRDP's fake
 * sound device; with WLOG_FILTER=com.freerdp.channels.rdpsnd.client:DEBUG
 * FreeRDP's log says what arrives.
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

#include <freerdp/channels/disp.h>
#include <freerdp/channels/rdpsnd.h>
#include <freerdp/addin.h>
#include <freerdp/client/channels.h>
#include <freerdp/client/cmdline.h>
#include <freerdp/client/disp.h>
#include <freerdp/client/rdpgfx.h>
#include <freerdp/event.h>
#include <freerdp/freerdp.h>
#include <freerdp/gdi/gdi.h>
#include <freerdp/gdi/gfx.h>
#include <freerdp/input.h>
#include <freerdp/session.h>
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

/* Display control, once the server has offered it and said its limits */
static DispClientContext* display_control;
static BOOL display_control_ready;

static UINT on_display_control_caps(DispClientContext* disp, UINT32 monitors, UINT32 factor_a,
                                    UINT32 factor_b)
{
	WINPR_UNUSED(disp);
	printf("display control offered: up to %u monitors, %ux%u\n", monitors, factor_a, factor_b);
	display_control_ready = TRUE;
	return CHANNEL_RC_OK;
}

static void on_channel_connected(void* context, const ChannelConnectedEventArgs* event)
{
	rdpContext* rdp = context;

	if (strcmp(event->name, RDPGFX_DVC_CHANNEL_NAME) == 0)
	{
		if (!gdi_graphics_pipeline_init(rdp->gdi, (RdpgfxClientContext*)event->pInterface))
			fprintf(stderr, "test-client: can't set up the graphics pipeline\n");
	}
	else if (strcmp(event->name, DISP_DVC_CHANNEL_NAME) == 0)
	{
		display_control = (DispClientContext*)event->pInterface;
		display_control->DisplayControlCaps = on_display_control_caps;
	}
}

static void on_channel_disconnected(void* context, const ChannelDisconnectedEventArgs* event)
{
	rdpContext* rdp = context;

	if (strcmp(event->name, RDPGFX_DVC_CHANNEL_NAME) == 0)
		gdi_graphics_pipeline_uninit(rdp->gdi, (RdpgfxClientContext*)event->pInterface);
	else if (strcmp(event->name, DISP_DVC_CHANNEL_NAME) == 0)
	{
		display_control = NULL;
		display_control_ready = FALSE;
	}
}

static BOOL load_channels(freerdp* instance)
{
	return freerdp_client_load_addins(instance->context->channels, instance->context->settings);
}

static BOOL ask_for_size(UINT32 width, UINT32 height)
{
	DISPLAY_CONTROL_MONITOR_LAYOUT layout = { 0 };
	layout.Flags = DISPLAY_CONTROL_MONITOR_PRIMARY;
	layout.Width = width;
	layout.Height = height;
	layout.DesktopScaleFactor = 100;
	layout.DeviceScaleFactor = 100;
	return display_control->SendMonitorLayout(display_control, 1, &layout) == CHANNEL_RC_OK;
}

/* The server changed the desktop's size (or the graphics pipeline reset it) */
static BOOL desktop_resize(rdpContext* context)
{
	const UINT32 width = freerdp_settings_get_uint32(context->settings, FreeRDP_DesktopWidth);
	const UINT32 height = freerdp_settings_get_uint32(context->settings, FreeRDP_DesktopHeight);
	printf("the server sized the desktop %ux%u\n", width, height);
	return gdi_resize(context->gdi, width, height);
}

/* The server saying the user has signed in, as Windows does */
static BOOL save_session_info(rdpContext* context, UINT32 type, const void* data)
{
	WINPR_UNUSED(context);
	if (type == INFO_TYPE_LOGON || type == INFO_TYPE_LOGON_LONG)
		printf("the server says %s is signed in\n", ((const logon_info*)data)->username);
	return TRUE;
}

static BOOL post_connect(freerdp* instance)
{
	if (!gdi_init(instance, PIXEL_FORMAT_BGRX32))
		return FALSE;
	instance->context->update->DesktopResize = desktop_resize;
	instance->context->update->SaveSessionInfo = save_session_info;
	return TRUE;
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

	unsigned int resize_width = 0, resize_height = 0;
	const char* resize = getenv("TEST_CLIENT_RESIZE");
	if (resize && sscanf(resize, "%ux%u", &resize_width, &resize_height) != 2)
		resize = NULL;
	const BOOL sound = getenv("TEST_CLIENT_SOUND") != NULL;
	if (resize || sound)
	{
		/* FreeRDP's own channels, built into its client library */
		if (freerdp_register_addin_provider(freerdp_channels_load_static_addin_entry, 0) !=
		    CHANNEL_RC_OK)
			return 1;
		instance->LoadChannels = load_channels;
		/* Only what's tested: other channels want a whole client's callbacks */
		if (!freerdp_settings_set_bool(settings, FreeRDP_RedirectClipboard, FALSE) ||
		    !freerdp_settings_set_bool(settings, FreeRDP_AudioPlayback, sound) ||
		    !freerdp_settings_set_bool(settings, FreeRDP_AudioCapture, FALSE) ||
		    !freerdp_settings_set_bool(settings, FreeRDP_DeviceRedirection, FALSE) ||
		    !freerdp_settings_set_bool(settings, FreeRDP_SupportDynamicChannels, TRUE))
			return 1;
	}
	if (sound)
	{
		const char* params[] = { RDPSND_CHANNEL_NAME, "sys:fake" };
		if (!freerdp_client_add_static_channel(settings, ARRAYSIZE(params), params) ||
		    !freerdp_client_add_dynamic_channel(settings, ARRAYSIZE(params), params))
			return 1;
	}
	if (resize)
	{
		if (!freerdp_settings_set_bool(settings, FreeRDP_SupportGraphicsPipeline, TRUE) ||
		    !freerdp_settings_set_bool(settings, FreeRDP_SupportDisplayControl, TRUE) ||
		    !freerdp_settings_set_bool(settings, FreeRDP_DynamicResolutionUpdate, TRUE) ||
		    PubSub_SubscribeChannelConnected(context->pubSub, on_channel_connected) < 0 ||
		    PubSub_SubscribeChannelDisconnected(context->pubSub, on_channel_disconnected) < 0)
			return 1;
	}

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
		if (resize_width && next >= argc && display_control_ready && time(NULL) - typed >= 2)
		{
			printf("asking for %ux%u (the desktop is %ux%u)\n", resize_width, resize_height,
			       context->gdi ? context->gdi->width : 0, context->gdi ? context->gdi->height : 0);
			if (!ask_for_size(resize_width, resize_height))
				fprintf(stderr, "test-client: couldn't ask for the new size\n");
			resize_width = 0;
		}
	}
	if (resize && context->gdi)
		printf("the desktop is %ux%u%s\n", context->gdi->width, context->gdi->height,
		       display_control ? "" : " (no display control)");
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
