# Patrone Taskbar (GNOME Shell)

Patrone Desktop's taskbar for the GNOME-based desktop Patrone runs today,
packaged as `patrone-gnome-taskbar`. It's a GNOME Shell extension
(`patrone-taskbar@patrone-linux`), on for every account, until Patrone's own
shell (`shell/`, GTK4) replaces GNOME Shell. The design carries over to that
shell.

Installing the package turns it on for every account, but GNOME Shell only
loads extensions when it starts: log out from inside the desktop (Start →
the account picture → Sign out, or GNOME's top bar's power menu → Log Out)
and sign in again. Disconnecting Remote Desktop or signing
out of the screen's session leaves the desktop, and its old shell, running.
The same goes for every update: the desktop keeps running the old taskbar,
whose settings a new Patrone Settings may not know, until the next sign-in.
So the taskbar watches its own `metadata.json` (which every package update
replaces) and then shows a notification, "The taskbar was updated", with a
Sign out button. (The user hit this twice: after 0.2.0 Patrone Settings'
clicks did nothing, and after 0.3.0 the layouts, Start icons and Royal
didn't show.)

## What it does

One compact bar along the bottom of the primary monitor, in place of GNOME's
top bar, which it hides: medium (42 pixels) unless Patrone Settings makes it
small (32) or large (52), its icons to match. Its look follows Patrone
Settings' theme (see `settings/README.md`): Patrone's own, Modern (flat),
Classic (plain and square), Royal (glossy royal blue, a green Start button)
or Studio (translucent, the buttons on a rounded shelf).

In Patrone, Modern and Studio the bar takes its **color from the
wallpaper** (`tint.js`): the picture's average hue, at the theme's own
lightness and see-through, with its saturation capped so a vivid picture
still gives a calm bar. Patrone's navy wallpaper gives a navy bar, Graphite
a graphite one, the light wallpaper a pale blue one. The picture is the one
the desktop shows in the current style (`picture-uri-dark` in the dark
style), decoded at 64 by 36 in a thread when it changes; a plain color
background gives its color, and a slideshow the theme's grey. The `color`
setting at `theme` keeps the theme's grey. Classic and Royal keep their own
colors. (The user, 2026-10-26: the grey bar didn't match the wallpaper.)

From the left:

- **Start** (the Patrone "P", or the Super key) opens the Start menu with
  the search box focused, so typing searches right away (below).
- **Search** opens the same menu in search mode: the most used apps and
  recent files, then results as you type.
- **Pinned items**: apps, and **slots** for a job: web browser, terminal,
  file manager, mail, documents, spreadsheets, presentations, PDF files and
  art (the defaults, in that order). A slot shows Patrone's icon for its
  job (a globe, a prompt, a folder, an envelope, a page, a sheet of cells,
  a screen with a chart, a red-banded page, a palette, in
  `icons/pin-*.svg`), or with the `pin-icons` setting at `app`, the icon of
  the app it opens. It opens whichever app is the default for its job, and
  that app's windows show under it, not as a separate button. For most
  that's the system default for its file types (Settings → Apps → Default
  Apps): WordCraft for documents while it's the default for .docx and
  .odt, LibreOffice Writer once that is (the user, 2026-10-26: "just icons
  if those are the default apps, otherwise if libreoffice is installed and
  its the primary app for each make those the primary"). The terminal is
  Patrone Terminal and art is ArtCraft (else LibreOffice Draw, GIMP,
  Krita, Inkscape or Pinta) unless changed in the slot's menu. A slot no
  installed app can do isn't shown. With Patrone's icons, Settings shows
  Patrone's gear too, so it matches.
- **Running apps** that aren't pinned, in the order they started. An app
  shows the moment it's opened, its icon pulsing until its first window
  appears, as on Windows, so a slow one (a snap or a Flatpak starting for
  the first time) still shows it was clicked. A short line under an app
  means it's running, a longer blue one that it's in front.
- On the right, the **tray**: the top bar's own status icons and menus
  (network, sound, power, input sources, other apps' indicators), then the
  clock, Windows-style (time over date), with the calendar and notifications
  behind it, and the **Show desktop** sliver at the far right.

Clicking a button works as on Windows: it starts the app, shows its window,
or minimizes it when it's already in front; with several windows each click
brings the next. A middle click, or Shift/Ctrl with a click, opens a new
window. Hovering shows the app's name.

**Dragging**: a button dragged along the bar moves there (a gap shows
where); an app that's only running is pinned where it's dropped, and so is
an app dragged from the Start menu (Pinned or All apps). In the Start menu,
pinned apps are dragged to reorder them, a blue line marking where.

### Right-click: jump lists

As on Windows 11: the app's **Recent** files (those GTK recorded for it, then
recent files of types it opens; not for browsers), or for the file manager its
**Pinned** folders (the file manager's bookmarks, else Home, Documents,
Downloads...); its **Tasks** (the app's own actions: New Window, New
Incognito Window...); its **Open windows** when it has several; then the app
itself (a new window), **Pin to taskbar** / **Unpin from taskbar**, for a slot
**Default web browser** (and so on) to change which app the job uses, and
**Close window** / **Close all windows**. Right-click on the empty bar:
Task Manager, Show desktop, and Taskbar settings (Patrone Settings, when
it's installed).

### The Start menu

A Windows 11 Start menu in a smaller size (540 pixels wide):

- **Search box**, focused when the menu opens. Typing searches apps at once,
  then, as typing pauses, everything GNOME's own search covers (Settings,
  Files, Calculator, Characters, Clocks...: the same providers as the
  Activities overview, so apps that add a search provider show up here too).
  Results: **Best match** first, then by kind, and **Search the web** last.
  Up and Down move through them, Enter opens the selected one: the best match,
  or a web search when nothing else matched. Web searches go to the
  `web-search-url` setting (Google by default) in the default browser.
- **Pinned**: GNOME's favorites (the same list as the Activities overview's
  dash), up to 18. **All apps** lists every app, A to Z.
- **Recommended**: recently used files.
- At the bottom, as on Windows 11: the **account** (picture and name), whose
  menu shows who's signed in, Change account settings (Settings → Users),
  **Lock** and **Sign out**; Settings; and **power**, whose menu has
  **Disconnect** while the desktop is seen through Remote Desktop (as on a
  Windows remote session: it ends the connection, or the screen's view, and
  leaves the desktop running for next time), then **Sleep**, **Shut down**
  and **Restart**, whichever the system allows. With none of those, the
  power button is hidden.

Right-click an app in the menu to pin it to Start or the taskbar.

### The Recycle Bin on the desktop

Ubuntu's desktop icons extension shows the account's desktop files, but
Patrone turns off its Home and Trash icons (`92_patrone-taskbar.gschema.override`,
`show-home` and `show-trash`), and the taskbar draws its own **Recycle Bin**
(`recycleBin.js`), as the user asked on 2026-10-26, so it can stay docked to
a corner: bottom right unless moved, inside the taskbar.

- Double-click opens it in the file manager (`trash:///`, which opens in
  whatever opens folders: Patrone File Manager).
- Its icon is Patrone's own (`icons/recycle-bin*.svg`; the user asked for
  one "more modern than windows/mac"): a Patrone Blue bin under a floating
  lid, with a white loop of an arrow on it, and when something's in it,
  two sheets of paper lifting the lid.
- Right-click: **Open**, **Empty Recycle Bin** (after asking; Cancel is the
  default), and **Keep docked to** Bottom right, Bottom left, Top right, Top
  left, or Not docked.
- Dragged while docked, it docks to the nearest corner; not docked, it stays
  where it's dropped.
- It sits above the wallpaper and the desktop icons' window and below every
  other window, and follows the free area when the screen size changes.
  That window starts as an ordinary one and only then becomes a desktop
  window and lowers itself, which GNOME Shell doesn't count as a restack;
  0.3.3 missed that, so the Recycle Bin stayed under it and couldn't be
  clicked or dragged (the user found it, 2026-10-26). It now follows each
  window's type, and knows the desktop icons' window by their own
  `customJS_ding` mark too. While it's dragged it holds the pointer, or the
  window the pointer passes over (that full-screen one) takes the drag.
  0.3.4 still didn't work: on Wayland the desktop icons extension takes its
  window out of `global.get_window_actors()` (its `gnomeShellOverride.js`),
  so the Recycle Bin looked there and never found it. 0.3.5 looks at the
  window group's own children. Checked once with the real desktop icons
  running in the throwaway shell: the Recycle Bin above their window, its
  own menu on right-click, and dragging.

- **Files and folders dropped on it go to the trash** (0.3.7; the user: "I
  can't drag into the trash a folder"). GNOME Shell's actors take no drops
  from apps, so the taskbar runs `dropTarget.js` (GJS and GTK 4, like the
  desktop icons' own window) as a Wayland client of its own: a window the
  Recycle Bin's size, made a desktop window once it shows (no taskbar
  button, Alt+Tab or focus), on every workspace, right under the Recycle
  Bin. While an app drags something (`dnd-enter` to `dnd-leave` from
  `global.backend.get_dnd()`) the Recycle Bin stops taking the pointer, so
  the drop reaches that window, which trashes what it's given; the Recycle
  Bin lights up while the drag is over it. Drawn fully clear, GTK gave the
  window no buffer and it never showed, so its background is 1% black.
- **The desktop icons keep out of its column** (0.3.7: a folder moved under
  it couldn't be clicked), through the desktop icons' area API
  (`DesktopIconsUsableArea.setMarginsForExtension`, which Ubuntu's dock
  uses too). It takes whole margins, so that's the strip from the screen's
  edge to the Recycle Bin's far side.
- **Remove from desktop** in its menu turns `show-recycle-bin` off; Patrone
  Settings' Desktop page brings it back, and so does **Show Recycle Bin** in
  GNOME's desktop menu (0.3.11; the user asked for it on the desktop's
  right-click). That's the menu a right-click on the empty desktop opens
  while the desktop bubble keeps the desktop icons out of sight. GNOME makes
  one for each screen and wallpaper (`ui/backgroundMenu.js`), so the
  Recycle Bin overrides `BackgroundMenu.prototype.open` through GNOME's
  `InjectionManager` and adds the item, under Change Background, as each
  opens; it shows only while the bin's off the desktop. With the bubble
  off, the right-click goes to the desktop icons' own menu, which is built
  inside their program (`desktopManager.js`, in Ubuntu's
  `gnome-shell-ubuntu-extensions`) and has no place to add to.
- **Over Remote Desktop the desktop starts with no screen**, until the
  client's arrives. 0.3.10 read the screen's size then, got nothing, and
  the error took the whole taskbar down at the user's sign-in
  (2026-10-27): GNOME's top bar came back, with the Recycle Bin and the
  terminal at sign-in but no taskbar. It never showed in the throwaway
  shell, which always had a screen and no desktop icons, whose area API is
  what led there. 0.3.11 places the Recycle Bin and the bubble only once
  there's a screen, and starts the extras (the Recycle Bin, the windows
  placed at sign-in, the bubble) through `startExtra` (`util.js`), so an
  error in one is logged and the taskbar carries on.
- Every two seconds it checks it's still right above the desktop icons, and
  moves back if not: once more, on the user's desktop, it was under them,
  and that session can't be looked into from here.

Not done: docked top left it can still sit where the first desktop file
would; the reserved strip moves that file over one column.

### The desktop bubble

The user's design (2026-10-26): the desktop's files kept off the desktop,
in a bubble that comes out of the screen's left edge (`desktopBubble.js`).
On for every account; Patrone Settings → Desktop → Desktop files, or the
`desktop-bubble` setting, turns it off.

- **The pill**: a small Patrone-blue tab at the middle of the primary
  screen's left edge, shown whenever no window covers its spot (a window
  moved over it, or maximized, hides it). Hovering shows how many items the
  desktop has. Until it's first opened, it bounces every six seconds with
  a tip, "Your desktop files are in here: click, or drag it out" (the user
  asked for that; `desktop-bubble-introduced` remembers it was opened).
- **Clicking it, or dragging it out**, opens the bubble: half an ellipse
  growing out of the edge, with a little overshoot, the pill riding at its
  tip as a handle. Dragged, it follows the pointer; let go more than a
  quarter of the way out and it opens, else it goes back. Dragging the
  pill back, clicking it, clicking outside the bubble or Escape closes it.
- **Its size** follows the files (`bubbleShape.js`): 250 pixels across for
  one file or none, growing a little wider than tall until it's as tall as
  the screen, then wider, up to half the screen. On a 1600×900 screen that
  holds 43 files; with more, the last place is "N more", which opens the
  Desktop folder.
- **The files** are those in the Desktop folder (`xdg-user-dir DESKTOP`;
  none when that's the home folder), hidden ones left out, folders first,
  then by name, filling down the left side first as on Windows. Their icons
  are the file's own or a thumbnail when one exists; a launcher the account
  allowed to run starts its app. Click opens; right-click has **Open**,
  **Show in folder** (the file manager, with it selected) and **Move to
  Recycle Bin**. Right-click on the bubble's empty part: **Open Desktop
  folder** and **Desktop settings**. Arrow keys and Tab move between files,
  Enter opens.
- **The Recycle Bin** leaves its corner as the bubble grows and rides on
  the bubble's lower right edge, above the windows (`recycleBin.hold()`),
  then goes back when it closes (`release()`). It still opens, empties and
  docks as usual while it's there. Since 0.3.12 it sits centred on the
  edge, half in the bubble and half out, and it's there even while it's
  off the desktop (from the start of the bubble's opening, since it has no
  corner to come from); its menu's last item then reads **Show on
  desktop** (the user's asks, 2026-10-27).
- **The bubble opens over any window**, like a menu (the user's choice:
  only the pill needs the desktop showing). It sits above the windows and
  below the taskbar and every menu.
- **The desktop icons** extension keeps running, but its windows (marked
  `customJS_ding`) wait in a hidden actor of the bubble's while the setting
  is on, so they neither draw nor take clicks, and a right-click on the
  empty desktop gets GNOME's own menu (Change Background, Display
  Settings). Clipping their actors to nothing didn't work: mutter still drew
  them. Mutter keeps restacking them there, as it does windows GNOME Shell's
  effects move. Turned off, they go back right above the wallpaper. Their
  window gets the keyboard when the last window closes, so the bubble
  takes it away from them (`unset_input_focus`), or keys would act on
  hidden files.

Not done: dropping files from apps onto the bubble (GNOME Shell takes no
drops); renaming or making files in it (File Manager does that); thumbnails
for files GNOME hasn't thumbnailed yet; the bubble on screens other than
the primary one. Ubuntu's desktop icons may show for a frame or two when
that extension starts, before they're hidden.

### The terminal at sign-in

Patrone Terminal opens at every sign-in (`patrone-terminal`'s autostart
entry), and the taskbar puts that window over the wallpaper's logo, which
shows through its see-through background, as the user arranged it
(2026-10-26: "when the GUI loads ... the terminal loads up in this position
and size making sure its sensitive to the logo backdrop placement"). Or,
the user's other choice, at its own size in the middle of the screen
("or it loads normal size, dead center"): the `sign-in-terminal` setting,
`logo` or `center`, which Patrone Settings' Desktop page and GNOME
Settings' Patrone page set ("Terminal at sign-in", Over the logo or
Centered). On Wayland an app can't place its own window, so the shell does
it (`startupWindows.js`).

- The place is set in the wallpaper's picture, not the screen: over the
  logo, from just right of the `>_` prompt at the bottom left to just left
  of the dashed line at the top right. In the 1672×941 original that's
  (190, 81)–(1393, 777), which at the user's 1536×988 is exactly their
  window, and at 1920×1080 is 218, 93, 1382×799. GNOME scales the picture to
  cover the screen (`picture-options` 'zoom'), cropping its sides on a
  narrower screen or its top and bottom on a wider one, so the terminal
  follows the logo at any size, and keeps 16 pixels inside the free area.
  Patrone Badge, with its logo at the bottom right, gets a terminal that
  stops above it; another picture, the same fractions of the screen.
- Centered keeps the terminal's own size (920×659 in the throwaway shell)
  and puts it in the middle of the free area, above the taskbar. GNOME often does the same for a first window, but not with
  other windows already open.
- Only the first Patrone Terminal window within 30 seconds of the desktop
  starting, and only at sign-in: GNOME Shell turns extensions off on the
  lock screen and on again after, and then it does nothing. The choice is
  read as the window shows, so a new one is for the next sign-in.
- Until the user moves, resizes, maximizes or tiles it, it keeps its place
  (over the logo, or in the middle) when the free area changes (a Remote
  Desktop window resized, the taskbar's size or layout). GNOME itself
  nudges windows out of a new taskbar's way just before it says the free
  area changed, so a move just before that isn't counted as the user's.

Checked in the throwaway shell (2026-10-26, a private copy of
`tests/headless.sh`, the dark style, the 8K wallpaper) at 1920×1080,
1536×988, 1280×1024 and 2560×1080; following the taskbar to the top and
back; a title bar drag, a move by a script and maximizing, each left
alone after; a second window left to GNOME; and the extension turned off
and on again, as after the lock screen. Centered at 1920×1080, and
Patrone Settings' Desktop page run in that shell: both cards, and clicking
one set the setting. Not tried at a real sign-in.

### Settings

`org.gnome.shell.extensions.patrone-taskbar` (Patrone Settings sets the
size; the theme is Patrone Settings' own `org.patrone.desktop theme`, which
the taskbar follows when it's installed):

| Key | Default | |
| --- | --- | --- |
| `pinned` | `['role:web', 'role:terminal', 'role:files', 'role:mail', 'role:documents', 'role:spreadsheets', 'role:presentations', 'role:pdf', 'role:art']` | Pinned items in order: desktop file IDs, or `role:` slots |
| `size` | `'medium'` | `small`, `medium` or `large` |
| `layout` | `'left'` | `left`, `center`, `split` or `top` |
| `start-icon` | `'patrone'` | The Start button's icon, from `icons/` |
| `color` | `'wallpaper'` | `wallpaper` (the wallpaper's hue, in Patrone, Modern and Studio) or `theme` (the theme's grey) |
| `pin-icons` | `'patrone'` | `patrone` (Patrone's icons for the web, terminal, files and mail slots) or `app` (the icons of the apps they open) |
| `show-recycle-bin` | `true` | Show the Recycle Bin on the desktop |
| `recycle-bin-dock` | `'bottom-right'` | `bottom-right`, `bottom-left`, `top-right`, `top-left`, or `none` (where it's dropped) |
| `recycle-bin-position` | `(1.0, 1.0)` | Where it is when not docked, across and down the free area from 0 to 1 |
| `desktop-bubble` | `true` | The desktop's files in the bubble at the left edge, not across the desktop |
| `desktop-bubble-introduced` | `false` | Whether the bubble has been opened yet; until then its pill bounces with a tip |
| `terminal-app` | `''` | The terminal slot's app; empty for Patrone Terminal (or another terminal) |
| `art-app` | `''` | The art slot's app; empty for ArtCraft, else the first drawing app installed |
| `web-search-url` | `'https://www.google.com/search?q=%s'` | Where web searches go |
| `sign-in-terminal` | `'logo'` | Where the terminal opened at sign-in goes: `logo` (over the wallpaper's logo) or `center` (its own size, in the middle) |

## Decisions (with the user, 2026-10-26)

- Left-aligned, as Windows 11 can be set ("option C"), and **compact**: the
  user wanted it smaller than the first mockup, to take less of the screen.
  Built at 32 pixels, then made a third larger, icons and all, once they'd
  used it (42 pixels).
- The account picture opens Lock and Sign out, and the power button Sleep,
  Shut down and Restart, as on Windows 11, with Disconnect for Remote
  Desktop.
- Icons are dragged to reorder them, on the taskbar and in Start.
- Patrone Settings sets the size (small, medium, large), the layout, the
  Start button's icon and a theme. Themes are a handful of looks with plain
  names: Patrone's own, Modern (flat and round), Classic (plain and square),
  Royal (glossy royal blue, a green Start button with no "start" on it) and
  Studio (soft and translucent). They change only the look: the layout
  (Left, Centered, Split with Start on the left and the apps in the middle,
  or Top, along the top of the screen) and which corner the window buttons
  go in are separate choices, which themes don't change.
- The Start button is the logo's "P", or one of seven other icons (a
  one-color P, an orb, a grid, three lines, a circle, a spark, a prompt);
  the one-color ones take the bar's text color. Clicking it lets you type to
  search at once. The search button stays, hooked to GNOME's search
  providers.
- Pinned slots open the default app for their job, which then shows under the
  slot instead of with its own icon; slots can be unpinned, and running apps
  pinned. The slots' icons are Patrone's own, with a setting for the apps'
  own instead (the user: "make our overridden icons look better, and allow
  an option to use the default app icons").
- The bar's color comes from the wallpaper, with a setting for the theme's
  own.
- Right-click gives Windows 11-style jump lists (recent files, tasks) along
  with the configuration (change the app, unpin).
- Enter with no match searches the web.
- No top bar: everything it had moves into the taskbar.
- Built as a GNOME Shell extension now, while the desktop is GNOME-based.
- Light or dark follows the system style (Settings → Appearance), with
  Patrone blue (`#288bfa`) for focus and selection.
- The desktop's files go in a bubble out of the left edge, opened from a
  pill there, with the Recycle Bin riding on its edge. Asked: the files show
  only in the bubble; the pill shows while its own spot is clear and the
  bubble opens over windows; on by default, the pill bouncing with a tip
  until it's first opened.

## How it works

| File | |
| --- | --- |
| `extension.js` | Turns the taskbar on and off |
| `taskbar.js` | The bar, its buttons, the tray, the clock, Show desktop, the Super key |
| `startMenu.js` | The Start menu and its search |
| `jumpList.js` | The buttons' right-click menus |
| `roles.js` | The slots, and which app each one opens |
| `tint.js` | The bar's color from the wallpaper |
| `recycleBin.js` | The Recycle Bin on the desktop |
| `startupWindows.js` | The terminal opened at sign-in, put over the wallpaper's logo |
| `dropTarget.js` | The Recycle Bin's drop target, a small GTK 4 window run with `gjs -m` (never imported by GNOME Shell) |
| `desktopBubble.js` | The desktop bubble and its pill, and hiding the desktop icons while it's on |
| `bubbleShape.js` | The bubble's size and its files' places, with no imports, so it can be tried with Node |
| `recent.js` | Recent files (`~/.local/share/recently-used.xbel`) and pinned folders (`~/.config/gtk-3.0/bookmarks`) |
| `common.css`, `stylesheet-light.css`, `stylesheet-dark.css` | Sizes and shapes in the first, colors in the others (GNOME Shell loads one of the two, which imports the first). St lets an importing sheet's rules beat the imported sheet's, however specific, so a color that replaces one the light and dark sheets set (the royal theme's) goes in both of them, after it |
| `icons/` | The Start button's icons: `patrone-start.svg` and `start-orb.svg` in color, `patrone-start-light.svg` (the P in light tones, for Royal's green button, where the blues washed out), the `-symbolic` ones drawn in the bar's text color. The Recycle Bin's, `recycle-bin.svg` and `recycle-bin-full.svg`. The slots' icons, `pin-web.svg`, `pin-terminal.svg`, `pin-files.svg` and `pin-mail.svg` (the terminal's and files' are Patrone Terminal's and Patrone File Manager's own) |
| `tests/headless.sh` | The throwaway GNOME Shell (below) |

- **Dragging** uses GNOME Shell's own drag and drop (`ui/dnd.js`): the
  buttons and Start's tiles and rows are draggable; the taskbar's buttons
  box and Start's Pinned grid take drops. A drop on the taskbar rewrites
  `pinned`; one in Pinned moves the favorite.

- **The top bar** stays alive but hidden, and stops reserving space. Its
  status area (`Main.panel._rightBox`, `_centerBox`) moves into the tray
  whole, so every indicator, including ones other extensions add later,
  comes along; their menus open upward. Turning the extension off puts it
  all back.
- **The screen-sharing indicator** is hidden in a desktop with no screen of
  its own, which is how Patrone's desktops run (see
  `remote-desktop/README.md`): there the Remote Desktop connection the
  desktop is seen through counts as screen sharing, so the indicator is
  always on, and any click on it (GNOME stops the sharing on a press of
  any button) cuts the connection. The user hit this by right-clicking
  near it. Elsewhere it shows, as a small orange pill.
- **The Super key**: GNOME's own handler for it (it opens the Activities
  overview) is blocked, and the taskbar's opens the Start menu. The overview
  is still there (Super+A for the app grid).
- **Search** reuses the overview's own search providers
  (`Main.overview.searchController`), minus Ubuntu's web search provider,
  since the menu has its own.
- GNOME Shell disables extensions on the lock screen and enables them again
  after, which takes the tray back and forth with it.

## Trying it

`tests/headless.sh` runs it in a throwaway GNOME Shell with no screen, with
its own D-Bus session, settings and Wayland display under
`/tmp/patrone-taskbar-test`, in Ubuntu's session mode like the real
desktop, so it never touches anyone's desktop:

```
tests/headless.sh start 1600x900 light   # or dark
tests/headless.sh start none             # no screen, as over Remote Desktop at sign-in
tests/headless.sh run gnome-text-editor  # an app, for running buttons
tests/headless.sh click 23 883           # the Start button
tests/headless.sh type sett
tests/headless.sh drag 631 879 752 879   # press, hold, move, let go
tests/headless.sh shot /tmp/shot.png
tests/headless.sh close-apps             # only the apps it started
tests/headless.sh stop
```

It also fills the recent files list with a few sample files, gives the
shell a Desktop folder of its own with a few more
(`/tmp/patrone-taskbar-test/desktop`, through `user-dirs.dirs`), so the
desktop bubble never shows or trashes the account's real desktop files,
and leaves out two of Ubuntu's extensions that reach outside the test: the
desktop icons
(when they start, they end every desktop-icons process of the account,
the real desktop's included; early test runs made the real desktop's icons
restart this way) and the snap permission prompts. A helper
extension, `tests/unsafe@patrone-linux`, turns on GNOME Shell's unsafe mode
in that shell, which is what lets the script run JavaScript there and take
screenshots; never install it on a real desktop.

What the tests covered (2026-10-26, GNOME Shell 50.1, light and dark): the
bar replacing the top bar; buttons packed left, the running and focused
lines; the Start menu opening from the button and from GNOME's overlay-key
signal (a simulated key press doesn't trigger GNOME's Super key detection
in a headless shell, so the key itself is untested there); typing "sett"
giving Settings as the best match, Settings pages, a Clocks result and the
web; Pinned, Recommended, All apps and the power options; jump lists for
the web, files and a running app; the quick settings and calendar menus
opening upward from the tray; click to minimize and restore; and turning the
extension off (the top bar comes back) and on again. Later, with
`tests/headless.sh drag`: reordering the taskbar's buttons, pinning a
running app by dragging it in, and reordering Start's pinned apps; and,
with Patrone Settings running in the shell, all three sizes and four
themes.

The desktop bubble (2026-10-26, 1600×900, light and dark): the pill and
its first-run tip; the bubble with 1, 7, 20 and 60 files (the last at half
the screen, with "21 more"); dragging it halfway out (the bubble at 41%,
the Recycle Bin on its way from its corner) and letting go; the Recycle
Bin on its edge and back in its corner after Escape; a click outside
closing it and one on its empty part not; both right-click menus; a click
on a file opening it (WordCraft for an .odt). With a stand-in for the
desktop icons' window (a Text Editor window marked as they mark theirs,
made a desktop window and lowered, and left out of
`global.get_window_actors()`): hidden, its keyboard focus taken away, a
right-click on the empty desktop giving GNOME's menu, still hidden after
another app restacked the windows, and back above the wallpaper, under the
other windows, when the setting was turned off. A window moved over the
pill hid it. Patrone Settings' Desktop page, run in the shell, turned the
bubble and the Recycle Bin off and on. Not tried: the real desktop icons
extension (it mustn't run in the test shell), and opening a launcher.

## Not done yet

- Window thumbnails when hovering a running app.
- A taskbar on other monitors (it's on the primary one only).
- Notification pop-ups still appear at the top right, not above the clock.
