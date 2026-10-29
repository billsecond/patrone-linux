/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*- */
/*
 * Patrone's page in GNOME Settings: the taskbar (size, layout, Start
 * button icon), the desktop (its files in a bubble or across it, the
 * Recycle Bin, and where the terminal opens at sign-in), the title bar
 * buttons (top right or top left, and their style), the theme and the desktop's font (Patrone Sans or Noto Sans,
 * which the page sets itself). The same settings as Patrone Settings' window,
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
  { "Patrone Sans", N_("Patrone Sans"), N_("PatroneOS's own typeface, drawn from scratch for screens") },
  { "Noto Sans", N_("Noto Sans"), N_("A widely used open font, with Greek and Cyrillic too") },
};
static const char * const font_packages[] = { "fonts-patrone-sans", "fonts-noto-core" };

/* What a font's card shows, in that font */
#define FONT_SAMPLE_BIG "Aa"
#define FONT_SAMPLE "Il1 O0 10:00"

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

/* A choice's card: a picture's, a Start icon's (no prefix) or a font's */
static GtkWidget *
make_card (CcPatronePanel *self, const char *prefix, const Choice *choice)
{
  if (prefix == NULL)
    return make_icon_card (self, choice);
  if (g_str_equal (prefix, "font"))
    return make_font_card (self, choice);
  return make_picture_card (prefix, choice);
}

/* A row of choices for one setting, in a group; `prefix` names their
 * pictures, or is NULL for Start button icons or "font" for fonts */
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
