# Default apps

Which apps Patrone uses by default, packaged as `patrone-default-apps`
(`packaging/patrone-default-apps`).

| File | Installed as | What it does |
| --- | --- | --- |
| `mimeapps.list` | `/etc/xdg/mimeapps.list` | Chromium opens web pages and links; Patrone File Manager opens folders (Nautilus is the fallback when it isn't installed); Patrone Unpack opens archives (zip, tar, 7z, rar and single compressed files), .deb packages and Flatpak files (`.flatpak`, `.flatpakref`, `.flatpakrepo`), in place of Nautilus for archives and Canonical's App Center for .deb packages, which stay as the fallbacks |
| `91_patrone-default-apps.gschema.override` | `/usr/share/glib-2.0/schemas/` | The favorites: Chromium first, in place of Firefox, then Patrone Terminal and Patrone File Manager (in place of Ubuntu's Files) |

The package depends on Ubuntu's `chromium-browser`, a small package that
installs Canonical's Chromium snap, which snapd keeps up to date (see
"Default apps" in `docs/PLAN.md`).

It doesn't remove Firefox. Ubuntu's `firefox` package is also a small one
that installs a snap, and `ubuntu-desktop-minimal` depends on it, so removing
it would take Ubuntu's desktop metapackage with it. Remove the snap instead,
and hold the `firefox` package, because each upgrade of it installs the snap
again:

```
sudo snap remove firefox
sudo apt-mark hold firefox
```

Patrone's ISO will leave Firefox out altogether, with a Patrone desktop
metapackage in place of `ubuntu-desktop-minimal`.

Users' own choices still win: `~/.config/mimeapps.list` (Settings → Apps →
Default Apps) and their own favorites.
