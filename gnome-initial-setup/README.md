# Patrone's first-login setup

The setup a new account gets at its first login is Ubuntu's
`gnome-initial-setup`, with Ubuntu's own pages ("Choose how Ubuntu looks",
Ubuntu's App Center, Ubuntu's logo and orange pictures). The user, after
installing from Patrone's ISO (2026-10-27): it shouldn't say Ubuntu, and "Then
it opens the Canonical app center". `build.sh` rebuilds Ubuntu's package with
Patrone's changes (`patrone.py`), as `gnome-settings/build.sh` does for
Settings:

- **Words:** "Choose how PatroneOS looks", "Welcome to PatroneOS"
  (the welcome page uses os-release's name, Patrone's; this is the fallback).
- **"Get started with more applications"** offers Patrone Software ("Open
  Patrone Software", GNOME Software) in place of Ubuntu's App Center, with
  `patrone-software.svg`, Patrone Software among the Craft apps, drawn by
  `make_software_picture.py`.
- **Pictures:** Patrone's P in place of Ubuntu's logo (the language page)
  and mascot (the welcome page); the rest in Patrone's blue where they're in
  Ubuntu's orange, by the installer's rule (`installer/patch_source.py`).

Ubuntu Pro's and Ubuntu Insights' pages stay out (patrone-desktop's
`vendor.conf`). Setup still makes skipped pages, out of sight, and Ubuntu
Insights' kept calling the crash reporter's settings service
(`whoopsie-preferences`, which Ubuntu's GNOME Settings needs, so it stays):
without the crash reporter, which `patrone-desktop` removes, each call asked
for an administrator's password, five times in one setup (2026-10-27). In
revision 2 the page doesn't call it.

From revision 3 the package names Patrone as its maintainer and Ubuntu
Developers as the original one, as Debian's guidelines for derivatives have
it, and its copyright file names Patrone's pictures and the icons in
`patrone-software.svg`: GNOME Software's (GPL-2+) and ten Craft apps',
which their makers (the ArtCraft Team and the contributors of each, and
MailCraft's) license as MIT OR Apache-2.0, used here under MIT, whose
notice the file carries (`docs/COMPLIANCE.md`, item 13).

From revision 4 the welcome page has no "View release notes" link: it opened
Ubuntu's release notes for the system's version (Ubuntu's notes for "0.1").
It comes back when Patrone has release notes of its own
(`docs/COMPLIANCE.md`, item 18).

```sh
gnome-initial-setup/build.sh      # on the dev VM; the package lands in out/
```

Build-Depends are Ubuntu's (`dpkg-checkbuilddeps` in the unpacked source);
`libgdm-dev` must be Patrone's GDM build's (`out/`), to match its `libgdm1`.
