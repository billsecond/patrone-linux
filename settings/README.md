# Patrone Settings

The settings Patrone adds to the desktop's own, packaged as
`patrone-settings`: a window of its own (Rust and GTK 4), and the engine
behind the Patrone page in Patrone's build of GNOME Settings
(`gnome-settings/`), which has the same choices. Each choice is a card with
a picture of what it does (`src/preview.rs`, drawn with cairo).

```
patrone-settings [--page taskbar|buttons|theme]
patrone-settings --set-theme THEME          apply a theme, and quit
patrone-settings --set-title-buttons STYLE  apply a title bar button style, and quit
patrone-settings --apply                    write the account's GTK styles as set
patrone-settings --export-previews DIR      save the choices' pictures as PNGs
```

The taskbar's right-click menu has "Taskbar settings", which opens GNOME
Settings' Patrone page, or this window's Taskbar page where Patrone's build
of GNOME Settings isn't installed; the launcher's actions open each page.

## Pages

| Page | Choices | Setting |
| --- | --- | --- |
| Taskbar | Size: Small (32 pixels), Medium (42, the default), Large (52). Layout: Left (the default), Centered, Split (Start on the left, the apps in the middle), Top (along the top of the screen). Start button: the Patrone P, one color, orb, grid, menu, circle, spark, prompt. Color: from the wallpaper (the default), or the theme's own. Pin icons: Patrone's (the default), or each app's own | `org.gnome.shell.extensions.patrone-taskbar` `size`, `layout`, `start-icon`, `color` and `pin-icons` (the taskbar's own schema; the page says so if the taskbar isn't installed, and leaves out Color and Pin icons for a taskbar older than 0.3.2). The Start button's icons are the taskbar's, from its `icons` folder |
| Title bar buttons | Position: top right (minimize, maximize, then close in the corner) or top left (close in the corner, then minimize, maximize). Style: Theme's own, Wide, Round, Dots, Glossy | Position: GNOME's `org.gnome.desktop.wm.preferences button-layout`; open apps follow at once. Style: `org.patrone.desktop title-buttons`, plus its style sheet for apps |
| Theme | Patrone, Modern, Classic, Royal, Studio | `org.patrone.desktop theme` (`data/org.patrone.desktop.gschema.xml`), plus the theme's style for apps |

The taskbar's layout, and the title bar buttons' corner and style, are the
account's own choices: no theme changes them (the user's decisions,
2026-10-26), except that the style's default is the theme's own.

## Themes

Five looks, named for what they are, not after other systems:

| Theme | Taskbar | Title bars | Its own button style |
| --- | --- | --- | --- |
| Patrone | Patrone's own, light or dark | Pale blue-grey; in the dark style deep navy with a line of Patrone Blue | Wide |
| Modern | Flat: rounder buttons, no line along the bar, a blue line under each running app | Flat, the window's own color, no line; normal-weight titles | Round |
| Classic | Plain and square: a solid light or dark grey bar | Solid slate, white titles, square windows | Wide |
| Royal | Glossy royal blue, a green Start button (just its icon), a lighter blue tray | Deep blue, white titles | Glossy |
| Studio | Translucent, the apps on a rounded shelf, dots under running apps | Soft brushed grey, bold titles | Dots |

Patrone's, Modern's and Studio's title bars are for GTK 4 apps, light and
dark (`@media (prefers-color-scheme: dark)`, which GTK 4 sets from the
style); GTK 3 has no such rule, so GTK 3 apps keep their own title bars
in those themes. Until 2026-10-26 only Classic and Royal had title bars of
their own, so choosing among the others seemed to do nothing to windows.

GTK reads the account's `gtk.css` once, when an app starts (`gtksettings.c`
loads it and never again), so open apps can't follow a change: they show
it when they're next opened.

Royal was Classic until 2026-10-26, when the user asked for Classic to be
plainer and for the old look to stay as a theme of its own, without the word
"start" on the Start button.

## Title bar button styles

Four, named for what they look like (the user, 2026-10-26: configurable,
"don't say windows or gnome"):

| Style | Look |
| --- | --- |
| Wide | Flat and wide, in the title's color; close turns red under the pointer |
| Round | Small symbols in soft circles |
| Dots | Red, yellow and green dots, their symbols on hover, grey in the background |
| Glossy | Framed blue squares with white symbols, a red close |

They were part of the themes until the user found the buttons looked
different from one app to the next: libadwaita apps, other GTK 4 apps
(which Ubuntu's Yaru theme styles, its button symbols at least 30 pixels)
and GTK 3 apps each kept parts of their own look, and some buttons
vanished. Each style (`data/titlebuttons/STYLE/`) now sets every property
of the buttons and their symbols, in every state, for GTK 4 (`windowcontrols`)
and GTK 3 (`titlebutton`), so all of them match. Patrone File Manager uses
GTK's own window controls for the same reason.

## How styles reach apps

The taskbar follows `org.patrone.desktop theme` itself. For apps, Patrone
Settings puts the style sheets (the theme's title bars from
`data/themes/THEME/`, then the button style from
`data/titlebuttons/STYLE/`, installed in `/usr/share/patrone`) into the
account's `~/.config/gtk-4.0/gtk.css` and `~/.config/gtk-3.0/gtk.css`, as
`@import`s between two marked lines at the top. Anything else in those
files is kept, and since it comes after, the account's own rules still win.
GTK reads these files when an app starts, so apps that are open pick a
change up when they're next opened; libadwaita apps read them too.
`--set-theme` and `--set-title-buttons` do the same without a window, for
GNOME Settings' Patrone page. `--apply` writes them for the settings as
they are: an autostart entry (`/etc/xdg/autostart/org.patrone.Settings-apply.desktop`)
runs it at every sign-in, so every account has the same buttons from the
start, and an account's lines move to new versions of the style sheets
(Modern's and Studio's sheets went away when the buttons became their own
setting, and came back in 0.3.1 with title bars only).

## Pictures

`--export-previews` draws every choice's picture at twice the cards' size
(376 by 236) and saves it as a PNG (through a GDK texture: this build of
cairo has no PNG writer): `size-*`, `layout-*`, `color-*`, `pins-*`,
`buttons-*`, `titlebuttons-*` and `theme-*`.
It needs no display. The package runs it at build time and installs them in
`/usr/share/patrone/settings/previews`, where GNOME Settings' Patrone page
finds them.

## Building and trying it

```
cargo build
cargo test
PATRONE_SETTINGS_DATA=$PWD/data target/debug/patrone-settings
```

The Theme page needs the `org.patrone.desktop` schema installed (or
compiled into a folder named by `GSETTINGS_SCHEMA_DIR`); the Taskbar page,
the taskbar's. `shell/gnome/tests/headless.sh` compiles both into its
throwaway GNOME Shell, where Patrone Settings runs with
`tests/headless.sh run env PATRONE_SETTINGS_DATA=... patrone-settings`.

The tests (`cargo test`) cover how the style sheets' lines go into and come
out of `gtk.css` without touching the account's own rules, which sheets each
theme and button style brings (and that every one of them exists in
`data/`), which corner a button layout uses, `--page` and the other
options, and that every theme is one the theme setting takes.

What was checked in the throwaway shell: all three sizes (the taskbar 33,
43 and 53 pixels tall with its border), Top left (open apps moved their
buttons at once), each theme on the taskbar in light and dark, Classic and
Studio on Text Editor's title bar with the buttons staying top left
(2026-10-26, before Royal), and the Taskbar page with its layouts and Start
icons (the one-color ones in the text color). The pictures were checked as
exported by the package build. For 0.3.0, each button style in five apps
(Disks on GTK 3, Calculator and Text Editor on libadwaita, Patrone Terminal
and Patrone Settings on plain GTK 4), which came out the same in each, and
Patrone File Manager's buttons in the Wide and Dots styles, top right and
top left.
