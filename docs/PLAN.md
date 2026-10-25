# Patrone Linux: project plan

Patrone Linux is an Ubuntu-based desktop OS with a customized GNOME, built as an
installable ISO.
Base: **Ubuntu 26.04.1 LTS ("Resolute Raccoon")**, GNOME as shipped in 26.04.

## Approach: a derivative, not a full fork

Ubuntu is tens of thousands of packages. We do not copy all of it. We use Ubuntu's
archive as-is and add a thin layer of our own packages on top, the way Pop!_OS,
Zorin and Linux Mint do. We fork an individual Ubuntu or GNOME package only when
configuration or an extension can't get us what we want, because every fork has
to be rebased whenever Ubuntu updates that package.

## Customization layers (cheapest to maintain first)

1. **Defaults and branding**, in `packages/patrone-desktop-settings/`
   GSettings schema overrides (`/usr/share/glib-2.0/schemas/90_*.gschema.override`),
   wallpaper, GTK and Shell theme, icons, fonts, dock favorites, default apps.
   No GNOME source changes.
2. **GNOME Shell extensions**, in `packages/patrone-shell-extensions/`
   Panel, dock, workspace and window-management behavior, shipped as system
   extensions and enabled by default.
3. **Forked GNOME components**, in `forks/` (only when needed)
   Start from Ubuntu's packaging (`git-ubuntu clone gnome-shell`, or `mutter`,
   `gnome-control-center`), carry our changes as patches in `debian/patches/`,
   and version them so they sort above Ubuntu's (for example `+patrone1`).

## Build pipeline

- **Packages:** build `.deb`s in a clean chroot with `sbuild`.
- **Apt repository:** a private repo generated with `reprepro`. Launchpad PPAs
  are always public, so they're out while the project is private.
- **ISO:** remaster the official 26.04.1 desktop ISO. Extract it, add our apt
  repo and packages inside the root filesystem, rebuild the squashfs layers and
  manifests, and repack with `xorriso`. This keeps Ubuntu's own installer. A
  from-scratch build (debootstrap and our own installer) waits until we outgrow
  remastering.
- **Test:** boot each ISO with QEMU/KVM inside the dev VM (nested virtualization
  is on), and do full installs in disposable Hyper-V VMs.

## Environments

| Machine | Purpose |
| --- | --- |
| `PatroneDev` (Hyper-V, Ubuntu 26.04.1 Desktop, 12 vCPU, 32 GB, 250 GB) | Day-to-day development. The repo lives here, packages and ISOs are built here, and Claude Code runs here over SSH from the Claude app on Windows. |
| Throwaway test VMs | Install and boot each built ISO from scratch. |

## Repo layout

```
docs/                  plan, decisions, how-to notes
packages/              our own packages (settings, branding, extensions)
forks/                 patched Ubuntu/GNOME packages, added only when needed
build/                 package, apt-repo and ISO build scripts
```

## Milestones

1. Dev VM installed, reachable by SSH, Claude Code running there, repo cloned.
2. A settings package that changes wallpaper, theme and dock, verified on the
   dev VM after logging out and back in.
3. First remastered ISO that boots, installs, and comes up with our defaults.
4. First patched GNOME component, if a goal needs one.
