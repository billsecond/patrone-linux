# Packaging

Debian packaging for each Patrone component, kept apart from the component's
source. `build.sh` puts the two together and builds the `.deb` on the dev VM:

```
./packaging/build.sh patrone-terminal -d
./packaging/build.sh patrone-shell-config
```

The packages land in `out/`. Install them with
`sudo apt install ./out/patrone-terminal_0.1.0_amd64.deb` (apt pulls in the
dependencies).

| Package | Source | What it installs |
| --- | --- | --- |
| `patrone-terminal` | `terminal/` | The terminal, its launcher entry and icon. Registers the `x-terminal-emulator` alternative (priority 60) and is the terminal `xdg-terminal-exec` picks in a Patrone Desktop session. |
| `patrone-shell-config` | `zsh/` | `/etc/skel/.zshrc` and `.zprofile`, and zsh as the login shell for users created afterwards. |

Notes:

- Both are native packages (`3.0 (native)`): Patrone is its own upstream.
- Rust packages build with Ubuntu's `cargo` and `rustc` (1.93), so the crates
  declare `rust-version = "1.93"`. `build.sh` vendors the crates so the build
  needs no network. On the dev VM Rust comes from rustup, so pass `-d` to skip
  the Build-Depends check, or install Ubuntu's `cargo` and `rustc`.
- `patrone-shell-config` changes defaults in `/etc/adduser.conf` and
  `/etc/default/useradd`, which belong to other packages. It adds its lines
  after a marker and removes them again when it's removed. Because those files
  are then modified, upgrades of `adduser` or `passwd` that change them will
  ask what to do.
- Later these builds move into a clean `sbuild` chroot, and the packages into
  the private `reprepro` repository (see `docs/PLAN.md`).
