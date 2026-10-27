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

Ubuntu's installer makes the new system from stacked layers in `casper/`:
the base (`minimal.squashfs`), sometimes the extra apps of Ubuntu's
"Extended selection" (`minimal.standard.squashfs`), and a language layer
on top (`minimal.en.squashfs`, and so on), as `casper/install-sources.yaml`
lists them. Patrone's ISO has one more, `minimal.patrone.squashfs`, on the
base:

- **It starts as Ubuntu's English layer**, so it has English's language
  packs, and the installer puts no language layer on it (the entry lists no
  `preinstalled_langs`). Other languages' packs come from the network during
  the install, as Ubuntu's own do for languages its ISO doesn't carry.
- **Patrone's packages are installed in it**: the newest build of each in
  `out/` (not `-dev` packages), with what they depend on and recommend from
  Ubuntu's archive, as on the dev VM. That includes the rebuilt GDM and
  GNOME Settings. The `firefox` package, a stub Ubuntu's desktop needs, is
  held, as `default-apps/README.md` says to.
- **Its snap seed** (`/var/lib/snapd/seed`, the snaps a new install sets up
  at first boot) has Chromium, and whatever it needs, in place of Firefox
  and App Center (the `snap-store` snap). `snap debug validate-seed` checks
  it.
- **It's the installer's only choice**, named "Patrone Linux". Its ID is
  Ubuntu's default choice's (`ubuntu-desktop-minimal`), which the installer's
  page of app choices knows.

The rest of Ubuntu's layers go, except the three of the live session (the
base, the standard layer and `minimal.standard.live`): the live session and
the installer are still Ubuntu's, and say Ubuntu. The boot menu says
"Try or Install Patrone Linux", the disc's name is "Patrone Linux 26.04.1
amd64", and `md5sum.txt` (which the live session checks) is brought up to
date. Booting is Ubuntu's, untouched: shim and GRUB signed for Secure Boot,
replayed onto the new disc by `xorriso` with the original disc's date, which
GRUB uses as its ID.

Not done yet: Patrone's own live session and installer (branding, and
Patrone Desktop as the session), languages other than English on the disc,
and a home for Patrone's packages' updates after an install.
