# Next steps

Last updated 2026-10-25, at the end of the Patrone Terminal and Hyper-V session.

## Where things stand

Done:

- `PatroneDev` VM installed with Ubuntu 26.04.1 and set up by
  `scripts/bootstrap-dev-vm.sh`: packaging and ISO tools, QEMU/KVM, the Wayland
  and GTK4 development libraries, and Hyper-V integration tools. The user is in
  the `kvm` group.
- Rust 1.99.1 installed on the VM with rustup (`~/.cargo/bin`).
- Claude Code installed and signed in on the VM. `gh` and `git` signed in as
  `billsecond`.
- Stack decided: Rust + Smithay compositor, GTK4 shell (see `docs/PLAN.md`).
- Branding: logo concept, measured palette, and wallpaper 01 with an 8K master,
  set as the VM's wallpaper (see `branding/README.md`).
- **Patrone Terminal, first version,** in `terminal/` (see its README). It
  builds, its unit tests pass, and it opens and renders correctly in the VM's
  GNOME session. Copy on select, paste on right-click, tabs and shortcuts are
  written but not yet tried by hand.
- **Patrone zshrc** in `zsh/zshrc`: Kali-style two-line prompt, written but not
  yet run (zsh wasn't installed).
- **Hyper-V:** root cause of the inactive daemons found and fixed in the
  bootstrap script; enhanced-session bridge written (`scripts/hyperv/guest/`).

Waiting on the user:

- **Remote Desktop resizing.** Remote Login works at full 4K (see section 2).
  Still to confirm with the user: that resizing the `mstsc` window resizes
  the session, and that copy and paste work both ways.
- **ASCII logo file.** The user shared an ASCII-art version of the logo as an
  image in chat, but it was never saved to disk. Ask for the file (for example
  in `C:\Users\wdaugherty\Downloads`) and add it under `branding/concepts/`.
  It also has to be redone as real text for `fastfetch` and `/etc/issue`.

## 1. Patrone Terminal: finish and package

- Have the user try copy on select, right-click paste, tabs and zoom, and
  confirm the look (Patrone Dark palette in `terminal/src/theme.rs`).
- After the bootstrap re-run: install with `./terminal/install-dev.sh --zshrc`,
  check the prompt, the plugins, and that new tabs open in the current
  directory (the zshrc reports it with OSC 7). `--default` makes it the
  terminal GNOME opens, through `~/.config/xdg-terminals.list`. Check whether
  Ctrl+Alt+T follows that or still opens Ptyxis.
- Ideas not done yet: Ctrl+click to open links, a preferences dialog, asking
  before closing a tab with a running program, the Patrone logo as the icon.
- Packaging: `patrone-terminal` (registers the `x-terminal-emulator`
  alternative; `-e` and `-T` already work) and `patrone-shell-config`
  (`zsh/zshrc` as `/etc/skel/.zshrc`, zsh as the default shell for new users).

Findings:

- The `vte4` 0.10 crate pairs with `gtk4` 0.11. Ubuntu 26.04 has VTE 0.84 and
  GTK 4.22, so the crate features are `v0_84` and `gnome_50`.
- Ubuntu's Yaru theme fails to load in plain (non-libadwaita) GTK4 apps on the
  VM: "resource /com/ubuntu/themes/Yaru/4.0/gtk.css does not exist". The
  terminal uses GTK's built-in `Default` theme, dark. The Patrone shell and
  settings apps will hit the same thing.
- Windows opened over SSH don't take keyboard focus in GNOME, so they don't
  interrupt the user. `scripts/hyperv/vmshot.ps1` shows the result.
- Don't run `pkill -f <pattern>` over SSH: the pattern also matches the SSH
  command's own shell, which kills the connection. Kill by PID.

## 2. Hyper-V guest support

The user wants clipboard copy and paste and desktop resizing when Patrone runs
as a Windows Hyper-V guest. `scripts/hyperv/README.md` has the user-facing
steps.

Findings:

- **Working:** Remote Desktop Connection from Windows to Desktop Sharing,
  set up with `scripts/hyperv/guest/setup-remote-desktop.sh sharing`. The
  session negotiates the clipboard channel, audio and H.264 graphics. The
  first attempt always fails with "connection reset by peer" during NLA,
  because `mstsc` drops it while asking the user to trust the self-signed
  certificate, then reconnects. Expect that in the GNOME Remote Desktop log.
- **Working:** Remote Login (`setup-remote-desktop.sh login`). The session
  gets a virtual monitor the size of the client (3840×2160 on the user's 4K
  monitors) at 100% scale; the user may want 200%. Signing in as the user who
  is logged in on the console **ends the console session** (seat0 went back
  to the login screen), so anything running there, like `claude
  remote-control`, has to be restarted in the remote session.
- Remote Login and Desktop Sharing have separate RDP credentials, and
  `mstsc` remembers credentials per address. After switching modes it offered
  the old ones, which failed with "Could not find user in SAM database".
  **More choices → Use a different account** fixes it.
- RDP first failed only because GNOME Remote Desktop had never been turned
  on: nothing listened on port 3389. The firewall (`ufw`) is disabled.
- GNOME 50 is **Wayland-only**, so the xrdp + Xorg recipe (Microsoft's
  linux-vm-tools) can't work.
- **The Hyper-V daemons were inactive** (now fixed and running) because
  `linux-cloud-tools` was installed after boot. Their units bind to udev-tagged devices
  (`/usr/lib/udev/rules.d/60-hv-*.rules`), and the devices had been created
  before the rules existed. The bootstrap script now re-triggers udev for
  them. `hv-fcopy-daemon` stays off: its unit waits for `/dev/vmbus/hv_fcopy`,
  which the 7.0 kernel's UIO-based fcopy doesn't create. It's only for
  `Copy-VMFile`, so it's low priority.
- **Host side:** `EnableEnhancedSessionMode` is already True, but `PatroneDev`
  uses the `VMBus` transport. Linux guests need
  `Set-VM -VMName PatroneDev -EnhancedSessionTransportType HvSocket`.
- **GNOME Remote Desktop** (50.2, checked against its source):
  - accepts **NLA only** (`RdpSecurity` and `TlsSecurity` off, `NlaSecurity`
    on);
  - resizes the remote screen only when it gives the client virtual monitors:
    **Remote Login** (headless) sessions, or Desktop Sharing in "extend" mode.
    Desktop Sharing normally mirrors the primary monitor at its fixed size.
  - has no vsock support.
- Nobody has published a working GNOME Remote Desktop + VMConnect setup. One
  forum attempt bridged vsock with socat and failed at sign-in ("connection
  reset by peer"); its bridge pointed at the Remote Login daemon, which
  redirects the client to a second connection, and VMConnect may not follow
  that.
- `freerdp-proxy` (FreeRDP 3.32) runs headless. Its client side can do NLA to
  GNOME Remote Desktop with a stored username and password, and its server
  side accepts TLS or standard RDP security without NLA.

Plan:

1. **Dev VM now.**
   - Desktop Sharing (clipboard, fixed size) and Remote Login (full
     resolution, separate session) both work over `mstsc`. Remote Login is
     the better daily driver; confirm resizing and clipboard in it.
   - Enhanced session: run `scripts/hyperv/guest/setup-enhanced-session.sh`,
     switch the VM to `HvSocket` (VM shut down), and connect with VMConnect.
     If it fails at sign-in, put `freerdp-proxy` between the bridge and GNOME
     Remote Desktop (TLS in, NLA out).
2. **Patrone ISO.** Ship the Hyper-V daemons, enabled only on Hyper-V
   (`systemd-detect-virt` reports `microsoft`), plus whichever bridge works.
3. **Patrone Desktop.** GNOME Remote Desktop only works with GNOME's own
   compositor (Mutter), so Patrone needs its own RDP path. Either implement the
   screen-cast and remote-desktop D-Bus APIs that GNOME Remote Desktop expects,
   or write a `patrone-remote-desktop` server on FreeRDP's server library.
   - Clipboard: RDP's clipboard channel connected to Wayland's clipboard.
   - Resizing: RDP's display-control channel connected to the compositor's
     output size.
   - The compositor should also handle `hyperv_drm` resolution changes in the
     basic console.

## 3. Milestone 2: minimal compositor

In `compositor/` (Rust + Smithay), build a compositor that runs as a window
inside the VM's GNOME session (Smithay's winit backend). It should:

- draw a background (the Patrone wallpaper)
- launch a terminal on start (Patrone Terminal)
- let windows be moved, resized and closed

Smithay's `anvil` example and the niri compositor are good references. Both
are GPL-3, so copying code from them makes the compositor GPL-3. Rendering in
the VM is software-only (Mesa llvmpipe), since Hyper-V gives Linux guests no
GPU.

## 4. fastfetch logo (when the ASCII logo exists)

The user wants `fastfetch` to show the Patrone logo. `fastfetch` is already
installed on Ubuntu 26.04 and currently shows Ubuntu's logo. Once the ASCII
logo exists as real text:

- ship it as a fastfetch logo file, with a system-wide config
  (`/etc/xdg/fastfetch/config.jsonc`) that points at it and uses the Patrone
  blues. Check that fastfetch reads that path.
- When Patrone gets its own `/etc/os-release` (`ID=patrone`,
  `ID_LIKE=ubuntu`), fastfetch would fall back to Ubuntu's logo without that
  config.
- Reuse the same text for `/etc/issue`.

## Suggested order

Get the user's bootstrap re-run and Remote Desktop settings done, then finish
testing Patrone Terminal and the zshrc. Then the compositor (milestone 2), then
Hyper-V support inside Patrone Desktop. The fastfetch logo waits on the ASCII
logo file.
