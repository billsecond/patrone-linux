# Patrone Settings

The settings Patrone adds to the desktop's own, packaged as
`patrone-settings`. It's a window of its own (Rust and GTK 4): GNOME
Settings takes no pages from other programs. Each choice is a card with a
picture of what it does (`src/preview.rs`, drawn with cairo).

```
patrone-settings [--page taskbar|buttons|theme]
```

The taskbar's right-click menu has "Taskbar settings", which opens the
Taskbar page; the launcher's actions open each page.

## Pages

| Page | Choices | Setting |
| --- | --- | --- |
| Taskbar | Small (32 pixels), Medium (42, the default), Large (52) | `org.gnome.shell.extensions.patrone-taskbar size` (the taskbar's own schema; the page says so if the taskbar isn't installed) |
| Title bar buttons | Top right (minimize, maximize, then close in the corner) or top left (close in the corner, then minimize, maximize) | GNOME's `org.gnome.desktop.wm.preferences button-layout`; open apps follow at once |
| Theme | Patrone, Modern, Classic, Studio | `org.patrone.desktop theme` (`data/org.patrone.desktop.gschema.xml`), plus the theme's style for apps |

Which corner the title bar buttons go in is the account's own choice: no
theme changes it (the user's decision, 2026-10-26).

## Themes

Four looks, named for what they are, not after other systems:

| Theme | Taskbar | Title bars and window buttons |
| --- | --- | --- |
| Patrone | Patrone's own, light or dark, the buttons on the left | GNOME's own |
| Modern | The same, the buttons centered | Flat, square buttons; close turns red |
| Classic | A deep blue bar, a green Start button that says "start", a lighter blue tray | Deep blue title bars with white titles, blue square buttons and a red close |
| Studio | Translucent, the buttons centered on a rounded shelf, dots under running apps | Round red, yellow and green buttons, their symbols on hover |

The taskbar follows `org.patrone.desktop theme` itself. For apps, Patrone
Settings puts the theme's style sheet (`data/themes/THEME/gtk-4.0.css` and
`gtk-3.0.css`, installed in `/usr/share/patrone/themes`) into the account's
`~/.config/gtk-4.0/gtk.css` and `~/.config/gtk-3.0/gtk.css`, as an `@import`
between two marked lines at the top. Anything else in those files is kept,
and since it comes after, the account's own rules still win. The Patrone
theme takes the lines out. GTK reads these files when an app starts, so
apps that are open pick a theme up when they're next opened; libadwaita
apps read them too.

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
button layout uses, and `--page`.

What was checked in the throwaway shell (2026-10-26): all three sizes (the
taskbar 33, 43 and 53 pixels tall with its border), Top left (open apps
moved their buttons at once), each theme on the taskbar, and Classic and
Studio on Text Editor's title bar, with the buttons staying top left.
