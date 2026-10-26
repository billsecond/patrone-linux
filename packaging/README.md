# Packaging

Debian packaging for each Patrone component, kept apart from the component's
source. `build.sh` puts the two together and builds the `.deb` on the dev VM:

```
./packaging/build.sh patrone-terminal -d
./packaging/build.sh patrone-shell-config
./packaging/build.sh patrone-branding
./packaging/build.sh patrone-default-apps
./packaging/build.sh patrone-remote-desktop
./packaging/build.sh patrone-gnome-taskbar
```

The packages land in `out/`. Install them with
`sudo apt install ./out/patrone-terminal_0.1.1_amd64.deb` (apt pulls in the
dependencies).

| Package | Source | What it installs |
| --- | --- | --- |
| `patrone-terminal` | `terminal/` | The terminal, its launcher entry and icon, and an autostart entry that opens it at every login. Registers the `x-terminal-emulator` alternative (priority 60) and is the terminal `xdg-terminal-exec` picks in a Patrone Desktop session. |
| `patrone-shell-config` | `shell-config/` | Patrone's bash prompt (`/usr/share/patrone/bashrc`, loaded from `/etc/skel/.bash_aliases`), and `/etc/skel/.zshrc` and `.zprofile` for users who choose zsh. Bash stays the default shell. |
| `patrone-branding` | `branding/` | Patrone's name and logos in place of Ubuntu's: `os-release`, the boot splash (graphical and text), the boot menu name, the login screen logo, Settings → About, `/etc/issue` and the login message. Also the wallpaper, the dock turned off, and fastfetch's config and logo. See `branding/README.md`. |
| `patrone-default-apps` | `default-apps/` | Chromium as the default browser (`/etc/xdg/mimeapps.list`) and in the favorites in place of Firefox. Depends on `chromium-browser`, which installs the Chromium snap. See `default-apps/README.md` for removing Firefox. |
| `patrone-remote-desktop` | `remote-desktop/` | Remote Desktop where each account in the `remote-desktop` group signs in with its own Linux password, checked by PAM: FreeRDP's proxy with Patrone's sign-in plugin, a root sign-in helper, and the `patrone-remote-desktop` command. Off until `patrone-remote-desktop enable`. Builds with `freerdp3-dev` and `libpam0g-dev`. See `remote-desktop/README.md`. |
| `patrone-gnome-taskbar` | `shell/gnome/` | Patrone's taskbar and Start menu in place of GNOME's top bar: a GNOME Shell extension in `/usr/share/gnome-shell/extensions/`, its settings schema, and an override that turns it on for everyone. Loads at the next sign-in. See `shell/gnome/README.md`. |

Notes:

- Both are native packages (`3.0 (native)`): Patrone is its own upstream.
- Rust packages build with Ubuntu's `cargo` and `rustc` (1.93), so the crates
  declare `rust-version = "1.93"`. `build.sh` vendors the crates so the build
  needs no network. On the dev VM Rust comes from rustup, so pass `-d` to skip
  the Build-Depends check, or install Ubuntu's `cargo` and `rustc`.
- `patrone-shell-config` touches no other package's files: Ubuntu's
  `/etc/skel/.bashrc` already loads `~/.bash_aliases`, which no package owns,
  so that's where the prompt hooks in.
- `patrone-branding` has to replace files that belong to Ubuntu's packages
  (`/usr/lib/os-release`, `/etc/issue` and others in `base-files`). Its
  `preinst` moves each aside with `dpkg-divert` (to `*.ubuntu`), so Ubuntu's
  updates land there instead of over Patrone's, and `postrm` moves them back
  when the package is removed. Three of Ubuntu's login-message scripts (help
  links, news, Ubuntu Pro) are moved aside with no replacement. The boot
  splash is chosen through the `default.plymouth` and `text.plymouth`
  alternatives (priority 200), and installing or removing the package
  rebuilds the initramfs and the GRUB menu.
- Ubuntu sets some GNOME defaults twice: plainly and again for its own
  session (`[schema:ubuntu]` in `10_ubuntu-settings.gschema.override`).
  The session-specific value beats any plain default, so Patrone's overrides
  set both.
- Later these builds move into a clean `sbuild` chroot, and the packages into
  the private `reprepro` repository (see `docs/PLAN.md`).
