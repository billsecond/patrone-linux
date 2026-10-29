/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*- */
/*
 * Patrone's page in GNOME Settings: the taskbar (size, layout, Start
 * button icon), the desktop (its files in a bubble or across it, the
 * Recycle Bin, and where the terminal opens at sign-in), the title bar
 * buttons (top right or top left, and their style), the theme and the desktop's font (Patrone Sans or Noto Sans,
 * which the page sets itself), and the mouse pointer (the cursor theme and
 * size, which the page sets itself, with pictures from each theme's own
 * cursors). The same settings as Patrone Settings' window,
 * which this page leans on: its pictures (/usr/share/patrone/settings/
 * previews) and, to apply a theme or button style, "patrone-settings
 * --set-theme" and "--set-title-buttons". The choices' values match
 * the settings schemas' (the taskbar's and org.patrone.desktop) and
 * settings/src in Patrone's repository.
 *
 * Copyright (C) 2026 William Daugherty
 *
 * This program is free software; you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation; either version 2 of the License, or
 * (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this program; if not, see <http://www.gnu.org/licenses/>.
 */

#include "config.h"

#include <string.h>
#include <glib/gi18n-lib.h>
#include <adwaita.h>

#include "cc-patrone-panel.h"
#include "shell/cc-application.h"

#define TASKBAR_SCHEMA "org.gnome.shell.extensions.patrone-taskbar"
#define DESKTOP_SCHEMA "org.patrone.desktop"
#define WM_SCHEMA "org.gnome.desktop.wm.preferences"
#define INTERFACE_SCHEMA "org.gnome.desktop.interface"
#define PREVIEWS_DIR "/usr/share/patrone/settings/previews"
#define TASKBAR_ICONS "gnome-shell/extensions/patrone-taskbar@patrone-linux/icons"

/* The title bar buttons at the top right or top left (GNOME's
 * button-layout), close nearest the corner */
#define LAYOUT_RIGHT "appmenu:minimize,maximize,close"
#define LAYOUT_LEFT "close,minimize,maximize:appmenu"

typedef struct
{
  const char *value;
  const char *title;
  const char *detail; /* for a Start button icon: its file */
} Choice;

static const Choice sizes[] = {
  { "small", N_("Small"), N_("More room for windows") },
  { "medium", N_("Medium"), N_("The standard size") },
  { "large", N_("Large"), N_("Bigger buttons, easier to hit") },
};

static const Choice layouts[] = {
  { "left", N_("Left"), N_("Start and your apps on the left") },
  { "center", N_("Centered"), N_("Start and your apps in the middle") },
  { "split", N_("Split"), N_("Start on the left, your apps in the middle") },
  { "top", N_("Top"), N_("Along the top of the screen, menus opening downward") },
};

static const Choice start_icons[] = {
  { "patrone", N_("Patrone"), "patrone-start.svg" },
  { "mono", N_("One Color"), "start-mono-symbolic.svg" },
  { "orb", N_("Orb"), "start-orb.svg" },
  { "grid", N_("Grid"), "start-grid-symbolic.svg" },
  { "menu", N_("Menu"), "start-menu-symbolic.svg" },
  { "circle", N_("Circle"), "start-circle-symbolic.svg" },
  { "spark", N_("Spark"), "start-spark-symbolic.svg" },
  { "terminal", N_("Prompt"), "start-terminal-symbolic.svg" },
};

static const Choice colors[] = {
  { "wallpaper", N_("From the Wallpaper"), N_("The wallpaper's own color, dark or light with the style") },
  { "theme", N_("Theme's Own"), N_("The theme's grey, whatever the wallpaper") },
};

static const Choice pin_icons[] = {
  { "patrone", N_("Patrone's"), N_("A globe, a prompt, a folder and an envelope") },
  { "app", N_("Each App's Own"), N_("The icon of the app each one opens, in its own style") },
};

/* Yes-or-no settings: the first choice turns them on */
static const Choice desktop_files[] = {
  { "bubble", N_("On"), N_("The desktop's files in a bubble, opened from a pill at the screen's left edge") },
  { "desktop", N_("Off"), N_("The desktop's files across the desktop, under your windows") },
};

static const Choice recycle_bin[] = {
  { "shown", N_("On the Desktop"), N_("Docked to a corner; drop files on it to delete them") },
  { "hidden", N_("Hidden"), N_("Still in File Manager's navigation pane") },
};

static const Choice sign_in_terminal[] = {
  { "logo", N_("Over the Logo"), N_("Large, over the wallpaper's logo, at any screen size") },
  { "center", N_("Centered"), N_("At its own size, in the middle of the screen") },
};

static const Choice sides[] = {
  { "right", N_("Top Right"), N_("Minimize, maximize, then close in the corner") },
  { "left", N_("Top Left"), N_("Close in the corner, then minimize, maximize") },
};

static const Choice button_styles[] = {
  { "theme", N_("Theme's Own"), N_("Whichever style the theme comes with") },
  { "wide", N_("Wide"), N_("Flat and wide; close turns red under the pointer") },
  { "round", N_("Round"), N_("Small symbols in soft circles") },
  { "dots", N_("Dots"), N_("Red, yellow and green dots, symbols on hover") },
  { "glossy", N_("Glossy"), N_("Framed blue squares and a red close") },
};

static const Choice themes[] = {
  { "patrone", N_("Patrone"), N_("Patrone's own look, light or dark") },
  { "modern", N_("Modern"), N_("Clean and flat, with rounder buttons and no lines") },
  { "classic", N_("Classic"), N_("Plain and square, with solid title bars") },
  { "royal", N_("Sapphire"), N_("Glossy sapphire blue, with a green Start button") },
  { "studio", N_("Studio"), N_("Soft and translucent, with round colored window buttons") },
};

/* The desktop's fonts, by family, the default first, and the packages that
 * install them */
static const Choice fonts[] = {
  { "Patrone Sans", N_("Patrone Sans"), N_("Patrone's font: Noto Sans with a tailed l and a slashed zero") },
  { "Noto Sans", N_("Noto Sans"), N_("The font Patrone Sans is based on, without those changes") },
};
static const char * const font_packages[] = { "fonts-patrone-sans", "fonts-noto-core" };

/* What a font's card shows, in that font */
#define FONT_SAMPLE_BIG "Aa"
#define FONT_SAMPLE "Il1 O0 10:00"

/* The mouse pointer's sizes, as GNOME's Accessibility page names them */
#define CURSOR_THEME_KEY "cursor-theme"
#define CURSOR_SIZE_KEY "cursor-size"
static const Choice pointer_sizes[] = {
  { "24", N_("Default"), N_("24 pixels, the usual size") },
  { "32", N_("Medium"), N_("32 pixels") },
  { "48", N_("Large"), N_("48 pixels") },
  { "64", N_("Larger"), N_("64 pixels") },
  { "96", N_("Largest"), N_("96 pixels") },
};

/* The pointers a theme's card shows, each by its names, newest first: the
 * arrow, the hand over links, the text cursor and the busy one */
static const char * const arrow_names[] = { "default", "left_ptr", NULL };
static const char * const hand_names[] = { "pointer", "hand2", "hand1", NULL };
static const char * const text_names[] = { "text", "xterm", NULL };
static const char * const busy_names[] = { "wait", "watch", NULL };
static const char * const * const shown_cursors[] = { arrow_names, hand_names, text_names, busy_names };
#define N_SHOWN G_N_ELEMENTS (shown_cursors)

/* The names GNOME asks for that a theme must have, or inherit, to be
 * offered. GNOME Shell (mutter's Xcursor loader) looks a cursor up by
 * these names in the theme and those it inherits from, then in the
 * system's default theme, and only then by its old X11 name, so a theme
 * with only the old names (DMZ, X.Org's whiteglass and redglass) shows the
 * default theme's pointers, and choosing it would seem to do nothing */
static const char * const needed_cursors[] = { "default", "pointer", "text" };

/* How deep Inherits chains are followed, as libXcursor does */
#define MAX_INHERITS 4

/* Patrone's words for the cursor themes it ships or knows */
static const Choice known_cursor_themes[] = {
  { "Patrone", NULL, N_("Patrone's own: blue, with a white edge") },
  { "Adwaita", NULL, N_("GNOME's own: black, with a white edge") },
  { "Yaru", NULL, N_("Yaru's: dark grey, with a white edge") },
  { "breeze_cursors", NULL, N_("KDE's Breeze: black, with a white edge") },
  { "Breeze_Light", NULL, N_("KDE's Breeze: white, with a dark edge") },
};

/* One picture of a cursor: its first frame at one size */
typedef struct
{
  guint nominal;
  double xhot;
  double yhot;
  cairo_surface_t *surface;
} CursorImage;

/* An installed cursor theme, and its pointers in shown_cursors' order (a
 * GPtrArray of CursorImage each, or NULL where it has none) */
typedef struct
{
  char *name;
  char *title;
  char *detail;
  GPtrArray *cursors[N_SHOWN];
} CursorTheme;

typedef struct _ChoiceSet ChoiceSet;
typedef char *(*ReadFunc) (ChoiceSet *set);
typedef void (*WriteFunc) (ChoiceSet *set, const char *value);

/* One row of choices for one setting: its buttons, how to read the
 * setting as one of their values, and how to set it */
struct _ChoiceSet
{
  GSettings *settings;
  const char *key;
  const Choice *choices;
  ReadFunc read;
  WriteFunc write;
  GPtrArray *buttons;
  gulong changed_id;
};

struct _CcPatronePanel
{
  CcPanel parent_instance;

  GSettings *taskbar;
  GSettings *desktop;
  GSettings *wm;
  GSettings *interface;
  GFile *icons_dir;
  GPtrArray *choice_sets;

  /* The mouse pointer: the themes installed and their choices, the arrow
   * of the theme that's set and the sizes' pictures, which draw it */
  GPtrArray *cursor_themes;
  Choice *cursor_choices;
  GPtrArray *size_arrow;
  GPtrArray *size_pictures;
  gulong cursor_theme_id;
  gulong cursor_size_id;
};

CC_PANEL_REGISTER (CcPatronePanel, cc_patrone_panel)

static GSettings *
settings_if_installed (const char *id)
{
  GSettingsSchemaSource *source = g_settings_schema_source_get_default ();
  g_autoptr(GSettingsSchema) schema = NULL;

  if (source != NULL)
    schema = g_settings_schema_source_lookup (source, id, TRUE);
  return schema != NULL ? g_settings_new_full (schema, NULL, NULL) : NULL;
}

/* The taskbar's icons folder: in the account's extensions, or the system's */
static GFile *
find_icons_dir (void)
{
  g_autofree char *user = g_build_filename (g_get_user_data_dir (), TASKBAR_ICONS, NULL);
  const char * const *system = g_get_system_data_dirs ();

  if (g_file_test (user, G_FILE_TEST_IS_DIR))
    return g_file_new_for_path (user);
  for (; *system != NULL; system++)
    {
      g_autofree char *path = g_build_filename (*system, TASKBAR_ICONS, NULL);
      if (g_file_test (path, G_FILE_TEST_IS_DIR))
        return g_file_new_for_path (path);
    }
  return NULL;
}

/* Reading and writing settings as choices */

static char *
read_string (ChoiceSet *set)
{
  return g_settings_get_string (set->settings, set->key);
}

static void
write_string (ChoiceSet *set, const char *value)
{
  g_settings_set_string (set->settings, set->key, value);
}

/* A yes-or-no setting as two choices, the first for yes */
static char *
read_boolean (ChoiceSet *set)
{
  return g_strdup (set->choices[g_settings_get_boolean (set->settings, set->key) ? 0 : 1].value);
}

static void
write_boolean (ChoiceSet *set, const char *value)
{
  g_settings_set_boolean (set->settings, set->key, g_str_equal (value, set->choices[0].value));
}

/* Which corner a button-layout puts the buttons in: the left if close is
 * before the colon (GNOME lists the left side's buttons first) */
static char *
read_side (ChoiceSet *set)
{
  g_autofree char *layout = g_settings_get_string (set->settings, set->key);
  g_auto(GStrv) halves = g_strsplit (layout, ":", 2);
  g_auto(GStrv) left = g_strsplit (halves[0] != NULL ? halves[0] : "", ",", -1);

  return g_strdup (g_strv_contains ((const char * const *) left, "close") ? "left" : "right");
}

static void
write_side (ChoiceSet *set, const char *value)
{
  g_settings_set_string (set->settings, set->key,
                         g_str_equal (value, "left") ? LAYOUT_LEFT : LAYOUT_RIGHT);
}

/* A theme or title bar button style also restyles apps through the
 * account's gtk.css, which Patrone Settings knows how to do */
static void
run_patrone_settings (const char *option, const char *value)
{
  const char *argv[] = { "patrone-settings", option, value, NULL };
  g_autoptr(GError) error = NULL;

  if (!g_spawn_async (NULL, (char **) argv, NULL, G_SPAWN_SEARCH_PATH,
                      NULL, NULL, NULL, &error))
    g_warning ("Couldn't run patrone-settings %s %s: %s", option, value, error->message);
}

static void
write_theme (ChoiceSet *set, const char *value)
{
  run_patrone_settings ("--set-theme", value);
}

static void
write_title_buttons (ChoiceSet *set, const char *value)
{
  run_patrone_settings ("--set-title-buttons", value);
}

/* The family of a font setting such as "Noto Sans Bold 11" */
static char *
read_font (ChoiceSet *set)
{
  g_autofree char *current = g_settings_get_string (set->settings, set->key);
  g_autoptr(PangoFontDescription) font = pango_font_description_from_string (current);

  return g_strdup (pango_font_description_get_family (font));
}

/* Gives a font setting another family, keeping its style and size */
static void
set_font_family (GSettings *settings, const char *key, const char *family)
{
  g_autofree char *current = g_settings_get_string (settings, key);
  g_autoptr(PangoFontDescription) font = pango_font_description_from_string (current);
  g_autofree char *changed = NULL;

  pango_font_description_set_family (font, family);
  changed = pango_font_description_to_string (font);
  if (g_strcmp0 (current, changed) != 0)
    g_settings_set_string (settings, key, changed);
}

/* The interface and document fonts, and the title bar font for windows
 * that don't draw their own title bars; the monospace font stays */
static void
write_font (ChoiceSet *set, const char *value)
{
  g_autoptr(GSettings) wm = g_settings_new (WM_SCHEMA);

  set_font_family (set->settings, "font-name", value);
  set_font_family (set->settings, "document-font-name", value);
  set_font_family (wm, "titlebar-font", value);
}

static gboolean
font_installed (GtkWidget *widget, const char *family)
{
  g_autofree PangoFontFamily **families = NULL;
  int n_families = 0;

  pango_context_list_families (gtk_widget_get_pango_context (widget), &families, &n_families);
  for (int i = 0; i < n_families; i++)
    if (g_ascii_strcasecmp (pango_font_family_get_name (families[i]), family) == 0)
      return TRUE;
  return FALSE;
}

/* A whole-number setting (the pointer's size) as choices */
static char *
read_int (ChoiceSet *set)
{
  return g_strdup_printf ("%d", g_settings_get_int (set->settings, set->key));
}

static void
write_int (ChoiceSet *set, const char *value)
{
  g_settings_set_int (set->settings, set->key, (int) g_ascii_strtoll (value, NULL, 10));
}

/* The mouse pointer's themes and pictures */

static void
cursor_image_free (CursorImage *image)
{
  cairo_surface_destroy (image->surface);
  g_free (image);
}

static void
cursor_theme_free (CursorTheme *theme)
{
  g_free (theme->name);
  g_free (theme->title);
  g_free (theme->detail);
  for (guint i = 0; i < N_SHOWN; i++)
    g_clear_pointer (&theme->cursors[i], g_ptr_array_unref);
  g_free (theme);
}

/* Where cursor themes are, in the order they're looked up (libXcursor's
 * and GTK's): the account's, then the system's */
static GStrv
cursor_search_dirs (void)
{
  g_autoptr(GStrvBuilder) builder = g_strv_builder_new ();
  g_autoptr(GPtrArray) dirs = g_ptr_array_new_with_free_func (g_free);
  const char * const *system = g_get_system_data_dirs ();

  g_ptr_array_add (dirs, g_build_filename (g_get_user_data_dir (), "icons", NULL));
  g_ptr_array_add (dirs, g_build_filename (g_get_home_dir (), ".icons", NULL));
  for (; *system != NULL; system++)
    g_ptr_array_add (dirs, g_build_filename (*system, "icons", NULL));

  for (guint i = 0; i < dirs->len; i++)
    {
      const char *dir = g_ptr_array_index (dirs, i);
      gboolean seen = FALSE;

      for (guint j = 0; j < i && !seen; j++)
        seen = g_str_equal (dir, g_ptr_array_index (dirs, j));
      if (!seen)
        g_strv_builder_add (builder, dir);
    }
  return g_strv_builder_end (builder);
}

/* A value from a theme's index.theme, untranslated */
static char *
cursor_theme_index_value (const char * const *dirs, const char *theme, const char *key)
{
  for (; *dirs != NULL; dirs++)
    {
      g_autofree char *path = g_build_filename (*dirs, theme, "index.theme", NULL);
      g_autoptr(GKeyFile) index = g_key_file_new ();

      if (g_key_file_load_from_file (index, path, G_KEY_FILE_NONE, NULL))
        return g_key_file_get_string (index, "Icon Theme", key, NULL);
    }
  return NULL;
}

/* The file for cursor `name` in `theme` or a theme it inherits from (not
 * the system's default theme, which a missing one falls back to) */
static char *
find_cursor (const char * const *dirs, const char *theme, const char *name, guint depth)
{
  g_autofree char *inherits = NULL;
  g_auto(GStrv) parents = NULL;

  if (depth > MAX_INHERITS)
    return NULL;
  for (const char * const *dir = dirs; *dir != NULL; dir++)
    {
      g_autofree char *path = g_build_filename (*dir, theme, "cursors", name, NULL);

      if (g_file_test (path, G_FILE_TEST_IS_REGULAR))
        return g_steal_pointer (&path);
    }

  inherits = cursor_theme_index_value (dirs, theme, "Inherits");
  parents = g_strsplit_set (inherits != NULL ? inherits : "", ",;", -1);
  for (char **parent = parents; *parent != NULL; parent++)
    {
      char *found;

      g_strstrip (*parent);
      if (**parent == '\0' || g_str_equal (*parent, theme))
        continue;
      found = find_cursor (dirs, *parent, name, depth + 1);
      if (found != NULL)
        return found;
    }
  return NULL;
}

static gboolean
read_word (GBytes *bytes, gsize at, guint32 *word)
{
  gsize length;
  const guint8 *data = g_bytes_get_data (bytes, &length);

  if (at > length || length - at < 4)
    return FALSE;
  memcpy (word, data + at, 4);
  *word = GUINT32_FROM_LE (*word);
  return TRUE;
}

/* The first frame at each size in an Xcursor file, or NULL */
static GPtrArray *
parse_xcursor (GBytes *bytes)
{
  const guint32 image_type = 0xfffd0002;
  g_autoptr(GPtrArray) images = g_ptr_array_new_with_free_func ((GDestroyNotify) cursor_image_free);
  gsize length;
  const guint8 *data = g_bytes_get_data (bytes, &length);
  guint32 header, count;

  if (length < 16 || memcmp (data, "Xcur", 4) != 0 ||
      !read_word (bytes, 4, &header) || !read_word (bytes, 12, &count))
    return NULL;

  for (guint32 entry = 0; entry < MIN (count, 4096); entry++)
    {
      guint32 type, nominal, position, chunk_header, width, height, xhot, yhot;
      gsize at = (gsize) header + (gsize) entry * 12, start;
      gboolean seen = FALSE;
      CursorImage *image;
      guint8 *pixels;
      int stride;

      if (!read_word (bytes, at, &type) || !read_word (bytes, at + 4, &nominal) ||
          !read_word (bytes, at + 8, &position))
        return NULL;
      for (guint i = 0; i < images->len && !seen; i++)
        seen = ((CursorImage *) g_ptr_array_index (images, i))->nominal == nominal;
      if (type != image_type || seen)
        continue;
      if (!read_word (bytes, position, &chunk_header) || !read_word (bytes, (gsize) position + 16, &width) ||
          !read_word (bytes, (gsize) position + 20, &height) || !read_word (bytes, (gsize) position + 24, &xhot) ||
          !read_word (bytes, (gsize) position + 28, &yhot))
        return NULL;
      if (width == 0 || height == 0 || width > 0x7fff || height > 0x7fff)
        continue;
      start = (gsize) position + chunk_header;
      if (start > length || (length - start) / 4 / width < height)
        return NULL;

      image = g_new0 (CursorImage, 1);
      image->nominal = nominal;
      image->xhot = xhot;
      image->yhot = yhot;
      /* Little-endian ARGB words, premultiplied: cairo's ARGB32 in this
       * machine's byte order */
      image->surface = cairo_image_surface_create (CAIRO_FORMAT_ARGB32, (int) width, (int) height);
      pixels = cairo_image_surface_get_data (image->surface);
      stride = cairo_image_surface_get_stride (image->surface);
      for (guint32 y = 0; y < height; y++)
        for (guint32 x = 0; x < width; x++)
          {
            guint32 pixel;

            read_word (bytes, start + ((gsize) y * width + x) * 4, &pixel);
            memcpy (pixels + (gsize) y * stride + x * 4, &pixel, 4);
          }
      cairo_surface_mark_dirty (image->surface);
      g_ptr_array_add (images, image);
    }
  return images->len > 0 ? g_steal_pointer (&images) : NULL;
}

/* A cursor by the first of `names` that `theme` has, or NULL */
static GPtrArray *
load_cursor (const char * const *dirs, const char *theme, const char * const *names)
{
  for (; *names != NULL; names++)
    {
      g_autofree char *path = find_cursor (dirs, theme, *names, 0);
      g_autoptr(GFile) file = NULL;
      g_autoptr(GBytes) bytes = NULL;

      if (path == NULL)
        continue;
      file = g_file_new_for_path (path);
      bytes = g_file_load_bytes (file, NULL, NULL, NULL);
      return bytes != NULL ? parse_xcursor (bytes) : NULL;
    }
  return NULL;
}

static int
compare_cursor_themes (gconstpointer a, gconstpointer b)
{
  const CursorTheme *first = *(CursorTheme * const *) a;
  const CursorTheme *second = *(CursorTheme * const *) b;
  gboolean first_patrone = g_str_equal (first->name, "Patrone");
  gboolean second_patrone = g_str_equal (second->name, "Patrone");

  if (first_patrone != second_patrone)
    return first_patrone ? -1 : 1;
  return g_utf8_collate (first->title, second->title);
}

/* The cursor themes installed that GNOME can show, Patrone's first */
static GPtrArray *
find_cursor_themes (void)
{
  g_auto(GStrv) dirs = cursor_search_dirs ();
  GPtrArray *themes = g_ptr_array_new_with_free_func ((GDestroyNotify) cursor_theme_free);
  g_autoptr(GHashTable) seen = g_hash_table_new_full (g_str_hash, g_str_equal, g_free, NULL);

  for (guint d = 0; dirs[d] != NULL; d++)
    {
      g_autoptr(GDir) dir = g_dir_open (dirs[d], 0, NULL);
      const char *name;

      while (dir != NULL && (name = g_dir_read_name (dir)) != NULL)
        {
          g_autofree char *cursors = g_build_filename (dirs[d], name, "cursors", NULL);
          gboolean usable = TRUE;
          CursorTheme *theme;

          if (g_str_equal (name, "default") || g_hash_table_contains (seen, name) ||
              !g_file_test (cursors, G_FILE_TEST_IS_DIR))
            continue;
          g_hash_table_add (seen, g_strdup (name));
          for (guint i = 0; i < G_N_ELEMENTS (needed_cursors) && usable; i++)
            {
              g_autofree char *path = find_cursor ((const char * const *) dirs, name, needed_cursors[i], 0);
              usable = path != NULL;
            }
          if (!usable)
            continue;

          theme = g_new0 (CursorTheme, 1);
          theme->name = g_strdup (name);
          theme->title = cursor_theme_index_value ((const char * const *) dirs, name, "Name");
          if (theme->title == NULL)
            theme->title = g_strdup (name);
          for (guint i = 0; i < G_N_ELEMENTS (known_cursor_themes) && theme->detail == NULL; i++)
            if (g_str_equal (name, known_cursor_themes[i].value))
              theme->detail = g_strdup (_(known_cursor_themes[i].detail));
          if (theme->detail == NULL)
            theme->detail = g_strdup (d < 2 ? _("Installed for you") : _("Installed for everyone"));
          for (guint i = 0; i < N_SHOWN; i++)
            theme->cursors[i] = load_cursor ((const char * const *) dirs, name, shown_cursors[i]);
          g_ptr_array_add (themes, theme);
        }
    }
  g_ptr_array_sort (themes, compare_cursor_themes);
  return themes;
}

/* The picture to draw at `size` pixels: the smallest at least twice that
 * (scaled down, it stays sharp on a dense screen), or the biggest */
static CursorImage *
cursor_image_for (GPtrArray *cursor, double size)
{
  CursorImage *best = NULL, *biggest = NULL;

  for (guint i = 0; cursor != NULL && i < cursor->len; i++)
    {
      CursorImage *image = g_ptr_array_index (cursor, i);

      if (image->nominal >= size * 2 && (best == NULL || image->nominal < best->nominal))
        best = image;
      if (biggest == NULL || image->nominal > biggest->nominal)
        biggest = image;
    }
  return best != NULL ? best : biggest;
}

/* A cursor's picture at `size` pixels, its hotspot at (x, y) */
static void
draw_cursor (cairo_t *cr, GPtrArray *cursor, double x, double y, double size)
{
  CursorImage *image = cursor_image_for (cursor, size);
  double scale;

  if (image == NULL)
    return;
  scale = size / image->nominal;
  cairo_save (cr);
  cairo_translate (cr, x - image->xhot * scale, y - image->yhot * scale);
  cairo_scale (cr, scale, scale);
  cairo_set_source_surface (cr, image->surface, 0, 0);
  cairo_pattern_set_filter (cairo_get_source (cr), CAIRO_FILTER_GOOD);
  cairo_paint (cr);
  cairo_restore (cr);
}

static void
set_color (cairo_t *cr, guint32 rgb, double alpha)
{
  cairo_set_source_rgba (cr, ((rgb >> 16) & 0xff) / 255.0, ((rgb >> 8) & 0xff) / 255.0,
                         (rgb & 0xff) / 255.0, alpha);
}

static void
fill_rounded (cairo_t *cr, double x, double y, double w, double h, double r, guint32 rgb)
{
  r = MIN (r, MIN (w, h) / 2);
  cairo_new_sub_path (cr);
  cairo_arc (cr, x + w - r, y + r, r, -G_PI / 2, 0);
  cairo_arc (cr, x + w - r, y + h - r, r, 0, G_PI / 2);
  cairo_arc (cr, x + r, y + h - r, r, G_PI / 2, G_PI);
  cairo_arc (cr, x + r, y + r, r, G_PI, 1.5 * G_PI);
  cairo_close_path (cr);
  set_color (cr, rgb, 1.0);
  cairo_fill (cr);
}

/* Patrone Settings' desktop: navy to black, with a streak of blue light */
static void
draw_desktop (cairo_t *cr, double w, double h)
{
  cairo_pattern_t *gradient = cairo_pattern_create_linear (0, 0, w, h);

  cairo_new_sub_path (cr);
  cairo_arc (cr, w - 8, 8, 8, -G_PI / 2, 0);
  cairo_arc (cr, w - 8, h - 8, 8, 0, G_PI / 2);
  cairo_arc (cr, 8, h - 8, 8, G_PI / 2, G_PI);
  cairo_arc (cr, 8, 8, 8, G_PI, 1.5 * G_PI);
  cairo_close_path (cr);
  cairo_clip (cr);
  cairo_pattern_add_color_stop_rgb (gradient, 0, 0.06, 0.12, 0.22);
  cairo_pattern_add_color_stop_rgb (gradient, 1, 0.01, 0.01, 0.03);
  cairo_set_source (cr, gradient);
  cairo_paint (cr);
  cairo_pattern_destroy (gradient);
  cairo_move_to (cr, w * 0.58, 0);
  cairo_line_to (cr, w * 0.70, 0);
  cairo_line_to (cr, w * 0.32, h);
  cairo_line_to (cr, w * 0.20, h);
  cairo_close_path (cr);
  set_color (cr, 0x288bfa, 0.22);
  cairo_fill (cr);
}

/* A theme's pointers where they'd be (shown_cursors' order): the arrow and
 * the busy one on the desktop, the hand over a window's button and the
 * text cursor in its text. As Patrone Settings draws them */
static void
draw_pointer_theme (GtkDrawingArea *area, cairo_t *cr, int width, int height, gpointer data)
{
  CursorTheme *theme = data;
  double w = width, h = height;
  double x = w * 0.42, y = h * 0.14, ww = w * 0.5, wh = h * 0.72, bar = wh * 0.22;
  const double spots[N_SHOWN][2] = {
    { w * 0.13, h * 0.2 },
    { x + ww * 0.7, y + wh * 0.78 },
    { x + ww * 0.36, y + wh * 0.38 },
    { w * 0.2, h * 0.68 },
  };

  draw_desktop (cr, w, h);
  fill_rounded (cr, x, y, ww, wh, 6, 0xffffff);
  fill_rounded (cr, x, y, ww, bar, 6, 0xeceef2);
  cairo_rectangle (cr, x, y + bar / 2, ww, bar / 2);
  cairo_fill (cr);
  fill_rounded (cr, x + ww * 0.12, y + wh * 0.36, ww * 0.72, 4, 2, 0xc4c9d2);
  fill_rounded (cr, x + ww * 0.12, y + wh * 0.5, ww * 0.48, 4, 2, 0xc4c9d2);
  fill_rounded (cr, x + ww * 0.46, y + wh * 0.68, ww * 0.42, wh * 0.18, 4, 0x288bfa);
  for (guint i = 0; i < N_SHOWN; i++)
    draw_cursor (cr, theme->cursors[i], spots[i][0], spots[i][1], 26);
}

/* The set theme's arrow at a size, as big as it'd be on the screen, in the
 * middle of the picture */
static void
draw_pointer_size (GtkDrawingArea *area, cairo_t *cr, int width, int height, gpointer data)
{
  CcPatronePanel *self = data;
  double size = GPOINTER_TO_INT (g_object_get_data (G_OBJECT (area), "patrone-size"));
  CursorImage *image = cursor_image_for (self->size_arrow, size);
  double scale;

  draw_desktop (cr, width, height);
  if (image == NULL)
    return;
  scale = size / image->nominal;
  draw_cursor (cr, self->size_arrow,
               (width - cairo_image_surface_get_width (image->surface) * scale) / 2 + image->xhot * scale,
               (height - cairo_image_surface_get_height (image->surface) * scale) / 2 + image->yhot * scale,
               size);
}

static gboolean
restore_pointer (gpointer data)
{
  gtk_widget_set_cursor (GTK_WIDGET (data), NULL);
  return G_SOURCE_REMOVE;
}

/* Shows the pointer that's over this window in the new theme or size at
 * once. GNOME Shell drops its cursors when they change, but the one on
 * screen stays until an app asks for another; so for a moment the window
 * asks for the busy one, then for none of its own again */
static void
refresh_pointer (CcPatronePanel *self)
{
  GtkWidget *window = GTK_WIDGET (gtk_widget_get_root (GTK_WIDGET (self)));

  if (window == NULL)
    return;
  gtk_widget_set_cursor_from_name (window, "progress");
  g_timeout_add_full (G_PRIORITY_DEFAULT, 300, restore_pointer, g_object_ref (window), g_object_unref);
}

/* The theme changed, here or elsewhere: the sizes' pictures show its arrow */
static void
on_cursor_theme_changed (CcPatronePanel *self)
{
  g_auto(GStrv) dirs = cursor_search_dirs ();
  g_autofree char *theme = g_settings_get_string (self->interface, CURSOR_THEME_KEY);

  g_clear_pointer (&self->size_arrow, g_ptr_array_unref);
  self->size_arrow = load_cursor ((const char * const *) dirs, theme, arrow_names);
  for (guint i = 0; i < self->size_pictures->len; i++)
    gtk_widget_queue_draw (g_ptr_array_index (self->size_pictures, i));
  refresh_pointer (self);
}

/* Whether the settings have a key: an older Patrone Settings' schema may
 * not, and reading a missing key would end GNOME Settings */
static gboolean
has_key (GSettings *settings, const char *key)
{
  g_autoptr(GSettingsSchema) schema = NULL;

  g_object_get (settings, "settings-schema", &schema, NULL);
  return schema != NULL && g_settings_schema_has_key (schema, key);
}

static void
choice_set_free (ChoiceSet *set)
{
  g_ptr_array_unref (set->buttons);
  g_free (set);
}

/* Shows the setting's value picked */
static void
choice_set_sync (ChoiceSet *set)
{
  g_autofree char *current = set->read (set);

  for (guint i = 0; i < set->buttons->len; i++)
    {
      GtkToggleButton *button = g_ptr_array_index (set->buttons, i);
      const char *value = g_object_get_data (G_OBJECT (button), "patrone-value");

      if (g_strcmp0 (value, current) == 0 && !gtk_toggle_button_get_active (button))
        gtk_toggle_button_set_active (button, TRUE);
    }
}

static void
on_choice_toggled (GtkToggleButton *button,
                   ChoiceSet       *set)
{
  const char *value = g_object_get_data (G_OBJECT (button), "patrone-value");
  g_autofree char *current = NULL;

  if (!gtk_toggle_button_get_active (button))
    return;
  current = set->read (set);
  if (g_strcmp0 (value, current) != 0)
    set->write (set, value);
}

/* The cards */

static GtkWidget *
make_label (const char *text, const char *style)
{
  GtkWidget *label = gtk_label_new (text);

  gtk_label_set_xalign (GTK_LABEL (label), 0.0);
  gtk_label_set_wrap (GTK_LABEL (label), TRUE);
  gtk_widget_add_css_class (label, style);
  return label;
}

/* A picture (from Patrone Settings), a name and a line about it */
static GtkWidget *
make_picture_card (const char *prefix, const Choice *choice)
{
  GtkWidget *box = gtk_box_new (GTK_ORIENTATION_VERTICAL, 6);
  g_autofree char *path = g_strdup_printf ("%s/%s-%s.png", PREVIEWS_DIR, prefix, choice->value);

  if (g_file_test (path, G_FILE_TEST_EXISTS))
    {
      GtkWidget *picture = gtk_picture_new_for_filename (path);

      gtk_picture_set_content_fit (GTK_PICTURE (picture), GTK_CONTENT_FIT_CONTAIN);
      gtk_picture_set_can_shrink (GTK_PICTURE (picture), TRUE);
      /* A height of its own: in a window of ordinary height the flow box
       * gave a picture that can shrink its minimum, none at all */
      gtk_widget_set_size_request (picture, -1, 104);
      gtk_box_append (GTK_BOX (box), picture);
    }
  gtk_box_append (GTK_BOX (box), make_label (_(choice->title), "heading"));
  gtk_box_append (GTK_BOX (box), make_label (_(choice->detail), "dim-label"));
  return box;
}

/* A Start button icon and its name; GTK draws the -symbolic ones in the
 * text color */
static GtkWidget *
make_icon_card (CcPatronePanel *self, const Choice *choice)
{
  GtkWidget *box = gtk_box_new (GTK_ORIENTATION_VERTICAL, 8);
  GtkWidget *label = gtk_label_new (_(choice->title));

  if (self->icons_dir != NULL)
    {
      g_autoptr(GFile) file = g_file_get_child (self->icons_dir, choice->detail);
      g_autoptr(GIcon) icon = g_file_icon_new (file);
      GtkWidget *image = gtk_image_new_from_gicon (icon);

      gtk_image_set_pixel_size (GTK_IMAGE (image), 40);
      gtk_box_append (GTK_BOX (box), image);
    }
  gtk_widget_add_css_class (label, "heading");
  gtk_box_append (GTK_BOX (box), label);
  return box;
}

/* Text in a font, at a size in points, in its regular weight (buttons'
 * text is bold here) */
static GtkWidget *
make_sample (const char *text, const char *family, int points)
{
  GtkWidget *label = gtk_label_new (text);
  g_autoptr(PangoAttrList) attributes = pango_attr_list_new ();
  g_autoptr(PangoFontDescription) font = pango_font_description_new ();

  pango_font_description_set_family (font, family);
  pango_font_description_set_weight (font, PANGO_WEIGHT_NORMAL);
  pango_font_description_set_size (font, points * PANGO_SCALE);
  pango_attr_list_insert (attributes, pango_attr_font_desc_new (font));
  gtk_label_set_attributes (GTK_LABEL (label), attributes);
  gtk_label_set_xalign (GTK_LABEL (label), 0.0);
  return label;
}

/* Text in the font where the other cards have a picture, its name and a
 * line about it; a font that isn't installed says which package installs
 * it, and its card can't be picked ("patrone-unavailable") */
static GtkWidget *
make_font_card (CcPatronePanel *self, const Choice *choice)
{
  GtkWidget *box = gtk_box_new (GTK_ORIENTATION_VERTICAL, 6);
  GtkWidget *sample = gtk_box_new (GTK_ORIENTATION_VERTICAL, 2);
  gboolean installed = font_installed (GTK_WIDGET (self), choice->value);
  g_autofree char *missing = g_strdup_printf (_("Not installed: the %s package"),
                                              font_packages[choice - fonts]);

  gtk_widget_add_css_class (sample, "patrone-font-sample");
  gtk_widget_set_size_request (sample, -1, 104);
  gtk_box_append (GTK_BOX (sample), make_sample (FONT_SAMPLE_BIG, choice->value, 30));
  gtk_box_append (GTK_BOX (sample), make_sample (FONT_SAMPLE, choice->value, 15));
  gtk_box_append (GTK_BOX (box), sample);
  gtk_box_append (GTK_BOX (box), make_label (_(choice->title), "heading"));
  gtk_box_append (GTK_BOX (box), make_label (installed ? _(choice->detail) : missing, "dim-label"));
  if (!installed)
    g_object_set_data (G_OBJECT (box), "patrone-unavailable", GINT_TO_POINTER (TRUE));
  return box;
}

/* A mouse pointer theme's card ("pointer") or a pointer size's
 * ("pointer-size"): a picture drawn from the theme's own cursors, its name
 * and a line about it */
static GtkWidget *
make_pointer_card (CcPatronePanel *self, const char *prefix, const Choice *choice)
{
  GtkWidget *box = gtk_box_new (GTK_ORIENTATION_VERTICAL, 6);
  GtkWidget *picture = gtk_drawing_area_new ();

  /* As wide as the card, and as high as the other cards' pictures */
  gtk_drawing_area_set_content_width (GTK_DRAWING_AREA (picture), 120);
  gtk_widget_set_size_request (picture, -1, 104);
  gtk_widget_set_hexpand (picture, TRUE);
  if (g_str_equal (prefix, "pointer"))
    {
      gtk_drawing_area_set_draw_func (GTK_DRAWING_AREA (picture), draw_pointer_theme,
                                      g_ptr_array_index (self->cursor_themes, choice - self->cursor_choices),
                                      NULL);
    }
  else
    {
      g_object_set_data (G_OBJECT (picture), "patrone-size",
                         GINT_TO_POINTER ((int) g_ascii_strtoll (choice->value, NULL, 10)));
      gtk_drawing_area_set_draw_func (GTK_DRAWING_AREA (picture), draw_pointer_size, self, NULL);
      g_ptr_array_add (self->size_pictures, picture);
    }
  gtk_box_append (GTK_BOX (box), picture);
  gtk_box_append (GTK_BOX (box), make_label (_(choice->title), "heading"));
  gtk_box_append (GTK_BOX (box), make_label (_(choice->detail), "dim-label"));
  return box;
}

/* A choice's card: a picture's, a Start icon's (no prefix), a font's or a
 * mouse pointer's */
static GtkWidget *
make_card (CcPatronePanel *self, const char *prefix, const Choice *choice)
{
  if (prefix == NULL)
    return make_icon_card (self, choice);
  if (g_str_equal (prefix, "font"))
    return make_font_card (self, choice);
  if (g_str_has_prefix (prefix, "pointer"))
    return make_pointer_card (self, prefix, choice);
  return make_picture_card (prefix, choice);
}

/* A row of choices for one setting, in a group; `prefix` names their
 * pictures, or is NULL for Start button icons, "font" for fonts, or
 * "pointer" or "pointer-size" for the mouse pointer's */
static void
add_choices (CcPatronePanel      *self,
             AdwPreferencesGroup *group,
             GSettings           *settings,
             const char          *key,
             ReadFunc             read,
             WriteFunc            write,
             const Choice        *choices,
             guint                n_choices,
             const char          *prefix,
             guint                per_line)
{
  ChoiceSet *set = g_new0 (ChoiceSet, 1);
  GtkWidget *flow = gtk_flow_box_new ();
  GtkToggleButton *first = NULL;
  g_autofree char *signal = g_strdup_printf ("changed::%s", key);

  set->settings = settings;
  set->key = key;
  set->choices = choices;
  set->read = read;
  set->write = write;
  set->buttons = g_ptr_array_new ();

  gtk_flow_box_set_selection_mode (GTK_FLOW_BOX (flow), GTK_SELECTION_NONE);
  gtk_flow_box_set_homogeneous (GTK_FLOW_BOX (flow), TRUE);
  gtk_flow_box_set_min_children_per_line (GTK_FLOW_BOX (flow), per_line);
  gtk_flow_box_set_max_children_per_line (GTK_FLOW_BOX (flow), per_line);
  gtk_flow_box_set_column_spacing (GTK_FLOW_BOX (flow), 12);
  gtk_flow_box_set_row_spacing (GTK_FLOW_BOX (flow), 12);

  for (guint i = 0; i < n_choices; i++)
    {
      GtkWidget *button = gtk_toggle_button_new ();
      GtkWidget *card = make_card (self, prefix, &choices[i]);

      gtk_button_set_child (GTK_BUTTON (button), card);
      gtk_widget_set_sensitive (button, g_object_get_data (G_OBJECT (card), "patrone-unavailable") == NULL);
      gtk_widget_add_css_class (button, "patrone-choice");
      /* A name for screen readers, which otherwise hear a toggle button
       * with none: GTK doesn't name a button by the labels in a box inside
       * it (a Start icon's detail is its file, so it has no description) */
      gtk_accessible_update_property (GTK_ACCESSIBLE (button),
                                      GTK_ACCESSIBLE_PROPERTY_LABEL, _(choices[i].title),
                                      -1);
      if (prefix != NULL)
        gtk_accessible_update_property (GTK_ACCESSIBLE (button),
                                        GTK_ACCESSIBLE_PROPERTY_DESCRIPTION, _(choices[i].detail),
                                        -1);
      if (first != NULL)
        gtk_toggle_button_set_group (GTK_TOGGLE_BUTTON (button), first);
      else
        first = GTK_TOGGLE_BUTTON (button);
      g_object_set_data (G_OBJECT (button), "patrone-value", (gpointer) choices[i].value);
      gtk_flow_box_append (GTK_FLOW_BOX (flow), button);
      g_ptr_array_add (set->buttons, button);
    }

  /* Picked before the handlers, so showing the setting doesn't set it */
  choice_set_sync (set);
  for (guint i = 0; i < set->buttons->len; i++)
    g_signal_connect (g_ptr_array_index (set->buttons, i), "toggled",
                      G_CALLBACK (on_choice_toggled), set);
  set->changed_id = g_signal_connect_swapped (settings, signal, G_CALLBACK (choice_set_sync), set);

  adw_preferences_group_add (group, flow);
  g_ptr_array_add (self->choice_sets, set);
}

static AdwPreferencesGroup *
add_group (AdwPreferencesPage *page, const char *title, const char *description)
{
  AdwPreferencesGroup *group = ADW_PREFERENCES_GROUP (adw_preferences_group_new ());

  adw_preferences_group_set_title (group, title);
  if (description != NULL)
    adw_preferences_group_set_description (group, description);
  adw_preferences_page_add (page, group);
  return group;
}

static void
add_note (AdwPreferencesGroup *group, const char *title, const char *subtitle)
{
  GtkWidget *row = adw_action_row_new ();

  adw_preferences_row_set_title (ADW_PREFERENCES_ROW (row), title);
  adw_action_row_set_subtitle (ADW_ACTION_ROW (row), subtitle);
  adw_preferences_group_add (group, row);
}

static void
load_style (void)
{
  static gboolean loaded = FALSE;
  g_autoptr(GtkCssProvider) provider = NULL;

  if (loaded || gdk_display_get_default () == NULL)
    return;
  provider = gtk_css_provider_new ();
  gtk_css_provider_load_from_string (provider,
    ".patrone-choice { padding: 8px; border-radius: 12px; }\n"
    ".patrone-choice:checked { box-shadow: inset 0 0 0 2px var(--accent-bg-color); }\n"
    ".patrone-font-sample { padding: 8px 12px; border-radius: 8px; background: alpha(currentColor, 0.05); }\n");
  gtk_style_context_add_provider_for_display (gdk_display_get_default (),
                                              GTK_STYLE_PROVIDER (provider),
                                              GTK_STYLE_PROVIDER_PRIORITY_APPLICATION);
  loaded = TRUE;
}

static void
cc_patrone_panel_dispose (GObject *object)
{
  CcPatronePanel *self = CC_PATRONE_PANEL (object);

  /* No more setting changes reach the choices, which go with the panel */
  for (guint i = 0; self->choice_sets != NULL && i < self->choice_sets->len; i++)
    {
      ChoiceSet *set = g_ptr_array_index (self->choice_sets, i);
      g_clear_signal_handler (&set->changed_id, set->settings);
    }
  if (self->interface != NULL)
    {
      g_clear_signal_handler (&self->cursor_theme_id, self->interface);
      g_clear_signal_handler (&self->cursor_size_id, self->interface);
    }
  g_clear_object (&self->taskbar);
  g_clear_object (&self->desktop);
  g_clear_object (&self->wm);
  g_clear_object (&self->interface);
  g_clear_object (&self->icons_dir);

  G_OBJECT_CLASS (cc_patrone_panel_parent_class)->dispose (object);
}

static void
cc_patrone_panel_finalize (GObject *object)
{
  CcPatronePanel *self = CC_PATRONE_PANEL (object);

  g_clear_pointer (&self->choice_sets, g_ptr_array_unref);
  /* After the cards, whose pictures drew from these */
  g_clear_pointer (&self->cursor_choices, g_free);
  g_clear_pointer (&self->cursor_themes, g_ptr_array_unref);
  g_clear_pointer (&self->size_arrow, g_ptr_array_unref);
  g_clear_pointer (&self->size_pictures, g_ptr_array_unref);

  G_OBJECT_CLASS (cc_patrone_panel_parent_class)->finalize (object);
}

static void
cc_patrone_panel_class_init (CcPatronePanelClass *klass)
{
  GObjectClass *object_class = G_OBJECT_CLASS (klass);

  object_class->dispose = cc_patrone_panel_dispose;
  object_class->finalize = cc_patrone_panel_finalize;
}

static void
cc_patrone_panel_init (CcPatronePanel *self)
{
  GtkWidget *view = adw_toolbar_view_new ();
  AdwPreferencesPage *page = ADW_PREFERENCES_PAGE (adw_preferences_page_new ());
  AdwPreferencesGroup *group;

  load_style ();
  self->choice_sets = g_ptr_array_new_with_free_func ((GDestroyNotify) choice_set_free);
  self->taskbar = settings_if_installed (TASKBAR_SCHEMA);
  self->desktop = settings_if_installed (DESKTOP_SCHEMA);
  self->wm = g_settings_new (WM_SCHEMA);
  self->interface = g_settings_new (INTERFACE_SCHEMA);
  self->icons_dir = find_icons_dir ();

  if (self->taskbar != NULL)
    {
      group = add_group (page, _("Taskbar Size"), NULL);
      add_choices (self, group, self->taskbar, "size", read_string, write_string,
                   sizes, G_N_ELEMENTS (sizes), "size", 3);

      group = add_group (page, _("Taskbar Layout"),
                         _("Where the taskbar and its buttons go. Themes don't change it."));
      add_choices (self, group, self->taskbar, "layout", read_string, write_string,
                   layouts, G_N_ELEMENTS (layouts), "layout", 2);

      group = add_group (page, _("Start Button"), NULL);
      add_choices (self, group, self->taskbar, "start-icon", read_string, write_string,
                   start_icons, G_N_ELEMENTS (start_icons), NULL, 4);

      if (has_key (self->taskbar, "color"))
        {
          group = add_group (page, _("Taskbar Color"),
                             _("In the Patrone, Modern and Studio themes. Classic and Sapphire "
                               "keep their own colors."));
          add_choices (self, group, self->taskbar, "color", read_string, write_string,
                       colors, G_N_ELEMENTS (colors), "color", 2);
        }

      if (has_key (self->taskbar, "pin-icons"))
        {
          group = add_group (page, _("Pin Icons"),
                             _("What the taskbar's web, terminal, files and mail pins show."));
          add_choices (self, group, self->taskbar, "pin-icons", read_string, write_string,
                       pin_icons, G_N_ELEMENTS (pin_icons), "pins", 2);
        }

      if (has_key (self->taskbar, "desktop-bubble"))
        {
          group = add_group (page, _("Desktop Bubble"),
                             _("On, the desktop's files are kept off the desktop: a small pill at "
                               "the screen's left edge, shown whenever no window covers it, opens "
                               "them. Click it, or drag it out. It changes at once."));
          add_choices (self, group, self->taskbar, "desktop-bubble", read_boolean, write_boolean,
                       desktop_files, G_N_ELEMENTS (desktop_files), "files", 2);
        }

      if (has_key (self->taskbar, "show-recycle-bin"))
        {
          group = add_group (page, _("Recycle Bin"), NULL);
          add_choices (self, group, self->taskbar, "show-recycle-bin", read_boolean, write_boolean,
                       recycle_bin, G_N_ELEMENTS (recycle_bin), "bin", 2);
        }

      if (has_key (self->taskbar, "sign-in-terminal"))
        {
          group = add_group (page, _("Terminal at Sign-In"),
                             _("Patrone Terminal opens each time you sign in. Once you move it, "
                               "it stays where you put it. A new choice is for the next sign-in."));
          add_choices (self, group, self->taskbar, "sign-in-terminal", read_string, write_string,
                       sign_in_terminal, G_N_ELEMENTS (sign_in_terminal), "terminal", 2);
        }
    }
  else
    {
      group = add_group (page, _("Taskbar"), NULL);
      add_note (group, _("Patrone's taskbar isn't installed"),
                _("Install the patrone-gnome-taskbar package"));
    }

  group = add_group (page, _("Title Bar Buttons"),
                     _("Which corner of windows' title bars minimize, maximize and close go in. "
                       "Themes don't change it."));
  add_choices (self, group, self->wm, "button-layout", read_side, write_side,
               sides, G_N_ELEMENTS (sides), "buttons", 2);

  if (self->desktop != NULL && has_key (self->desktop, "title-buttons"))
    {
      group = add_group (page, _("Title Bar Button Style"),
                         _("How minimize, maximize and close look, the same in every app. "
                           "Apps that are open pick up a new style when they're next opened."));
      add_choices (self, group, self->desktop, "title-buttons", read_string, write_title_buttons,
                   button_styles, G_N_ELEMENTS (button_styles), "titlebuttons", 3);
    }

  if (self->desktop != NULL)
    {
      group = add_group (page, _("Theme"),
                         _("The look of the taskbar and title bars, and of the window buttons "
                           "when their style is the theme's own. Apps that are open pick up "
                           "a new theme when they're next opened."));
      add_choices (self, group, self->desktop, "theme", read_string, write_theme,
                   themes, G_N_ELEMENTS (themes), "theme", 3);
    }

  group = add_group (page, _("Font"),
                     _("For the desktop's and apps' text, at the size it is now. Monospace "
                       "text keeps its own font."));
  add_choices (self, group, self->interface, "font-name", read_font, write_font,
               fonts, G_N_ELEMENTS (fonts), "font", 2);

  group = add_group (page, _("Mouse Pointer"),
                     _("The pointer's look. It changes at once, on the desktop and in apps. "
                       "Themes made only for older X11 apps aren't listed: GNOME can't show "
                       "their pointers."));
  self->cursor_themes = find_cursor_themes ();
  self->cursor_choices = g_new0 (Choice, MAX (self->cursor_themes->len, 1));
  for (guint i = 0; i < self->cursor_themes->len; i++)
    {
      CursorTheme *theme = g_ptr_array_index (self->cursor_themes, i);

      self->cursor_choices[i] = (Choice) { theme->name, theme->title, theme->detail };
    }
  if (self->cursor_themes->len > 0)
    add_choices (self, group, self->interface, CURSOR_THEME_KEY, read_string, write_string,
                 self->cursor_choices, self->cursor_themes->len, "pointer", 3);
  else
    add_note (group, _("No cursor themes are installed"),
              _("Install the adwaita-icon-theme package"));

  group = add_group (page, _("Pointer Size"),
                     _("The same sizes as the Accessibility page's."));
  self->size_pictures = g_ptr_array_new ();
  add_choices (self, group, self->interface, CURSOR_SIZE_KEY, read_int, write_int,
               pointer_sizes, G_N_ELEMENTS (pointer_sizes), "pointer-size", 3);
  /* The sizes' pictures show the set theme's arrow, and the pointer over
   * the window changes at once */
  self->cursor_theme_id = g_signal_connect_swapped (self->interface, "changed::" CURSOR_THEME_KEY,
                                                    G_CALLBACK (on_cursor_theme_changed), self);
  self->cursor_size_id = g_signal_connect_swapped (self->interface, "changed::" CURSOR_SIZE_KEY,
                                                   G_CALLBACK (refresh_pointer), self);
  {
    g_auto(GStrv) dirs = cursor_search_dirs ();
    g_autofree char *theme = g_settings_get_string (self->interface, CURSOR_THEME_KEY);

    self->size_arrow = load_cursor ((const char * const *) dirs, theme, arrow_names);
  }

  adw_toolbar_view_add_top_bar (ADW_TOOLBAR_VIEW (view), adw_header_bar_new ());
  adw_toolbar_view_set_content (ADW_TOOLBAR_VIEW (view), GTK_WIDGET (page));
  adw_navigation_page_set_child (ADW_NAVIGATION_PAGE (self), view);
}

/* The page shows only where Patrone's taskbar or Patrone Settings is
 * installed */
void
cc_patrone_panel_static_init_func (void)
{
  g_autoptr(GSettings) taskbar = settings_if_installed (TASKBAR_SCHEMA);
  g_autoptr(GSettings) desktop = settings_if_installed (DESKTOP_SCHEMA);
  CcApplication *application;

  if (taskbar != NULL || desktop != NULL)
    return;
  application = CC_APPLICATION (g_application_get_default ());
  cc_shell_model_set_panel_visibility (cc_application_get_model (application),
                                       "patrone", CC_PANEL_HIDDEN);
}
