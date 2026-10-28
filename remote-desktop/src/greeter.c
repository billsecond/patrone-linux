/* The login screen Hyper-V's enhanced session shows (patrone-remote-desktop).
 *
 * An enhanced session is Remote Desktop over a Hyper-V socket (vsock port
 * 3389), which only the Hyper-V host can open, and it brings no username or
 * password: VMConnect shows the guest's own login screen in its window, as
 * with Windows guests. Patrone's proxy for it (vmconnect.ini, with the
 * patrone-vmconnect plugin) connects each new connection here first. This
 * draws a login screen in the style of Patrone's own, has the username and
 * password checked by the sign-in helper (auth --vmconnect: PAM, for any
 * account that may sign in at the computer), and on success sends the proxy a
 * Server Redirection PDU with the account, its desktop's port and the secret
 * that opens it. The proxy's own client takes that (VMConnect never sees it),
 * and the plugin connects it to the account's own GNOME Remote Desktop
 * instead, while VMConnect's window stays connected.
 *
 * systemd runs one for each connection to /run/patrone-remote-desktop/
 * greeter.sock, which only the proxy's user can open, with the connection as
 * stdin.
 *
 *   greeter [--certificate FILE] [--key FILE] [--logo FILE]
 *           [--auth-socket PATH] [--keyboard LAYOUT]
 *
 * Copyright 2026 William Daugherty
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <limits.h>
#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/socket.h>
#include <sys/un.h>
#include <time.h>
#include <unistd.h>

#include <cairo.h>
#include <pango/pangocairo.h>
#include <xkbcommon/xkbcommon.h>

#include <freerdp/codec/planar.h>
#include <freerdp/crypto/certificate.h>
#include <freerdp/crypto/privatekey.h>
#include <freerdp/freerdp.h>
#include <freerdp/peer.h>
#include <freerdp/pointer.h>
#include <freerdp/redirection.h>
#include <freerdp/settings.h>
#include <freerdp/update.h>
#include <winpr/ssl.h>
#include <winpr/string.h>
#include <winpr/synch.h>

#define STATE_DIRECTORY "/var/lib/patrone-remote-desktop/vmconnect"
#define CERTIFICATE STATE_DIRECTORY "/tls.crt"
#define PRIVATE_KEY STATE_DIRECTORY "/tls.key"
#define LOGO "/usr/share/pixmaps/patrone-logo-login.png"
#define AUTH_SOCKET "/run/patrone-remote-desktop/vmconnect-auth.sock"

/* What the sign-in helper accepts: Linux account names are 32 bytes at most */
#define USER_MAX 32
#define PASSWORD_MAX 255
#define REPLY_MAX 256
#define SECRET_MAX 128
/* Nobody signing in for this long: give the connection up */
#define IDLE_SECONDS (15 * 60)
#define TILE 64

/* Patrone's login screen: Patrone Blue, Patrone Sans, a navy-to-black
 * background (branding/system/gdm/patrone-gdm.css) */
#define BLUE 0x288bfa
#define ERROR_RED 0xff8a80
#define FONT "Patrone Sans, Noto Sans, sans-serif"

typedef struct
{
	int x, y, width, height;
} Box;

typedef enum
{
	TYPING,
	CHECKING,
	SIGNED_IN
} Stage;

typedef struct
{
	rdpContext context; /* FreeRDP's own, first */

	/* The screen, drawn whole and sent in tiles */
	cairo_surface_t* surface;
	UINT32 width, height;
	double scale;
	BITMAP_PLANAR_CONTEXT* planar;
	cairo_surface_t* logo;
	BOOL active, suppressed;
	Box form, user_box, password_box, button;
	int message_y;

	char user[USER_MAX + 1];
	char password[PASSWORD_MAX + 1];
	size_t password_dots; /* shown while the password itself is gone */
	BOOL on_password;
	char message[256];
	BOOL message_is_error;
	Stage stage;

	struct xkb_context* xkb;
	struct xkb_keymap* keymap;
	struct xkb_state* keys;

	/* The sign-in helper's answer, while it's coming */
	int auth;
	HANDLE auth_event;
	char reply[REPLY_MAX];
	size_t reply_length;

	time_t last_input;
	char hostname[HOST_NAME_MAX + 1];
} Greeter;

static const char* auth_socket = AUTH_SOCKET;
static const char* keyboard_layout;

/* Drawing */

static void set_color(cairo_t* cr, UINT32 rgb, double alpha)
{
	cairo_set_source_rgba(cr, ((rgb >> 16) & 0xff) / 255.0, ((rgb >> 8) & 0xff) / 255.0,
	                      (rgb & 0xff) / 255.0, alpha);
}

static void rounded_rectangle(cairo_t* cr, const Box* box, double radius)
{
	double x = box->x, y = box->y, w = box->width, h = box->height;
	cairo_new_sub_path(cr);
	cairo_arc(cr, x + w - radius, y + radius, radius, -M_PI / 2, 0);
	cairo_arc(cr, x + w - radius, y + h - radius, radius, 0, M_PI / 2);
	cairo_arc(cr, x + radius, y + h - radius, radius, M_PI / 2, M_PI);
	cairo_arc(cr, x + radius, y + radius, radius, M_PI, 3 * M_PI / 2);
	cairo_close_path(cr);
}

static PangoLayout* text(cairo_t* cr, const char* string, double size, PangoWeight weight)
{
	PangoLayout* layout = pango_cairo_create_layout(cr);
	PangoFontDescription* font = pango_font_description_from_string(FONT);
	pango_font_description_set_absolute_size(font, size * PANGO_SCALE);
	pango_font_description_set_weight(font, weight);
	pango_layout_set_font_description(layout, font);
	pango_font_description_free(font);
	pango_layout_set_text(layout, string, -1);
	return layout;
}

/* Text centered on x, with its top at y, or with its middle at y; wrapped
 * at wrap pixels, unless that's 0 */
static void draw_centered(cairo_t* cr, const char* string, double size, PangoWeight weight,
                          double x, double y, BOOL middle, int wrap)
{
	PangoLayout* layout = text(cr, string, size, weight);
	int width = 0, height = 0;
	if (wrap > 0)
	{
		pango_layout_set_width(layout, wrap * PANGO_SCALE);
		pango_layout_set_wrap(layout, PANGO_WRAP_WORD);
		pango_layout_set_alignment(layout, PANGO_ALIGN_CENTER);
	}
	pango_layout_get_pixel_size(layout, &width, &height);
	/* Wrapped, Pango centers each line in the wrapping width itself */
	if (wrap > 0)
		width = wrap;
	cairo_move_to(cr, round(x - width / 2.0), round(middle ? y - height / 2.0 : y));
	pango_cairo_show_layout(cr, layout);
	g_object_unref(layout);
}

/* A plain figure on Patrone Blue, as the login screen shows for an account
 * without a picture */
static void draw_avatar(cairo_t* cr, double x, double top, double size)
{
	cairo_save(cr);
	cairo_arc(cr, x, top + size / 2, size / 2, 0, 2 * M_PI);
	set_color(cr, BLUE, 1);
	cairo_fill_preserve(cr);
	cairo_clip(cr);
	set_color(cr, 0xffffff, 1);
	cairo_arc(cr, x, top + size * 0.38, size * 0.16, 0, 2 * M_PI);
	cairo_fill(cr);
	cairo_save(cr);
	cairo_translate(cr, x, top + size * 0.92);
	cairo_scale(cr, size * 0.32, size * 0.26);
	cairo_arc(cr, 0, 0, 1, 0, 2 * M_PI);
	cairo_restore(cr);
	cairo_fill(cr);
	cairo_restore(cr);
}

/* An entry: its text (or, with dots, that many dots), or its placeholder */
static void draw_entry(Greeter* g, cairo_t* cr, const Box* box, const char* string, size_t dots,
                       const char* placeholder, BOOL focused)
{
	const double s = g->scale;
	char shown[(PASSWORD_MAX + 1) * 3 + 1] = "";

	rounded_rectangle(cr, box, 8 * s);
	set_color(cr, 0xffffff, 0.08);
	cairo_fill_preserve(cr);
	set_color(cr, focused ? BLUE : 0xffffff, focused ? 1 : 0.16);
	cairo_set_line_width(cr, (focused ? 2 : 1) * s);
	cairo_stroke(cr);

	if (string)
		snprintf(shown, sizeof(shown), "%s", string);
	else
	{
		for (size_t i = 0; i < dots && i <= PASSWORD_MAX; i++)
			strcat(shown, "●");
	}
	const BOOL empty = shown[0] == '\0';
	PangoLayout* layout = text(cr, empty ? placeholder : shown, 15 * s, PANGO_WEIGHT_NORMAL);
	int width = 0, height = 0;
	pango_layout_get_pixel_size(layout, &width, &height);

	const double padding = 12 * s, inside = box->width - 2 * padding;
	const double x = box->x + padding, y = round(box->y + (box->height - height) / 2.0);
	/* Where typing happens, the end, stays in view */
	const double shift = (!empty && width > inside) ? width - inside : 0;
	cairo_save(cr);
	/* Room for the caret just before the placeholder */
	cairo_rectangle(cr, x - 3 * s, box->y, inside + 3 * s, box->height);
	cairo_clip(cr);
	set_color(cr, 0xffffff, empty ? 0.45 : 1);
	cairo_move_to(cr, x - shift, y);
	pango_cairo_show_layout(cr, layout);
	if (focused && g->stage == TYPING)
	{
		const double caret = empty ? x - 2 * s : x - shift + width + s;
		cairo_rectangle(cr, round(caret), box->y + 10 * s, ceil(1.5 * s), box->height - 20 * s);
		set_color(cr, 0xffffff, 1);
		cairo_fill(cr);
	}
	cairo_restore(cr);
	g_object_unref(layout);
}

static void lay_out(Greeter* g)
{
	const double s = g->scale;
	const int width = (int)(320 * s), field = (int)(40 * s), gap = (int)(12 * s);
	const int avatar = (int)(96 * s), message = (int)(40 * s);
	const int height = avatar + (int)(28 * s) + field + gap + field + gap + message + gap + field;
	int x = ((int)g->width - width) / 2;
	int y = ((int)g->height - height) / 2 - (int)(24 * s);
	if (y < (int)(48 * s))
		y = (int)(48 * s);

	g->form = (Box){ x, y, width, height };
	g->user_box = (Box){ x, y + avatar + (int)(28 * s), width, field };
	g->password_box = (Box){ x, g->user_box.y + field + gap, width, field };
	g->message_y = g->password_box.y + field + gap;
	g->button = (Box){ x, g->message_y + message + gap, width, field };
}

static void draw(Greeter* g)
{
	const double s = g->scale, middle = g->width / 2.0;
	cairo_t* cr = cairo_create(g->surface);

	cairo_pattern_t* background = cairo_pattern_create_linear(0, 0, 0, g->height);
	cairo_pattern_add_color_stop_rgb(background, 0, 0x0c / 255.0, 0x1a / 255.0, 0x30 / 255.0);
	cairo_pattern_add_color_stop_rgb(background, 1, 0x03 / 255.0, 0x03 / 255.0, 0x03 / 255.0);
	cairo_set_source(cr, background);
	cairo_paint(cr);
	cairo_pattern_destroy(background);

	/* The computer's name, where the login screen's top bar would be */
	set_color(cr, 0xffffff, 0.75);
	draw_centered(cr, g->hostname, 14 * s, PANGO_WEIGHT_SEMIBOLD, middle, 12 * s, FALSE, 0);

	draw_avatar(cr, middle, g->form.y, 96 * s);
	draw_entry(g, cr, &g->user_box, g->user, 0, "Username", !g->on_password);
	draw_entry(g, cr, &g->password_box, NULL, g->password_dots, "Password", g->on_password);

	if (g->message[0])
	{
		set_color(cr, g->message_is_error ? ERROR_RED : 0xffffff, g->message_is_error ? 1 : 0.8);
		draw_centered(cr, g->message, 13 * s, PANGO_WEIGHT_NORMAL, middle,
		              g->message_y + 20 * s, TRUE, g->form.width);
	}

	rounded_rectangle(cr, &g->button, 8 * s);
	set_color(cr, BLUE, g->stage == TYPING ? 1 : 0.5);
	cairo_fill(cr);
	set_color(cr, 0xffffff, 1);
	draw_centered(cr, "Sign In", 15 * s, PANGO_WEIGHT_SEMIBOLD, middle,
	              g->button.y + g->button.height / 2.0, TRUE, 0);

	/* Patrone's logo at the bottom, as on the login screen, where it fits */
	if (g->logo)
	{
		const double width = cairo_image_surface_get_width(g->logo) * s;
		const double height = cairo_image_surface_get_height(g->logo) * s;
		const double top = g->height - height - 32 * s;
		if (top > g->form.y + g->form.height + 16 * s)
		{
			cairo_save(cr);
			cairo_translate(cr, round(middle - width / 2), round(top));
			cairo_scale(cr, s, s);
			cairo_set_source_surface(cr, g->logo, 0, 0);
			cairo_paint(cr);
			cairo_restore(cr);
		}
	}

	cairo_destroy(cr);
	cairo_surface_flush(g->surface);
}

/* Sending the screen: 64-pixel tiles, compressed with RDP 6's planar codec,
 * one to an update as Windows 11 needs (FreeRDP's shadow server does the
 * same). The surface is a whole number of tiles; the client clips. */
static BOOL send_area(Greeter* g, int x, int y, int width, int height)
{
	if (!g->active || g->suppressed || !g->surface)
		return TRUE;

	const int right = x + width < (int)g->width ? x + width : (int)g->width;
	const int bottom = y + height < (int)g->height ? y + height : (int)g->height;
	const BYTE* data = cairo_image_surface_get_data(g->surface);
	const int stride = cairo_image_surface_get_stride(g->surface);

	for (int top = (y > 0 ? y : 0) / TILE * TILE; top < bottom; top += TILE)
	{
		for (int left = (x > 0 ? x : 0) / TILE * TILE; left < right; left += TILE)
		{
			UINT32 size = 0;
			BYTE* tile = freerdp_bitmap_compress_planar(g->planar, data + top * stride + left * 4,
			                                            PIXEL_FORMAT_BGRX32, TILE, TILE,
			                                            (UINT32)stride, NULL, &size);
			if (!tile)
				return FALSE;

			BITMAP_DATA bitmap = { 0 };
			bitmap.destLeft = (UINT32)left;
			bitmap.destTop = (UINT32)top;
			bitmap.destRight = (UINT32)((left + TILE < (int)g->width ? left + TILE : (int)g->width) - 1);
			bitmap.destBottom =
			    (UINT32)((top + TILE < (int)g->height ? top + TILE : (int)g->height) - 1);
			bitmap.width = TILE;
			bitmap.height = TILE;
			bitmap.bitsPerPixel = 32;
			bitmap.compressed = TRUE;
			bitmap.bitmapDataStream = tile;
			bitmap.bitmapLength = size;
			bitmap.cbScanWidth = TILE * 4;
			bitmap.cbUncompressedSize = TILE * TILE * 4;
			bitmap.cbCompMainBodySize = size;

			BITMAP_UPDATE update = { 0 };
			update.number = 1;
			update.rectangles = &bitmap;
			BOOL sent = g->context.update->BitmapUpdate(&g->context, &update);
			free(tile);
			if (!sent)
				return FALSE;
		}
	}
	return TRUE;
}

static BOOL redraw_form(Greeter* g)
{
	draw(g);
	return send_area(g, g->form.x - TILE, g->form.y - TILE, g->form.width + 2 * TILE,
	                 g->form.height + 2 * TILE);
}

static void set_message(Greeter* g, const char* message, BOOL error)
{
	snprintf(g->message, sizeof(g->message), "%s", message);
	g->message_is_error = error;
}

static void forget_password(Greeter* g)
{
	explicit_bzero(g->password, sizeof(g->password));
	g->password_dots = 0;
}

/* Signing in */

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

/* Asks the sign-in helper, as the network proxy's plugin does: the username,
 * the password and the client, each ended by a NUL byte. Its answer comes
 * later, through the main loop. */
static void start_check(Greeter* g)
{
	struct sockaddr_un address = { .sun_family = AF_UNIX };
	static const char client[] = "vmconnect";

	if (!g->user[0])
	{
		g->on_password = FALSE;
		set_message(g, "Type your username.", TRUE);
		return;
	}
	if (!g->password[0])
	{
		g->on_password = TRUE;
		set_message(g, "Type your password.", TRUE);
		return;
	}

	int fd = socket(AF_UNIX, SOCK_STREAM | SOCK_CLOEXEC, 0);
	snprintf(address.sun_path, sizeof(address.sun_path), "%s", auth_socket);
	BOOL asked = fd >= 0 && connect(fd, (struct sockaddr*)&address, sizeof(address)) == 0 &&
	             write_all(fd, g->user, strlen(g->user) + 1) &&
	             write_all(fd, g->password, strlen(g->password) + 1) &&
	             write_all(fd, client, sizeof(client)) && shutdown(fd, SHUT_WR) == 0 &&
	             fcntl(fd, F_SETFL, O_NONBLOCK) == 0;
	/* Asked or not, the password isn't needed again */
	explicit_bzero(g->password, sizeof(g->password));
	if (asked)
		g->auth_event = CreateFileDescriptorEvent(NULL, FALSE, FALSE, fd, WINPR_FD_READ);
	if (!asked || !g->auth_event)
	{
		fprintf(stderr, "can't reach the sign-in helper at %s: %s\n", auth_socket,
		        strerror(errno));
		if (fd >= 0)
			close(fd);
		forget_password(g);
		set_message(g, "Signing in isn't working right now. Try again later.", TRUE);
		return;
	}
	g->auth = fd;
	g->reply_length = 0;
	g->stage = CHECKING;
	set_message(g, "Signing in…", FALSE);
}

/* Hands the connection to the account's desktop: the proxy's client takes
 * this, and connects there with the username and the secret. */
static BOOL redirect(Greeter* g, unsigned int port, const char* secret)
{
	freerdp_peer* peer = g->context.peer;
	rdpRedirection* redirection = redirection_new();
	size_t length = 0;
	char where[16];

	snprintf(where, sizeof(where), "%u", port);
	WCHAR* wide = ConvertUtf8ToWCharAlloc(secret, &length);
	BOOL sent = redirection && wide &&
	            redirection_set_flags(redirection,
	                                  LB_USERNAME | LB_PASSWORD | LB_LOAD_BALANCE_INFO) &&
	            redirection_set_session_id(redirection, 0) &&
	            redirection_set_string_option(redirection, LB_USERNAME, g->user) &&
	            redirection_set_byte_option(redirection, LB_PASSWORD, (const BYTE*)wide,
	                                        (length + 1) * sizeof(WCHAR)) &&
	            redirection_set_byte_option(redirection, LB_LOAD_BALANCE_INFO, (const BYTE*)where,
	                                        strlen(where)) &&
	            peer->SendServerRedirection(peer, redirection);
	if (wide)
	{
		explicit_bzero(wide, (length + 1) * sizeof(WCHAR));
		free(wide);
	}
	/* Its copies of the secret go with it */
	redirection_free(redirection);
	return sent;
}

static void finish_check(Greeter* g)
{
	unsigned int port = 0;
	char secret[SECRET_MAX] = "";

	g->reply[g->reply_length] = '\0';
	CloseHandle(g->auth_event);
	g->auth_event = NULL;
	close(g->auth);
	g->auth = -1;
	forget_password(g);
	g->stage = TYPING;

	if (sscanf(g->reply, "ok %u %127s", &port, secret) == 2 && port > 0 && port <= 65535)
	{
		if (redirect(g, port, secret))
		{
			fprintf(stderr, "signed in %s; opening their desktop\n", g->user);
			g->stage = SIGNED_IN;
			g->last_input = time(NULL);
			set_message(g, "Opening your desktop…", FALSE);
		}
		else
		{
			fprintf(stderr, "couldn't hand %s's connection to their desktop\n", g->user);
			set_message(g, "Your desktop couldn't be opened. Try again.", TRUE);
		}
	}
	else if (strncmp(g->reply, "denied", 6) == 0)
	{
		g->on_password = TRUE;
		set_message(g, "That username or password isn't right. Try again.", TRUE);
	}
	else if (strncmp(g->reply, "busy", 4) == 0)
		set_message(g,
		            "This account has a desktop open on the computer's own screen that can't be "
		            "shared. Sign out of it there first.",
		            TRUE);
	else
		set_message(g, "Your desktop couldn't start. Try again.", TRUE);

	explicit_bzero(secret, sizeof(secret));
	explicit_bzero(g->reply, sizeof(g->reply));
	g->reply_length = 0;
}

/* Reads what has come of the answer; finishes at its end of line or end */
static void read_reply(Greeter* g)
{
	while (g->reply_length + 1 < sizeof(g->reply))
	{
		ssize_t got = read(g->auth, g->reply + g->reply_length, sizeof(g->reply) - 1 - g->reply_length);
		if (got < 0 && errno == EINTR)
			continue;
		if (got < 0 && (errno == EAGAIN || errno == EWOULDBLOCK))
			return;
		if (got <= 0)
			break;
		g->reply_length += (size_t)got;
		if (memchr(g->reply, '\n', g->reply_length))
			break;
	}
	finish_check(g);
	/* Signed in, the proxy has already moved on */
	if (g->stage != SIGNED_IN && !redraw_form(g))
		fprintf(stderr, "couldn't update the screen\n");
}

/* The keyboard */

/* VMConnect sends where each key is (set 1 scancodes), not what it types.
 * Linux numbers keys the same way, apart from the extended ones. */
static xkb_keycode_t keycode(UINT16 flags, UINT8 code)
{
	if (!(flags & KBD_FLAGS_EXTENDED))
		return code == 0x54 ? 99 + 8 : (code > 0 && code <= 0x58 ? code + 8 : 0);
	switch (code)
	{
		case 0x1c: return 96 + 8;  /* Enter, on the keypad */
		case 0x1d: return 97 + 8;  /* Right Ctrl */
		case 0x35: return 98 + 8;  /* Keypad / */
		case 0x37: return 99 + 8;  /* Print Screen */
		case 0x38: return 100 + 8; /* Right Alt (AltGr) */
		case 0x47: return 102 + 8; /* Home */
		case 0x48: return 103 + 8; /* Up */
		case 0x49: return 104 + 8; /* Page Up */
		case 0x4b: return 105 + 8; /* Left */
		case 0x4d: return 106 + 8; /* Right */
		case 0x4f: return 107 + 8; /* End */
		case 0x50: return 108 + 8; /* Down */
		case 0x51: return 109 + 8; /* Page Down */
		case 0x52: return 110 + 8; /* Insert */
		case 0x53: return 111 + 8; /* Delete */
		case 0x5b: return 125 + 8; /* Left Windows key */
		case 0x5c: return 126 + 8; /* Right Windows key */
		case 0x5d: return 127 + 8; /* Menu */
		default: return 0;
	}
}

/* The layout the computer's own screen types with (/etc/default/keyboard,
 * which Ubuntu's installer writes), as VMConnect only sends key positions */
static struct xkb_keymap* load_keymap(struct xkb_context* xkb)
{
	char model[64] = "", layout[64] = "", variant[64] = "", options[256] = "";
	char line[512];
	FILE* file = keyboard_layout ? NULL : fopen("/etc/default/keyboard", "re");

	while (file && fgets(line, sizeof(line), file))
	{
		char* value = strchr(line, '=');
		if (!value)
			continue;
		*value++ = '\0';
		value[strcspn(value, "\r\n")] = '\0';
		if (*value == '"')
		{
			value++;
			value[strcspn(value, "\"")] = '\0';
		}
		if (strcmp(line, "XKBMODEL") == 0)
			snprintf(model, sizeof(model), "%s", value);
		else if (strcmp(line, "XKBLAYOUT") == 0)
			snprintf(layout, sizeof(layout), "%s", value);
		else if (strcmp(line, "XKBVARIANT") == 0)
			snprintf(variant, sizeof(variant), "%s", value);
		else if (strcmp(line, "XKBOPTIONS") == 0)
			snprintf(options, sizeof(options), "%s", value);
	}
	if (file)
		fclose(file);
	if (keyboard_layout)
		snprintf(layout, sizeof(layout), "%s", keyboard_layout);

	struct xkb_rule_names names = {
		.rules = "evdev",
		.model = model[0] ? model : NULL,
		.layout = layout[0] ? layout : NULL,
		.variant = variant[0] ? variant : NULL,
		.options = options[0] ? options : NULL,
	};
	struct xkb_keymap* keymap = xkb_keymap_new_from_names(xkb, &names, XKB_KEYMAP_COMPILE_NO_FLAGS);
	if (!keymap)
	{
		fprintf(stderr, "can't load the keyboard layout \"%s\"; using xkb's default\n", layout);
		keymap = xkb_keymap_new_from_names(xkb, NULL, XKB_KEYMAP_COMPILE_NO_FLAGS);
	}
	return keymap;
}

static void type_text(Greeter* g, const char* utf8)
{
	char* field = g->on_password ? g->password : g->user;
	const size_t room = g->on_password ? PASSWORD_MAX : USER_MAX;
	const size_t length = strlen(field), adding = strlen(utf8);

	if (length + adding > room)
		return;
	memcpy(field + length, utf8, adding + 1);
	if (g->on_password)
		g->password_dots++;
}

static void backspace(Greeter* g)
{
	char* field = g->on_password ? g->password : g->user;
	size_t length = strlen(field);

	if (length == 0)
		return;
	do
		length--;
	while (length > 0 && (field[length] & 0xc0) == 0x80);
	explicit_bzero(field + length, strlen(field + length));
	if (g->on_password && g->password_dots > 0)
		g->password_dots--;
}

static void clear_field(Greeter* g)
{
	if (g->on_password)
		forget_password(g);
	else
		g->user[0] = '\0';
}

/* Returns whether the form changed */
static BOOL press(Greeter* g, xkb_keysym_t key, const char* utf8, BOOL ctrl)
{
	if (g->stage != TYPING)
		return FALSE;
	switch (key)
	{
		case XKB_KEY_Tab:
		case XKB_KEY_ISO_Left_Tab:
		case XKB_KEY_Up:
		case XKB_KEY_Down:
			g->on_password = !g->on_password;
			return TRUE;
		case XKB_KEY_Return:
		case XKB_KEY_KP_Enter:
			if (!g->on_password && g->user[0] && !g->password[0])
				g->on_password = TRUE;
			else
				start_check(g);
			return TRUE;
		case XKB_KEY_BackSpace:
			backspace(g);
			return TRUE;
		case XKB_KEY_Escape:
			clear_field(g);
			return TRUE;
		default:
			break;
	}
	if (ctrl)
	{
		if (key == XKB_KEY_u || key == XKB_KEY_U)
		{
			clear_field(g);
			return TRUE;
		}
		return FALSE;
	}
	if (!utf8[0] || (unsigned char)utf8[0] < 0x20 || utf8[0] == 0x7f)
		return FALSE;
	type_text(g, utf8);
	if (g->message_is_error)
		g->message[0] = '\0';
	return TRUE;
}

static BOOL keyboard_event(rdpInput* input, UINT16 flags, UINT8 code)
{
	Greeter* g = (Greeter*)input->context;
	const xkb_keycode_t key = keycode(flags, code);
	const BOOL down = !(flags & KBD_FLAGS_RELEASE);
	char utf8[16] = "";

	if (!key || !g->keys)
		return TRUE;
	g->last_input = time(NULL);
	xkb_state_update_key(g->keys, key, down ? XKB_KEY_DOWN : XKB_KEY_UP);
	if (!down)
		return TRUE;

	const xkb_keysym_t sym = xkb_state_key_get_one_sym(g->keys, key);
	const BOOL ctrl =
	    xkb_state_mod_name_is_active(g->keys, XKB_MOD_NAME_CTRL, XKB_STATE_MODS_EFFECTIVE) > 0;
	xkb_state_key_get_utf8(g->keys, key, utf8, sizeof(utf8));
	BOOL changed = press(g, sym, utf8, ctrl);
	explicit_bzero(utf8, sizeof(utf8));
	return !changed || redraw_form(g);
}

/* Some clients type characters, not keys */
static BOOL unicode_event(rdpInput* input, UINT16 flags, UINT16 code)
{
	Greeter* g = (Greeter*)input->context;
	char utf8[8] = "";

	if (flags & KBD_FLAGS_RELEASE)
		return TRUE;
	g->last_input = time(NULL);
	if (code >= 0xd800 && code <= 0xdfff)
		return TRUE;
	if (code < 0x80)
		utf8[0] = (char)code;
	else if (code < 0x800)
	{
		utf8[0] = (char)(0xc0 | (code >> 6));
		utf8[1] = (char)(0x80 | (code & 0x3f));
	}
	else
	{
		utf8[0] = (char)(0xe0 | (code >> 12));
		utf8[1] = (char)(0x80 | ((code >> 6) & 0x3f));
		utf8[2] = (char)(0x80 | (code & 0x3f));
	}
	BOOL changed = press(g, XKB_KEY_NoSymbol, utf8, FALSE);
	explicit_bzero(utf8, sizeof(utf8));
	return !changed || redraw_form(g);
}

/* Caps Lock and Num Lock as the client has them */
static BOOL synchronize_event(rdpInput* input, UINT32 flags)
{
	Greeter* g = (Greeter*)input->context;
	xkb_mod_mask_t locked = 0;

	if (!g->keys)
		return TRUE;
	const xkb_mod_index_t caps = xkb_keymap_mod_get_index(g->keymap, XKB_MOD_NAME_CAPS);
	const xkb_mod_index_t num = xkb_keymap_mod_get_index(g->keymap, XKB_MOD_NAME_NUM);
	if ((flags & KBD_SYNC_CAPS_LOCK) && caps != XKB_MOD_INVALID)
		locked |= 1u << caps;
	if ((flags & KBD_SYNC_NUM_LOCK) && num != XKB_MOD_INVALID)
		locked |= 1u << num;
	xkb_state_update_mask(g->keys, xkb_state_serialize_mods(g->keys, XKB_STATE_MODS_DEPRESSED), 0,
	                      locked, 0, 0, 0);
	return TRUE;
}

static BOOL inside(const Box* box, int x, int y)
{
	return x >= box->x && x < box->x + box->width && y >= box->y && y < box->y + box->height;
}

static BOOL mouse_event(rdpInput* input, UINT16 flags, UINT16 x, UINT16 y)
{
	Greeter* g = (Greeter*)input->context;

	g->last_input = time(NULL);
	if (!(flags & PTR_FLAGS_DOWN) || !(flags & PTR_FLAGS_BUTTON1) || g->stage != TYPING)
		return TRUE;
	if (inside(&g->user_box, x, y))
		g->on_password = FALSE;
	else if (inside(&g->password_box, x, y))
		g->on_password = TRUE;
	else if (inside(&g->button, x, y))
		start_check(g);
	else
		return TRUE;
	return redraw_form(g);
}

/* The connection */

/* A screen the client's size, made again if the size changes */
static BOOL make_screen(Greeter* g)
{
	rdpSettings* settings = g->context.settings;
	const UINT32 width = freerdp_settings_get_uint32(settings, FreeRDP_DesktopWidth);
	const UINT32 height = freerdp_settings_get_uint32(settings, FreeRDP_DesktopHeight);
	const UINT32 factor = freerdp_settings_get_uint32(settings, FreeRDP_DesktopScaleFactor);

	if (g->surface && width == g->width && height == g->height)
		return TRUE;
	if (width < 200 || height < 200 || width > 8192 || height > 8192)
	{
		fprintf(stderr, "refused a screen of %ux%u\n", width, height);
		return FALSE;
	}
	if (g->surface)
		cairo_surface_destroy(g->surface);
	g->width = width;
	g->height = height;
	/* VMConnect's own scale on a HiDPI host */
	g->scale = factor >= 100 && factor <= 300 ? factor / 100.0 : 1.0;
	g->surface = cairo_image_surface_create(CAIRO_FORMAT_RGB24,
	                                        (int)((width + TILE - 1) / TILE * TILE),
	                                        (int)((height + TILE - 1) / TILE * TILE));
	if (cairo_surface_status(g->surface) != CAIRO_STATUS_SUCCESS)
		return FALSE;
	lay_out(g);
	draw(g);
	return TRUE;
}

static BOOL post_connect(freerdp_peer* peer)
{
	return make_screen((Greeter*)peer->context);
}

static BOOL activate(freerdp_peer* peer)
{
	Greeter* g = (Greeter*)peer->context;
	POINTER_SYSTEM_UPDATE pointer = { .type = SYSPTR_DEFAULT };

	if (!make_screen(g))
		return FALSE;
	g->active = TRUE;
	g->suppressed = FALSE;
	return peer->context->update->pointer->PointerSystem(peer->context, &pointer) &&
	       send_area(g, 0, 0, (int)g->width, (int)g->height);
}

static BOOL refresh_rect(rdpContext* context, BYTE count, const RECTANGLE_16* areas)
{
	Greeter* g = (Greeter*)context;
	for (BYTE i = 0; i < count; i++)
	{
		if (!send_area(g, areas[i].left, areas[i].top, areas[i].right - areas[i].left,
		               areas[i].bottom - areas[i].top))
			return FALSE;
	}
	return TRUE;
}

static BOOL suppress_output(rdpContext* context, BYTE allow, const RECTANGLE_16* area)
{
	Greeter* g = (Greeter*)context;
	WINPR_UNUSED(area);
	g->suppressed = !allow;
	return !allow || send_area(g, 0, 0, (int)g->width, (int)g->height);
}

static BOOL context_new(freerdp_peer* peer, rdpContext* context)
{
	Greeter* g = (Greeter*)context;
	WINPR_UNUSED(peer);

	g->auth = -1;
	g->scale = 1.0;
	g->last_input = time(NULL);
	if (gethostname(g->hostname, sizeof(g->hostname) - 1) != 0)
		snprintf(g->hostname, sizeof(g->hostname), "Patrone Linux");
	g->planar = freerdp_bitmap_planar_context_new(PLANAR_FORMAT_HEADER_NA | PLANAR_FORMAT_HEADER_RLE,
	                                              TILE, TILE);
	g->xkb = xkb_context_new(XKB_CONTEXT_NO_FLAGS);
	g->keymap = g->xkb ? load_keymap(g->xkb) : NULL;
	g->keys = g->keymap ? xkb_state_new(g->keymap) : NULL;
	return g->planar && g->keys;
}

static void context_free(freerdp_peer* peer, rdpContext* context)
{
	Greeter* g = (Greeter*)context;
	WINPR_UNUSED(peer);

	if (g->auth_event)
		CloseHandle(g->auth_event);
	if (g->auth >= 0)
		close(g->auth);
	forget_password(g);
	explicit_bzero(g->reply, sizeof(g->reply));
	if (g->surface)
		cairo_surface_destroy(g->surface);
	if (g->logo)
		cairo_surface_destroy(g->logo);
	freerdp_bitmap_planar_context_free(g->planar);
	xkb_state_unref(g->keys);
	xkb_keymap_unref(g->keymap);
	xkb_context_unref(g->xkb);
}

int main(int argc, char** argv)
{
	const char* certificate_file = CERTIFICATE;
	const char* key_file = PRIVATE_KEY;
	const char* logo_file = LOGO;
	int status = 1;

	for (int i = 1; i + 1 < argc; i += 2)
	{
		if (strcmp(argv[i], "--certificate") == 0)
			certificate_file = argv[i + 1];
		else if (strcmp(argv[i], "--key") == 0)
			key_file = argv[i + 1];
		else if (strcmp(argv[i], "--logo") == 0)
			logo_file = argv[i + 1];
		else if (strcmp(argv[i], "--auth-socket") == 0)
			auth_socket = argv[i + 1];
		else if (strcmp(argv[i], "--keyboard") == 0)
			keyboard_layout = argv[i + 1];
		else
		{
			fprintf(stderr, "unknown option %s\n", argv[i]);
			return 2;
		}
	}

	/* The connection, from systemd. stdin itself becomes /dev/null, and stdout,
	 * where FreeRDP logs, goes with stderr: it may be the connection too. */
	int connection = fcntl(STDIN_FILENO, F_DUPFD_CLOEXEC, 3);
	int null = open("/dev/null", O_RDONLY | O_CLOEXEC);
	if (connection < 0 || null < 0 || dup2(null, STDIN_FILENO) < 0 ||
	    dup2(STDERR_FILENO, STDOUT_FILENO) < 0)
	{
		fprintf(stderr, "no connection on stdin\n");
		return 1;
	}
	close(null);

	if (!winpr_InitializeSSL(WINPR_SSL_INIT_DEFAULT))
		return 1;
	freerdp_peer* peer = freerdp_peer_new(connection);
	if (!peer)
		return 1;
	peer->ContextSize = sizeof(Greeter);
	peer->ContextNew = context_new;
	peer->ContextFree = context_free;
	if (!freerdp_peer_context_new(peer))
	{
		fprintf(stderr, "can't set up the connection (or the keyboard layout)\n");
		freerdp_peer_free(peer);
		return 1;
	}
	Greeter* g = (Greeter*)peer->context;
	rdpSettings* settings = peer->context->settings;

	g->logo = cairo_image_surface_create_from_png(logo_file);
	if (cairo_surface_status(g->logo) != CAIRO_STATUS_SUCCESS)
	{
		cairo_surface_destroy(g->logo);
		g->logo = NULL;
	}

	rdpPrivateKey* key = freerdp_key_new_from_file_enc(key_file, NULL);
	rdpCertificate* certificate = freerdp_certificate_new_from_file(certificate_file);
	if (!key || !certificate ||
	    !freerdp_settings_set_pointer_len(settings, FreeRDP_RdpServerRsaKey, key, 1) ||
	    !freerdp_settings_set_pointer_len(settings, FreeRDP_RdpServerCertificate, certificate, 1))
	{
		fprintf(stderr, "can't read the certificate %s and key %s\n", certificate_file, key_file);
		goto out;
	}

	/* TLS on the local socket, and nothing to sign in with: this asks */
	if (!freerdp_settings_set_bool(settings, FreeRDP_RdpSecurity, FALSE) ||
	    !freerdp_settings_set_bool(settings, FreeRDP_TlsSecurity, TRUE) ||
	    !freerdp_settings_set_bool(settings, FreeRDP_NlaSecurity, FALSE) ||
	    !freerdp_settings_set_bool(settings, FreeRDP_ExtSecurity, FALSE) ||
	    !freerdp_settings_set_uint32(settings, FreeRDP_ColorDepth, 32) ||
	    !freerdp_settings_set_bool(settings, FreeRDP_RefreshRect, TRUE) ||
	    !freerdp_settings_set_bool(settings, FreeRDP_SuppressOutput, TRUE) ||
	    !freerdp_settings_set_uint32(settings, FreeRDP_MultifragMaxRequestSize, 0xFFFFFF))
		goto out;

	peer->PostConnect = post_connect;
	peer->Activate = activate;
	peer->context->input->KeyboardEvent = keyboard_event;
	peer->context->input->UnicodeKeyboardEvent = unicode_event;
	peer->context->input->SynchronizeEvent = synchronize_event;
	peer->context->input->MouseEvent = mouse_event;
	peer->context->update->RefreshRect = refresh_rect;
	peer->context->update->SuppressOutput = suppress_output;

	if (!peer->Initialize(peer))
		goto out;

	for (;;)
	{
		HANDLE handles[MAXIMUM_WAIT_OBJECTS];
		DWORD count = peer->GetEventHandles(peer, handles, MAXIMUM_WAIT_OBJECTS - 1);
		if (count == 0)
			break;
		if (g->auth_event)
			handles[count++] = g->auth_event;
		if (WaitForMultipleObjects(count, handles, FALSE, 1000) == WAIT_FAILED)
			break;
		if (!peer->CheckFileDescriptor(peer))
			break;
		if (g->auth >= 0)
			read_reply(g);
		if (g->stage == TYPING && time(NULL) - g->last_input > IDLE_SECONDS)
		{
			fprintf(stderr, "nobody signed in for %d minutes; closing\n", IDLE_SECONDS / 60);
			break;
		}
		/* The proxy hangs up as soon as it has the redirection */
		if (g->stage == SIGNED_IN && time(NULL) - g->last_input > 60)
		{
			fprintf(stderr, "the proxy didn't move on to the desktop; closing\n");
			break;
		}
	}
	status = 0;

out:
	peer->Disconnect(peer);
	freerdp_peer_context_free(peer);
	freerdp_peer_free(peer);
	return status;
}
