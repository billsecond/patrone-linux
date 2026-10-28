# Patrone's GNOME Settings

GNOME Settings can't take pages from other programs: its sidebar is fixed
when it's built. So Patrone builds Ubuntu's GNOME Settings
(`gnome-control-center`) again with these changes, as the user asked
(2026-10-26 and 27):

- **A Patrone page** in the sidebar, after Appearance and Ubuntu's Desktop
  page: the taskbar's size, layout, Start button icon, color and pin
  icons, the title bar buttons' corner and style, the theme and the font
  (Patrone Sans or Noto Sans). The same choices as Patrone Settings'
  window (`settings/`), with its pictures; the font cards show text in
  each font instead.
- **No Telemetry page** under Privacy & Security. Its switches send error
  reports and system information to Canonical, which Patrone doesn't do
  (`patrone-desktop` removes the programs that would), so they did nothing.
- **Patrone's logo on the About page** (revision 8; the user found Ubuntu's
  there, 2026-10-27). Ubuntu's `debian/rules` builds Ubuntu's logo in
  (`distributor_logo`, `/usr/share/pixmaps/ubuntu-logo-text.svg` and its
  `-dark` twin), so `os-release`'s `LOGO=` is never read. `build.sh` points
  them at `patrone-logo-text.svg` and `patrone-logo-text-dark.svg` from
  `patrone-branding` (0.1.7), and stops if Ubuntu's lines change.
- **Remote Desktop first in System › Remote Desktop** (revision 9, reworked
  in 11; the user asked, 2026-10-27: "Make that part of the system >
  remote login, and just say Enable Remote Desktop", then 2026-10-28: "make
  the Remote Desktop as the main and first option that enables rdp, call it
  by Remote Desktop, and add explain what the otehers actually do"). With
  `patrone-remote-desktop` 0.4.5 or later installed, the page's first tab is
  Patrone Remote Desktop's, named Remote Desktop, in place of GNOME's Remote
  Login tab; Desktop Sharing comes after it. One switch, "Remote Desktop",
  turns it on or off for the account using Settings (an administrator's
  password), then asks for the account's own password once so it can
  connect at once; the tab shows the computer's name and IP addresses, the
  username and a connection file to save. Under "Other Ways to Connect" it
  says, in a sentence each, what Desktop Sharing, Secure Shell (SSH) and
  GNOME's Remote Login do and how they differ; Desktop Sharing's row opens
  its tab, and SSH's opens its dialog where an SSH server is installed
  (Patrone doesn't install one, and the row says so). System's own rows say
  what each is for: Remote Desktop "Use this computer's desktop from another
  one", Secure Shell "Run commands on this computer from another one".
  GNOME's Remote Login drives gnome-remote-desktop's system daemon, which
  Patrone Remote Desktop turns off (both use port 3389). Without the
  package (the ISO's live session), the page is GNOME's as it was.
  `remote-desktop/README.md` has how the switch's helper works.

| File | |
| --- | --- |
| `panels/patrone/cc-patrone-panel.c` | The page. It reads and writes the settings itself, shows Patrone Settings' pictures (`/usr/share/patrone/settings/previews`) and the taskbar's Start icons, applies a theme with `patrone-settings --set-theme`, which also restyles apps through the account's `gtk.css`, and sets the font's family in GNOME's font settings, keeping their sizes |
| `panels/patrone/meson.build`, `gnome-patrone-panel.desktop.in`, `org.patrone.Settings-symbolic.svg` | Its build, its sidebar entry (and search keywords), and its icon, the one-color P |
| `patches/patrone-register-patrone-panel.patch` | Adds the page to the panel list, the loader and the sidebar order |
| `patches/patrone-hide-telemetry.patch` | Hides the Telemetry row |
| `remote-login/cc-patrone-remote-login.c`, `.h` | The Remote Desktop tab, added to `panels/system/remote-desktop/`. It runs `/usr/libexec/patrone-remote-desktop/settings` through `pkexec` for the switch's state (`status`), turning it on or off, and the password; the connection file comes from `patrone-remote-desktop rdp-file`. Its "Other Ways to Connect" rows open Desktop Sharing's tab and GNOME's Secure Shell dialog |
| `patches/patrone-remote-login.patch` | Builds that tab and, where the helper is installed, shows it in Remote Login's place as the page's first tab (`cc-remote-login-page.c`, `cc-remote-desktop-page.c`), keeps the page shown even where Desktop Sharing can't run, and gives System's Remote Desktop and Secure Shell rows plain subtitles |
| `patches/patrone-desktop-switches.patch` | Ubuntu's Desktop page (`panels/ubuntu`): where Patrone's taskbar is installed, switches for the desktop bubble and the Recycle Bin on the desktop come first, in place of Ubuntu's Desktop Icons settings. File Manager draws Patrone's desktop and Ubuntu's icons are off then, and their switch there would bring them back beside it |
| `build.sh` | Fetches Ubuntu's source, adds the page and the Remote Login page (each as a patch of its own) and the patches, versions it Ubuntu's version plus `patrone` and the revision, and builds |

The page hides itself where neither Patrone's taskbar nor Patrone Settings
is installed, and each part says so if its settings are missing.

The pages are GPL-2.0-or-later, GNOME Settings' own license, not
GPL-3.0-or-later like the rest of Patrone: they're built into GNOME
Settings, and GPL-3 code there could clash with a GPL-2-only library it
uses.

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
sudo apt install ./out/gnome-control-center_*patrone11_amd64.deb ./out/gnome-control-center-data_*patrone11_all.deb
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

`1:50.3-0ubuntu0.2patrone9` (Remote Login), the same way (2026-10-27), with
`patrone-remote-desktop` 0.4.5 installed on the dev VM, where Remote
Desktop was on for the user's account: the patches applied, both files
compiled with no warnings, and the Remote Login tab showed the switch on,
the computer's `.local` name, its IP address and the username. A copy
button copied, with a toast; Save… wrote a connection file with Windows'
line ends, and Escape in the save dialog did nothing. Turning the switch
off brought up GNOME Shell's polkit dialog ("Authentication is required to
turn off Remote Desktop"); cancelling it put the switch back on and left
Remote Desktop on. The "Confirm Your Password" steps were tried with a
stand-in helper bind-mounted over the real one in that one Settings
process's own mount namespace, since nobody may type the user's password:
the row and the dialog, Confirm greyed out while the box is empty, the
password reaching the helper as typed, "That password isn't right. Try
again." for a wrong one, and "Remote Desktop is ready" with the row gone
for the right one. The same stand-in showed the switch's "on for other
accounts" and off states, read again each time the tab was shown. Not
tried: turning it on or off with a real administrator's password, and
connecting with the hash Settings records (its PAM check and SAM entry
were tried from the command line; see `remote-desktop/README.md`).

`1:50.3-0ubuntu0.2patrone11` (Remote Desktop first), the same way
(2026-10-28), with `patrone-remote-desktop` 0.4.6 on the dev VM: all four
changed files compiled with no warnings. System's rows read "Remote
Desktop: Use this computer's desktop from another one" and "Secure Shell:
Run commands on this computer from another one". System › Remote Desktop
opened on its Remote Desktop tab, with Desktop Sharing second, the switch
named just "Remote Desktop", and "Other Ways to Connect" under How to
Connect; its Desktop Sharing row switched to that tab, and its Secure
Shell row opened GNOME's Secure Shell dialog. With the helper hidden (an
empty file bind-mounted over it in that one Settings process's own mount
namespace, as on the live session, which hasn't the package), the page was
GNOME's: Desktop Sharing, then Remote Login.

## Keeping up with Ubuntu

The version is Ubuntu's with `patrone` and a revision added
(`1:50.3-0ubuntu0.2patrone7`), which apt prefers to the same Ubuntu
version. The revision is at the top of `build.sh`: one more with each
change to the page or the patches, so a new build installs as an upgrade. When Ubuntu updates Settings
(a newer `0ubuntuN`), apt installs Ubuntu's in its place and the Patrone
page goes away until `build.sh` is run again and the new packages are
installed. The patches are small, so a new Ubuntu version usually needs no
change. Once Patrone has its own apt repository, its builds can be pinned
there so updates come through Patrone.
