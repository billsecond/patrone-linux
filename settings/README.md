# Patrone Settings

The settings Patrone adds to the desktop's own, packaged as
`patrone-settings`: a window of its own (Rust and GTK 4), and the engine
behind the Patrone page in Patrone's build of GNOME Settings
(`gnome-settings/`), which has the same choices. Each choice is a card with
a picture of what it does (`src/preview.rs`, drawn with cairo).

```
patrone-settings [--page taskbar|desktop|buttons|theme|pointer]
patrone-settings --set-theme THEME          apply a theme, and quit
patrone-settings --set-title-buttons STYLE  apply a title bar button style, and quit
patrone-settings --apply                    write the account's GTK styles as set
patrone-settings --export-previews DIR      save the choices' pictures as PNGs
```

The taskbar's right-click menu has "Taskbar settings", which opens GNOME
Settings' Patrone page, or this window's Taskbar page where Patrone's build
of GNOME Settings isn't installed; the launcher's actions (its right-click
menu in the Patrone menu and on the taskbar) open each page.

## Pages

| Page | Choices | Setting |
| --- | --- | --- |
| Taskbar | Size: Small (32 pixels), Medium (42, the default), Large (52). Layout: Left (the default), Centered, Split (Start on the left, the apps in the middle), Top (along the top of the screen). Start button: the Patrone P, one color, orb, grid, menu, circle, spark, prompt. Color: from the wallpaper (the default), or the theme's own; only the Patrone, Modern and Studio themes take the wallpaper's color, and the page says so. Pin icons: Patrone's (the default), or each app's own | `org.gnome.shell.extensions.patrone-taskbar` `size`, `layout`, `start-icon`, `color` and `pin-icons` (the taskbar's own schema; the page says so if the taskbar isn't installed, and leaves out Color and Pin icons for a taskbar older than 0.3.2). The Start button's icons are the taskbar's, from its `icons` folder |
| Desktop | Desktop bubble: On (the default: the desktop's files in a bubble, which a pill at the screen's left edge opens, over any window) or Off (the files across the desktop); it changes at once. GNOME Settings' Desktop page has the same as a switch. Recycle Bin: On the desktop (the default) or Hidden. Terminal at sign-in: Over the logo (the default: large, over the wallpaper's logo, at any screen size) or Centered (at its own size, in the middle of the screen) | `org.gnome.shell.extensions.patrone-taskbar` `desktop-bubble` and `show-recycle-bin`, both yes-or-no, and `sign-in-terminal` (`logo` or `center`) (the taskbar's own schema; each is left out for a taskbar too old to have it). See `shell/gnome/README.md`, "The desktop bubble" and "The terminal at sign-in" |
| Title bar buttons | Position: top right (minimize, maximize, then close in the corner) or top left (close in the corner, then minimize, maximize). Style: Theme's own, Wide, Round, Dots, Glossy | Position: GNOME's `org.gnome.desktop.wm.preferences button-layout`; open apps follow at once. Style: `org.patrone.desktop title-buttons`, plus its style sheet for apps |
| Theme | Themes: Patrone, Modern, Classic, Sapphire, Studio. Font: Patrone Sans (the default, `fonts/`, the `fonts-patrone-sans` package) or Noto Sans (the font it's based on) | Theme: `org.patrone.desktop theme` (`data/org.patrone.desktop.gschema.xml`), plus the theme's style for apps. Font: the family of GNOME's `org.gnome.desktop.interface font-name` and `document-font-name` and `org.gnome.desktop.wm.preferences titlebar-font`, their sizes and styles kept (`src/font.rs`); the monospace font stays. Open apps follow at once. A font that isn't installed can't be picked, and its card names its package |
| Mouse pointer | Pointer: each cursor theme installed that GNOME can show (below), Patrone's first. Size: Default (24 pixels), Medium (32), Large (48), Larger (64), Largest (96), as GNOME Settings' Accessibility page has them | GNOME's `org.gnome.desktop.interface cursor-theme` and `cursor-size` (`src/cursor.rs`). The pointer changes at once (below) |

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
| Sapphire | Glossy sapphire blue, a green Start button (just its icon), a lighter blue tray | Deep blue, white titles | Glossy |
| Studio | Translucent, the apps on a rounded shelf, dots under running apps | Soft brushed grey, bold titles | Dots |

Patrone's, Modern's and Studio's title bars are for GTK 4 apps, light and
dark (`@media (prefers-color-scheme: dark)`, which GTK 4 sets from the
style); GTK 3 has no such rule, so GTK 3 apps keep their own title bars
in those themes. Until 2026-10-26 only Classic and Sapphire had title bars of
their own, so choosing among the others seemed to do nothing to windows.

GTK reads the account's `gtk.css` once, when an app starts (`gtksettings.c`
loads it and never again), so open apps can't follow a change: they show
it when they're next opened.

Sapphire was Classic until 2026-10-26, when the user asked for Classic to be
plainer and for the old look to stay as a theme of its own, without the word
"start" on the Start button. It was called Royal until 0.3.7, which renamed
it to lean less on Windows XP's look (`docs/COMPLIANCE.md`, item 15); its
setting value is still `royal`, so the choice of anyone who had it stays.

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

## Mouse pointer

The user asked for it, 2026-10-29: "add a place where I can choose my mouse
theme". GNOME Settings has no cursor theme choice, only a size (under
Accessibility). The page lists the cursor themes in
`~/.local/share/icons`, `~/.icons` and the system's `icons` folders
(`/usr/share/icons`), the account's first, as GTK and libXcursor look them
up. Each card's picture is drawn from the theme's own Xcursor files: its
arrow and busy pointer on a desktop, its hand over a button and its text
cursor in a window's text. The sizes' pictures show the set theme's arrow
at each size.

Only themes that have (or inherit) the cursor names GNOME asks for,
`default`, `pointer` and `text`, are listed. GNOME Shell's Xcursor loader
looks a cursor up by its name in the theme and the themes it inherits from,
then in the system's default theme (`/usr/share/icons/default`, Patrone's),
and only then by its old X11 name (`left_ptr`, `hand2`, `xterm`). So DMZ
White and DMZ Black (`dmz-cursor-theme`) and X.Org's whiteglass, redglass
and handhelds (`xcursor-themes`), which have only the old names, show
Patrone's pointers in GNOME, and picking one would seem to do nothing
(checked in the throwaway shell, 2026-10-29). A fresh install has Patrone's,
Adwaita (GNOME's), Yaru, and, with `patrone-desktop` 0.2.5, Breeze Dark and
Breeze Light (`desktop/README.md`).

GNOME Shell drops its cursors when the theme or size changes, but a pointer
already on screen keeps its picture until the app under it asks for another
(a size change shows at once). So after a change, this window asks for the
busy pointer for 300 milliseconds and then for none of its own, and the
pointer over it shows the new theme at once; GNOME Settings' Patrone page
does the same. Other windows, and the taskbar, show it as soon as the
pointer enters them: GTK 4 and GTK 3 apps (Disks, checked) ask GNOME Shell
for pointers by name (the cursor-shape protocol), so GNOME Shell draws them.
Not checked: X11 apps running through Xwayland, which load their pointers
themselves and may keep the old one until they're opened again.

## Pictures

The font cards have no picture: each shows "Aa" and "Il1 O0 10:00" in its
own font, drawn by GTK, here and in GNOME Settings, since the package's
build has no Patrone Sans to draw pictures with. The mouse pointer cards'
pictures are drawn when the page opens, from the themes installed, and
aren't exported.

`--export-previews` draws every choice's picture at twice the cards' size
(376 by 236) and saves it as a PNG (through a GDK texture: this build of
cairo has no PNG writer): `size-*`, `layout-*`, `color-*`, `pins-*`,
`files-*`, `bin-*`, `buttons-*`, `titlebuttons-*` and `theme-*`.
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
options, that every theme is one the theme setting takes, and reading
Xcursor files and cursor themes (sizes, broken files, Inherits chains, a
theme with only the old X11 names).

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

For 0.3.8 (2026-10-29), every card on every page was clicked with a
virtual pointer in the throwaway shell, first with 0.3.7 as built from
`main`, then with 0.3.8: each one picked itself, set its setting, and the
taskbar followed at once (its size, place, layout, Start icon, color and
theme read back from GNOME Shell; the pin icons seen in screenshots; the
bubble's and the Recycle Bin's windows came and went), as did `gtk.css` for
the theme and button styles and
GNOME's three font keys. Every page opened from the sidebar, from
`--page` and from the launcher's actions; 0.3.7's launcher had no Desktop
action. On the Mouse pointer page, each theme and size set GNOME's setting,
and the pointer standing over the card just clicked was in the new theme
in a screenshot taken a second later (Adwaita, Yaru and Patrone; Breeze was
tried from the test's own folder, which GNOME Shell doesn't search, so only
its cards and setting).
