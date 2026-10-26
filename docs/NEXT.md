# Next steps

Last updated 2026-10-26, after the branding, one-sign-in RDP and terminal
autostart work.

## Where things stand

Done:

- `PatroneDev` VM set up by `scripts/bootstrap-dev-vm.sh` (packaging, ISO and
  QEMU tools, Wayland and GTK4 libraries, Patrone Terminal's libraries, fonts
  and zsh plugins, Hyper-V tools). Rust 1.99.1 (stable) and 1.93 (Ubuntu's
  rustc, for package builds) through rustup. Claude Code, `gh` and `git`
  signed in.
- Stack decided: Rust + Smithay compositor, GTK4 shell (see `docs/PLAN.md`).
  Default apps decided, including **Chromium in place of Firefox**. License:
  **GPL-3.0-or-later** (`LICENSE`), set in the crates and Debian packaging;
  the compositor keeps Smithay's MIT notice in `compositor/NOTICE`.
- **Milestone 2: the compositor** (`compositor/`) runs nested in GNOME with
  the wallpaper and a terminal inside it (section 2).
- **Patrone Terminal** (`terminal/`, see its README): copy on select, paste on
  right-click, Ctrl+click links, tabs, close confirmation, Preferences,
  `--standalone`, placeholder icon. Installed for the user with
  `install-dev.sh --zshrc --bashrc --default`, which also made it the terminal of
  Ctrl+Alt+T, the dock and the app grid in place of Ptyxis.
- **Shell setup** (`shell-config/`): the user wants **bash as the default,
  with the Kali-style prompt** (2026-10-26). `shell-config/bashrc` is that
  prompt, loaded from `~/.bash_aliases`; installed for the user
  (`install-dev.sh --bashrc`) and checked in an interactive bash in a pty.
  `shell-config/zshrc` and `zprofile` remain for anyone who picks zsh.
- **Packages** (`packaging/`): `patrone-terminal` (0.1.1, now with a login
  autostart entry), `patrone-shell-config`, `patrone-branding` and
  `patrone-default-apps` (Chromium in place of Firefox) build on the VM. Not
  installed anywhere yet.
- **Branding** (section 6): `patrone-branding` replaces Ubuntu's name and
  logos (boot splash, boot menu, login screen, About, console banner, login
  message), restyles the login screen, makes blue the accent color and turns
  Ubuntu's dock off. Built and inspected; not installed yet.
- **Terminal at login:** GNOME opens Patrone Terminal at every login, for the
  user now (`install-dev.sh --autostart`) and for everyone once
  `patrone-terminal` is installed. The Patrone Desktop session's compositor
  already opens one.
- **No screen lock over RDP:** done for the user, and applied live to their
  Remote Login session (section 3).
- **Remote Desktop:** Remote Desktop Connection works, at full 4K with Remote
  Login. The Hyper-V daemons are running.
- **fastfetch** shows the Patrone logo for the user (`branding/fastfetch/`,
  installed with `branding/fastfetch/install-dev.sh`): the mark traced from
  the concept by `branding/tools/make_terminal_logo.py`, with a gradient in
  the Patrone blues. Checked with plain `fastfetch` in Patrone Terminal. (At
  first it was only previewed with `-c`/`--logo`, so the user's plain
  `fastfetch` still showed Ubuntu's logo and looked like a false claim. Say
  exactly how something was checked.)

Waiting on the user:

- **Install the packages, remove Firefox and reboot** (needs their sudo
  password), in `~/src/patrone-linux`:
  `sudo apt install ./out/patrone-branding_0.1.0_all.deb ./out/patrone-terminal_0.1.1_amd64.deb ./out/patrone-default-apps_0.1.0_all.deb`,
  `sudo snap remove firefox`, `sudo apt-mark hold firefox`, `sudo reboot`.
  Then reset their own favorites, which still name Firefox, to Patrone's
  (`gsettings reset org.gnome.shell favorite-apps`, no sudo), and check
  (sections 4 and 6).
- **One-sign-in RDP:** from SSH or the VM's console, not from inside RDP,
  log out of the Remote Login desktop and run
  `./scripts/hyperv/guest/setup-remote-desktop.sh headless`, entering their
  Linux username and password when asked. Then connect with `mstsc` (More
  choices → Use a different account if it offers old credentials).
- **Login screen style:** the user asked for an "enterprise and/or school"
  look. It now keeps the list of users, restyled. Ask whether they'd rather
  have the username-and-password form schools and companies use
  (`disable-user-list=true` in `system/gdm/95-patrone-settings`), and
  whether to add a banner message.
- **Switch the login shell back to bash:** on 2026-10-26 the user ran
  `chsh -s /usr/bin/zsh` on my earlier advice, then decided on bash. They run
  `chsh -s /bin/bash` (asks for their password).
- **Describe the desktop design** (panel, start menu, desktop icons, windows,
  system area, look and feel, login and lock screens) before milestone 4;
  then mockups, then `docs/DESIGN.md`.
- **Confirm in the RDP session** that resizing the `mstsc` window resizes the
  desktop, and that copy and paste work both ways. Set 200% scale in Settings
  → Displays if text is too small.
- **ASCII logo file (optional now).** The user shared an ASCII-art version of
  the logo as an image in chat, never saved to disk. The fastfetch logo is now
  traced from the concept instead; their file would only matter for
  `/etc/issue` or if they prefer that look.
- **Try the compositor's input handling** (section 2).

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

## 2. Compositor: milestone 2 done, next milestone 3

`compositor/` (see its README) runs nested in GNOME through Smithay 0.7's
winit backend, with OpenGL ES on Mesa llvmpipe. It draws the wallpaper,
starts `patrone-terminal --standalone` inside itself (falling back to
`ptyxis --standalone`, then `foot`), and handles xdg-shell windows: client
move and resize, Alt/Super+drag, Super+Q, click to focus, popups. Based on
Smithay's `smallvil` (MIT, notice kept in `compositor/NOTICE`). Smithay and
its `anvil` example are MIT; niri is GPL-3, which Patrone's GPL-3.0-or-later
license now lets us borrow from.

Verified: it builds, 13 tests, Clippy and rustfmt pass; its `--screenshot`
frames show the wallpaper, a centered Patrone Terminal, two windows
cascading, focus passing on when a window closes, and the `#030303`
fallback. Not verified (needs input in the user's session): moving,
resizing, the shortcuts, click to focus, menus, clipboard, cursors. Ask the
user to try `cargo run --manifest-path compositor/Cargo.toml -- --modifier alt`
in the RDP session.

Findings:

- EGL works for nested clients in the Remote Login session (llvmpipe, GLES
  3.2); the "failed to get driver name / ZINK" lines are only Mesa's GPU probe.
- GNOME stops sending frame callbacks while its screen is locked, which froze
  `eglSwapBuffers` at Mesa's default swap interval. The compositor turns
  vsync off and times its own frames (60 a second at most, only on change).
- GNOME keeps Super+drag for itself (`mouse-button-modifier`), so nested runs
  use `--modifier alt`.
- Reading back the window's framebuffer broke the next swap, so screenshots
  draw the frame again off screen.
- Ctrl+C or SIGTERM leaves the `wayland-1` socket file behind; the next run
  reuses it.

**Maximize and fullscreen** (`src/maximize.rs`) and **xdg-decoration**
(apps are always told to draw their own title bars) are done. Verified with
`tools/window-states.py`, a GTK window that requests each state on a timer
and prints what it gets: maximize and fullscreen give exactly the output size
(1600×900), restoring returns 640×400, leaving fullscreen returns to
maximized when it was, and a window that opens maximized is placed at the
top left and, once restored, centered to the pixel (measured from a
screenshot). `WAYLAND_DEBUG` showed `zxdg_decoration_manager_v1` offered and
only two window capabilities (maximize, fullscreen); GTK then greys out its
minimize button. Not verified (needs input): Super+Up and Super+F11, and
dragging a maximized window off its maximized state.

**Milestone 3 runs on the real screen** (2026-10-26, on the Hyper-V console
from tty3): `src/udev.rs` is a
DRM/KMS + GBM + libinput backend with a libseat session (based on anvil, cut
down to one GPU and one screen), chosen automatically when there's no
`WAYLAND_DISPLAY`/`DISPLAY`. It accepts software rendering (anvil refuses
it): on Hyper-V, Mesa's `kms_swrast` and `dri_gbm` are installed, which is
how Mutter draws on `hyperv_drm` (`/dev/dri/card1`, no render node). It
draws the cursor from the xcursor theme, switches virtual terminals with
Ctrl+Alt+F1–F12, quits with Super+Shift+E, takes relative and absolute
pointer motion, and picks the largest mode on Hyper-V (whose "preferred" mode
is the 1024×768 boot mode). `data/` and `install-dev-session.sh` add a
"Patrone" login-screen session. Verified: it builds (debug and release), 21
tests pass (mode choice, VT keys, quit shortcut, CLI), Clippy and rustfmt are
clean, nested mode still behaves the same (window-states sequence and a
screenshot), and over SSH it finds `/dev/dri/card1` and fails with a clear
message, since only the session on the screen may open it.

**Console test results.** The user logged in on tty3 in VMConnect (Ctrl+Alt+F3
can be sent with `scripts/hyperv/vminput.ps1 -Combo Ctrl+Alt+F3`) and ran
`~/src/patrone-linux/compositor/target/release/patrone-compositor 2> ~/patrone.log`.
It took the screen at 1920×1080 ("using /dev/dri/card1 (hyperv_drm), drawing
on the CPU"), drew the wallpaper, Patrone Terminal and the pointer, and a
click (sent with `vminput.ps1 -Click`) focused the terminal, which then took
keyboard input; `vmshot.ps1` showed each change. Findings:

- The first run froze after its first frames: no vblank arrived and the
  compositor waited forever. A 50 ms vblank timeout now keeps it going; on
  the second run the display did report vblank ("the display sends vblank
  events") and no timeout was logged, so the cause isn't fully pinned down.
  Watch the log for "no vblank" warnings.
- Harmless warnings on hyperv_drm: "Unable to become drm master" (logind
  already made the fd master), "failed to create signaled syncobj" (no sync
  objects), "Failed to destroy old mode property blob".
- Stopping it with SIGTERM from SSH hands the screen back cleanly; the
  console returns to its text prompt.
- Started from a terminal inside the RDP desktop, it runs nested instead, as
  designed: the log's first line says which.
- Moving a window by its title bar works (the user did it). It left old
  pointer images on screen: Hyper-V's driver copies each frame to its VRAM
  without waiting for Mesa to finish drawing it (dumb buffers have no
  fences). The compositor now waits on the frame's sync point before showing
  it, as anvil does with `renderer_sync`. Not retested on screen yet.
- Patrone Terminal looked unfocused (no text cursor) until clicked, though
  new windows are meant to get focus. Check whether that's real.

Not tried yet: dragging and resizing windows, the shortcuts, and switching
virtual terminals while it runs. Then: the login-screen session
(`install-dev-session.sh`, needs sudo; a login there as the same user ends
the Remote Login session), primary selection, and a keyboard layout from the
system settings.

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
- **One sign-in** (the user's request, 2026-10-26: `mstsc` asks for the
  credentials and they open the desktop). Remote Login can't do it: its NLA
  credentials are GNOME Remote Desktop's own, then GDM asks again. The
  `headless` mode of `setup-remote-desktop.sh` does: GDM 50's
  `gnome-headless-session@USER.service` runs the user's desktop from boot
  with no monitor, and `gnome-remote-desktop-headless.service` (a user unit,
  `WantedBy=gnome-session.target`, conflicts with Desktop Sharing's) serves
  it with `grdctl --headless` settings. The RDP credentials are a separate
  copy the user sets equal to their Linux account's; GNOME Remote Desktop 50
  can only check its own copy or Kerberos (`set-auth-methods`), not PAM. With
  no TPM in the VM (`/dev/tpm*` absent) it keeps them in
  `credentials.ini`. Written and syntax-checked, and its "other desktop
  sessions" check finds the user's Remote Login session (425) and ignores
  the tty3 text session. Not run yet: it needs the user's sudo password and
  their credentials, and it ends the RDP connection they're using.
- **Screen lock over RDP:** the user doesn't expect an RDP session to lock.
  `scripts/hyperv/guest/rdp-no-lock.sh` runs at login and, in a session with
  no seat (Remote Login and headless sessions have none; `Remote=yes` for
  Remote Login), sets `lock-enabled=false` and `idle-delay=0`; on the console
  it resets them. Installed for the user and run once against their Remote
  Login session: both settings read back as `false` and `0`.

Plan:

1. **Dev VM:** `mstsc` with Remote Login gives clipboard and full resolution.
   Resizing with the window is expected but unconfirmed: the session was
   still 3840×2160 with no resize logged when last checked. The VMConnect
   enhanced session is optional for the dev VM. To try it: run
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

Done 2026-10-26 (the user asked to replace Firefox): `patrone-default-apps`
(`default-apps/`) depends on Ubuntu's `chromium-browser` stub, which
installs the snap, makes Chromium the default browser through
`/etc/xdg/mimeapps.list`, and puts it in the favorites in place of Firefox.
Checked: it builds; with a stand-in `chromium_chromium.desktop`,
`gio mime` picks Chromium for `https` and `text/html`; the favorites
override gives the new list in both the `ubuntu` session and plain GNOME.
Not checked until the user installs it: the snap installing, links opening in
Chromium. Findings:

- Ubuntu's `firefox` package is a stub that installs the snap, and
  `ubuntu-desktop-minimal` depends on it, so the package can't simply
  conflict with Firefox. Its `preinst` runs `snap install firefox` on every
  install **and upgrade**, so after `snap remove firefox` the stub must be
  held (`apt-mark hold firefox`). Patrone's ISO needs its own desktop
  metapackage in place of `ubuntu-desktop-minimal`.
- Ubuntu's `ubuntu-mimeapps.list` and `gnome-mimeapps.list` in
  `/usr/share/applications` name Firefox first. A list in `/etc/xdg` is read
  before them (the session's `XDG_CONFIG_DIRS` is
  `/etc/xdg/xdg-ubuntu:/etc/xdg`), and a user's
  `~/.config/mimeapps.list` before that.

Still to do:

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
`/etc/xdg/fastfetch/config.jsonc`) and the traced logo; `patrone-branding`
installs both. With Patrone's own `os-release` (`ID=patrone`), fastfetch
would otherwise fall back to Ubuntu's logo through `ID_LIKE`.

## 6. Branding (patrone-branding)

The user asked (2026-10-26) to remove the Ubuntu logo everywhere, then for a
login screen and avatar that don't look like Ubuntu, "more enterprise and/or
school". `branding/README.md` has the table of what replaces what. What was
checked: the package builds; its file list, conffiles and maintainer scripts
were inspected; the settings override compiles with `glib-compile-schemas
--strict` against Ubuntu's schemas and gives the new defaults (with no user
settings: the Patrone wallpaper, the dock in `disabled-extensions`); the
login theme has all 11 of Yaru's resources plus Patrone's rules at the end of
`gdm.css`; `agetty --show-issue` renders the console banner as
"Patrone Linux 0.1" over "DevDistro-PatroneLinux (10.0.0.61) on tty1". Not
checked until the user installs it and reboots: the boot splash, the boot
menu name, the login screen's look, Settings → About, the login message, and
that the dock goes away.

After the reboot: `vmshot.ps1` on the login screen (before anyone logs in on
the console); `cat /etc/os-release`; `update-alternatives --query
default.plymouth`, `text.plymouth` and `gdm-theme.gresource`; `grep -i
patrone /boot/grub/grub.cfg`; an SSH login for the message; the RDP desktop
for the dock, the accent and the terminal opening by itself.

Findings:

- Ubuntu sets the wallpaper, lock-screen picture and favorites again for its
  own session (`[org.gnome.desktop.background:ubuntu]` and so on), and the
  user's GNOME session runs with `XDG_CURRENT_DESKTOP=ubuntu:GNOME`, so a
  plain default loses there. Patrone's overrides set both; checked with
  `gsettings get` under both desktop names against a scratch copy of the
  compiled schemas.
- Ubuntu's login screen looked like Ubuntu through four things: its logo,
  the orange accent (`accent-color='orange'` from
  `10_ubuntu-settings.gschema.override`, used 211 times in Yaru's login
  CSS), Yaru's warm grey background (`#lockDialogGroup`, `#222222`) and the
  Ubuntu Sans typeface.
- Plymouth's initramfs hook copies the theme folder with `cp -a`, keeping
  symlinks as symlinks, and copies another folder only if `ImageDir` names
  it. Links from the Patrone theme to the spinner's images would have
  dangled at boot, so the package copies them.
- GDM's greeter (`gnome-session-service --session=gnome-login`) doesn't run
  `/etc/xdg/autostart` entries; the user's GNOME session does (it ran
  `evolution-alarm-notify` and `snap userd`, the greeter didn't). No systemd
  `app-*@autostart` units exist: GNOME 50's session starts them itself.
- `desktop-file-validate` rejects `NotShowIn=Patrone;`: unregistered desktop
  names need an `X-` prefix. `XDG_CURRENT_DESKTOP=Patrone` works anyway, but
  matters if Patrone's session ever reads autostart entries.
- Non-interactive SSH commands run in the user's login shell, still zsh,
  which doesn't load `~/.cargo/env`: prefix `PATH=$HOME/.cargo/bin:$PATH` for
  builds over SSH.
- The dock is turned off through `disabled-extensions`, whose description
  (`gsettings describe org.gnome.shell disabled-extensions`) says it applies
  "even if loaded as part of the current mode" (the `ubuntu` mode loads the
  dock). Confirm after the reboot. A session that's
  already running keeps its dock until the next login.

Later: the desktop's own typeface (still Ubuntu Sans; part of the design
discussion), the lock screen inside a GNOME session (Yaru's, with the new
blue accent), and a Patrone-made avatar set for Settings → Users.

## Suggested order

The user's quick steps first (bash login shell, RDP checks, trying the
compositor, the desktop design). Then the compositor's next features and
milestone 3, the Chromium defaults package, then Patrone's own RDP path.
