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
| `org.patrone.Software.Featured.xml` | `/usr/share/swcatalog/xml/` | Patrone's picks: the banners at the top of Explore, Editor's Choice, and each category's picks. The Craft apps (the user's list, 2026-10-27), and only them |
| `patrone-app-index.flatpakrepo` | `/usr/share/patrone-software/` | Patrone's app index, where the Craft apps come from; postinst adds it as a system Flatpak remote |
| `flathub.flatpakrepo` | `/usr/share/patrone-software/` | Flathub's own remote file (from `dl.flathub.org`): the Craft apps' runtime, and Flathub's apps |

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

All of them are in Editor's Choice and their categories' picks. They come
from **Patrone's app index** (since 0.1.4), a Flatpak repository in the
user's GitHub that holds no apps: [billsecond/patrone-app-index](https://github.com/billsecond/patrone-app-index),
published with GitHub Pages at `https://billsecond.github.io/patrone-app-index/repo/`.
Each entry is an app's listing (name, icon, description, permissions, from
the `.flatpak` on its latest GitHub release) and the address and checksum of
the Linux tarball on that release; installing the app makes Flatpak download
the tarball from the project's release and unpack it (Flatpak's "extra
data"). A GitHub Actions job there checks for new releases once a day, so a
release needs nothing from anyone. The user's terms (2026-10-27): host only
the index, never the apps or installers, and nothing that needs regular
upkeep. GNOME Software doesn't read a project's GitHub releases by itself:
only the catalogs of apt repositories and Flatpak remotes.

postinst adds the index and Flathub (whose freedesktop runtime the apps
run on) as system remotes on the first install, or the upgrade to 0.1.4,
so every account sees the apps and a remote someone deletes stays deleted.
Removing the package deletes the index's remote unless apps installed from
it still need it.

Not in the index: **ScreenGrabCraft**, whose repository is private, so its
releases can't be downloaded by others (one line in the index's `apps.list`
once they're public); AniCraft, which has no releases yet; and ArtCraft,
which has no Linux build (its releases are for macOS and Windows).

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
- **Names it Patrone Software** (0.1.2; the user looked for it by that
  name): GNOME Software's launcher,
  `/usr/share/applications/org.gnome.Software.desktop`, is diverted to
  `org.gnome.Software.desktop.gnome` (not read, not ending in `.desktop`),
  and `make-software-launcher` (`/usr/libexec/patrone-software/`) writes it
  again with `Name=Patrone Software`, keeping its desktop file ID, so pins,
  the default favorites and the Start menu's suggestion still find it. A
  trigger on `/usr/share/applications` writes it again whenever
  gnome-software updates its launcher. Translations of the name are left as
  they are, and the app's own window still says Software.
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

The well-known apps that first stood in (GIMP, Inkscape, Krita, Blender,
VLC, Audacity, OBS Studio) are gone since 0.1.3: the user wants only the
Craft apps featured. Until those are in a repository GNOME Software reads,
Explore shows only its categories. What the stand-ins showed (2026-10-27,
GNOME Software 50.0, in the throwaway shell), which holds for the Craft
apps too:

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
