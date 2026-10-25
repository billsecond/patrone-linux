# Next steps

Last updated 2026-10-25, at the end of the setup session.

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

Waiting on the user:

- **ASCII logo file.** The user shared an ASCII-art version of the logo as an
  image in chat, but it was never saved to disk. Ask for the file (for example
  in `C:\Users\wdaugherty\Downloads`) and add it under `branding/concepts/`.
  It also has to be redone as real text for `fastfetch` and `/etc/issue`.

## 1. Milestone 2: minimal compositor

In `compositor/` (Rust + Smithay), build a compositor that runs as a window
inside the VM's GNOME session (Smithay's winit backend). It should:

- draw a background (the Patrone wallpaper)
- launch a terminal on start
- let windows be moved, resized and closed

Smithay's `anvil` example and the niri compositor are good references. Both
are GPL-3, so copying code from them makes the compositor GPL-3. Rendering in
the VM is software-only (Mesa llvmpipe), since Hyper-V gives Linux guests no
GPU.

## 2. Patrone Terminal (new request)

The user wants a new default terminal that looks a lot like Kali Linux's, with:

- **Copy on select:** selecting text copies it to the clipboard.
- **Paste on right-click.**
- A dark, "terminal-ish" look: Kali-style two-line prompt, dark theme, a good
  monospace font.

Proposed design (confirm the look with the user as you go):

- **App:** `patrone-terminal` in Rust, using GTK4 and VTE through the `vte4`
  crate (system library `libvte-2.91-gtk4-dev` 0.84). VTE handles the terminal
  emulation. On top of it:
  - copy to the clipboard when the selection changes
  - paste on right-click instead of a context menu
  - Ctrl+Shift+C/V, Ctrl+Plus/Minus zoom, Ctrl+Shift+T/N for tabs and windows
  - 10,000+ lines of scrollback
- **Look:** a dark palette inspired by Kali's, using the Patrone blues from
  `branding/README.md`, with Fira Code (`fonts-firacode`) or JetBrains Mono
  (`fonts-jetbrains-mono`).
- **Shell:** zsh as the default shell for new users, with
  `zsh-syntax-highlighting` and `zsh-autosuggestions`, and a Kali-style
  two-line prompt:

  ```
  ┌──(user㉿host)-[~/src]
  └─$
  ```

  Blue for normal users, red for root. Write our own `.zshrc` rather than
  copying Kali's.
- **Packaging:** `patrone-terminal`, plus `patrone-shell-config` for zsh
  defaults in `/etc/skel`, registered as the `x-terminal-emulator` alternative.
  It can also be tried as the default terminal in the GNOME dev VM before
  Patrone Desktop exists.

All of those packages are confirmed available in Ubuntu 26.04 (`zsh` 5.9,
`zsh-syntax-highlighting` 0.8.0, `zsh-autosuggestions` 0.7.1,
`fonts-firacode` 6.2).

## 3. Hyper-V guest support (new request)

The user wants Patrone to work well as a Windows Hyper-V guest: clipboard
copy and paste, desktop resizing, and similar.

Findings so far, on the dev VM:

- GNOME 50 is **Wayland-only**: no X11 session exists. The usual Linux
  "enhanced session" recipe (xrdp + Xorg, Microsoft's linux-vm-tools) can't
  work.
- The kernel has `hv_sock`, `vsock`, `hv_utils`, `hyperv_drm` and `hid_hyperv`
  loaded. `hv_sock` is the transport Hyper-V's console uses for enhanced
  sessions.
- `hv-kvp-daemon`, `hv-vss-daemon` and `hv-fcopy-daemon` are enabled but were
  **inactive**. Find out why; they may need a reboot after
  `linux-cloud-tools-virtual` was installed.
- `gnome-remote-desktop` 50.2 is installed. `grdctl` has no option to listen
  on vsock, so a bridge is needed for Hyper-V's enhanced session, for example
  `socat VSOCK-LISTEN:3389,fork TCP:127.0.0.1:3389` (install `socat`).
- `xrdp` 0.10.1 and `socat` are available in the archive.

Plan:

1. **Dev VM now.**
   - Quickest way to get clipboard and resizing today: the user enables
     **Settings → System → Remote Desktop** (they set the RDP password
     themselves), then connects from Windows with Remote Desktop Connection to
     `DevDistro-PatroneLinux.local`.
   - Then try a real enhanced session in the Hyper-V console window: on the
     host, `Set-VM PatroneDev -EnhancedSessionTransportType HvSocket` (and
     check `Get-VMHost` shows `EnableEnhancedSessionMode` is True); in the
     guest, bridge vsock port 3389 to GNOME Remote Desktop.
2. **Patrone ISO.** Ship the Hyper-V daemons, enabled only on Hyper-V
   (`systemd-detect-virt` reports `microsoft`), plus the vsock bridge.
3. **Patrone Desktop.** GNOME Remote Desktop only works with GNOME's own
   compositor (Mutter), so Patrone needs its own RDP path. Either implement the
   screen-cast and remote-desktop D-Bus APIs that GNOME Remote Desktop expects,
   or write a `patrone-remote-desktop` server on FreeRDP's server library.
   - Clipboard: RDP's clipboard channel connected to Wayland's clipboard.
   - Resizing: RDP's display-control channel connected to the compositor's
     output size.
   - The compositor should also handle `hyperv_drm` resolution changes in the
     basic console.

## Suggested order

Start Patrone Terminal and the dev-VM Hyper-V convenience (step 3.1) first:
both give the user something visible quickly. Then the compositor
(milestone 2), then Hyper-V support inside Patrone Desktop.
