# Patrone Linux: project plan

Patrone Linux is an Ubuntu-based operating system with its own, brand-new desktop
environment, **Patrone Desktop**, built as an installable ISO.

## What comes from Ubuntu, and what is ours

Ubuntu 26.04.1 LTS ("Resolute Raccoon") supplies everything below the desktop:
the kernel and drivers, systemd, the package archive and security updates,
NetworkManager, PipeWire audio, Bluetooth, power management, printing, and the
apps people install. We don't fork any of that.

Patrone Desktop replaces GNOME. It is a new Wayland desktop that talks to the
same standard Linux services GNOME uses, so ordinary GTK, Qt and Electron apps
run on it unchanged, and X11 apps run through Xwayland.

## Patrone Desktop components

| Component | Job | Notes |
| --- | --- | --- |
| Compositor | Draws every window, handles input, multiple monitors, window management | The core of the desktop, and the biggest piece of work |
| Shell | Panel, app launcher, dock, notifications, quick settings | Separate programs that draw on the screen through the layer-shell protocol |
| Lock screen | Locks and unlocks the session | Uses the ext-session-lock protocol |
| Settings app | Displays, input, network, sound, appearance | Talks to NetworkManager, PipeWire and UPower over D-Bus |
| Session | Starts everything at login | A session entry the login screen can pick, plus systemd user units |
| Portals | Screenshots, screen sharing, file pickers for sandboxed apps | Start from the existing GTK portal; add our own backend where needed |
| Terminal | Patrone Terminal, the default terminal | GTK4 + VTE; Kali-style look, copy on select, paste on right-click, zsh with a two-line prompt (see `docs/NEXT.md`) |
| Remote desktop | RDP access, including Hyper-V enhanced sessions | Clipboard sharing and desktop resizing when Patrone runs as a Hyper-V guest (see `docs/NEXT.md`) |

At first the login screen is GDM, Ubuntu's own, which can start any Wayland
session. A Patrone login screen can come later.

## Default apps

| Job | App | Notes |
| --- | --- | --- |
| Terminal | Patrone Terminal | Ours (`terminal/`) |
| Shell | bash with Patrone's two-line prompt (decided 2026-10-26); zsh set up the same way for those who choose it | `patrone-shell-config` |
| Web browser | Chromium, replacing Firefox (decided 2026-10-25) | Canonical's Chromium snap, kept current automatically by snapd. Not forked or rebranded: Chromium ships security fixes about weekly and a major release every four weeks, and a fork would make us responsible for every one of them. Patrone's look and defaults come from Chromium's enterprise policies instead, in `/etc/chromium-browser/policies/managed/` (the snap's `etc-chromium-browser-policies` plug reads that folder). |

## Technology (decided 2026-10-25)

- **Compositor: Rust with [Smithay](https://github.com/Smithay/smithay).**
  Rust keeps the most privileged process on the desktop memory-safe, and Smithay
  is proven: System76's COSMIC desktop and the niri compositor are both built on
  it. We chose it over C with wlroots, the library behind Sway and labwc.
- **Shell and settings UI: GTK4 through gtk4-rs, with gtk4-layer-shell.** This
  gives us accessibility, input methods, high-quality text and theming without
  writing them ourselves. Patrone's look comes from our own widgets and styling.
- **License: GPL-3.0-or-later** for all of Patrone's own code. GPL code (such
  as niri's) can be borrowed, and MIT or Apache code (such as Smithay's) can
  be included as long as its notice is kept.

## Development loop

The compositor can run as a window inside the dev VM's existing GNOME desktop,
so we can rebuild and relaunch it in seconds without logging out. Once it can
run a full session, we install a session entry and choose "Patrone" at the login
screen.

**Hyper-V limitation:** Hyper-V gives Linux guests no GPU acceleration, so
rendering in the VM is done in software by Mesa. That's fine for building the
desktop, but animation smoothness and GPU-specific behavior need checking on
real hardware before a release.

## Build pipeline

- **Packages:** each component becomes a `.deb` (`patrone-compositor`,
  `patrone-shell`, `patrone-settings`, `patrone-session`, and the
  `patrone-desktop` metapackage), built in a clean chroot with `sbuild`.
  `patrone-terminal`, `patrone-shell-config` and `patrone-branding` exist
  already (see `packaging/README.md`).
- **Apt repository:** a private repo generated with `reprepro`. Launchpad PPAs
  are always public, so they're out while the project is private.
- **ISO:** remaster the official 26.04.1 desktop ISO. Add our packages, make
  Patrone Desktop the default session, apply branding, and repack with
  `xorriso`. Ubuntu's installer stays in place. A from-scratch image build waits
  until we outgrow remastering.
- **Test:** boot each ISO with QEMU/KVM inside the dev VM (nested virtualization
  is on), and do full installs in disposable Hyper-V VMs.

## Environments

| Machine | Purpose |
| --- | --- |
| `PatroneDev` (Hyper-V, Ubuntu 26.04.1 Desktop, 12 vCPU, 32 GB, 250 GB) | Day-to-day development. The repo lives here, everything is built here, and Claude Code runs here over SSH from the Claude app on Windows. |
| Throwaway test VMs | Install and boot each built ISO from scratch. |
| Real hardware (later) | Check GPU rendering and performance. |

## Repo layout

```
docs/                  plan, decisions, how-to notes
branding/              logo, colors and artwork, and the system files that
                       replace Ubuntu's branding (patrone-branding)
default-apps/          default browser and favorites (patrone-default-apps)
compositor/            the Patrone compositor
shell/                 panel, launcher, dock, notifications
settings/              the settings app
terminal/              Patrone Terminal
shell-config/          bash prompt and zsh setup (patrone-shell-config)
session/               session entry, systemd user units, defaults
packaging/             Debian packaging for each component
iso/                   ISO remaster scripts
scripts/               dev environment setup and helpers
```

## Milestones

1. Dev VM installed, reachable by SSH, toolchain and Claude Code in place, repo
   cloned. (Done.)
2. A minimal compositor running as a window inside GNOME: a background, a
   terminal that opens on launch, and windows you can move, resize and close.
   (Done 2026-10-25; input handling still to be tried by hand.)
3. The compositor runs as a real session chosen at the login screen.
4. A panel and app launcher, and a notification daemon.
5. Lock screen and a first settings app (displays, input, network, sound).
6. First Patrone ISO with Patrone Desktop as the default session.
