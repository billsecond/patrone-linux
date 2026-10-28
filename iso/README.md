# Patrone's ISO

`build-iso.sh` makes Patrone Linux's installer ISO from Ubuntu's 26.04.1
desktop ISO (PLAN.md, "Build pipeline": remaster Ubuntu's, keep its
installer). Build Patrone's packages first (`packaging/build.sh`, into
`out/`), then, on the dev VM:

```sh
sudo iso/build-iso.sh ~/iso/ubuntu-26.04.1-desktop-amd64.iso out /var/tmp/patrone-linux-26.04.1-amd64.iso
```

Try it in a new Hyper-V VM with `scripts/hyperv/new-test-vm.ps1`.

Build it from a commit that's pushed, with nothing uncommitted: the disc
records the commit, and its source is that commit (below).

## Its source code

Whoever passes on the disc image has to offer the source of the free
software on it, and GPL-2 asks for it from "the same place"
(`docs/COMPLIANCE.md`, item 1). So each disc image is published with its
source beside it:

```sh
iso/build-source.sh /var/tmp/patrone-linux-26.04.1-amd64.iso /var/tmp/patrone-linux-26.04.1-source
```

`build-iso.sh` writes two files onto the disc for it: `SOURCE.txt` at the
root, saying where the source is (`https://patronelinux.com/source`, and
Patrone's own on GitHub, at the disc's commit), and `casper/sources.manifest`,
every source package of what's on the disc, name and exact version: what
Patrone's two layers and the three of Ubuntu's they stand on have installed
(Ubuntu's include what Patrone's layers remove, still on the disc, hidden),
the package pool's, and the snaps in the seeds. `build-source.sh` (as
anyone, with a network; about half an hour and 7 GB) fetches each of
Ubuntu's from Launchpad at its exact version (Ubuntu's mirrors keep only
the newest), maps Patrone's rebuilds (`...patroneN`) to the Ubuntu version
they're built from, and adds Patrone's repository at the disc's commit and
the installer's upstream source, with a `README.txt` and `SHA256SUMS`. Run
it again if it says something failed; it keeps what it has. Keep each
published disc's source for at least three years (GPL-2's written offer
asks that), and as long as the disc is offered.

## How it's put together

Ubuntu's disc is made of stacked layers in `casper/`: the base
(`minimal.squashfs`), the extra apps of Ubuntu's "Extended selection"
(`minimal.standard.squashfs`), language layers (`minimal.en.squashfs`,
...), and the live session's own (`minimal.standard.live.squashfs`).
`casper/install-sources.yaml` lists what the installer can install. The
user's rule (2026-10-27): **nothing on screen says Ubuntu**. So Patrone's ISO
has two more layers, each with Patrone's packages installed (the newest
build of each in `out/`, not `-dev` packages, with what they depend on and
recommend from Ubuntu's archive, as on the dev VM, including the rebuilt
GDM and GNOME Settings):

- **`minimal.patrone`, what the installer installs**, on the base. It starts
  as Ubuntu's English layer, so it has English's language packs, and the
  installer puts no language layer on it (the entry lists no
  `preinstalled_langs`); other languages' packs come from the network during
  the install. It's the installer's only choice, named "Patrone Linux"; its
  ID is Ubuntu's default choice's (`ubuntu-desktop-minimal`), which the
  installer's page of app choices knows.
- **`minimal.standard.live.patrone`, the live session**, on Ubuntu's live
  layer. The live desktop is Patrone's, and so is the installer that starts
  in it (Ubuntu's `ubuntu-desktop-bootstrap` snap): `patrone-branding` gives
  it Patrone's name, blue, wordmark and P through
  `/usr/share/desktop-provision/` (`branding/system/desktop-provision/`).
  Patrone Terminal doesn't open at login here, so the installer has the
  screen; Ctrl+Alt+T opens it.

In both:

- **Ptyxis, Ubuntu's terminal, is removed** (the user: "the other terminal
  shouldn't even show up"); Ubuntu's desktop only recommends it.
- **No web browser.** Each account picks its first one: the taskbar's Web
  browser button, or the first link opened, brings up Patrone Browser
  Chooser, which has Patrone Software install the browser from Flathub
  (`browser-chooser/`). The snap seed (`/var/lib/snapd/seed`, the snaps a
  new system sets up at first boot) has neither Firefox nor Chromium, nor
  App Center (the `snap-store` snap) or Security Center (Ubuntu Pro's and
  the snaps' settings); `snap debug validate-seed` checks it. The build
  stops if a layer has a browser after all: a package that provides
  `x-www-browser` or `gnome-www-browser`, a launcher in the `WebBrowser`
  category, or a browser's snap in the seed (`check_no_browser`).
  Ubuntu's layers come with their seed's snaps set up already
  ("preseeded"), which kept App Center, Firefox and Security Center and
  ignored the seed's changes (found by a search of the installed system,
  2026-10-27): each layer is reset (`snap-preseed --reset`) before the seed
  is changed and preseeded again when it's closed, or, if that fails, sets
  its snaps up at first boot. The `firefox` package, a stub that installs
  Firefox's snap whenever it's installed or upgraded, is removed; Ubuntu's
  desktop only recommends it. So is `chromium-browser`, the same for
  Chromium's snap, which `patrone-default-apps` depended on before 0.2.0.
- **No Thunderbird or hwctl** (`docs/COMPLIANCE.md`, items 11 and 12): the
  `thunderbird` stub and its language packs, which Ubuntu's full desktop in
  the live layer recommends, are removed, and the seed has neither
  Thunderbird's snap (Canonical's build of Mozilla's app) nor `hwctl`'s
  (Canonical's hardware-certification tool).
- **Ubuntu's metapackages go**: `patrone-desktop` 0.2.0 lists Ubuntu's
  desktop packages itself, less Ubuntu Pro's client, Software Updater,
  Ubuntu's Help and `hwctl` (`desktop/README.md`), so installing it removes
  `ubuntu-desktop-minimal`, `ubuntu-minimal` and, in the live layer,
  `ubuntu-desktop`. In the installer's layer, what nothing installed needs
  any more is removed too (`apt-get autoremove`, listed in the build's
  output), so a new system's apt doesn't start by offering to: in the build
  of 2026-10-28, 16 packages, Ubuntu's release upgrader
  (`ubuntu-release-upgrader-core`, `python3-distupgrade`), Software
  Updater's library (`python3-update-manager`), and what only the packages
  Patrone removes had pulled in (`gdb`, `libc6-dbg`, `patch`, `screen`,
  `python3-debian`, `distro-info`, WebKit's and libhandy's GObject
  bindings and their libraries). Anyone who wants `gdb` or `screen` can
  install them.
- **What Patrone's packages remove is purged**, so nothing is left of their
  settings (`rc`): `ubuntu-report`'s autostart entry, `update-notifier`'s
  ("Ubuntu Pro is available" at login), which `patrone-desktop` takes the
  place of, and Ubuntu Pro's apt hook.

**The live session's boot image** (`casper/initrd`) is Ubuntu's own, which
knows the disc (`conf/uuid.conf`) and how to start the live session, with
Patrone's boot splash (Plymouth's `patrone` and `patrone-text` themes in
place of Ubuntu's) and Patrone's live layer as the one to boot
(`conf/conf.d/default-layer.conf`).

The rest of Ubuntu's layers go. The boot menu starts with "Install Patrone
Linux", the installer alone, full screen, then "Try Patrone Linux", the live
desktop (`installer/README.md`, "Installing with nothing else"); the disc's
name is "Patrone Linux 26.04.1 amd64", `.disk/info` says
Patrone Linux (the live session's user and computer are named `patrone`
after it), and `md5sum.txt` (which the live session checks) is brought up to
date. Ubuntu's `ubuntu` link at the disc's root, back to the disc itself,
goes: nothing uses it (apt and the installer read `file:///cdrom`), and
Windows showed it as a file named UBUNTU. The package archive on the disc
(`dists/`, `pool/`) stays Ubuntu's, signed by Ubuntu, so its files carry
Ubuntu's names and versions. Booting is Ubuntu's, untouched: shim and GRUB signed for Secure Boot,
replayed onto the new disc by `xorriso` with the original disc's date, which
GRUB uses as its ID.

Not done yet: languages other than English on the disc, Patrone Desktop
(the compositor) as the session, and a home for Patrone's packages'
updates after an install.
