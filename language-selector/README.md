# Patrone's Language Support

Language Support (Settings › Region & Language › Manage Installed
Languages) is Ubuntu's `language-selector`. Its words pointed at Ubuntu,
which implies Ubuntu supports Patrone (`docs/COMPLIANCE.md`, item 18).
`build.sh` rebuilds Ubuntu's package with Patrone's changes, as
`gnome-initial-setup/build.sh` does for first-login setup:

- **The keyboard input method's tip** said "The recommended value for
  Ubuntu is "IBus"". Now "The recommended value is "IBus"": IBus is what
  Patrone's desktop uses too.
- **A failed install** asked for a bug report in Launchpad, Ubuntu's bug
  tracker. Now it says only that it may be a bug.
- **The help** (`help/C/index.docbook`) described Ubuntu: "Ubuntu has been
  translated", "Ubuntu uses the GNU gettext technology", "Ubuntu with
  GNOME". It names Patrone Linux, which is built on Ubuntu, so what it says
  is as true. Its credits and licence (CC BY-SA 2.5) stay, with a line saying
  what Patrone changed, as the licence asks.
- **Patrone maintains the build**, with Ubuntu Developers as the original
  maintainer, as Debian's guidelines for derivatives have it (item 13).

`build.sh` stops if a string it changes isn't there any more, or if the
help names Ubuntu anywhere else, so Ubuntu's next version can't slip
through unchanged. Ubuntu's code comments that link to Launchpad bugs stay.
Translations of the changed words fall back to English.

```sh
language-selector/build.sh      # on the dev VM; the packages land in out/
```

Install `language-selector-common` and `language-selector-gnome` together.
`packaging/ubuntu-updates.sh` builds Ubuntu's updates of it again, and
`desktop/apt/patrone.pref` keeps Patrone's build installed until then.
