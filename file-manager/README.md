# Patrone File Manager

The file manager of Patrone Linux ("File Manager" in menus): Windows 11's
File Explorer, rebuilt in Rust with GTK4. Same layout, same right-click
menus, same options, same keyboard shortcuts, in Patrone's colors. App ID
`org.patrone.FileManager`, command `patrone-file-manager`, package
`patrone-file-manager`.

## What it does

- **Window buttons:** GTK's own minimize, maximize and close, as in every
  other app, so they go in the corner the account chose (Settings' Patrone
  page) and look the way its title bar button style says. (They were drawn
  here, Windows 11's, and so ignored both.)
- **Window:** tabs in the title bar (new, close, middle-click to close, a
  right-click menu with Duplicate, Move to new window, Move to window ▸,
  Close other tabs and Close tabs to the right); Back, Forward, Up and
  Refresh; breadcrumbs of the whole path from This PC (This PC › Local Disk
  (/) › home › you › folder), any of which goes there when clicked, with
  chevrons listing each folder's subfolders; when the path doesn't fit, its
  start folds into a « button that lists it, as on Windows. Clicking the
  empty space turns it into a path field. "Previous locations"; the search
  box.
- **Dragging tabs**, as in Windows 11: a window's only tab moves the window,
  like the title bar. With several tabs, dragging along the strip reorders
  them; pulling a tab out of the strip (or holding Ctrl) picks it up, and
  dropping it on another File Manager window's title bar adds it there
  (a blue line shows where), while dropping it anywhere else opens it in a
  window of its own. A window whose last tab leaves closes. Files dragged
  over a tab switch to it; dropped on a tab, they go into its folder.
  Wayland doesn't let apps place windows, so a torn-off tab's window opens
  where GNOME puts new windows, not under the pointer.
- **Commands in the address bar**, as in Windows: what isn't a place runs in
  the folder on screen. Apps with a window of their own start directly
  (`gnome-text-editor notes.txt`); anything else runs in a terminal through
  your shell, so pipes and `sudo` work, and the terminal stays open there.
  Windows' names work too: `cmd`, `powershell` and `wt` open a terminal
  there, `notepad`, `calc`, `taskmgr`, `control` and `regedit` open their
  Patrone counterparts, `explorer` a new window; `cd PLACE` goes there.
- **Command bar:** New (folder, text document, anything in Templates);
  Cut, Copy, Paste, Rename, Share and Delete; Sort, View and Filter; buttons
  for the selection (Empty Recycle Bin, Restore, Extract all, Set as
  background, Connect to server, Eject); "See more" (Undo, Redo, Pin to Quick
  access, Copy path, Select all/none, Invert selection, Open in Terminal,
  Properties, Options); and Details.
- **Navigation pane:** Home, Gallery, your home folder; the folders pinned to
  Quick access; This PC with its drives, Network and the Recycle Bin. Folders
  expand (chevrons appear on hover); middle-click opens a new tab; files can
  be dropped on any folder or drive.
- **Pages:** Home (Quick access tiles of pinned and frequent folders,
  Favorites, Recent files), Gallery (every picture and video in Pictures,
  newest first), This PC (drives with usage bars, network locations), the
  Recycle Bin (Original location and Date deleted columns, Restore, Empty).
- **Views:** Extra large, Large, Medium and Small icons, List, Details,
  Tiles and Content, plus Compact view. Details has sortable, resizable,
  reorderable columns (right-click a header for more columns). Sort and
  Group by every column, with Windows' groups (Today, Yesterday, Earlier this
  week...; Tiny, Small, Medium... for sizes). Each folder remembers its own
  view, as on Windows; Pictures and Videos open in Large icons, Downloads
  grouped by date.
- **Right-click menus in Windows 11's form:** the icon row (Cut, Copy,
  Rename, Share, Delete), then Open, Open with, Open in new tab and window,
  Pin to Quick access, Add to Favorites, Compress to (ZIP, TAR), Extract
  All, Copy as path, Set as desktop background, Properties, Open in
  Terminal, and "Show more options" for the full classic menu with Send to.
  The background menu has View, Sort by, Group by, Undo, Paste, Paste
  shortcut, New and Properties.
- **Panes:** Details (a large thumbnail and the properties of what's
  selected, or of the folder) and Preview (pictures, text, video and sound,
  and thumbnails of PDFs and other files).
- **Operations** through GIO, so they work on local drives, removable
  drives, network shares and the Recycle Bin: copy, move, delete to the
  Recycle Bin or for good, restore, rename in place (F2), new folder, new
  file, shortcuts (symbolic links), compress to ZIP or TAR and extract.
  Copies into the same folder become "name - Copy". Folders merge.
  Conflicts are asked about once for the whole operation (Replace, Skip, or
  "Let me decide for each file", where keeping both adds a number). Errors
  offer Try Again, Skip or Cancel. Long operations show Windows' progress
  window with pause, cancel and "More details". **Undo and Redo** (Ctrl+Z,
  Ctrl+Y) cover all of them, shared by every window.
- **Drag and drop** between folders, tabs' breadcrumbs, the navigation pane
  and other apps: a move within a drive, a copy to another drive; Shift
  forces a move, Ctrl a copy.
- **Folder Options** (See more → Options) with Windows' General, View and
  Search tabs: where windows open, same or new window, single- or
  double-click, recent files and frequent folders (and clearing them),
  hidden files, file name extensions, thumbnails, tooltips, the status bar,
  item check boxes, typing to select or to search, restoring windows at
  sign-in, and search options. OK, Cancel, Apply and Restore Defaults behave
  as on Windows.
- **Properties** (Alt+Enter): General (name, type, the app it opens with
  and Change..., location, size and size on disk, what a folder contains,
  dates, Read-only and Hidden), Permissions and Details. Drives show a chart
  of used and free space.
- **Permissions**, Linux's version of Windows' Security tab: the owner and
  group (changeable), what the owner, the group and everyone else may do
  (Read, Write, Run; for folders List contents, Add and delete, Open),
  "Allow running this file as a program", the special permissions (set user
  ID, set group ID, sticky), for folders "Apply to everything inside", and
  the mode in numbers and letters (`0755`, `-rwxr-xr-x`). **Other people and
  groups** get access by name, as with Windows' Add... button: "Give access
  to" adds a row with their own Read, Write and Run boxes (starting, as on
  Windows, with read, and open for a folder or a program) and a button to
  take it away. These are ACLs (`getfacl` and `setfacl` from the `acl`
  package); a folder hands them on to new items inside, and the mode shows
  `+` as `ls` does. A change you may not make yourself is made as
  administrator after the system asks for your password (pkexec).
- **System folder hints** (View → Show, or Folder Options → View), for
  people who know Windows: Details gains a Purpose column saying what system
  folders and well-known files are for (`/etc` "Settings", `/usr/bin`
  "Programs", `~/.config` "App settings"; pointing at it explains more),
  Tiles and Content add it after the type ("File folder · Settings"), the
  other views put it in the tooltip, and inside one a banner explains it and names
  what plays the same part on Windows (`/etc`: the Registry's
  HKEY_LOCAL_MACHINE and `C:\ProgramData`). Permissions then also shows the
  `chmod` and `chown` commands for its settings, with `sudo` when they need
  it. The table is in `src/hints.rs`.
- **System folders have their own icons** wherever they're shown, as
  Windows' do: a gear for `/etc` and `~/.config`, a terminal for the
  program folders, a power button for `/boot`, a chip for `/dev` and `/sys`,
  layers for the libraries, people for `/home`, a shield for `/root`, a
  package for `/opt`, `/usr` and snaps, a clock for `/tmp`, a log for
  `/var/log`, and so on.
- **Search:** names anywhere, ignoring case, with `*` and `?` wildcards,
  through subfolders; optionally inside text files and ZIP files. Results
  stream in, with a Folder column.
- **Light and dark** follow the desktop's preference. Thumbnails follow the
  freedesktop.org thumbnail spec (shared with other apps), made here for
  pictures and by the system's thumbnailers for everything else.
- `org.freedesktop.FileManager1` on D-Bus, so apps can ask it to show a file
  in its folder.

Hidden files (names starting with a dot, and names listed in a folder's
`.hidden` file) stay hidden unless Folder Options or View → Show → Hidden
items says otherwise, and show faded when they're shown. Hidden in
Properties adds the name to `.hidden`, which every GTK app honors.

## Keyboard

| Keys | Action |
| --- | --- |
| Ctrl+T / Ctrl+W / Ctrl+N | New tab / close tab / new window |
| Ctrl+Tab, Ctrl+Shift+Tab, Ctrl+1 to 9 | Switch tabs |
| Alt+Left / Alt+Right / Alt+Up, Backspace | Back / Forward / Up, Back |
| F5 or Ctrl+R | Refresh |
| Ctrl+L, Alt+D or F4 | Type a path in the address bar |
| Ctrl+E, Ctrl+F or F3 | Search |
| Ctrl+Shift+N | New folder |
| F2 | Rename |
| Delete or Ctrl+D / Shift+Delete | Delete to the Recycle Bin / delete for good |
| Ctrl+C / Ctrl+X / Ctrl+V | Copy / cut / paste |
| Ctrl+Z / Ctrl+Y | Undo / redo |
| Ctrl+A | Select all |
| Ctrl+Shift+C | Copy as path |
| Alt+Enter | Properties |
| Ctrl+Shift+1 to 8, Ctrl+mouse wheel | Change the view |
| Alt+P / Alt+Shift+P | Preview pane / details pane |
| Ctrl+Shift+E | Expand the navigation pane to the open folder |
| Shift+F10 or the Menu key | Right-click menu |
| F11 | Full screen |
| Ctrl while dragging a tab | Pick the tab up, even a window's only one |

Typing in a file list selects the item that starts with what was typed.

## Settings

Settings live in GSettings (`org.patrone.FileManager`, see
`data/org.patrone.FileManager.gschema.xml`), so Patrone or an administrator
can change the defaults for everyone with a `.gschema.override` file, and
`gsettings` can read and change them. Folder Options writes only what was
changed. Each folder's own view is kept in
`~/.local/share/patrone-file-manager/folder-views.ini`, and the visit
counts behind "frequent folders" in `history.ini` beside it. Pinned folders
and Favorites are settings (`pinned-folders`, `favorite-files`).

## Icons

Every icon is original artwork drawn for Patrone in the style of Windows 11's
Fluent design; nothing is copied from Microsoft. The color icons (folders,
file types, drives, places) are SVGs in `data/icons`, drawn by
`tools/make_icons.py`; `tools/preview_icons.py` renders them all on light
and dark backgrounds for checking. The line glyphs of the command bar and
menus are vector paths in `src/glyph.rs`, drawn in the text color so they
follow light and dark mode. The text is Patrone's typeface, Noto Sans, at
Windows' 9 points; file lists draw
their icons at 20 pixels, which with the artwork's margins makes the item
itself about Windows' 16.

## Build and try it

On the dev VM (GTK 4.22 and the GLib tools come from the desktop and
`scripts/bootstrap-dev-vm.sh`):

```
cargo run --manifest-path file-manager/Cargo.toml -- ~/Documents
cargo test --manifest-path file-manager/Cargo.toml
```

`cargo run` uses the settings schema compiled into the build folder, so
nothing has to be installed. To install it for your user, with a launcher
entry, icon and settings:

```
./file-manager/install-dev.sh             # the app
./file-manager/install-dev.sh --default   # and open folders with it, and put it in the dock, in place of Files
```

The package is built with `packaging/build.sh patrone-file-manager -d` (see
`packaging/README.md`). `patrone-default-apps` makes it the app for folders
system-wide (`inode/directory` in `/etc/xdg/mimeapps.list`) and puts it in
the favorites in Files' place, so the taskbar's Files button opens it.

Command line: `patrone-file-manager [--new-window] [--select] [LOCATION...]`;
a file opens in its app, `--select` shows items selected in their folders.
Every window shares one process.

## Testing without a desktop session

- **GTK's Broadway backend** shows the app in a web page, with working mouse
  clicks, menus and dialogs, and single key presses (letters typed as text,
  Ctrl+C and Ctrl+W don't arrive; browsers keep those). Run
  `gtk4-broadwayd --port 8087 :7`, start the app with
  `GDK_BACKEND=broadway BROADWAY_DISPLAY=:7 dbus-run-session -- patrone-file-manager`,
  and open `http://localhost:8087` (tunnel the port over SSH from Windows).
- **A screenshot under Wayland:** inside a throwaway
  `dbus-run-session gnome-shell --headless --virtual-monitor 1600x1000 --wayland --no-x11 --wayland-display NAME`,
  run the app with `WAYLAND_DISPLAY=NAME PATRONE_FM_SCREENSHOT=out.png`; it
  draws its first window into `out.png` after
  `PATRONE_FM_SCREENSHOT_DELAY` milliseconds (2000) and quits.
  `PATRONE_FM_COLOR_SCHEME=dark` or `light` forces a color scheme. With
  `PATRONE_FM_SCREENSHOT_DIALOG=1` it draws a dialog open by then instead
  (open Properties with `gdbus call --session --dest org.patrone.FileManager
  --object-path /org/freedesktop/FileManager1 --method
  org.freedesktop.FileManager1.ShowItemProperties "['file:///etc']" ""`),
  and `PATRONE_FM_SCREENSHOT_PAGE=N` shows its Nth tab first.
- **Real mouse and keyboard input:** in that headless GNOME Shell, Mutter's
  RemoteDesktop and ScreenCast D-Bus APIs move and click the pointer
  (`NotifyPointerMotionAbsolute` takes the stream's path as a string),
  type, and stream the screen to PipeWire, where
  `gst-launch-1.0 pipewiresrc path=NODE ! videoconvert ! pngenc snapshot=true ! filesink`
  saves a screenshot. That's how tab dragging was tried: on GNOME's Wayland a
  drag dropped where nothing accepts it ends as an error, not "no target".
- **Settings without dconf:** `GSETTINGS_BACKEND=keyfile` with
  `XDG_CONFIG_HOME` pointing at a scratch folder reads
  `glib-2.0/settings/keyfile` there.
- Test runs in a private D-Bus session get their own dconf service, which
  fights the desktop's over `~/.config/dconf/user`. Give them their own
  database: `DCONF_PROFILE=` a file containing `user-db:pfm-test`.

## Not done yet

- Group headers show in Details and Content; the icon views and List sort by
  group but don't show the headers (GTK's grid view has no sections).
- A narrow window clips the command bar instead of moving buttons into
  "See more".
- Windows' Gallery timeline and collections, OneDrive-style cloud states,
  libraries, and indexed search (search reads the disk each time).
- Folders don't show previews of their contents on their icons.
