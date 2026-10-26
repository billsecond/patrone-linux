# Patrone Taskbar (GNOME Shell)

Patrone Desktop's taskbar for the GNOME-based desktop Patrone runs today,
packaged as `patrone-gnome-taskbar`. It's a GNOME Shell extension
(`patrone-taskbar@patrone-linux`), on for every account, until Patrone's own
shell (`shell/`, GTK4) replaces GNOME Shell. The design carries over to that
shell.

## What it does

One compact bar (32 pixels) along the bottom of the primary monitor, in place
of GNOME's top bar, which it hides. From the left:

- **Start** (the Patrone "P", or the Super key) opens the Start menu with
  the search box focused, so typing searches right away (below).
- **Search** opens the same menu in search mode: the most used apps and
  recent files, then results as you type.
- **Pinned items**: apps, and **slots** for a job: web browser, terminal,
  file manager, mail (the defaults, in that order). A slot shows a generic
  icon, opens whichever app is the default for its job, and that app's
  windows show under it, not as a separate button. For the browser, files
  and mail that's the system default (Settings → Apps → Default Apps); for
  the terminal, Patrone Terminal unless changed in the slot's menu.
- **Running apps** that aren't pinned, in the order they started. A short
  line under an app means it's running, a longer blue one that it's in
  front.
- On the right, the **tray**: the top bar's own status icons and menus
  (network, sound, power, input sources, other apps' indicators), then the
  clock, Windows-style (time over date), with the calendar and notifications
  behind it, and the **Show desktop** sliver at the far right.

Clicking a button works as on Windows: it starts the app, shows its window,
or minimizes it when it's already in front; with several windows each click
brings the next. A middle click, or Shift/Ctrl with a click, opens a new
window. Hovering shows the app's name.

### Right-click: jump lists

As on Windows 11: the app's **Recent** files (those GTK recorded for it, then
recent files of types it opens; not for browsers), or for the file manager its
**Pinned** folders (the file manager's bookmarks, else Home, Documents,
Downloads...); its **Tasks** (the app's own actions: New Window, New
Incognito Window...); its **Open windows** when it has several; then the app
itself (a new window), **Pin to taskbar** / **Unpin from taskbar**, for a slot
**Default web browser** (and so on) to change which app the job uses, and
**Close window** / **Close all windows**. Right-click on the empty bar:
Task Manager and Show desktop.

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
- At the bottom: the account's picture and name, Settings, and power (Lock,
  Sign out, Sleep, Restart, Shut down: whichever the system allows).

Right-click an app in the menu to pin it to Start or the taskbar.

### Settings

`org.gnome.shell.extensions.patrone-taskbar`, no settings window yet:

| Key | Default | |
| --- | --- | --- |
| `pinned` | `['role:web', 'role:terminal', 'role:files', 'role:mail']` | Pinned items in order: desktop file IDs, or `role:` slots |
| `terminal-app` | `''` | The terminal slot's app; empty for Patrone Terminal (or another terminal) |
| `web-search-url` | `'https://www.google.com/search?q=%s'` | Where web searches go |

## Decisions (with the user, 2026-10-26)

- Left-aligned, as Windows 11 can be set ("option C"), and **compact**: the
  user wanted it smaller than the first mockup, to take less of the screen.
- The Start button is the logo's "P", and clicking it lets you type to search
  at once. The search button stays, hooked to GNOME's search providers.
- Pinned slots open the default app for their job, which then shows under the
  slot instead of with its own icon; slots can be unpinned, and running apps
  pinned.
- Right-click gives Windows 11-style jump lists (recent files, tasks) along
  with the configuration (change the app, unpin).
- Enter with no match searches the web.
- No top bar: everything it had moves into the taskbar.
- Built as a GNOME Shell extension now, while the desktop is GNOME-based.
- Light or dark follows the system style (Settings → Appearance), with
  Patrone blue (`#288bfa`) for focus and selection.

## How it works

| File | |
| --- | --- |
| `extension.js` | Turns the taskbar on and off |
| `taskbar.js` | The bar, its buttons, the tray, the clock, Show desktop, the Super key |
| `startMenu.js` | The Start menu and its search |
| `jumpList.js` | The buttons' right-click menus |
| `roles.js` | The slots, and which app each one opens |
| `recent.js` | Recent files (`~/.local/share/recently-used.xbel`) and pinned folders (`~/.config/gtk-3.0/bookmarks`) |
| `common.css`, `stylesheet-light.css`, `stylesheet-dark.css` | Sizes in the first, colors for each style in the others (GNOME Shell loads one of the two, which imports the first) |

- **The top bar** stays alive but hidden, and stops reserving space. Its
  status area (`Main.panel._rightBox`, `_centerBox`) moves into the tray
  whole, so every indicator, including ones other extensions add later,
  comes along; their menus open upward. Turning the extension off puts it
  all back.
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
tests/headless.sh run gnome-text-editor  # an app, for running buttons
tests/headless.sh click 23 883           # the Start button
tests/headless.sh type sett
tests/headless.sh shot /tmp/shot.png
tests/headless.sh stop
```

It also fills the recent files list with a few sample files, and leaves out
two of Ubuntu's extensions that reach outside the test: the desktop icons
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
extension off (the top bar comes back) and on again.

## Not done yet

- Window thumbnails when hovering a running app.
- Dragging to reorder the taskbar's buttons and the Start menu's pinned apps.
- A taskbar on other monitors (it's on the primary one only).
- Notification pop-ups still appear at the top right, not above the clock.
- A settings window (Taskbar settings on the bar's right-click menu).
