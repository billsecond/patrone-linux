# Next steps

Last updated 2026-10-25, at the end of the second Patrone Terminal and Hyper-V
session.

## Where things stand

Done:

- `PatroneDev` VM set up by `scripts/bootstrap-dev-vm.sh` (packaging, ISO and
  QEMU tools, Wayland and GTK4 libraries, Patrone Terminal's libraries, fonts
  and zsh plugins, Hyper-V tools). Rust 1.99.1 (stable) and 1.93 (Ubuntu's
  rustc, for package builds) through rustup. Claude Code, `gh` and `git`
  signed in.
- Stack decided: Rust + Smithay compositor, GTK4 shell (see `docs/PLAN.md`).
  Default apps decided, including **Chromium in place of Firefox**.
- **Patrone Terminal** (`terminal/`, see its README): copy on select, paste on
  right-click, Ctrl+click links, tabs, close confirmation, Preferences,
  `--standalone`, placeholder icon. Installed for the user with
  `install-dev.sh --zshrc --default`, which also made it the terminal of
  Ctrl+Alt+T, the dock and the app grid in place of Ptyxis.
- **zsh setup** (`zsh/zshrc`, `zsh/zprofile`): Kali-style prompt, plugins,
  directory reporting. Installed as the user's `~/.zshrc`, but their login
  shell is still bash (see below).
- **Packages** (`packaging/`): `patrone-terminal` and `patrone-shell-config`
  build on the VM. Not installed anywhere yet.
- **Remote Desktop:** Remote Desktop Connection works, at full 4K with Remote
  Login. The Hyper-V daemons are running.
- **fastfetch** config in the Patrone colors (`branding/fastfetch/`), with a
  placeholder logo.

Waiting on the user:

- **Switch the login shell to zsh:** `chsh -s /usr/bin/zsh` (asks for their
  password), then sign out and in. Until then Patrone Terminal shows Ubuntu's
  bash prompt, which the user took for "the old terminal".
- **Confirm in the RDP session** that resizing the `mstsc` window resizes the
  desktop, and that copy and paste work both ways. Set 200% scale in Settings
  → Displays if text is too small.
- **ASCII logo file.** The user shared an ASCII-art version of the logo as an
  image in chat, but it was never saved to disk. Ask for the file (for example
  in `C:\Users\wdaugherty\Downloads`), add it under `branding/concepts/`, and
  redraw it as text in `branding/fastfetch/logo.txt`.
- **Chromium on the dev VM** if wanted now: `sudo snap install chromium`, then
  swap it for Firefox in the dock and run
  `xdg-settings set default-web-browser chromium_chromium.desktop`.
- **A license for Patrone.** None is chosen; the Debian `copyright` files say
  "all rights reserved" until one is. It matters before anything is shared,
  and for which code the compositor may borrow (MIT vs GPL).

## 1. Patrone Terminal

Verified by hand (through GTK's Broadway backend, see the terminal README):
copy on select, right-click paste, link detection on hover, the close
confirmation (and that confirming stops the program), Preferences saving and
applying, tabs, closing a tab. Not exercised: Ctrl+click actually opening a
browser (it would have opened one on the user's screen), keyboard shortcuts
(Broadway doesn't receive letter keys from browser automation).

Next:

- Ask the user how it looks and feels in the RDP session, especially the
  palette in `terminal/src/theme.rs` and the font size at 4K.
- A real icon once the Patrone mark has a vector master.
- Install the packages for real (with sudo) to check the alternatives and
  `/etc/skel` behaviour on a live system, then move package builds into
  `sbuild`.

Findings:

- `vte4` 0.10 pairs with `gtk4` 0.11; the crate features are `v0_84` and
  `gnome_50`. Everything builds with Rust 1.93, so `rust-version = "1.93"`.
- Ubuntu's Yaru theme fails to load in plain (non-libadwaita) GTK4 apps on the
  VM: "resource /com/ubuntu/themes/Yaru/4.0/gtk.css does not exist". The
  terminal uses GTK's built-in `Default` theme, dark. The shell and settings
  apps will hit the same thing.
- GTK 4.22 draws with Vulkan (Mesa lavapipe) in the VM. When the console
  session ended under an open test window, GTK logged 500 MB of "surface
  lost" warnings in 30 seconds before exiting. Send test windows' output to
  `/dev/null`.
- Windows opened over SSH don't take keyboard focus, so they don't interrupt
  the user. The user's desktop is now the Remote Login session; the console
  shows the login screen, so `scripts/hyperv/vmshot.ps1` no longer shows it.
- `xdg-terminal-exec` caches its choice in `~/.cache/xdg-terminal-exec`;
  delete it after changing `xdg-terminals.list`.
- `dh_clean` deletes `*.orig` files, including each vendored crate's
  `Cargo.toml.orig` that cargo checksums: `dh_clean -XCargo.toml.orig`.
- Don't run `pkill -f <pattern>` over SSH: the pattern also matches the SSH
  command's own shell, which kills the connection. Anchor it (`^/path`) or
  kill by PID.

## 2. Milestone 2: minimal compositor

In progress on branch `compositor-m2` (`compositor/`). See the section added
when it merges.

## 3. Hyper-V guest support

`scripts/hyperv/README.md` has the user-facing steps.

Findings:

- **Working:** Remote Desktop Connection to GNOME Remote Desktop, set up with
  `scripts/hyperv/guest/setup-remote-desktop.sh`:
  - `sharing` (Desktop Sharing): the console session, fixed size. The session
    negotiates the clipboard channel, audio and H.264 graphics.
  - `login` (Remote Login): a separate session on a virtual monitor the size
    of the client, 3840×2160 at 100% scale on the user's 4K monitors. Signing
    in as the user who is logged in on the console **ends the console
    session**, so anything running there (like `claude remote-control`) has
    to be restarted in the remote session.
  - Each mode has its own RDP credentials, and `mstsc` remembers credentials
    per address: after switching modes it offered the old ones ("Could not
    find user in SAM database"). **More choices → Use a different account**.
  - The first attempt of each connection fails with "connection reset by
    peer" during NLA: `mstsc` drops it while asking to trust the self-signed
    certificate, then reconnects.
- GNOME 50 is **Wayland-only**, so Microsoft's xrdp recipe can't work.
- **GNOME Remote Desktop** 50.2 (from its source): NLA only; resizes only on
  virtual monitors (Remote Login, or Desktop Sharing's "extend" mode); no
  vsock.
- **Hyper-V daemons** were inactive because `linux-cloud-tools` was installed
  after boot, after udev had created the devices their units wait for. The
  bootstrap script re-triggers udev. `hv-fcopy-daemon` stays off: its unit
  waits for `/dev/vmbus/hv_fcopy`, which the 7.0 kernel's UIO-based fcopy
  doesn't create (it's only for `Copy-VMFile`).
- **Host:** `EnableEnhancedSessionMode` is True, but `PatroneDev` uses the
  `VMBus` transport. Linux guests need `HvSocket`.
- `freerdp-proxy` (FreeRDP 3.32) runs headless and could sit between VMConnect
  and GNOME Remote Desktop (TLS in, NLA out).

Plan:

1. **Dev VM:** `mstsc` with Remote Login covers clipboard and resizing. The
   VMConnect enhanced session is optional for the dev VM. To try it: run
   `scripts/hyperv/guest/setup-enhanced-session.sh`, shut the VM down, run
   `Set-VM -VMName PatroneDev -EnhancedSessionTransportType HvSocket`, start
   it, and connect with VMConnect. Its main value is learning whether
   VMConnect does NLA, which decides how Patrone's own RDP server must work.
2. **Patrone ISO:** ship `linux-cloud-tools-virtual` (its units only run on
   Hyper-V) and the RDP setup.
3. **Patrone Desktop:** GNOME Remote Desktop only works with Mutter, so
   Patrone needs its own RDP path: implement the screen-cast and
   remote-desktop D-Bus APIs GNOME Remote Desktop uses, or write
   `patrone-remote-desktop` on FreeRDP's server library, listening on vsock
   as well as TCP. Clipboard through RDP's clipboard channel, resizing
   through display control, and `hyperv_drm` resolution changes in the basic
   console.

## 4. Chromium instead of Firefox

Decided 2026-10-25 (see "Default apps" in `docs/PLAN.md`): Canonical's
Chromium snap, kept current by snapd (stable was 156, two days old, when
checked), customized with enterprise policies rather than forked or rebranded.

- The snap reads policies from `/etc/chromium-browser/policies` (its
  `etc-chromium-browser-policies` plug, seen in `meta/snap.yaml`). After
  installing, check `snap connections chromium` shows that plug connected.
- Write Patrone's policies in `/etc/chromium-browser/policies/managed/`:
  homepage, bookmarks, default search engine, `BrowserThemeColor` Patrone
  Blue. Package them (for example `patrone-chromium-defaults`).
- ISO: replace the Firefox snap with Chromium in the seed, the dock
  favourites and the default browser (`/etc/xdg/mimeapps.list`).

## 5. fastfetch logo

`branding/fastfetch/` has the system-wide config (fastfetch 2.57 reads
`/etc/xdg/fastfetch/config.jsonc`) and a placeholder logo. When the user's
ASCII logo arrives, redraw it as text in `logo.txt`, reuse it for
`/etc/issue`, and package both (for example in a `patrone-branding` package
with the wallpaper). With its own `/etc/os-release` (`ID=patrone`,
`ID_LIKE=ubuntu`) Patrone would otherwise get Ubuntu's logo.

## Suggested order

The user's quick steps first (zsh login shell, RDP checks, the logo file, a
license). Then merge and continue the compositor (milestone 2), then the
Chromium defaults package, then Patrone's own RDP path.
