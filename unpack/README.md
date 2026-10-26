# Patrone Unpack

Patrone's app for archives and installers, packaged as `patrone-unpack`
(Rust and GTK 4, app ID `org.patrone.Unpack`). Ubuntu 26.04 has no archive
app of its own: archives opened in Nautilus, Ubuntu's Files, which Patrone
File Manager replaces, and `.deb` packages in Canonical's App Center (the
`snap-store` snap). Nothing opened Flatpak files: double-clicking a
downloaded `.flatpak` bundle showed GNOME's "No Apps Available" (the
user's report, 2026-10-26). `patrone-default-apps` 0.1.3 makes Unpack the
app for all of them.

```
patrone-unpack [FILE...]               open each file in a window
patrone-unpack --extract-here FILE...  extract each archive next to it, then close
patrone-unpack --extract-to DIR FILE.. extract each archive into DIR, then close
patrone-unpack --list FILE...          print what's in each archive (no window)
```

With no file it shows a window with "Open a file…".

## Archives

What it opens: zip, tar (plain, gzip, xz, bzip2, zstd, lzma, lzip), 7z, rar
(versions 4 and 5), cpio, cab, and single compressed files (`notes.txt.gz`).
All through libarchive, the library GNOME's own Files uses (Ubuntu's
`libarchive13t64`, which the desktop already has). libarchive reads rar
with its own free code, so no non-free `unrar` is needed.

The window is the archive as a folder, as Windows' File Explorer shows a
zip: Name, Type, Size and Date modified, folders first, double-click (or
Enter) to go into a folder, Backspace or the up arrow to come back, and a
status bar with the item count, the format, the number of files and
folders and their size once extracted. Double-clicking a file extracts it
to a private temporary folder and opens it in its usual app, as Windows
does.

- **Extract all…** is Windows' "Extract Compressed (Zipped) Folders": a
  destination (next to the archive, named after it), Browse…, and "Show
  extracted files when complete" (on by default; it opens the folder in the
  file manager). An archive holding just one folder with the destination's
  name extracts its contents straight in, not into `name/name/`. If files
  are already there it asks once: replace them, skip them, or cancel.
- **Extract here** extracts next to the archive without asking: an archive
  holding one folder or one file extracts as that folder or file, anything
  else goes into a new folder named after the archive. A name already taken
  gets " (2)", " (3)", as in Windows.

Both show progress in the status bar with Cancel, then "Extracted to …"
with Open folder. Password-protected zips ask for the password (and again
if it's wrong). libarchive can't decrypt 7z or rar, and says so.

**Safe extraction.** Unpack writes the files itself rather than through
libarchive's disk writer, so that nothing lands outside the destination:
absolute paths and `..` are refused, and no file is written through a
folder that's really a link, which an archive could make first and then
put files "inside". Links the archive holds are made as links. Files keep
their run permission, never set-user-ID, and the account's umask applies.
An archive's items refused this way are counted in the status bar.

libarchive's mtree reader (a BSD listing format) takes any line of text for
an archive, so Unpack turns on every other format one by one and leaves it
out. With it, a gzipped one-line text file listed as an empty file.

## Installing .deb packages

A `.deb` opens with what it is, read with `dpkg-deb`: its name, summary and
description, version, download size, size installed, the computers it's
made for, who packaged it, its category and website. Then whether it's
installed already (not installed, this version, an older one, which it
updates, or a newer one, which it goes back from; the button says Install,
Reinstall, Update or Install older version), and what else installing it
changes, from apt's simulation (`apt-get --simulate`, which needs no
administrator): the packages it also installs and updates, and in orange
any it removes. If apt can't install it, its reason is shown in red, in
apt's words, and Install is greyed out.

Install runs `pkexec /usr/libexec/patrone-unpack/install-deb FILE`, under
the polkit action `org.patrone.unpack.install-deb`
(`data/org.patrone.unpack.policy`): "Installing software for everyone on
this computer needs an administrator's password", asked every time
(`auth_admin`, not kept). The helper (`data/install-deb`) copies the file
into a folder of root's own, checks it's a package, and runs
`apt-get install --yes` on the copy, so apt fetches what it needs from the
archive; no questions (debconf's defaults, changed settings files kept).
apt's progress (`APT::Status-Fd`) drives the progress bar, and the rest of
its output is under Details. Cancelling the password prompt leaves things
as they were ("no administrator's password was given"). Afterwards, Open
starts the package's app, if it has one.

## Installing Flatpak apps

The user's first need: a downloaded `.flatpak` bundle that opens and
installs with a double-click.

- `.flatpak` (a bundle, the app itself): its icon, name, summary,
  description, version, developer and website from the bundle's AppStream
  data, its download size (and size installed, when the bundle says), the
  runtime it needs, and **what it can use**, from its metadata in plain
  words ("Your Documents folder", "The internet and your network"; the
  whole home folder, the system's files, all devices, X11 without a Wayland
  fallback, SSH keys and running commands outside the sandbox in orange).
  The screen, the graphics card and its own files aren't listed: every app
  has those. All read from the bundle's header, a GVariant (OSTree's static
  delta superblock, whose first part is Flatpak's), checked against a real
  bundle, PdfCraft 0.9.0.
- `.flatpakref` (an app in a repository, what Flathub's Install buttons
  give): its title, app ID, branch, and where it comes from.
- `.flatpakrepo` (a repository): Add sets it up, named after the file, as
  Flatpak's own instructions do (`flathub.flatpakrepo` adds `flathub`).

Everything installs for the account that opens it: `flatpak install --user
--assumeyes --bundle FILE` (`--from` for a `.flatpakref`, `remote-add
--user` for a `.flatpakrepo`), which needs no password, as suits a
double-click. Progress shows in the window, and Open starts the app
(`flatpak run`). When Flatpak was installed after the session started, the
session doesn't look in Flatpak's folders yet, so the window says the app
reaches Start at the next sign-in (Flatpak's own note says the same).

A bundle doesn't hold its runtime, the platform Flatpak apps share
(`org.freedesktop.Platform`, `org.gnome.Platform`...). When that runtime
isn't installed and comes from Flathub (the bundle names Flathub's
repository, or none), and Flathub isn't set up, Unpack first runs
`flatpak remote-add --user --if-not-exists flathub` with Flathub's own
`.flatpakrepo`, which the package ships as
`/usr/share/patrone-unpack/flathub.flatpakrepo` (fetched from
`dl.flathub.org` on 2026-10-26; it holds Flathub's address and signing
key). Flathub is run by the Flathub project, not Patrone; a bundle naming
another repository gets that one from Flatpak itself. The window says
beforehand that the runtime is downloaded the first time ("can be a few
hundred MB"; PdfCraft's runtime and graphics drivers came to 1.4 GB on
disk).

`patrone-unpack` depends on `flatpak`, which Ubuntu's desktop doesn't
install. Installing for everyone (`--system`) would need an administrator's
password on Patrone's desktops: Ubuntu's Flatpak (its
`org.freedesktop.Flatpak.rules`) skips the password for `sudo` members only
in local sessions, with a seat, and Patrone's desktops have none.

## Look

GTK's own theme (Ubuntu's Yaru doesn't load in plain GTK 4 apps), with
Patrone's title bar button style from the account's `gtk.css`, and
`data/style.css` for the command bar, status bar and Patrone Blue main
button, light and dark. GTK 4.22's Default theme stayed light when the
desktop preferred dark (the preference did reach GTK and the style sheet),
so Unpack asks for its dark variant (`gtk-application-prefer-dark-theme`)
whenever the preference changes. The icon (`data/org.patrone.Unpack.svg`)
is a white box with a zip on a Patrone-blue tile.

## Build and try it

On the dev VM:

```
cargo build --release
cargo test --release
target/release/patrone-unpack ~/Downloads/something.zip
PATRONE_UNPACK_HELPER=$PWD/data/install-deb target/release/patrone-unpack some.deb
```

`PATRONE_UNPACK_HELPER` points Install at the helper in this folder, for
running uninstalled; pkexec then asks with polkit's generic message, since
the policy isn't installed. The package: `./packaging/build.sh patrone-unpack -d`.

The tests (run by the package build too) make real archives with
libarchive's writer, so they need no other tools: zip, 7z and tar with each
compression listed and extracted, Extract here's names, extracting into a
folder of the archive's name, replacing and skipping, paths that try to
leave the destination (`..`, absolute, through a link, hard links outside),
hard links inside, a single compressed file, a password-protected zip
(none, wrong and right), opening one file, cancelling; control fields,
apt's simulation and its errors; `.flatpakref`, `.flatpakrepo`, a bundle
laid out as `flatpak build-bundle` does (with a real one's keys), the
install commands and the permission wording; sizes and paths.

`PATRONE_UNPACK_FLATHUB` points the Flathub step at `data/flathub.flatpakrepo`
when running uninstalled.

## Checked

On the VM, 2026-10-26, in a throwaway GNOME Shell (`headless.sh`, run as a
copy with its own folder and display) with real clicks:

- A zip holding one folder: the list, going into the folder, Extract all's
  dialog, Extract (contents straight into the suggested folder, `run.sh`
  still runnable, the folder opened in File Manager), Extract here (into
  "Holiday Photos (2)"), Extract all again (asked about the 5 files already
  there; Skip left them and said so), and double-clicking README.md (opened
  from a temporary copy in its usual app).
- `--extract-here` on a tar.gz of loose files: into a new folder named
  after it, then the window closed.
- A `.deb` (`patrone-shell-config`): its details, "Nothing else needs to be
  installed", Install up to polkit's password prompt, Cancel (not
  installed, and it said why). A `.deb` needing a package that doesn't
  exist: apt's reason, Install greyed out.
- A `.flatpakref`: its details, and that Flatpak isn't installed on the VM.
- The user's own bundle, `~/Downloads/pdfcraft-0.9.0-linux-x86_64.flatpak`:
  its icon, name, version 0.9.0, developer, 61.9 MB, runtime
  (`org.freedesktop.Platform` 25.08) and permissions (Documents and
  Downloads). Then **installed end to end** with Ubuntu's Flatpak 1.16.6
  unpacked into a scratch folder (Flatpak isn't installed on the VM, and
  installing it needs the user's password) and scratch per-user and system
  folders, so nothing reached the user's account: Install added Flathub,
  installed PdfCraft 0.9.0 with `org.freedesktop.Platform` 25.08 and its
  graphics drivers, showed progress, said "Installed. Open starts it now; it
  appears in Start after you next sign in", and Open started PdfCraft.
  Everything was deleted afterwards.
- Light and dark, and switching while open.
- With `patrone-default-apps` 0.1.3's list and the session's search paths,
  `gio mime` picks Unpack for zip, tar with each compression, gzip, 7z,
  rar, `.deb` (ahead of App Center), and the three Flatpak types.

Not checked yet: installing a `.deb` for real (needs the user's password),
the installed policy's own message, a Flatpak install with the installed
package (system paths and its session setup rather than the scratch copy),
a `.flatpakref` install, rar files (libarchive can't write one, and none
was at hand), and password-protected archives through the window (the
tests cover them).

Left for later: GNOME's "Open With" dialog (xdg-desktop-portal-gnome
50.0, Ubuntu's build) has a "Find more in App Center" button that starts
`snap-store_snap-store.desktop`, else GNOME Software. App Center is still
installed on the dev VM, so the button works there; on a system without it
the button fails ("Failed to start App Center"). With Unpack as the default
for archives, packages and Flatpak files, those no longer reach that
dialog.
