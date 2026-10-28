# Patrone's ISO

`build-iso.sh` makes Patrone Linux's installer ISO from Ubuntu's 26.04.1
desktop ISO (PLAN.md, "Build pipeline": remaster Ubuntu's, keep its
installer). Build Patrone's packages first (`packaging/build.sh`, into
`out/`), then, on the dev VM:

```sh
sudo iso/build-iso.sh ~/iso/ubuntu-26.04.1-desktop-amd64.iso out /var/tmp/patrone-linux-26.04.1-amd64.iso
```

Try it in a new Hyper-V VM with `scripts/hyperv/new-test-vm.ps1`.

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
- **The snap seed** (`/var/lib/snapd/seed`, the snaps a new system sets up
  at first boot) has Chromium, and the snaps it needs, in place of Firefox
  and App Center (the `snap-store` snap), and without Security Center (Ubuntu
  Pro's and the snaps' settings); `snap debug validate-seed` checks it.
  Ubuntu's layers come with their seed's snaps set up already
  ("preseeded"), which kept App Center, Firefox and Security Center and
  ignored the seed's changes (found by a search of the installed system,
  2026-10-27): each layer is reset (`snap-preseed --reset`) before the seed
  is changed and preseeded again when it's closed, or, if that fails, sets
  its snaps up at first boot. The `firefox` package, a stub that installs
  Firefox's snap whenever it's installed or upgraded, is removed; Ubuntu's
  desktop only recommends it.
- **What Patrone's packages remove is purged**, so nothing is left of their
  settings (`rc`): `ubuntu-report`'s autostart entry, and `update-notifier`'s
  ("Ubuntu Pro is available" at login), which `patrone-desktop` takes the
  place of.

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
date. Booting is Ubuntu's, untouched: shim and GRUB signed for Secure Boot,
replayed onto the new disc by `xorriso` with the original disc's date, which
GRUB uses as its ID.

Not done yet: languages other than English on the disc, Patrone Desktop
(the compositor) as the session, and a home for Patrone's packages'
updates after an install.
