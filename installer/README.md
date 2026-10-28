# Patrone's installer

Patrone's ISO (`iso/`) uses Ubuntu's installer: the `ubuntu-desktop-bootstrap`
snap, a Flutter front end (`ubuntu_bootstrap`, from
[canonical/ubuntu-desktop-provision](https://github.com/canonical/ubuntu-desktop-provision))
over Ubuntu's install engine (subiquity and curtin). Its look comes from
files (`branding/system/desktop-provision/`, in `patrone-branding`), but its
name doesn't: the front end knows only Ubuntu's flavors (`ubuntu_flavor`'s
`UbuntuFlavor`) and says "Welcome to Ubuntu" and "Install Ubuntu". The
user, 2026-10-27: "IT SHOULDN'T SAY UBUNTU"; they chose rebuilding Ubuntu's
front end with a Patrone Linux flavor.

`patrone-installer` is that build. It's the front end at the commit the
ISO's snap was built from (`6623407`, ubuntu-desktop-bootstrap 26.04.1,
snap revision 665), with these changes, all made by `build.sh`:

- **Pictures** (`patch_source.py`; the user, 2026-10-27: "remove all of the
  images that have the ubuntu orange"): Patrone's wordmark and P in place of
  Ubuntu's logo, mascot and disk icon (the one beside the new system's
  partition), and every other picture's orange turned to Patrone's blue at
  the same lightness, so the drawings keep their shading. The installer
  takes pictures only from its own files; a path to a file elsewhere in its
  settings shows nothing.
- **The spinner** (`patch_source.py`): Flutter's plain turning ring, in
  Patrone's blue, in place of Yaru's (on the Next button while a page
  works, and on the loading page).
- **The licence agreement** (`patrone_licence_page.dart`, added by
  `patch_source.py`; the user, 2026-10-28: "a strong EULA ... that we put
  that ssame one up on the install page"): a page after "Try or install"
  shows `legal/eula.md`, the agreement the website's download shows, which
  `build.sh` puts beside the installer (`data/patrone-eula.md`). Next stays
  off until "I have read and accept" is ticked. Accepting writes the
  version and time to `/var/log/installer/patrone-licence-accepted`, which
  the install engine copies to the new system with its logs; it goes
  nowhere else. It's an Accept page, not a lock (the free software's own
  licences let anyone run it; `docs/COMPLIANCE.md`, item 24): the live
  session isn't behind it, automated installs show no pages, and without
  the file the page is skipped.
- **The suggested username** (`patch_source.py`): the first initial and last
  name, "wdaugherty" for "William Daugherty" (Ubuntu's runs the whole name
  together).

- **Ubuntu's flavor is named Patrone Linux** (`ubuntu_flavor` 0.5.0+1, from
  pub.dev, with `ubuntu('Ubuntu')` made `ubuntu('Patrone Linux')`). The
  installer's text names the system as `{DISTRO}`, the flavor's name, so
  it says "Welcome to Patrone Linux" and "Install Patrone Linux"; it still
  behaves as Ubuntu's flavor does.
- **The few messages that say "Ubuntu" themselves** say Patrone Linux
  (`debrand_l10n.py`; the window's title, "Patrone Linux Installer", among
  them), and their translations, which say Ubuntu, give way to the English.
  Ubuntu Pro's pages keep their text; the installer doesn't show them.

The install engine stays the snap's: its `subiquity-server` service runs in
the live session, and Patrone's front end talks to it on
`/run/subiquity/socket`, as the snap's does. That's why the build follows
the snap's commit: the front end and the engine have to agree.

In the live session (the ISO's live layer has the package):

- `ubuntu-desktop-installer.service.d/patrone.conf` makes the user service
  that starts the installer at login start `patrone-installer`, and keeps
  it from turning Ubuntu's desktop icons and dock back on when it closes.
- `patrone-installer` (`/usr/bin`) starts it as the snap's launcher starts
  Ubuntu's (`LIVE_RUN=1`, drives kept from mounting by themselves).
- `ubuntu-desktop-bootstrap_ubuntu-desktop-bootstrap.desktop` hides the
  snap's "Install Ubuntu" launcher; `patrone-installer.desktop` is "Install
  Patrone Linux".
- `patrone-branding` hides the page that offers an update of the snap,
  which would bring back Ubuntu's front end.

## Installing with nothing else

The ISO's boot menu (`iso/build-iso.sh`) starts with **"Install Patrone
Linux"**: the installer alone, full screen, with no desktop behind it, as a
computer's own setup shows (the user, 2026-10-27, "installer only"). **"Try
Patrone Linux"** is the live desktop, with the installer on it. The first
adds `patrone-install-only` to the kernel's command line, and `kiosk/` does
the rest:

- `patrone-install-only.service` (enabled; runs only with that word, before
  AccountsService and GDM) runs `install-only`, which gives the live user,
  whom casper has GDM log in by itself, the session `patrone-installer` in
  place of the desktop.
- That session (`patrone-installer.desktop` in `wayland-sessions`,
  `patrone-installer.session`, `session.conf` for its systemd target) is
  GNOME Kiosk (`gnome-kiosk`, Mutter for one app, which it makes full
  screen) and `patrone-installer-kiosk.service`, which runs `kiosk`: the
  installer, without its "Try or install" page, started again if it's
  closed. GNOME's settings daemons for accessibility, the keyboard and media
  keys run too; desktop autostart doesn't (some of the live session's
  entries open windows, which Kiosk would make full screen over the
  installer).
- The live desktop's own start of the installer
  (`ubuntu-desktop-installer.service.d/patrone.conf`) stays out of it.

## Building it

```sh
installer/build.sh                       # on the dev VM, as yourself
./packaging/build.sh patrone-installer
```

`build.sh` fetches the installer's source, Flutter 3.44.1 (the version the
installer pins in `.tool-versions`) and `ubuntu_flavor` into
`~/.cache/patrone-installer` the first time, builds the app with Ubuntu's own
lock files, and leaves it in `installer/bundle/` (not in git). Ubuntu's
26.04.2 ISO will bring a new snap: build against its commit then (its
`snap/manifest.yaml`).
