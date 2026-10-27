# Patrone's first-login setup

The setup a new account gets at its first login is Ubuntu's
`gnome-initial-setup`, with Ubuntu's own pages ("Choose how Ubuntu looks",
Ubuntu's App Center, Ubuntu's logo and orange pictures). The user, after
installing from Patrone's ISO (2026-10-27): it shouldn't say Ubuntu, and "Then
it opens the Canonical app center". `build.sh` rebuilds Ubuntu's package with
Patrone's changes (`patrone.py`), as `gnome-settings/build.sh` does for
Settings:

- **Words:** "Choose how Patrone Linux looks", "Welcome to Patrone Linux"
  (the welcome page uses os-release's name, Patrone's; this is the fallback).
- **"Get started with more applications"** offers Patrone Software ("Open
  Patrone Software", GNOME Software) in place of Ubuntu's App Center, with
  `patrone-software.svg`, Patrone Software among the Craft apps, drawn by
  `make_software_picture.py`.
- **Pictures:** Patrone's P in place of Ubuntu's logo (the language page)
  and mascot (the welcome page); the rest in Patrone's blue where they're in
  Ubuntu's orange, by the installer's rule (`installer/patch_source.py`).

Ubuntu Pro's and Ubuntu Insights' pages stay out (patrone-desktop's
`vendor.conf`).

```sh
gnome-initial-setup/build.sh      # on the dev VM; the package lands in out/
```

Build-Depends are Ubuntu's (`dpkg-checkbuilddeps` in the unpacked source);
`libgdm-dev` must be Patrone's GDM build's (`out/`), to match its `libgdm1`.
