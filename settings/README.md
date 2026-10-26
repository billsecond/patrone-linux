# Patrone Settings

The settings Patrone adds to the desktop's own, packaged as
`patrone-settings`: a window of its own (Rust and GTK 4), and the engine
behind the Patrone page in Patrone's build of GNOME Settings
(`gnome-settings/`), which has the same choices. Each choice is a card with
a picture of what it does (`src/preview.rs`, drawn with cairo).

```
patrone-settings [--page taskbar|buttons|theme]
patrone-settings --set-theme THEME       apply a theme, and quit
patrone-settings --export-previews DIR   save the choices' pictures as PNGs
```

The taskbar's right-click menu has "Taskbar settings", which opens GNOME
Settings' Patrone page, or this window's Taskbar page where Patrone's build
of GNOME Settings isn't installed; the launcher's actions open each page.

## Pages

| Page | Choices | Setting |
| --- | --- | --- |
| Taskbar | Size: Small (32 pixels), Medium (42, the default), Large (52). Layout: Left (the default), Centered, Split (Start on the left, the apps in the middle), Top (along the top of the screen). Start button: the Patrone P, one color, orb, grid, menu, circle, spark, prompt | `org.gnome.shell.extensions.patrone-taskbar` `size`, `layout` and `start-icon` (the taskbar's own schema; the page says so if the taskbar isn't installed). The Start button's icons are the taskbar's, from its `icons` folder |
| Title bar buttons | Top right (minimize, maximize, then close in the corner) or top left (close in the corner, then minimize, maximize) | GNOME's `org.gnome.desktop.wm.preferences button-layout`; open apps follow at once |
| Theme | Patrone, Modern, Classic, Royal, Studio | `org.patrone.desktop theme` (`data/org.patrone.desktop.gschema.xml`), plus the theme's style for apps |

The taskbar's layout and which corner the title bar buttons go in are the
account's own choices: no theme changes them (the user's decisions,
2026-10-26).

## Themes

Five looks, named for what they are, not after other systems:

| Theme | Taskbar | Title bars and window buttons |
| --- | --- | --- |
| Patrone | Patrone's own, light or dark | GNOME's own |
| Modern | Flat: rounder buttons, no line along the bar, a blue line under each running app | Flat, square buttons; close turns red |
| Classic | Plain and square: a solid light or dark grey bar | Solid slate title bars with white titles, flat square buttons, close red under the pointer, square windows |
| Royal | Glossy royal blue, a green Start button (just its icon), a lighter blue tray | Deep blue title bars with white titles, blue square buttons and a red close |
| Studio | Translucent, the apps on a rounded shelf, dots under running apps | Round red, yellow and green buttons, their symbols on hover |

Royal was Classic until 2026-10-26, when the user asked for Classic to be
plainer and for the old look to stay as a theme of its own, without the word
"start" on the Start button.

The taskbar follows `org.patrone.desktop theme` itself. For apps, Patrone
Settings puts the theme's style sheet (`data/themes/THEME/gtk-4.0.css` and
`gtk-3.0.css`, installed in `/usr/share/patrone/themes`) into the account's
`~/.config/gtk-4.0/gtk.css` and `~/.config/gtk-3.0/gtk.css`, as an `@import`
between two marked lines at the top. Anything else in those files is kept,
and since it comes after, the account's own rules still win. The Patrone
theme takes the lines out. GTK reads these files when an app starts, so
apps that are open pick a theme up when they're next opened; libadwaita
apps read them too. `--set-theme` does the same without a window, for
GNOME Settings' Patrone page.

## Pictures

`--export-previews` draws every choice's picture at twice the cards' size
(376 by 236) and saves it as a PNG (through a GDK texture: this build of
cairo has no PNG writer): `size-*`, `layout-*`, `buttons-*` and `theme-*`.
It needs no display. The package runs it at build time and installs them in
`/usr/share/patrone/settings/previews`, where GNOME Settings' Patrone page
finds them.

## Building and trying it

```
cargo build
cargo test
PATRONE_SETTINGS_THEMES=$PWD/data/themes target/debug/patrone-settings
```

The Theme page needs the `org.patrone.desktop` schema installed (or
compiled into a folder named by `GSETTINGS_SCHEMA_DIR`); the Taskbar page,
the taskbar's. `shell/gnome/tests/headless.sh` compiles both into its
throwaway GNOME Shell, where Patrone Settings runs with
`tests/headless.sh run env PATRONE_SETTINGS_THEMES=... patrone-settings`.

The tests (`cargo test`) cover how the theme's lines go into and come out
of `gtk.css` without touching the account's own rules, which corner a
button layout uses, `--page` and the other options, and that every theme is
one the theme setting takes.

What was checked in the throwaway shell: all three sizes (the taskbar 33,
43 and 53 pixels tall with its border), Top left (open apps moved their
buttons at once), each theme on the taskbar in light and dark, Classic and
Studio on Text Editor's title bar with the buttons staying top left
(2026-10-26, before Royal), and the Taskbar page with its layouts and Start
icons (the one-color ones in the text color). The pictures were checked as
exported by the package build.
