# Patrone's GNOME Settings

GNOME Settings can't take pages from other programs: its sidebar is fixed
when it's built. So Patrone builds Ubuntu's GNOME Settings
(`gnome-control-center`) again with two changes, as the user asked
(2026-10-26):

- **A Patrone page** in the sidebar, after Appearance and Ubuntu's Desktop
  page: the taskbar's size, layout, Start button icon, color and pin
  icons, the title bar buttons' corner and style, the theme and the font
  (Noto Sans or Patrone Sans). The same choices as Patrone Settings'
  window (`settings/`), with its pictures; the font cards show text in
  each font instead.
- **No Telemetry page** under Privacy & Security. Its switches send error
  reports and system information to Canonical, which Patrone doesn't do
  (`patrone-desktop` removes the programs that would), so they did nothing.

| File | |
| --- | --- |
| `panels/patrone/cc-patrone-panel.c` | The page. It reads and writes the settings itself, shows Patrone Settings' pictures (`/usr/share/patrone/settings/previews`) and the taskbar's Start icons, applies a theme with `patrone-settings --set-theme`, which also restyles apps through the account's `gtk.css`, and sets the font's family in GNOME's font settings, keeping their sizes |
| `panels/patrone/meson.build`, `gnome-patrone-panel.desktop.in`, `org.patrone.Settings-symbolic.svg` | Its build, its sidebar entry (and search keywords), and its icon, the one-color P |
| `patches/patrone-register-patrone-panel.patch` | Adds the page to the panel list, the loader and the sidebar order |
| `patches/patrone-hide-telemetry.patch` | Hides the Telemetry row |
| `build.sh` | Fetches Ubuntu's source, adds the page (as a patch of its own) and the patches, versions it Ubuntu's version plus `patrone` and the revision, and builds |

The page hides itself where neither Patrone's taskbar nor Patrone Settings
is installed, and each part says so if its settings are missing.

The page is GPL-2.0-or-later, GNOME Settings' own license, not
GPL-3.0-or-later like the rest of Patrone: it's built into GNOME Settings,
and GPL-3 code there could clash with a GPL-2-only library it uses.

## Building

On the dev VM, with Ubuntu's build dependencies for `gnome-control-center`
installed (`dpkg-checkbuilddeps` in the unpacked source lists any that are
missing):

```
./gnome-settings/build.sh
```

It fetches the newest resolute source (`pull-lp-source`, the one step that
needs the network), or a given version (`build.sh 1:50.3-0ubuntu0.2`).
Install the two packages that go together:

```
sudo apt install ./out/gnome-control-center_*patrone4_amd64.deb ./out/gnome-control-center-data_*patrone4_all.deb
```

## How it was checked

`1:50.3-0ubuntu0.2patrone1`, built on the dev VM (2026-10-26): the patches
applied cleanly and the page compiled with no warnings. The built Settings,
unpacked into a folder and run in `shell/gnome/tests/headless.sh`'s
throwaway GNOME Shell (with that folder first in `XDG_DATA_DIRS`), showed
the Patrone row with its icon after Ubuntu's Desktop page, each section's
pictures, and the current values picked. Clicking a choice in each section
set its setting, and the theme's `gtk.css` import; a setting changed
elsewhere moved the page's pick. Privacy & Security had no Telemetry row.
`apt-get -s install` of the two packages upgrades just them.

## Keeping up with Ubuntu

The version is Ubuntu's with `patrone` and a revision added
(`1:50.3-0ubuntu0.2patrone4`), which apt prefers to the same Ubuntu
version. The revision is at the top of `build.sh`: one more with each
change to the page or the patches, so a new build installs as an upgrade. When Ubuntu updates Settings
(a newer `0ubuntuN`), apt installs Ubuntu's in its place and the Patrone
page goes away until `build.sh` is run again and the new packages are
installed. The patches are small, so a new Ubuntu version usually needs no
change. Once Patrone has its own apt repository, its builds can be pinned
there so updates come through Patrone.
