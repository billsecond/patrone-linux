# Packaging

Debian packaging for each Patrone component, kept apart from the component's
source. `build.sh` puts the two together and builds the `.deb` on the dev VM:

```
./packaging/build.sh patrone-terminal -d
./packaging/build.sh patrone-shell-config
./packaging/build.sh patrone-branding
```

The packages land in `out/`. Install them with
`sudo apt install ./out/patrone-terminal_0.1.1_amd64.deb` (apt pulls in the
dependencies).

| Package | Source | What it installs |
| --- | --- | --- |
| `patrone-terminal` | `terminal/` | The terminal, its launcher entry and icon, and an autostart entry that opens it at every login. Registers the `x-terminal-emulator` alternative (priority 60) and is the terminal `xdg-terminal-exec` picks in a Patrone Desktop session. |
| `patrone-shell-config` | `shell-config/` | Patrone's bash prompt (`/usr/share/patrone/bashrc`, loaded from `/etc/skel/.bash_aliases`), and `/etc/skel/.zshrc` and `.zprofile` for users who choose zsh. Bash stays the default shell. |
| `patrone-branding` | `branding/` | Patrone's name and logos in place of Ubuntu's: `os-release`, the boot splash (graphical and text), the boot menu name, the login screen logo, Settings → About, `/etc/issue` and the login message. Also the wallpaper, the dock turned off, and fastfetch's config and logo. See `branding/README.md`. |

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
- Later these builds move into a clean `sbuild` chroot, and the packages into
  the private `reprepro` repository (see `docs/PLAN.md`).
