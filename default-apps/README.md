# Default apps

Which apps Patrone uses by default, packaged as `patrone-default-apps`
(`packaging/patrone-default-apps`).

| File | Installed as | What it does |
| --- | --- | --- |
| `mimeapps.list` | `/etc/xdg/mimeapps.list` | Chromium opens web pages and links; Patrone File Manager opens folders (Nautilus is the fallback when it isn't installed); Patrone Unpack opens archives (zip, tar, 7z, rar and single compressed files), .deb packages and Flatpak files (`.flatpak`, `.flatpakref`, `.flatpakrepo`), in place of Nautilus for archives and GNOME Software for .deb packages, which stay as the fallbacks (Canonical's App Center is gone: `patrone-software`) |
| `91_patrone-default-apps.gschema.override` | `/usr/share/glib-2.0/schemas/` | The favorites: Chromium first, in place of Firefox, then Patrone Terminal and Patrone File Manager (in place of Ubuntu's Files) |

The package depends on Ubuntu's `chromium-browser`, a small package that
installs Canonical's Chromium snap, which snapd keeps up to date (see
"Default apps" in `docs/PLAN.md`).

It doesn't remove Firefox. Ubuntu's `firefox` package is also a small one
that installs a snap, every time it's installed or upgraded.
`ubuntu-desktop-minimal` only recommends it, so it can go without taking
Ubuntu's desktop with it. Patrone's ISO removes it (`iso/build-iso.sh`). On
a system installed some other way, or from an ISO made before 2026-10-28:

```
sudo snap remove firefox
sudo apt purge firefox
```

A hold (`apt-mark hold firefox`), which this page used to suggest, isn't
enough: it leaves it to every updater to honour, and Ubuntu has published an
update to the stub since 26.04's release.

Users' own choices still win: `~/.config/mimeapps.list` (Settings → Apps →
Default Apps) and their own favorites.
