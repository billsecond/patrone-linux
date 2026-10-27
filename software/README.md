# Patrone's app store

`patrone-software` (`packaging/patrone-software`) makes GNOME Software
Patrone's app store, in place of Ubuntu's App Center (the user's choice,
2026-10-27). App Center is Canonical's own: the `snap-store` snap, written in
Flutter, which browses only Canonical's Snap Store, shows Canonical's picks
and calls itself the "App Store for Ubuntu". GNOME Software is GTK 4, comes
from Ubuntu's archive (universe), finds apps in Ubuntu's archive and on
Flatpak remotes such as Flathub, and shows a distribution's own picks from a
file on the machine, so Patrone's list needs no server.

| File | Installed as | What it does |
| --- | --- | --- |
| `org.patrone.Software.Featured.xml` | `/usr/share/swcatalog/xml/` | Patrone's picks: the banners at the top of Explore, Editor's Choice, and each category's picks. **A placeholder for now**: the user is sending the real list |

The package:

- **Depends on** `gnome-software` (which brings its plugin for Ubuntu's
  archive, `gnome-software-plugin-deb`) and `gnome-software-plugin-flatpak`.
  It leaves out `gnome-software-plugin-snap`, so the Snap Store isn't
  browsed (the user: "removing Canonical's"). Installed snaps, such as
  Chromium, still update through snapd.
- **Moves GNOME's own list aside** (preinst): Ubuntu's GNOME Software ships
  GNOME's picks, 60 GNOME Circle apps, as
  `/usr/share/swcatalog/xml/org.gnome.Software.Featured.xml`. It's diverted
  to `org.gnome.Software.Featured.xml.gnome`, a name GNOME Software doesn't
  read, so only Patrone's picks show; removing the package puts it back
  (postrm).
- **Removes App Center** when it's first installed (postinst): `snap remove
  snap-store`. App Center is a snap, which Ubuntu's installer adds, so no
  package dependency can remove it. If snapd can't (no snapd, as in a
  chroot), the install goes on and says how to remove it by hand. Removing
  `patrone-software` doesn't bring App Center back: `sudo snap install
  snap-store` does.

## The list

Each app is a `<component merge="append">` with the app's AppStream ID, as
Ubuntu's archive or Flathub knows it (`org.gimp.GIMP`), and up to three
marks, as GNOME's `gnome-app-list` documents them:

| Mark | Where the app shows | At least |
| --- | --- | --- |
| `<custom><value key="GnomeSoftware::FeatureTile">True</value></custom>` | The banners at the top of Explore | 5 apps |
| `<kudos><kudo>GnomeSoftware::popular</kudo></kudos>` | Editor's Choice on Explore | 6 apps |
| `<categories><category>Featured</category></categories>` | The picks on the app's category page | 3 in a category |

An app that neither Ubuntu's archive nor a Flatpak remote has is skipped.
Patrone's own apps can't be listed yet: they're `.deb` files built on the
dev VM, and no package repository serves them.

Other things GNOME Software still reaches: Ubuntu's archive and the Flatpak
remotes for the apps themselves, and GNOME's reviews service
(`odrs.gnome.org`, run by GNOME) for ratings and reviews.
