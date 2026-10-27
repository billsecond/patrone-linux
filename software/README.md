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
| `org.patrone.Software.Featured.xml` | `/usr/share/swcatalog/xml/` | Patrone's picks: the banners at the top of Explore, Editor's Choice, and each category's picks. First the Craft apps (the user's list, 2026-10-27), then well-known apps that fill Explore until the Craft apps can show |

## The Craft apps

The user's list (2026-10-27): storytold's Crafting Apps
([github.com/storytold](https://github.com/storytold),
[getartcraft.com](https://getartcraft.com/)), most popular first, then
ScreenGrabCraft and MailCraft. Each is listed by the ID in its own
`packaging/linux/*.metainfo.xml.in`:

| App | ID | Banner |
| --- | --- | --- |
| PhotoCraft | `ai.storyteller.photocraft` | Yes |
| PdfCraft | `ai.storyteller.pdfcraft` | Yes |
| FilmCraft | `ai.storyteller.filmcraft` | Yes |
| WordCraft | `ai.storyteller.wordcraft` | |
| LightCraft | `ai.storyteller.lightcraft` | Yes |
| VectorCraft | `ai.storyteller.vectorcraft` | |
| EffectCraft | `ai.storyteller.effectcraft` | |
| CADCraft | `ai.storyteller.cadcraft` | |
| GridCraft | `ai.storyteller.gridcraft` | |
| DesignCraft | `ai.storyteller.designcraft` | |
| DeckCraft | `ai.storyteller.deckcraft` | |
| SoundCraft | `ai.storyteller.soundcraft` | |
| FigCraft | `ai.storyteller.figcraft` | |
| Craft Launcher | `ai.storyteller.artcraft-launcher` | |
| ScreenGrabCraft | `io.github.billsecond.screengrabcraft` | Yes |
| MailCraft | `io.github.billsecond.mailcraft` | Yes |

All of them are in Editor's Choice and their categories' picks. **None
shows yet**: none is in Ubuntu's archive or on Flathub. Each project's
GitHub releases have `.deb`, `.flatpak` and AppImage files, which GNOME
Software doesn't read; it reads the catalogs of apt repositories and
Flatpak remotes. A `.flatpak` file installed by hand gets a remote of its
own with no catalog (the user's PdfCraft is from `pdfcraft-origin`), so it
shows under Installed but never on Explore. They'll show once they're in a
Flatpak repository with a catalog (Flathub, or one of the Craft apps' own)
that's set up on the machine. ArtCraft has no Linux build (its releases are
for macOS and Windows), so it isn't listed.

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

What the placeholder showed (2026-10-27, GNOME Software 50.0, in the
throwaway shell):

- **The banners need Flathub.** GNOME Software puts an app in them only
  with a 128-pixel icon, and Ubuntu's archive has icons up to 64 (64@2 for
  HiDPI screens). With the user's account's Flathub (which has all seven
  apps, with large icons) the banners showed GIMP and four more, and
  Editor's Choice GIMP, Krita, Audacity and the rest. With the archive
  alone, Explore showed only the categories. Accounts get Flathub from
  Patrone Unpack when a Flatpak needs it; a system-wide Flathub would give
  every account the picks.
- **The archive's IDs aren't always Flathub's**: Blender is
  `blender.desktop` in Ubuntu's archive and `org.blender.Blender` on
  Flathub, VLC `org.videolan.vlc` and `org.videolan.VLC`. Listing both
  covers both.
- Editor's Choice needs six apps that resolve, and shows six or twelve.
Patrone's own apps can't be listed yet: they're `.deb` files built on the
dev VM, and no package repository serves them.

Other things GNOME Software still reaches: Ubuntu's archive and the Flatpak
remotes for the apps themselves, and GNOME's reviews service
(`odrs.gnome.org`, run by GNOME) for ratings and reviews.
