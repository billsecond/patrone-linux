# Patrone's GNOME Software

GNOME Software's tabs are fixed when it's built, so a Featured tab means
building it again, as Patrone does GDM and GNOME Settings. The user asked for
the tab on 2026-10-31: "Make it a Featured tab in front of explore".
`build.sh` builds Ubuntu's `gnome-software` with one patch,
`patches/patrone-featured-tab.patch`.

- **A Featured tab, before Explore** (`src/gs-featured-page.c`, new): banners
  for the apps with a large enough icon, then a tile for every app, by name.
  It's where the app opens, where going back ends up, and where it returns
  when it's opened again. Alt+1 is Featured, and Alt+2 to 4 are Explore,
  Installed and Updates. `gnome-software --mode=featured` opens it;
  `--mode=overview` is still Explore.
- **Why a page of its own:** Explore can't show a short list whole. It keeps
  five banners, and trims Editor's Choice to a multiple of six, so a list of
  seven would lose an app there, and a different one on different days.

The apps are the ones Patrone's list marks with
`GnomeSoftware::DeploymentFeatured` set to `patrone`
(`software/org.patrone.Software.Featured.xml`, `software/README.md`). That
mark is GNOME Software's own, for a distribution's picks. Explore only reads
it when there's a `deployment-featured.ini`, and Patrone ships none, so the
tab has it to itself. With no apps carrying GNOME's other two marks, Explore
hides its banners and Editor's Choice by itself, with no change to Explore.

The package is Ubuntu's otherwise, snap plugin included, which
`patrone-software` still leaves out. `build.sh` names Patrone as maintainer
and Debian's GNOME team as the original one, and adds Patrone's files to the
copyright file (`docs/COMPLIANCE.md`, item 13). Ubuntu's updates are built
again with the patch by `packaging/ubuntu-updates.sh`, and a change to the
patch needs one more `revision=` in `build.sh` to publish
(`packaging/README.md`, "Publishing").

Build it on the dev VM, which needs Ubuntu's build dependencies for
`gnome-software`:

```
./gnome-software/build.sh
sudo apt install ./out/gnome-software_50.0-1patrone1_amd64.deb \
    ./out/gnome-software-common_50.0-1patrone1_all.deb \
    ./out/gnome-software-plugin-flatpak_50.0-1patrone1_amd64.deb \
    ./out/gnome-software-plugin-deb_50.0-1patrone1_amd64.deb
```

The new words are in English only, since they aren't in GNOME's translations.
