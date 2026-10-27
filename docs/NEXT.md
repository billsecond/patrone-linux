# Next steps

Last updated 2026-10-26, after the branding, Chromium, terminal autostart and
Patrone Remote Desktop work.

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
- **Packages** (`packaging/`): `patrone-terminal` (0.1.1, with a login
  autostart entry), `patrone-shell-config`, `patrone-branding`,
  `patrone-default-apps` (Chromium in place of Firefox) and
  `patrone-remote-desktop` build on the VM. Everything the user asks for goes
  into these as Patrone's system-wide defaults (they said so, 2026-10-26);
  `scripts/hyperv/` is only dev-VM tooling.
- **Installed on the VM** (2026-10-26, by the user): `patrone-branding`
  0.1.0, `patrone-terminal` 0.1.1, `patrone-default-apps` 0.1.0 with the
  Chromium 156 snap; Firefox's snap removed and its stub held. Checked:
  `default.plymouth` and `gdm-theme.gresource` point at Patrone's,
  `/etc/os-release` says Patrone Linux. Not rebooted since, so the login
  screen and the user's session still showed Ubuntu's look.
- **Branding** (section 6): `patrone-branding` replaces Ubuntu's name and
  logos (boot splash, boot menu, login screen, About, console banner, login
  message), restyles the login screen, makes blue the accent color (and, in
  0.1.1, Yaru's blue GTK theme and icons) and turns Ubuntu's dock off.
- **Terminal at login:** GNOME opens Patrone Terminal at every login, for
  everyone (`patrone-terminal`'s `/etc/xdg/autostart` entry). The Patrone
  Desktop session's compositor already opens one.
- **Patrone Remote Desktop** (section 3, `remote-desktop/README.md`): each
  account in the `remote-desktop` group signs in once, with its own Linux
  password checked by PAM; nothing stores the password. Built, unit-tested,
  not installed or tried with a real client yet.
- **Patrone taskbar** (section 7, `shell/gnome/README.md`, package
  `patrone-gnome-taskbar`): a compact Windows 11-style taskbar at the bottom
  in place of GNOME's top bar, with the Start menu (type to search), role
  slots, jump lists and the tray. Built, and tried in a headless GNOME Shell
  on the VM; not installed yet.
- **Patrone File Manager** (`file-manager/`, package `patrone-file-manager`,
  section 8): Windows 11's File Explorer rebuilt in Rust and GTK4, with
  Patrone-drawn icons. `patrone-default-apps` 0.1.2 makes it the app for
  folders and puts it in Files' place in the favorites. The user installed
  0.1.0 and asked for changes; 0.2.0 (tab dragging between windows, typed
  commands, system folder hints, a new Permissions tab) is built and tried
  with real input in a headless GNOME Shell, not installed yet.
- **fastfetch** shows the Patrone logo for the user (`branding/fastfetch/`,
  installed with `branding/fastfetch/install-dev.sh`): the mark traced from
  the concept by `branding/tools/make_terminal_logo.py`, with a gradient in
  the Patrone blues. Checked with plain `fastfetch` in Patrone Terminal. (At
  first it was only previewed with `-c`/`--logo`, so the user's plain
  `fastfetch` still showed Ubuntu's logo and looked like a false claim. Say
  exactly how something was checked.)

Waiting on the user:

- **Install Patrone Task Manager** (section 15), which removes Ubuntu's
  Resources, then sign out and back in so Ctrl+Shift+Esc works:
  `sudo apt install ~/src/patrone-linux/out/patrone-task-manager_0.1.0_amd64.deb ~/src/patrone-linux/out/patrone-desktop_0.1.2_all.deb`.
  Try Ctrl+Shift+Esc, the taskbar's right-click "Task Manager", End task
  on an app, and Start or Stop on a system service (asks for the password).

- **Install the new packages, turn on Patrone Remote Desktop, reboot, then
  connect** (needs their sudo password; section 3 has the steps). Their
  favorites and icon theme were already reset to Patrone's defaults
  (2026-10-26, over SSH).
- **Chromium's first run** shows a "Chromium Additional Terms of Service"
  dialog that says "This Space Intentionally Blank": Chromium's placeholder
  where Google Chrome shows Google's terms (the strings sit with "Welcome to
  Chromium" in Chromium 156's resources). Accepting it is harmless. Find what
  triggers it and whether a policy or flag skips it, then add that to
  `patrone-default-apps`, since every new Patrone user would see it.
- **Login screen style:** the user asked for an "enterprise and/or school"
  look. It now keeps the list of users, restyled. Ask whether they'd rather
  have the username-and-password form schools and companies use
  (`disable-user-list=true` in `system/gdm/95-patrone-settings`), and
  whether to add a banner message.
- **Switch the login shell back to bash:** on 2026-10-26 the user ran
  `chsh -s /usr/bin/zsh` on my earlier advice, then decided on bash. They run
  `chsh -s /bin/bash` (asks for their password).
- **Installed 2026-10-26 by the user**: `patrone-gnome-taskbar` 0.3.0,
  `patrone-settings` 0.2.0, `patrone-branding` 0.1.5, `patrone-remote-desktop`
  0.4.3, `patrone-desktop` 0.1.1, `patrone-file-manager` 0.2.0 and
  Patrone's GNOME Settings (`...patrone1`), and GNOME Settings' build
  dependencies. Still to hear how they went: the Super key, the network
  menu's chevron, Shut down in a real session, Chromium's pages in Noto
  Sans and the wallpapers in Settings → Appearance.
- **The user's report on 0.3.0 (2026-10-26)**: "The themes are not the
  correct theme, the icons in the buttons are inconsistent, the start and
  buttons don't show the change, The taskbar layout doesn't change. Classic
  is royal, Royal is something else." Their GNOME Shell had started at
  15:26 and 0.3.0 was installed at 15:35, so the desktop ran 0.2.0's
  taskbar (the second time: Patrone Settings' first clicks hit the same
  thing at 14:59). And the title bar buttons did differ from app to app
  (section 9). Fixed in this round: install `patrone-gnome-taskbar` 0.3.1
  (it now says when to sign out), `patrone-settings` 0.3.0 (button styles
  of their own), `patrone-file-manager` 0.2.3 (GTK's own window buttons)
  and GNOME Settings `...patrone2` (the style choice, and pictures that
  were nearly invisible in a window of ordinary height), **then sign out
  from inside the desktop and back in**.
- **The user's next report (2026-10-26)**: the theme didn't change window
  title bars (only Classic and Royal had any), the Dots buttons touched
  File Manager's window edge, and the Start menu's default icons "look like
  shit, make clear ones". Then: the taskbar's grey should match the
  wallpaper; "the taskbar took about 30 seconds to update that I was
  running a new app"; and "make our overridden icons look better, and allow
  an option to use the default app icons". Fixed in this round: install
  `patrone-settings` 0.3.1 (title bars for every theme, a gap at the
  window's edge, the sign-in entry that writes the style sheets runs
  again, a new icon, the Color and Pin icons choices),
  `patrone-gnome-taskbar` 0.3.2 (its color from the wallpaper, Patrone's
  own pin icons or the apps', apps showing the moment they're opened),
  `patrone-terminal` 0.1.2 and `patrone-file-manager` 0.2.5 (new icons) and
  GNOME Settings `...patrone3` (the two new choices), then sign out and back
  in. Open apps keep their title bars until they're reopened: GTK reads
  `gtk.css` once. File Manager's tab bar doesn't follow the theme's title
  bar colors yet.
- **The 30 seconds were the whole desktop stalling, not the taskbar**: the
  user journal at that time has at-spi "Disabling unresponsive app" for
  GNOME Shell's own process, then `JS ERROR: Gio.IOErrorEnum: Timeout was
  reached` 25 seconds later (D-Bus's timeout) with no stack, then the RDP
  connection dropping. The taskbar makes no blocking calls, and in the
  throwaway shell apps reached it 20 ms after their windows. Which call
  stalled the shell isn't known; the session runs Ubuntu's ding,
  tiling-assistant, snapd-search-provider and web-search-provider
  extensions. If it happens again, note the time and what was clicked.
- **Installed 2026-10-26 by the user, then signed in again**: the round
  above (taskbar 0.3.2, Settings 0.3.1, Terminal 0.1.2, File Manager 0.2.5,
  GNOME Settings `...patrone3`); the taskbar loaded with no errors. Next,
  install `patrone-gnome-taskbar` 0.3.3 (the Recycle Bin docked to a corner
  of the desktop, and no Home icon, the user's asks), `patrone-terminal`
  0.1.3 (the see-through background, "like kali does it"), with
  `patrone-unpack` 0.1.1 and `patrone-default-apps` 0.1.3 (section 13),
  then sign out and back in. Checked in the throwaway shell only: each
  corner, dragging, double-click opening File Manager on Recycle Bin, the
  menu and the Empty dialog (cancelled; emptying wasn't tried there, since
  the trash service also empties other drives' trash); the desktop icons
  extension can't run there, so hiding Home was checked by compiling the
  override with the system's schemas.
- **Describe the rest of the desktop design** (desktop icons, windows, look
  and feel, login and lock screens; the taskbar and Start menu are decided,
  section 7) before milestone 4; then mockups, then `docs/DESIGN.md`.
- **Confirm in the RDP session** that resizing the `mstsc` window resizes the
  desktop, and that copy and paste work both ways. Set 200% scale in Settings
  → Displays if text is too small.
- **ASCII logo file (optional now).** The user shared an ASCII-art version of
  the logo as an image in chat, never saved to disk. The fastfetch logo is now
  traced from the concept instead; their file would only matter for
  `/etc/issue` or if they prefer that look.
- **Try the compositor's input handling** (section 2).
- **Install Patrone File Manager 0.2.10** (section 8; the user runs 0.2.9),
  from SSH or a terminal:
  `sudo apt install ~/src/patrone-linux/out/patrone-file-manager_0.2.10_amd64.deb`,
  then close every File Manager window so the new version starts. A name
  being typed (after Compress, New folder, New file or F2) is kept by a
  click anywhere else, and a new ZIP shows its icon and size at once.
- **Install Patrone Unpack** (section 13), from SSH or a terminal:
  `sudo apt install ~/src/patrone-linux/out/patrone-unpack_0.1.1_amd64.deb
  ~/src/patrone-linux/out/patrone-default-apps_0.1.3_all.deb` (apt adds
  Flatpak, which `patrone-unpack` depends on). Then double-click
  `~/Downloads/pdfcraft-0.9.0-linux-x86_64.flatpak` in File Manager: Install
  needs no password, adds Flathub for its runtime the first time, and Open
  starts PdfCraft (it reaches Start after the next sign-in, since Flatpak is
  new to the session). Also open a zip or tar.gz (Extract all, Extract here)
  and a `.deb`: Install asks for your password with "Installing software for
  everyone on this computer needs an administrator's password".
- **Install Patrone Sans and try it** (section 14), from SSH or a terminal:
  `sudo apt install ~/src/patrone-linux/out/fonts-patrone-sans_1.0.0_all.deb
  ~/src/patrone-linux/out/patrone-settings_0.3.2_amd64.deb
  ~/src/patrone-linux/out/gnome-control-center_50.3-0ubuntu0.2patrone4_amd64.deb
  ~/src/patrone-linux/out/gnome-control-center-data_50.3-0ubuntu0.2patrone4_all.deb`.
  Then Patrone Settings → Theme → Font (or GNOME Settings → Patrone → Font)
  → Patrone Sans: open apps and the taskbar change at once. Then decide
  whether Patrone Sans becomes Patrone's default (recommended, once tried;
  section 14 says what that changes).

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
- **Patrone Remote Desktop** (`remote-desktop/`, package
  `patrone-remote-desktop`; its README has the design). The user's
  requirements (2026-10-26): `mstsc` asks for the credentials, those are the
  Linux account's, they open the desktop with no second sign-in, the
  password is checked against the account each time and **never stored**
  (they rejected GNOME keeping its own copy), any account in a
  `remote-desktop` group may use it, off by default, and RDP sessions never
  lock. How:
  - GNOME Remote Desktop 50 can only check its own stored credentials or
    Kerberos (`grdctl rdp set-auth-methods`), never PAM. RDP's NLA can't
    check against PAM either: its NTLM step needs the server to hold the
    password's NT hash already, and `/etc/shadow`'s hashes can't produce it.
    So clients get TLS without NLA, and the password is checked when it
    arrives in the client info.
  - FreeRDP 3.32's proxy (Ubuntu's `freerdp-proxy`) on 3389 as user
    `patrone-rdp`, with Patrone's `patrone-login` plugin (C). Read in
    FreeRDP's source: in TLS mode its `ServerPeerLogon` filter runs before
    the client info arrives, so it can't see the password; the
    `ServerFetchTargetAddr` filter runs after, before the proxy connects on,
    and can pick a custom target (the proxy frees `target_address`). The
    client's settings (`peer->context->settings`) hold the plain password;
    the target connection's settings (`pdata->pc`) are separate.
  - A root helper per request (`patrone-remote-desktop-auth.socket`,
    `Accept=yes`) does PAM (`/etc/pam.d/patrone-remote-desktop`: group,
    UID ≥ 1000, `common-auth`, `common-account`), then `prepare-session`
    sets up the account's GNOME Remote Desktop with `grdctl --headless` (port
    20000 + UID, credentials a random secret in
    `/var/lib/patrone-remote-desktop/secrets`) and starts
    `gnome-headless-session@USER.service`.
  - `grdctl ... set-credentials USER` reads the password from stdin when it
    isn't a terminal, so the secret never appears on a command line. With no
    TPM it stores it in plain text in `credentials.ini` (mode 600): fine for
    a random secret, which is why it must never be a real password.
  - `no-lock` (from `/etc/xdg/autostart`) turns the lock and blanking off in
    seatless sessions, saving the account's own settings, and restores them
    on the console. The user's saved settings were set to GNOME's defaults,
    since an earlier per-user script had already forced them off.
  - Checked: `make check` (also run by the package build), 38 checks: the
    plugin refuses no credentials, a wrong password, "busy", a garbled
    answer and an unreachable helper, always clearing the password; on
    success it targets `127.0.0.1:21000` as the account with the secret,
    re-applies it before connecting and forgets it at session end; the real
    helper refuses malformed requests, odd names and unknown accounts.
    Scripts pass `bash -n`/`sh -n`, the units `systemd-analyze verify`, the
    autostart entry `desktop-file-validate`. The package's contents,
    dependencies, conffiles and not-enabled-by-default install were
    inspected. Built against headers extracted from `freerdp3-dev`,
    `libwinpr3-dev` and `libpam0g-dev` (same versions as the installed
    libraries) because the dev packages weren't installed;
    `bootstrap-dev-vm.sh` now installs them.
  - **Not checked yet:** a real sign-in. Whether `mstsc` prompts for and
    sends the password to a server without NLA, the proxy and plugin running
    under systemd's sandboxing, `prepare-session` as root (`runuser`,
    `systemctl --user` in the account's manager, the headless session
    starting), and display, clipboard and resizing through the proxy.
  - Steps for the user, from SSH (`ssh patronedev`) after logging out of the
    Remote Login desktop (one desktop per account, and turning it on stops
    Remote Login): `sudo apt install` the three new debs in `out/`
    (`patrone-branding_0.1.1`, `patrone-default-apps_0.1.1`,
    `patrone-remote-desktop_0.1.0`), `sudo patrone-remote-desktop enable`,
    `sudo adduser wdaugherty remote-desktop`, `sudo reboot`. Then delete the
    old saved RDP credentials on Windows (`cmdkey
    /delete:TERMSRV/DevDistro-PatroneLinux.local`) and connect. Logs:
    `journalctl -u patrone-remote-desktop` and `journalctl -t
    patrone-remote-desktop-auth` (the user is in `adm`, so readable over SSH).
- **First real try (2026-10-26):** the proxy never started. FreeRDP's INI
  parser rejects a file whose comments start with `#` ("failed to parse ini
  file"); fixed in 0.1.1 with `;`, and `make check` now runs the real
  `freerdp-proxy` on the shipped file (it fails on the old one). With 3389
  free, the user's own Desktop Sharing daemon (set up earlier with
  `setup-remote-desktop.sh sharing`) took it once they logged in on the
  console, so their `mstsc` reached Desktop Sharing: the same session,
  mirrored on both screens.
- **Session model: the user requires Windows behavior** (2026-10-26): one
  session per account; an RDP sign-in takes over the account's existing
  session, and the machine's own screen stops showing it while it's
  remote; never a second session. GNOME can't: its Remote Login kills the
  local session, and Desktop Sharing mirrors it (GNOME's own issue
  gnome-remote-desktop#92 asks for takeover). Proposed: sessions always run
  on the machine's seat (never headless). `gdm-new-session USER` without
  `--headless` asks GDM (`LocalDisplayFactory.CreateUserDisplay`) for a
  normal session; the proxy joins it through the account's own GNOME Remote
  Desktop in Desktop Sharing's `extend` mode (a virtual monitor the client's
  size); the machine's screen switches to the login screen; signing in
  there switches back to the same session (GDM's usual behavior) and ends
  the RDP connection; disconnecting locks the session. **The deciding
  unknown:** whether a GNOME session keeps drawing to its virtual monitor
  and taking input over RDP while another VT (the login screen) is on the
  machine's screen. Test that first, with the user.
- **Built (0.2.0): Windows-style sessions.** The user, 2026-10-26: "I want
  to make it MUCH MORE LIKE WINDOWS". The VT-switch plan above is dead:
  Mutter 50.1's `meta_renderer_pause` (run when its session stops being the
  one on screen) inhibits the frame clock of every view, RDP virtual
  monitors included, so a background session freezes for RDP. Instead, each
  account's one desktop is always GDM's headless GNOME session, and the
  machine's screen is a viewer: the "Patrone Desktop" login session
  (`/usr/share/wayland-sessions/patrone-desktop.desktop`) runs
  `libexec/console`, which asks `auth --console` (over `console.sock`, account
  from `SO_PEERCRED`, no password since GDM checked it) for the desktop and
  shows it with `sdl-freerdp` in `cage`. GNOME Remote Desktop 50.2's headless
  mode accepts a new client and then stops the others
  (`on_session_post_connect` → `maybe_stop_session`, from its source), which
  gives "last one in wins" in both directions. `prepare-session` answers
  "busy" only for a plain GNOME session on the screen (it now looks for
  `libexec/console` in the session's cgroup: logind's `Desktop` is empty for
  GDM's sessions). Checked: the package builds, `make check` passes
  (including `auth --console` refusing without a socket), the viewer check
  calls the user's live GNOME session (5) a plain desktop. Not tried yet:
  everything on the real screen and through `mstsc`. Possible snags: `cage`
  (wlroots) on `hyperv_drm` (the viewer sets `WLR_NO_HARDWARE_CURSORS=1`;
  `WLR_RENDERER=pixman` if it can't render), and whether FreeRDP's
  `/from-stdin:force` reads the secret through `cage`. The user's own GNOME
  Desktop Sharing was disabled from starting at login (it took 3389 before).
  Still to do: lock the screen after idle (end the viewer, back to the login
  screen), and Patrone's own compositor doing this natively.
- **Second try (0.2.0, 2026-10-26):** the proxy ran and listened on 3389,
  but `mstsc` sent no password: the plugin logged "sent no username and
  password". To a server without NLA, Remote Desktop Connection leaves
  sign-in to the server's own screen. 0.2.1 adds `patrone-remote-desktop
  rdp-file` (and `scripts/hyperv/PatroneDev.rdp`) with
  `enablecredsspsupport:i:0` and `prompt for credentials on client:i:1`,
  which should make it ask on Windows and send the credentials. Unverified:
  Microsoft's current RDP file reference doesn't list the second setting.
  The plugin now logs whether a username came without a password. If the
  file doesn't work, the fallback is a sign-in screen served by Patrone
  itself for clients that send nothing (as Windows does without NLA). Also,
  the user was signed in on the screen with Ubuntu's session (GDM
  preselects the last one), which would have been "busy"; they pick Patrone
  Desktop from the gear menu.
- **Third try (0.2.3, 2026-10-26): the RDP file doesn't help.** The proxy
  logged plain `mstsc` twice as "no username or password" and
  `PatroneDev.rdp` as "the username "wdaugherty" but no password": with
  `enablecredsspsupport:i:0` Remote Desktop Connection sends the file's
  username and leaves the password to a sign-in screen on the server. It
  doesn't send passwords to a server without NLA. Two ways forward, put to
  the user: (A) a Patrone sign-in screen served over RDP to clients that
  send nothing (what Windows does without NLA; nothing stored; the prompt
  is in the RDP window; needs a login screen plus a hand-off to the
  desktop, perhaps RDP server redirection), or (B) NLA as Windows does for
  local accounts: FreeRDP's proxy checks NLA against a SAM file of NT hashes
  (`[Server] SamFile=`), which a PAM hook keeps in step with the Linux
  password (on every password change and password sign-in), and PAM still
  checks the real password from CredSSP's TSCredentials on every
  connection. B stores a hash, which the user rejected for GNOME's own copy;
  it can't go stale, though. GNOME's Remote Login as the sign-in screen:
  undocumented when the user already has a desktop (one report says it
  kills the other session).
- **The user chose B (NLA) on 2026-10-26; built as 0.3.0.** The proxy
  requires NLA and checks it against `/var/lib/patrone-remote-desktop/sam`
  (WinPR's format, `user:::NTHASH:`; FreeRDP's NTLM retries the lookup
  without the client's domain, and matches usernames case-sensitively). NLA
  writes the decrypted TSPasswordCreds into the peer's settings; the client
  info that follows clears `FreeRDP_Password` (`rdp_read_info_string` sets it
  to NULL first), so the plugin takes the credentials in its
  `ServerPeerLogon` filter, which runs right after NLA. `auth --remember`
  (from `pam_exec quiet seteuid expose_authtok`, added to `common-auth` and
  `common-password` as "Additional" by a `pam-auth-update` profile) writes
  `remote-desktop` members' hashes with WinPR's `NTOWFv1W`; `pam_exec` runs
  password-type commands only in the update step, when `PAM_AUTHTOK` is the
  new password. Checked: `make check`, 50 checks (NLA path in the plugin, the
  textbook NT hashes of "password" and "Password1", read back with WinPR's
  SAM reader, one entry per account, others kept); a throwaway proxy on
  3399 with a test SAM: FreeRDP's client with the right made-up password got
  "Accepted client" (NLA passed), a wrong one `ERRCONNECT_LOGON_FAILURE`
  (NTLM's MIC check failed). Not yet: `pam-auth-update` on the VM, a real
  hash from a real sign-in, and `mstsc`.
- **0.3.0 on the VM (2026-10-26):** NLA and PAM both worked with the
  user's real password through `mstsc` ("signed in wdaugherty from
  10.0.0.57"). First refusal: they were signed in on the screen with
  Ubuntu's session, so "busy". Second: the headless desktop never started,
  `gdm-new-session: Failed: The connection is closed`, which is Ubuntu bug
  2171951 (filed October 2026): GDM's `gnome-headless-session@.service` runs
  as a DynamicUser, and systemd starts the system bus daemon with
  `SYSTEMD_NSS_DYNAMIC_BYPASS=1`, so the bus can't resolve it and drops the
  connection. 0.3.1 ships the bug's workaround as a drop-in (run it as
  `gdm`), and `prepare-session` now gives up as soon as the unit fails.
- **0.3.1 on the VM:** Remote Desktop reached the background desktop. Then
  the user signed in on the screen, the login screen chose Ubuntu's
  session (remembered), and GNOME's login screen ended the background
  desktop: gnome-shell's `gdm/loginDialog.js` `_findConflictingSession`
  treats any other active or online wayland/x11 session of the account as a
  conflict, shows `ConflictingSessionDialog`, and "continue" calls GDM's
  `StopConflictingSession`. It does so whichever session was chosen, so
  Patrone Desktop would have hit it too. 0.4.0: a login-screen extension
  (`session-modes: ["gdm"]`, enabled in GDM's dconf; GNOME Shell inits
  extensions in every mode, and the gdm mode, built on "restrictive", takes
  them from `enabled-extensions`) overrides `_findConflictingSession` to skip
  seatless sessions, falling back to GNOME's on an error; tested under gjs
  with GNOME Shell's modules stubbed (and the test fails if the seat check
  is removed). Ubuntu's `ubuntu.desktop` is diverted aside; the hidden
  `patrone-gnome.desktop` runs background desktops (GDM's daemon ignores
  `NoDisplay`, libgdm's menu honours it; headless needs
  `X-GDM-CanRunHeadless`). With `ubuntu` gone, GDM's fallback (`gnome`, else
  the first session file by name) is `patrone-desktop`. Not tried yet:
  needs a reboot (GDM reads it all at start).
- **0.4.0 after a reboot:** still "busy". With `ubuntu.desktop` diverted,
  the user's remembered "ubuntu" was invalid, GDM fell back (`gnome`, else
  the first session file by name), and its search includes its own
  `/usr/share/gdm/greeter/wayland-sessions/`, where `gnome-greeter.desktop`
  (`Exec=gnome-session`, `X-GDM-CanRunHeadless=true`) sorts before
  `patrone-desktop`: the screen got a bare GNOME session
  (`XDG_SESSION_DESKTOP=gnome-greeter`). AccountsService still says
  "ubuntu" (GDM didn't save the fallback). 0.4.1: Patrone Desktop is
  `gnome.desktop` (GDM's first fallback, for local and headless), running
  `libexec/session`, which picks the viewer on a seat and
  `gnome-session --session=ubuntu` without one (tested against the live
  seat0 session and a seatless one). Other packages' `gnome.desktop` is
  diverted too.
- **0.4.1 on the VM:** RDP works, and signing in on the screen no longer
  ends the desktop (the login-screen extension works). The screen's viewer
  failed two ways. (1) Screen first: GDM refused the background desktop,
  "There's already an opened session for user": `CreateUserDisplay` calls
  `gdm_find_graphical_sessions_for_username`, which counts any logind
  session of type wayland/x11/mir that's active or online, and the screen's
  own session is one. 0.4.2 starts the desktop from the PAM profile during
  the login screen's password check (`auth --screen-sign-in`, only for
  `gdm-password`/`-fingerprint`/`-smartcard`, not `gdm-autologin`, which GDM
  uses for the background desktop itself), before GDM opens the screen's
  session. (2) With the desktop already running (RDP first): `cage` failed
  `xwayland/xwm.c:592` (SDL had picked X11 through cage's Xwayland), and the
  client crashed with it. 0.4.2 runs the client with `DISPLAY` unset and
  `SDL_VIDEO_DRIVER=wayland`; checked that SDL starts on Wayland that way
  (on the background desktop's display, against a closed port).
- **0.4.2 works end to end (the user, 2026-10-26: "IT WORKS!!!").** One
  NLA prompt with the Linux password, the same desktop on the VM's screen
  and over RDP, each taking it over from the other, no desktop lost.
- **Multiple monitors (the user's question, 2026-10-26).** The desktop's
  monitors are virtual: GNOME Remote Desktop builds one per monitor in the
  client's layout (its "Layout manager", up to a maximum count; layouts can
  change mid-session over the display-control channel). Over RDP with
  "Use all my monitors" the desktop should get the client's monitors;
  connecting from a one-monitor client moves every window onto it, and
  going back to more monitors doesn't put them back (GNOME doesn't remember
  per-layout positions; Windows does, and Patrone's compositor could). Not
  yet seen through the proxy. The screen's viewer runs the client
  full-screen without `/multimon`, and `cage` extends across all outputs by
  default (`-m extend`), so a multi-monitor PC would likely show one monitor
  or one wide one. To do: `/multimon` in `libexec/console`, tested with a
  QEMU VM with several displays inside the dev VM (Hyper-V's console has
  one).
- Packaging lessons: debhelper replaces `#DEBHELPER#` even inside comments
  (0.2.1's install failed; `build.sh` now refuses it), and for units
  installed with `--no-start` its generated `preinst` stops them on every
  upgrade without starting them again (0.2.3's `postinst` restarts enabled
  ones).
- The earlier per-user attempt (a `headless` mode in
  `setup-remote-desktop.sh` with the Linux password as GNOME's credentials,
  and `rdp-no-lock.sh`) was removed: it stored the password, which the user
  rejected.

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

## 7. Patrone taskbar (patrone-gnome-taskbar)

The user's taskbar decisions (2026-10-26) and how it works are in
`shell/gnome/README.md`. In short: Windows 11's taskbar, left-aligned and
compact ("smaller, not taking up so much real estate"; built at 32 pixels,
then made a third larger, 42, once the user had used it), in place
of GNOME's top bar; the logo's "P" opens Start, where typing searches at
once (GNOME's search providers, then the web); a search button; pinned slots
that open the default web browser, terminal, file manager and mail app, with
that app's windows under the slot; running apps, pinnable; Windows 11-style
right-click jump lists that also hold the configuration. Built as a GNOME
Shell extension for now (the user said "Build it out"); the design carries
over to Patrone's own shell.

Checked in a headless GNOME Shell 50.1 on the VM (`shell/gnome/tests/
headless.sh`, light and dark): see the README's "Trying it" for the list.
The package builds and installs its schema with the system's, where a system
extension finds it; its override compiles with Ubuntu's schemas and turns the
extension on under both desktop names. Not checked until installed: the real
Super key (a simulated one doesn't reach GNOME's Super-key detection in a
headless shell; GNOME's overlay-key signal was tested instead), the power
options a real session allows, and the RDP and console views.

To install and load it:

```
sudo apt install ~/src/patrone-linux/out/patrone-gnome-taskbar_0.2.0_all.deb
```

then **log out from inside the desktop** and sign in again: GNOME Shell
loads extensions only when it starts. With the taskbar, that's Start → the
account picture → Sign out (in 0.1.2 and before, Start → power → Sign out);
with GNOME's top bar, its power menu, top right, → Log Out.
With Patrone's Windows-style sessions, disconnecting Remote Desktop or
signing out of the screen's session only disconnects, and the desktop, with
its old shell, keeps running; until it's restarted, `gnome-extensions info
patrone-taskbar@patrone-linux` in it says the extension "doesn't exist". An
account that has its own `enabled-extensions` list keeps it;
`gnome-extensions enable patrone-taskbar@patrone-linux` adds the taskbar.

Findings:

- GNOME Shell 50 picks `stylesheet-light.css` or `stylesheet-dark.css` from
  an extension by the system style, and St's CSS supports `@import`, so the
  sizes live once in `common.css`.
- A hidden actor tracked as chrome still reserves its space (the top bar's
  strut stays), so the taskbar untracks `panelBox` while it's hidden.
- Clutter's `BinLayout` only honors a child's alignment where the child
  expands, and expanding sideways propagates up and stretches the button
  along the bar: the taskbar buttons' insides expand vertically only.
- The overlay-key handler is blocked with
  `GObject.signal_handlers_block_matched(global.display, {signalId:
  'overlay-key'})` and unblocked when the extension turns off.
- A throwaway shell's document portal mounts a FUSE folder in its runtime
  folder; `headless.sh stop` ends everything on the test's D-Bus session and
  unmounts it.
- Ubuntu's desktop icons extension (DING) ends every desktop-icons process
  of the account when it starts, across sessions: each throwaway shell made
  the user's real desktop relaunch its icons (seen in the user journal,
  "Launching DING process" at each test start). `headless.sh` now turns DING
  and the snap permission prompts off in the throwaway shell.
- **0.1.1 in the user's desktop** (2026-10-26, over Remote Desktop): "some
  strange icons in the middle", and a right-click on what looked like empty
  space "logged me out". The desktop wasn't logged out (its shell kept
  running); the Remote Desktop connection was cut three times, each logged
  by GNOME Remote Desktop as "Disconnected by EIS" with
  `ERRINFO_RPC_INITIATED_DISCONNECT`. The icons were GNOME's screen-sharing
  indicator (`ui/status/remoteAccess.js`): a shared-screen icon and a stop
  square, unstyled outside the top bar (the theme styles it under
  `#panel`), about 100 pixels wide, and stopping the sharing on a press of
  any button. In a desktop with no seat (mutter: "No seat assigned, running
  headlessly") the Remote Desktop connection is itself a screen-sharing
  handle, so the indicator is always on. A throwaway shell has no client,
  so it never showed there. 0.1.2 hides it when
  `global.backend.is_headless()`. A right-click on the bar's real empty
  space opened the taskbar's own menu (Task Manager started at 13:09).
- GNOME's top bar buttons (`PanelMenu.ButtonBox`) set `x_expand`, but
  `St.Bin` containers don't pass it up, so the tray didn't stretch; 0.1.2
  sets `x_expand = false` on the moved boxes anyway.
- **0.1.3 in the user's desktop**: the quick settings' "Wired" chevron did
  nothing. Not the taskbar: GNOME binds the network toggles' `reactive` to
  polkit's `org.freedesktop.NetworkManager.network-control`, which is
  "yes" only for local sessions, and polkit counts a session as local only
  when it has a seat. Patrone's desktops have none (logind: Remote=yes, no
  seat), so polkit treated them like SSH logins. Fixed in
  `patrone-remote-desktop` 0.4.3 (section 3, `remote-desktop/README.md`
  step 7).
- **0.2.0** (the user asked for dragging, a taskbar size setting, and
  themes): dragging uses GNOME's `ui/dnd.js`. `set_size()` fixes an
  actor's height, which Clutter then reports as its preferred height, so
  the bar never took a new size until `_relayout` dropped it first. St
  can't take negative margins (the classic Start button had one, and the
  whole bar's layout came out at x = 4294967296), and a style sheet's own
  rules beat those of a sheet it imports, however specific (the classic
  bar's colors, and the light style's screen-sharing pill, lost to the
  light sheet's until moved into it). In the throwaway shell, an app misses
  a click whose pointer arrives with it; `headless.sh click` now moves
  first, and the first click into a new window was still lost.
- `pkill -f NAME` matched the SSH command that ran it, and could have hit
  the same app on the real desktop: `headless.sh close-apps` ends only the
  apps it started.
- **0.3.0** (the user, 2026-10-26: Classic "shouldn't look like windows 10
  royal, it should just look a bit more basic", then "create one that is
  just like the royal theme but not 'start'"; "allow the P to be replaced
  with a series of other icons"; "the taskbar should have its own set of
  possible layouts"): Royal is the old Classic without the "start" label,
  Classic is plain and square, and Modern got a look of its own (rounder
  buttons, no line, blue indicators) once it no longer centered the
  buttons. Layouts are their own setting: Left, Centered, Split and Top.
  At the top, menus open downward (`updateArrowSide`, the tray menus too),
  tooltips go below, and the indicators run along the screen's edge. Eight
  Start icons; the `-symbolic` ones, loaded as file icons, take the bar's
  text color. Checked in the throwaway shell: each theme in light and dark,
  each layout, Start opening downward from the top, every icon (white on
  Royal). A highlight on the tray at the top was the test's own pointer,
  left where the tray landed.

## 8. Patrone File Manager (patrone-file-manager)

Built 2026-10-26 at the user's request: an exact replica of Windows 11's
File Explorer, configurable the same way, with its right-click menus, "All
RUST if it makes sense". The user named it **File Manager** ("Patrone File
Manager"): app ID `org.patrone.FileManager`, command
`patrone-file-manager`. Rust and gtk4-rs without libadwaita; GTK's Default
theme restyled by `data/style.css`, light and dark. For icons the user chose
Fluent-style chrome with Windows-style file icons, then asked for "nice
versions of what Microsoft uses ... professional, but not exact copies", so
every icon is drawn for Patrone (`tools/make_icons.py` for the color icons,
`src/glyph.rs` for the line glyphs) and none of Microsoft's files are used.
`file-manager/README.md` lists what it does.

Verified on the dev VM:

- It builds; 12 unit tests pass (sizes, natural sort, sorting and grouping,
  search matching, ZIP and TAR round trips, every glyph path); Clippy and
  rustfmt are clean. The packages build (`patrone-file-manager` 0.1.0, whose
  build runs the tests; `patrone-default-apps` 0.1.2); their contents and
  dependencies were inspected; the desktop files validate. In a scratch XDG
  setup with a stand-in binary on `PATH`, `gio mime inode/directory` picks
  File Manager from the new `/etc/xdg/mimeapps.list`, and the favorites
  override compiles against Ubuntu's schemas and puts File Manager in Files'
  place under both `GNOME` and `ubuntu:GNOME`.
- By hand through GTK's Broadway backend (mouse clicks and single key
  presses): Details and Large icons, switching views keeping the selection,
  each folder's view remembered across restarts, Home, This PC with drive
  usage, Gallery, the Recycle Bin page, the item and background right-click
  menus, Open with (opening on hover), Show more options, the View and See
  more dropdowns, the Details pane, Properties (General, Permissions),
  Folder Options (Show hidden files applied and kept), copy and paste
  ("notes - Copy"), a paste conflict through "Compare info" to "keep both"
  (`budget (2).csv`), the progress window, Delete to the Recycle Bin and Undo
  Delete restoring the file, New → Folder renaming in place, F2 and Ctrl+Z,
  search as you type and Escape, Ctrl+T and closing a tab, Compress to ZIP
  and Extract All, and dark mode.
- A screenshot under Wayland, from the app itself
  (`PATRONE_FM_SCREENSHOT`) in a throwaway headless GNOME Shell: the
  client-side title bar with tabs and window buttons, following the session's
  dark preference.

Not verified yet: drag and drop, Ctrl+C, Ctrl+V and Ctrl+W from the keyboard
(the browser keeps them in Broadway), network places and mounting, removable
drives and Eject, Share, the Open with dialog, Properties' Apply (rename,
Read-only, Hidden, permissions), restoring windows at sign-in,
`org.freedesktop.FileManager1`, and daily use in the user's own desktop.

Findings:

- A GTK 4.20+ app style sheet sees `@media (prefers-color-scheme: dark)`
  only when the provider's `prefers-color-scheme` is bound to
  `GtkSettings:gtk-interface-color-scheme`; without it the app's palette
  stayed light while GTK's own theme went dark.
- `GtkStack` sizes to its largest page unless `hhomogeneous` is off; a
  hidden page's minimum width pushed the visible one off screen inside a
  `GtkPaned`.
- `GtkGridView` items take right-clicks before a bubble-phase gesture on the
  grid sees them: catch them in the capture phase. List cells keep the
  position they were bound at when items are added above them; read
  `GtkListItem:position` instead. GIO reports a new file twice (created,
  then changed), and `GtkDirectoryList` replaces the item, dropping its
  selection; a directory monitor also reported the folder itself once, so
  the view filters out anything not inside the folder it shows.
- Test runs in a private `dbus-run-session` start their own dconf service,
  which the desktop's own one overwrites in `~/.config/dconf/user`: give
  tests their own database with `DCONF_PROFILE` (a file with
  `user-db:pfm-test`).
- Papers 50's `papers-thumbnailer` exits 1 for every PDF on the VM, even run
  by hand, so PDFs show their icon instead of a thumbnail.
- Nautilus owns `org.freedesktop.FileManager1` through D-Bus activation
  (`/usr/share/dbus-1/services/org.freedesktop.FileManager1.service`), and
  Ubuntu's desktop-icons extension starts it at sign-in, so a browser's "Show
  in folder" will keep opening Nautilus until Patrone's ISO leaves Nautilus
  out (or the package diverts that service file).

### 0.2.0 (2026-10-26, after the user tried 0.1.0)

The user's notes on 0.1.0, and what changed:

- "The tab is too large on the initial one, and it must be draggable to
  change the location of the window, just like the header bar ... figure
  out what happens if I want to attach that tab to another file explorer":
  tabs have a fixed width; a window's only tab moves the window; with
  several, dragging along the strip reorders and pulling a tab out (or Ctrl)
  carries it: dropped on another window's title bar it joins that window,
  anywhere else it opens in its own window, and an emptied window closes.
  The tab menu's "Move to new window" now moves the tab, and "Move to
  window" lists the others.
- "I can't seem to close multiple tabs": the tab's press handler claimed
  every left click, which cancelled its own close button's click. Fixed.
- A "hint" option showing what system folders are for, with a banner naming
  the Windows equivalent; the user then asked for it as a View option ("there
  is no help menu"): View → Show → System folder hints, and Folder Options →
  View. Tags beside system folders and well-known files, a banner inside
  them, the table in `file-manager/src/hints.rs`.
- "The permissions window is a bit strange ... it should allow us to mark
  something as executable. If the help/hint area is on ... show what the
  command would be": a new Permissions tab (owner and group, Read/Write/Run
  for owner, group and everyone else, "Allow running this file as a
  program", special permissions, apply inside a folder, the mode as `0755`
  and `-rwxr-xr-x`), changes as administrator through pkexec when needed,
  and with hints on the `chmod`/`chown` commands, with `sudo` when needed.
- Text looked too large and icons too small, and the special folders'
  emblems too small: the text is 9 pt (Windows' size) in Patrone's Noto
  Sans (section 10), list icons are drawn at 20
  px (the artwork's margins make the item about 16), the emblems are 1.4
  times larger, and the system drive and its "P" are larger.
- At `/`, known folders get their own icons (a gear for `/etc`, a terminal
  for the program folders, and so on).
- The empty-folder message sits in the middle of the view.
- A command typed in the address bar runs in that folder, as in Windows:
  apps with windows directly, anything else in a terminal that stays open;
  `cmd`, `notepad`, `explorer` and the like work; `cd PLACE` navigates.

Verified on the dev VM, in a throwaway headless GNOME Shell driven with real
pointer and keyboard input through Mutter's RemoteDesktop API and watched
through its ScreenCast stream (see the README's testing notes):

- Dragging a window's only tab moved the window by exactly the drag.
  Dragging the second of two tabs to the left reordered them. Pulling a tab
  down and dropping it on the desktop opened it in a new window (a floating
  copy of the tab follows the pointer, the original fades). Ctrl-dragging
  that window's only tab onto the first window's title bar added it there
  (with the blue insertion line), and the emptied window closed. Clicking ×
  on an inactive tab closed it. "+" opens a tab.
- `touch made-by-command.txt` typed in the address bar ran in a terminal in
  that folder and created the file; `notepad notes.txt` opened Text Editor
  on the folder's `notes.txt` with no terminal.
- Screenshots: `/` in Details with hints on (tags, banner, the new icons),
  the Permissions tab of a script and of `/etc` (with the "root owns this"
  notice and `sudo chown`/`chmod` commands), the empty-folder message, the
  sample folder at the new sizes, and an icon sheet of every folder and
  drive icon.
- 14 unit tests pass (new: hint lookup, symbolic and recursive modes);
  Clippy and rustfmt are clean; the 0.2.0 package builds.

Not verified yet: applying permission changes (directly or through pkexec's
password prompt, which needs the user), dropping files on a tab, and the tab
menu's "Move to window".

Findings:

- GNOME's Wayland ends a drag dropped where nothing accepts it with
  `GDK_DRAG_CANCEL_ERROR`, not `NO_TARGET`, so tab tear-off checks both.
- GTK 4.22 can't use `xdg_toplevel_drag_v1` (Mutter has it, GTK doesn't use
  it), so a carried tab is a floating copy and the new window opens where
  GNOME places new windows, not under the pointer.
- Mutter's `NotifyPointerMotionAbsolute` takes the stream path as a string
  (`(sdd)`), not an object path.
- `gsettings set` hung in a private `dbus-run-session`; GLib's keyfile
  backend (`GSETTINGS_BACKEND=keyfile`, `XDG_CONFIG_HOME` pointing at a
  scratch folder) avoids dconf for tests altogether.
- Pango adds a hyphen where it breaks a word, which reads as part of a path;
  path and command labels turn hyphens off.

### 0.2.1 (2026-10-26, the user's notes on 0.2.0)

- "I can't close the folder options menu": a closed dialog was never freed
  (its own close button and Escape handler held the window strongly), so
  reopening Folder Options brought back the old one, whose OK and Cancel
  were dead. Dialogs now hold their window weakly, and Folder Options only
  reuses a dialog that's still on screen. Verified with real clicks: close,
  reopen, Cancel; reopen, OK.
- "Instead of using a tag ... add a column ... use a field that blends in":
  a Purpose column after Name in Details while hints are on (plain text like
  the other columns, sortable, with the explanation as its tooltip); Tiles
  and Content add it after the type; the other views put it in the tooltip.
  Screenshots of `/` in Details and Tiles.
- "In the path bar, I should be able to see the full path as breadcrumbs to
  a point (just like in windows)": the breadcrumbs start at This PC and the
  drive for every local folder (they used to start at the home or user
  folder); a layout manager (`CrumbLayout` in `addressbar.rs`) shows as many
  as fit from the end and folds the rest into a « button that lists them.
  Verified with real clicks: a parent breadcrumb goes up, « lists the folded
  folders nearest first, and a maximized window shows the whole path.
- Asked how to give a user named "base" access: there's no such account on
  the VM. The Permissions tab follows Linux's owner, group and everyone
  else; giving one more named person access needs an ACL (`setfacl`, in the
  `acl` package, which is installed), which the tab doesn't offer yet.

### 0.2.2 (2026-10-26)

The user asked for Windows-style per-person access in Permissions ("Add...
a user or group with Read/Write/Run ... using ACLs"). The tab now reads the
item's ACL (`getfacl`), lists everyone given access by name under Owner,
Group and Everyone else with their own boxes and a remove button, and adds
people and groups from "Give access to". Apply removes entries (`setfacl
-x`, or `-b -k` when none are left) before `chmod`, and sets the rest after
it (`setfacl -m g::...,u:NAME:...`), because with an ACL chmod's group bits
are the mask; a folder also gets default entries so new items inherit them.
The Group row shows the owning group's own rights (getfacl's `group::`), not
the mask.

Verified with real clicks in the headless GNOME Shell, reading the result
with `getfacl` each time: adding root to a file (`user:root:r--`, the
group's `rw-` kept), giving root Write (`rw-`), removing root (back to a
plain `-rw-rw-r--`); adding root to a folder (access and default entries; a
file made inside afterwards inherited root's entry) and removing it again
(defaults gone too). With hints on, an item with an ACL showed its
`setfacl -m` command. 15 unit tests pass (new: reading getfacl's output).

Not verified yet: the pkexec path for someone else's items, groups added by
name (only root, the VM's one other account, was tried), and "Apply to
everything inside" together with named entries.

### 0.2.4 (2026-10-26)

Found on the user's VM by the Terminal session: GNOME 50's session skipped
the "reopen folders at sign-in" entry ("sets X-GNOME-Autostart-Phase, but
gnome-session no longer manages session services"). systemd's
xdg-autostart generator still makes a unit for such an entry, but marks it
NotShowIn=GNOME, so its ExecCondition fails under `ubuntu:GNOME`. The entry
no longer sets the phase. Checked by running
`/usr/lib/systemd/user-generators/systemd-xdg-autostart-generator` on the old
and new entries: the old unit's condition exits 1 for `ubuntu:GNOME`, the
new unit has none. Not yet seen across a real sign-in.

### 0.2.6 (2026-10-26)

The user picked a more 3D title bar style in Patrone Settings (Studio, Wide
buttons); Terminal's title bar turned raised but File Manager's stayed flat.
Patrone Settings' styles draw GTK's `headerbar` (raised gradient when
active, flat under `:backdrop`), and File Manager's tab strip and dialog
title bars were plain boxes painted flat by its own style sheet. They are
now boxes whose CSS node is `headerbar` (`util::header_box`), with File
Manager's look as the default that the account's gtk.css overrides. Classic
and Royal also restyle labels and buttons inside a header bar (bold, white),
so a small style sheet (`data/over-theme.css`) at priority USER + 1 keeps
tab names in their usual weight and the selected tab's text and close
button in File Manager's colors.

Verified in the headless GNOME Shell with the account's gtk.css set as
Patrone Settings sets it: Studio with Wide buttons in dark mode, two windows
(the active one a gradient from #42424a to #313135, the inactive one flat
#2c2c30, Studio's colors), and Royal with Glossy buttons in light mode
(glossy blue when active, pale blue when not, the selected tab readable;
a Properties dialog's title bar royal blue with a bold white title).

### 0.2.7 (2026-10-26)

- At the user's go-ahead on the Unpack session's offer: Extract All… and a
  new Extract here cover every archive Patrone Unpack opens, through
  `patrone-unpack --extract-to` / `--extract-here` when it's on PATH; ZIP
  and TAR keep File Manager's own extraction (Extract here without Unpack
  makes a folder named after the archive, " (2)" when taken). Unpack is a
  single-instance app whose process can return before the work is done, so
  for Extract All… File Manager makes the destination folder first and
  opens it while the files arrive.
- "Make the searchbox smaller and a fixed size, allowing more space for the
  folder path": 240 pixels, not stretching.

Verified with real clicks in the headless GNOME Shell, with Unpack 0.1.0's
files unpacked to a scratch folder on PATH (not installed): a `.tar.zst`'s
right-click menu offered Extract All… and Extract here, and Extract here
left its one folder, `stuff`, next to it; Extract all from the command bar
on a `.tar.xz` suggested `arc-xz`, extracted `stuff` into it and showed the
folder. Screenshots of the address row at 820 pixels and maximized (the
whole path from This PC, the search box 240 pixels). Not checked: rar and
7z (no tool on the VM to make them), single compressed files, and Extract
here on several archives at once.

### 0.2.8 (2026-10-26)

- "Remove the old filemanager": `apt-get -s remove nautilus` also removes
  `ubuntu-session`, `xdg-desktop-portal-gnome` (GNOME 50's file dialogs for
  Snap and Flatpak apps are Nautilus), `ubuntu-desktop-minimal` and
  `patrone-desktop`, so Nautilus is retired instead: patrone-file-manager
  diverts its menu entry (replaced by a hidden one with no MimeType) and its
  `org.freedesktop.FileManager1` D-Bus service (replaced by one that runs
  `patrone-file-manager --gapplication-service`; the app now waits 10 s for
  the call, and owns the name with ALLOW_REPLACEMENT | REPLACE). Nautilus
  still runs on demand: DING (desktop icons) activates org.gnome.Nautilus
  at sign-in, and Nautilus then holds FileManager1 itself until it idles
  out. DING (`use-nemo` false) sends "Show in Files" and Properties through
  FileManager1 (ShowItems, ShowItemProperties: File Manager answers) and
  copy, move and trash on the desktop through Nautilus's FileOperations2,
  which keeps working since Nautilus stays. The chain that blocks removal:
  xdg-desktop-portal-gnome Depends `nautilus (>= 47~beta)`, ubuntu-session
  Depends xdg-desktop-portal-gnome, ubuntu-desktop-minimal Depends both, and
  patrone-desktop Depends ubuntu-desktop-minimal. The account's
  `org.gnome.shell favorite-apps` still lists org.gnome.Nautilus.desktop;
  that's the user's to change. Verified in the headless shell: with the new service file first in
  XDG_DATA_DIRS and no File Manager running, a FileManager1.ShowItems call
  started it and showed the folder with the file selected.
- "It didn't open flatpak": the user's own mimeapps.list said
  nm-connection-editor opens application/vnd.flatpak, set by File Manager's
  Open with: it pre-selected the first of "More apps" (Advanced Network
  Configuration), so Always made that the default. Removed from
  ~/.config/mimeapps.list on the VM (backup beside it,
  mimeapps.list.before-pfm-fix); Unpack is now the default. Open with
  selects nothing when nothing is suggested, and Always / Just once wait for
  a pick (checked on screen with an unknown file type).
- Home's Recent and Favorites columns were laid out per row from their
  text, so they didn't line up; every column now asks for a fixed width.
  Checked on screen.
- `util::content_type` asked FileInfos for an attribute they lacked,
  logging "GFileInfo created without standard::content-type".

### 0.2.9 (2026-10-26)

At the Patrone Sans session's request: the style sheet no longer names a
font family (it said Noto Sans), so File Manager uses the desktop font that
Patrone Settings → Theme → Font sets (Noto Sans or Patrone Sans), still at
9 points; monospace stays Ubuntu Sans Mono. Checked in the headless shell
with GTK reading a test setting directly (`GDK_DEBUG=no-portals`, keyfile
`font-name='DejaVu Serif 11'`): File Manager drew in DejaVu Serif. (Without
`no-portals`, GTK takes the font from the desktop portal, which reads the
account's real setting.)

### 0.2.10 (2026-10-26)

- "When I compress a file, and click exit or out of it with the mouse it
  should commit. Its still highlighted": the name editor (after Compress,
  New folder, New file and F2) only closed when it lost keyboard focus, and
  a click on a file, on empty space or on the title bar doesn't move focus.
  Now a press anywhere in the window outside the editor, or the window
  losing focus, keeps the name as typed; Enter keeps it and Esc cancels, as
  before (`filecell.rs`, `RenameHooks`: a capture-phase click on the window
  and an `is-active` handler, both removed when the editor closes). The
  click still does what it was for: clicking another file commits and
  selects that file. Checked with real clicks in the headless shell:
  Compress then a click on another file and on the status bar, New folder
  then a click on the title bar, New text document then another window
  opening, F2 then a click on another File Manager window, F2 then Esc, F2
  then Enter.
- A new archive showed as "Empty document" with a blank icon and 0 bytes:
  GtkDirectoryList re-reads a file on created, renamed and
  attribute-changed reports but ignores content changes, and the archive
  was created empty and filled in place. It's now built as a hidden
  `.NAME.part` and hard-linked to its name when finished (a link fails
  rather than replace a file that took the name meanwhile; drives without
  links get a checked rename), so it appears whole. Copies don't have this
  problem: GIO sets their times at the end, an attribute change. Files that
  other programs write in place still show their old size until a refresh.
- A renamed or pasted item that sorts out of view was scrolled to one row
  short: a row that has just arrived has no height yet. The view scrolls
  again 100 ms later (`check_pending`).

Next: the user's verdict on 0.2.10, then the gaps in the README's "Not done
yet" (group headers in the icon views, the command bar folding into "See
more" when narrow, Gallery's timeline), and FileManager1.

## 9. Patrone Settings (patrone-settings)

Asked for 2026-10-26: Patrone's own settings, first the taskbar's size
(small, medium or large; medium was the size then), then where the
minimize, maximize and close buttons go, shown in pictures (top right or
top left, "a user option" that themes don't change), and themes: "a
handful of options" with an Apple-like feel, the current one, an XP-like
and a Windows 11-like one, not named after either. `settings/README.md`
has the pages, the four themes (Patrone, Modern, Classic, Studio) and how
they reach apps. It's a window of its own, GTK 4 in Rust without
libadwaita (the VM has no libadwaita headers): GNOME Settings takes no
pages from other programs.

Checked in the throwaway shell: see the README. Not checked: on the real
desktop, the theme in apps that aren't GTK (Chromium's own title bar), and
Studio's and Classic's look in GTK 3 apps.

**0.2.0** adds the taskbar's layout and Start icon to the Taskbar page, and
Royal (section 7), and two options for GNOME Settings' Patrone page:
`--set-theme` and `--export-previews` (the pictures, drawn at build time
without a display and saved through a GDK texture, since this cairo has no
PNG writer). The title bar buttons' picture lost its title word, which ran
into the buttons.

**The Patrone page in GNOME Settings** (the user, 2026-10-26: "I am not
seeing the Patrone settings in the settings menu"; they chose a Patrone
build of GNOME Settings over shortcuts or making Patrone Settings the main
Settings). `gnome-settings/` has the page (C, libadwaita, GPL-2.0-or-later
like GNOME Settings), the patches that register it and hide the Telemetry
page, and `build.sh`, which builds Ubuntu's source with them as
`gnome-control-center` `1:50.3-0ubuntu0.2patrone1`. Built on the VM once
the user had installed the build dependencies: the patches applied
cleanly, the page compiled with no warnings, and `dh_missing` found nothing
left out. Checked by running the built Settings, unpacked, in the throwaway
shell: the Patrone row after Ubuntu's Desktop page, every section with its
pictures and the current values picked; clicking Large, Orb, Top Left and
Royal set `size`, `start-icon`, `button-layout` and `theme` (and Royal's
`@import` in `gtk.css`, through `patrone-settings --set-theme`); setting
the layout elsewhere moved the page's pick to Top, while the taskbar moved
to the top and Settings' own buttons to the top left; Privacy & Security
has no Telemetry row. (There the sidebar said "Ubuntu Desktop": the
unpacked package's own desktop file came first. Installed, Ubuntu's file is
diverted by `patrone-branding`.) Not checked until installed: the real
desktop.

**Title bar button styles (patrone-settings 0.3.0, GNOME Settings
`...patrone2`, File Manager 0.2.3).** The user found "the icons in the
buttons are inconsistent". In the throwaway shell, five apps under each
theme showed why: libadwaita apps (Text Editor, Calculator), plain GTK 4
apps that Ubuntu's Yaru styles (Patrone Settings, Patrone Terminal), GTK 3
apps (Disks) and File Manager (its own Windows 11 buttons) each kept parts
of their own look; under Classic and Modern Patrone Settings' buttons
vanished, and under Studio Terminal's were big grey circles and Disks'
disappeared. Asked which look the Patrone theme should have, the user said
to make it configurable, with descriptive names, not "windows" or "gnome".
So the style is its own setting (`org.patrone.desktop title-buttons`):
Wide, Round, Dots, Glossy, or the theme's own (the default). Each style's
sheets set every property of the buttons and their symbols in every state
(Yaru gives the symbols a 30-pixel minimum, which showed as the big
circles), the themes keep only their title bars, File Manager uses GTK's
window controls, and `patrone-settings --apply` runs at every sign-in so
accounts get the sheets (and move off Modern's and Studio's, which went
away). Checked in the throwaway shell: each style in those five apps, the
same in each; File Manager in Wide and Dots, top right and top left; the
page's Style section, a click writing the setting and both `gtk.css`
files, and a change made elsewhere moving the pick.

Also found: in a window of ordinary height the page's pictures were nearly
invisible (a flow box gives a picture that can shrink its minimum height,
none); they now have a height of their own. And the taskbar (0.3.1) shows
"The taskbar was updated", with Sign out, when a package update replaces
its files, since the running desktop keeps the old code until the next
sign-in; checked by renaming a copy of `metadata.json` over it, as dpkg
does.

## 10. Typeface and Settings' Ubuntu wording (patrone-branding 0.1.3, 0.1.4)

Asked for 2026-10-26: a font "closer to what Microsoft has and closer to
what we added to MailCraft", everywhere, the browser included. MailCraft
draws with Windows' Segoe UI on Windows and its bundled Inter elsewhere,
so 0.1.3 made it Inter (Ubuntu's `fonts-inter`). Before installing it, the
user asked for "the same font as the file explorer" as the system default:
File Manager 0.1.0, which they had installed, set its text in Noto Sans. So
0.1.4 makes it Noto Sans (`fonts-noto-core`, already a dependency) and
drops `fonts-inter`, and File Manager 0.2.0 names Noto Sans again (it had
switched to Inter first, falling back to Noto Sans). How it's set
everywhere: `branding/README.md`, "Typeface". The system's size stays 11
pt, GNOME's; the File Manager's 9 pt is Windows' size for its own lists.

Checked for 0.1.4, on the VM: the GNOME defaults compile with Ubuntu's
schemas (`--strict`) and give Noto Sans 11 and Noto Sans Bold 11 for titles
under both desktop names; fontconfig with the rule first and Ubuntu's own
after (as installed) gives Noto Sans for sans-serif, system-ui and Segoe
UI, Bold for Segoe UI Semibold (Ubuntu alone gives Regular), and Noto Sans
Display for Segoe UI Variable Display; Settings in the throwaway shell
with the font set draws in Noto Sans. Ubuntu's Noto Sans has only regular
and bold, so titles are bold rather than semibold. Not checked until
installed: Chromium itself (its snap reads `/etc/fonts/snap-override`, per
the GNOME platform snap's `desktop-launch`), and the login screen.

Findings: `fc-match sans-serif` reads the hyphen as the start of a size
and asks for "sans"; test with `fc-match :family=sans-serif`. Chromium uses
a font substituted for one asked for by name only through a strong
(`binding="same"`) alias. Ubuntu's `56-language-selector-prefer.conf` puts
Noto Sans first for sans-serif, so Patrone's rule is numbered 05.

The user also saw "Ubuntu Desktop" in Settings: GNOME Settings names each
page from its desktop file, so `patrone-branding` puts its own
`gnome-ubuntu-panel.desktop` ("Desktop") in place of Ubuntu's. The rest of
Settings' Ubuntu wording is Canonical's reporting (section 11).

## 11. Nothing to Canonical (patrone-desktop)

Asked for 2026-10-26: drop whoopsie and ubuntu-insights "so it sends nothing
to Canonical". `patrone-desktop` 0.1.0 is Patrone's desktop metapackage:
Ubuntu's desktop and `patrone-branding`, Patrone's apps recommended, and the
programs that report to Canonical removed. `desktop/README.md` lists what
reports to Canonical, what happens to each, and what else still reaches
Canonical's servers.

Then the user chose to move the rest off Canonical too: "point the network
check and the clock at non-Canonical servers, turn off Ubuntu release-upgrade
offers, and skip the Ubuntu Pro page in first-login setup". That's 0.1.1:
NTS servers from Cloudflare, Netnod, PTB and SIDN Labs; GNOME's
`nmcheck.gnome.org`; `Prompt=never` in a `release-upgrades.d` drop-in; and
`ubuntu_pro` in setup's skip list (`desktop/README.md` has how each works).

Checked for 0.1.1 on the VM, without installing it:

- `NetworkManager --print-config` with Ubuntu's `conf.d` and Patrone's file
  gives GNOME's URI, which answers with NetworkManager's default
  "NetworkManager is online".
- A copy of chronyd run with `-Q` (it measures and doesn't set the clock; a
  copy, because AppArmor keeps the real one to its own folders) read
  Patrone's sources and `nocerttimecheck 1`, and all four servers answered
  over NTS, the clock 0.1 ms off. `openssl s_client` completed each
  server's NTS key exchange with a valid certificate.
- Ubuntu's `check-new-release`, with Patrone's drop-in bound into
  `release-upgrades.d` by bwrap and no network: "Prompt is set to never so
  upgrading is not possible", before any download. Without it, it went
  looking.
- First-login setup with 0.1.1's `vendor.conf` bound over
  `/etc/gnome-initial-setup/vendor.conf`: Welcome, Location Services,
  Appearance, Ready to go. No Ubuntu Pro or "Help Improve Ubuntu" page, and
  no password prompt.
- `apt-get -s install` of 0.1.1 over the installed 0.1.0 upgrades it and
  removes nothing.

Not checked until installed: the postinst's chrony steps (debconf, ucf,
removing Ubuntu's pools file) and the restarts, which need root.

Checked after the user installed 0.1.0: whoopsie, ubuntu-insights,
ubuntu-report and apport-gtk are gone (their settings files kept, as
`apt remove` does), apport stays, and systemd loads both Ubuntu Pro
drop-ins. The user's session still lists ubuntu-insights' two timers, whose
units are gone, until the next sign-in.

Checked on the VM for 0.1.0, before it was installed:

- `apt-get -s install` (a simulation) removes exactly apport-gtk,
  ubuntu-insights, ubuntu-report and whoopsie. `ubuntu-desktop-minimal`,
  Settings, `ubuntu-dev-tools` and apport stay. Without apport in
  Recommends, apt would have left apport to `apt autoremove`.
- First-login setup in the throwaway shell, with Patrone's `vendor.conf`
  bound over Ubuntu's by bwrap: Welcome, Ubuntu Pro, Location Services, then
  Appearance. With Ubuntu's, "Help Improve Ubuntu" comes after Location
  Services. (Setup reads `/etc/gnome-initial-setup/vendor.conf` before
  `/usr/share`'s, per its source.)
- The Ubuntu Pro drop-in passes `systemd-analyze verify`, and its condition
  fails on the VM, which isn't attached to Ubuntu Pro.

Not checked until installed: the removal itself, and systemd applying the
drop-ins after `daemon-reload`.

Findings:

- Nothing had been sent from the VM. There's no Insights consent file, so
  its weekly upload had nothing to send, and the crash reports in
  `/var/crash` aren't marked for upload. `/etc/whoopsie` did have
  `report_metrics=true`.
- Ubuntu builds Settings' Telemetry page (Privacy & Security → Telemetry:
  "Send error reports to Canonical", system reports) with whoopsie and
  Insights support compiled in, and shows it whether or not they're
  installed (`debian/patches/ubuntu/telemetry-Repurpose-diagnostics-panel-for-all-telemetry-a.patch`).
  Settings depends on `whoopsie-preferences` and `libinsights0`, so those
  stay. Hiding the page takes a Patrone build of Settings.
- Neither Settings nor first-login setup calls libinsights' upload. They
  record consent and build reports locally, and ubuntu-insights' timer
  uploaded them.
- `motd-news` is already off: there's no `/etc/default/motd-news`, so
  `ENABLED` isn't 1. But since `patrone-branding` moved `50-motd-news`
  aside, `motd-news.timer` starts a script that isn't there, so
  `motd-news.service` will fail twice a day. Fix in `patrone-branding`: a
  drop-in with `ConditionPathExists=/etc/update-motd.d/50-motd-news`.
- First-login setup still says Ubuntu: the Welcome page shows 26.04's
  raccoon, the Appearance page says "Choose how Ubuntu looks", and the last
  page offers "Ubuntu's App Center". A job for `patrone-branding`.
- Setup reads `/etc/gnome-initial-setup/vendor.conf` and ignores
  `/usr/share`'s when it's there, so once `patrone-desktop` is installed,
  test a new skip list by binding it over the `/etc` file.
- `shell/gnome/tests/headless.sh` uses one fixed folder
  (`/tmp/patrone-taskbar-test`) and display, so two sessions running it at
  once replace each other's test shell (it happened on 2026-10-26). Run a
  copy with its own `base` and `display` meanwhile; the script could take
  them from the environment.
- A process started through `bwrap` survives `headless.sh close-apps`,
  which kills only bwrap. Later launches of the same app then just raise
  the old window.
- `firefox` is only recommended by `ubuntu-desktop-minimal` 1.570.4
  (section 4 says it depends on it), so a Patrone package could remove it
  the same way.
- `/var/crash` holds a root-only `patrone-remote-desktop.0.crash` from 10:03
  that day: apport's name for a package that failed to install or
  configure. Read it with sudo.

Still reaching Canonical, by choice: the update archive and the Snap Store
(Chromium).

## 12. Wallpaper family (patrone-branding 0.1.4)

Asked for 2026-10-26: "a few new versions of the background, one that is a
light mode version, and several that have no logo, several that have small
logos, and maybe a few other interesting versions". Eleven variants of
wallpaper 01, all in Settings → Appearance (`branding/README.md`, "The
family", has previews and how they're made): light; no logo (dark, light,
and Clean without the corner details); a small P (dark and light), a small
badge; Midnight, Blue and Graphite color schemes; and Watermark, a giant
faint P. The light ones are paired with their dark ones, and the default
is now a pair too: new accounts start in Ubuntu's light style, so they see
the light version first.

`tools/make_wallpapers.py` makes them from the original, on Windows: the VM
has no NumPy or OpenCV. Its parts come from `tools/wallpaper_layers.py`,
taken out of `upscale_wallpaper.py`, which still makes the 8K wallpaper
byte for byte (checked).

The user installed 0.1.4 and asked to "remove the ubuntu images". GNOME
Shell depends on `ubuntu-wallpapers` (Ubuntu's patch, for its default
wallpaper), so 0.1.5 provides it, and conflicts with and replaces
`ubuntu-wallpapers` and `ubuntu-wallpapers-resolute`: 63 MB of images, and
both lists in Settings. Only GNOME's default-wallpaper settings point at
those files (Ubuntu's override, which Patrone's beats); first-login setup
doesn't use them. `apt-get -s install` of 0.1.5 removes exactly those two
packages. An account whose wallpaper was one of Ubuntu's gets a plain color.

Checked: the package builds (63 MB, 33 of them the 8K original), every file
the list names is in it, and the list parses; in the throwaway shell,
Settings → Appearance shows all nine entries, the pairs as split tiles,
with the default pair ticked and the style previews in light and dark.
Not checked until installed: switching between them on the real desktop.

Findings:

- JPEG brings the dark gradients' banding back even at quality 97 with
  full-resolution color: it smooths away the grain that dithered them.
- An 8×8 ordered dither stops banding better than random grain (its
  average stays 0.05 levels from the true gradient, against 0.6) and
  makes a 4K PNG 2.9 MB instead of 8.
- Painting the logo out with inpainting leaves its outline, and the
  bottom-left details' box overlaps the logo's, so its left rule counted
  as a detail. The cleared background refills the logo's whole area from
  its surroundings, and the details leave the logo's box alone.

## 13. Patrone Unpack (patrone-unpack)

Asked for 2026-10-26: "a minimal flatpack app that can be used to uncompress
and install things. Make it default, we removed ubuntus". Asked what
"flatpack" meant, the user chose a normal Patrone `.deb` app that also
installs Flatpak apps (Patrone had no Flatpak at all). "Ubuntu's": Ubuntu
26.04 ships no archive app (File Roller was never installed); archives
opened in Nautilus, which File Manager replaced, and `.deb` packages in
Canonical's App Center (`snap-store`). Patrone Unpack (`unpack/`, its
README has the details) does both: an archive opens as a folder with
Extract all (Windows' dialog) and Extract here; a `.deb` or Flatpak file
opens with what it is, what else installing it changes (apt's simulation),
and Install. `patrone-default-apps` 0.1.3 makes it the default for all of
them, with Nautilus and App Center as fallbacks.

Then the user's actual need came through (via the main session): a
downloaded `.flatpak` bundle (PdfCraft 0.9.0, in `~/Downloads`) showed
GNOME's "No Apps Available" with a "Find more in App Center" button. 0.1.1
installs Flatpak apps for the account that opens them (`--user`, no
password), shows a bundle's icon, version, developer, size and permissions,
sets Flathub up for the account (from Flathub's own `.flatpakrepo`, shipped
in the package) when the runtime isn't installed and comes from there, and
depends on `flatpak`.

How: Rust and GTK 4; libarchive for every archive format, rar included
(its own free code, no `unrar`), linked by its run-time name; extraction
written by Unpack itself so nothing lands outside the destination; `.deb`
installs through `pkexec` and a root helper running apt, under a polkit
action of its own (`auth_admin`, every time); Flatpak through `flatpak
install --user`, which needs no password.

Checked on the VM: the package builds and its build runs 19 tests (real zip,
7z and tar archives made with libarchive's writer, paths that try to leave
the destination, passwords, apt's output, Flatpak files); Clippy and
rustfmt are clean; the desktop file validates; `gio mime` picks Unpack for
every type with the new list and the session's paths. In a throwaway GNOME
Shell with real clicks: the archive window, Extract all into the suggested
folder (and the folder opened in File Manager), Extract here naming
"(2)", the replace/skip question, opening a file from the archive,
`--extract-here`, a `.deb` up to polkit's password prompt and Cancel, a
`.deb` apt can't install, a `.flatpakref` without Flatpak, light and dark.
For 0.1.1, the user's PdfCraft bundle **installed end to end** through the
window, with Ubuntu's Flatpak unpacked into a scratch folder and scratch
user and system folders (nothing reached the user's account; all deleted
after): Flathub added, PdfCraft with its runtime and graphics drivers
installed, progress, and Open started PdfCraft. Not checked: a real `.deb`
install (needs the password), the installed policy's message, a Flatpak
install with the installed package's own paths, `.flatpakref` installs, rar
files.

Findings:

- libarchive's mtree reader takes any line of text for an archive: a
  gzipped one-line text file listed as an "mtree" archive holding an empty
  file. Unpack turns on the other formats one by one instead of
  `archive_read_support_format_all`.
- GTK 4.22's Default theme stayed light in a dark session although
  `gtk-interface-color-scheme` was dark (an app style sheet bound to it went
  dark). Unpack sets `gtk-application-prefer-dark-theme` from it. Other
  plain GTK 4 apps on the Default theme may need the same.
- `apt-get --simulate install ./file.deb` works without root and lists
  `Inst`/`Remv` lines; apt 3's solver adds "Reached two conflicting
  assignments" and its reasoning to the error, which Unpack leaves out when
  the unmet dependency is already listed.
- Ubuntu's Flatpak lets `sudo` members install without a password only in
  local sessions (with a seat). Patrone's seatless desktops get Flatpak's
  defaults for `--system`: an administrator's password. Hence `--user`.
- Flatpak bundles hold a `runtime-repo` (Flathub's `.flatpakrepo` for
  PdfCraft), icons as PNGs (`icon-64`, `icon-128`), the metadata keyfile and
  gzipped AppStream XML in the superblock's first dictionary; PdfCraft's has
  no `installed-size`.
- Flatpak's CLI can run unpacked from its `.deb` for tests
  (`LD_LIBRARY_PATH`, `FLATPAK_SYSTEM_DIR`, `FLATPAK_USER_DIR`), but even a
  read-only `flatpak list` fails until the system folder has an OSTree repo
  (`repo/config` and its folders), and `--system` changes go through the
  system helper over D-Bus.
- **Left for later:** GNOME's "Open With" dialog (xdg-desktop-portal-gnome
  50.0) has "Find more in App Center", which starts
  `snap-store_snap-store.desktop`, else GNOME Software. App Center is still
  installed on the VM, so it works there; without it the button fails
  ("Failed to start App Center").
- To reach a throwaway shell's polkit agent, start the app from inside the
  shell (`Gio.Subprocess` through `headless.sh eval`): pkexec asks the agent
  of its own login session, and each SSH command is a session of its own.
- File Manager has its own Extract All for zip and tar; it could hand 7z,
  rar and Extract here to `patrone-unpack --extract-here` /
  `--extract-to DIR` (its session was told).

## 14. Patrone Sans (fonts-patrone-sans)

Asked for 2026-10-26: a font that looks and feels mostly like the current
one (Noto Sans), changed by best practice for a system font, with a choice
between the two in Patrone Settings. Done:

- **Patrone Sans 1.000** (`fonts/`, see its README and FONTLOG): Noto Sans
  2.015 from its own sources, with l tailed (the foot of Noto's t) and the
  slashed zero (Noto's own) made the default, so I, l, 1, | and O, 0 are
  told apart. One weight axis (100 to 900), upright and italic. Spacing,
  line height (exactly Noto's), features and languages unchanged; figures
  were already tabular. Reproducible: `fonts/build.sh` gives the same bytes
  each time. OFL 1.1 (Noto's license has no Reserved Font Name; "Noto" is a
  trademark, so the name is Patrone's own).
- **fonts-patrone-sans 1.0.0** installs it in
  `/usr/share/fonts/truetype/patrone-sans/`.
- **The Font choice**: `patrone-settings` 0.3.2 (Theme page) and GNOME
  Settings `...patrone4` (Patrone page). It changes the family of
  `font-name`, `document-font-name` and `titlebar-font`, keeping sizes;
  the monospace font stays. Each card shows text in its own font (not a
  picture: the package build has no Patrone Sans to draw with). A font
  that isn't installed can't be picked.
- **The default stays Noto Sans.** Recommended: make Patrone Sans the
  default once the user has tried it on their desktop. It changes only l
  and 0, keeps every line the same height, and brings the medium and
  semibold weights Ubuntu's Noto Sans lacks. That's the user's call. Making
  it the default touches `branding/` (another session's): the font lines
  of `system/schemas/90_patrone.gschema.override` and
  `system/gdm/95-patrone-settings` (login screen), the aliases in
  `system/fontconfig/05-patrone-fonts.conf` (sans-serif, system-ui, Segoe
  UI, for Chromium and web pages too), and a dependency of
  `patrone-branding` on `fonts-patrone-sans`. File Manager names "Noto
  Sans" in its style sheet, so it keeps Noto Sans whatever is picked; its
  session was asked to follow the system font instead.

Checked: fontbakery 1.1.0's universal checks (5 fails, all inherited from
Noto Sans 2.015, which fails the same checks built the same way; see
`fonts/README.md`); the glyph set and GDEF classes equal Noto's; the
comparison sheet (`fonts/comparison.png`, Noto Sans and Patrone Sans at 9,
10 and 11 pt, light and dark, as GTK draws them); `cargo test` (12 tests,
3 new); GNOME Settings revision 4 compiled with no warnings in the page;
both settings apps in the throwaway shell: the cards, picking Patrone Sans
and Noto Sans in each (the four keys checked after each click), Patrone
Settings' own text and the taskbar's clock switching at once, the font
cards in light and dark, and Patrone Settings without Patrone Sans (its
card greyed and naming the package; clicking it changed nothing).
`fonts-patrone-sans` passes lintian with no tags. Not checked: installed
for real, the login screen, Chromium and web pages (they follow fontconfig's
sans-serif, still Noto Sans), apps other than these.

Findings:

- The VM's python3 has no pip or venv module (`python3-venv` isn't
  installed). `python3 -m venv --without-pip` and PyPA's `get-pip.py` make
  a venv without sudo; `fonts/build.sh` does that itself.
- `glyphs2ufo` needs `--generate-GDEF --write-public-skip-export-glyphs` to
  match `fontmake -g`; without them the font kept 36 unexported glyphs and
  called spacing accents marks.
- With GNOME's default "slight" hinting, FreeType ignores a TrueType font's
  instructions: Ubuntu's Noto Sans and a copy without its instructions drew
  identical pixels (and different ones at "full"). So Patrone Sans ships
  without instructions, and FreeType autohints it at every setting.
- A throwaway shell (`headless.sh`) finds fonts added to its
  `data/fonts` only in apps started afterwards, not in the shell itself.
  Exporting `FONTCONFIG_FILE` (a file that includes `/etc/fonts/fonts.conf`
  and adds a `<dir>`) before `headless.sh start` gives the shell and its
  apps the fonts without touching the account.
- libadwaita draws buttons' text in bold, so text inside a card button
  needs its weight set to show a font's regular weight.

## 15. Patrone Task Manager (patrone-task-manager)

The user asked (2026-10-26) for "a more windows like task manager,
replacing the current one", which was Ubuntu's Resources. `task-manager/`
is a Rust and GTK 4 app laid out like Windows 11's Task Manager: Processes
(apps, background processes, system processes, each with its processes
under it), Performance, App history, Startup apps, Users, Details and
Services. `task-manager/README.md` has how it works and what was learned.

- **Replacing Resources:** `patrone-task-manager` has `Conflicts` and
  `Replaces: resources`. Resources is only recommended by
  `ubuntu-desktop-minimal` (`apt-cache rdepends --installed resources`), so
  `apt-get install -s` of the package removes Resources and nothing else.
  `patrone-desktop` 0.1.2 recommends `patrone-task-manager`.
- **Ctrl+Shift+Esc:** a custom shortcut in a Patrone dconf database
  (`/etc/dconf/db/patrone.d/`, compiled by `dconf update` in postinst, as
  ibus does) read through a vendor profile,
  `/usr/share/dconf/profile/user`. Not a gschema override: a custom
  shortcut's values live at a path of its own, which an override can't
  reach, and overriding the shared `custom-keybinding` schema would start
  every new shortcut made in Settings as Task Manager's. Nothing else
  Patrone ships sets `custom-keybindings`. An account with custom
  shortcuts of its own keeps its own list, without this one.
- **The taskbar** already lists `org.patrone.TaskManager.desktop` first in
  its "Task Manager" menu item (main session).

Checked, all in a private copy of the throwaway GNOME Shell
(`/tmp/patrone-tm-test`, display `wayland-patrone-tm`, so other sessions'
use of the shared one was untouched), with clicks and keys from the
shell's virtual devices:

- Every page, with screenshots: Processes (sections, totals in the column
  headers, shading, a terminal's shells under the terminal), Performance
  (CPU, memory, two disks, Ethernet; this VM has no graphics card that
  reports how busy it is, so no GPU page), App history (from the journal),
  Startup apps, Users (two accounts), Details, Services.
- End task on a stand-in app (a `sleep` in an `app-gnome-…` scope) ended
  it at once; End task on a plain process asked first, and ended it. The
  right-click menu, search, Properties and Run new task (it started the
  program) worked.
- A Startup apps switch wrote the per-account copy with `Hidden=true` and
  removed it again (in the test shell's own config folder).
- Stop on a throwaway user service (`systemd-run --user`) stopped it, from
  a copy run on the user's session bus (the test shell's own bus has no
  systemd). System services' Start, Stop and Restart weren't tried: they
  ask for the user's password.
- **Ctrl+Shift+Esc** opened Task Manager: the keyfile compiled with
  `dconf compile`, a profile found through `XDG_DATA_DIRS` (as
  `/usr/share/dconf/profile/user` will be, but pointing at a test file
  with `file-db:`), and `gsd-media-keys` running in that shell. The
  installed `system-db:patrone` and postinst's `dconf update` need root,
  so they weren't tried.
- The package builds with `build.sh -d` (9 tests pass) and with Rust 1.93,
  Ubuntu's.
- Dark: checked with the user's real appearance settings (a copy on their
  session bus): consistent after the stylesheet change below.
- Its own use: about 4% of one core on Processes, 3% on Performance, in
  this VM where GTK draws in software.

Not checked: the installed package itself, the shortcut in the user's real
session, a graphics card's page, and per-process network on a busy
browser (UDP and QUIC aren't counted; the README says why). Once, running
on the user's session bus, the app quit while switching to "Your
services" with nothing in its log and no kernel trap logged; it didn't
happen again in ten more tries, with gdb and without.

Findings (more in `task-manager/README.md`):

- GTK lists keep their place by a row and follow it as rows move, so a list
  re-sorted every second wandered off and hid the first section's header.
  Making GTK re-anchor right after rows change crashed it with a divide by
  zero (`libgtk-4.so.1.2200.4`); `table.rs` instead keeps a list that's at
  the top there, safely.
- A `PopoverMenu` made and opened at once is measured without its
  separators and cuts off its last items: make each menu once.
- In the user's session GTK draws its parts dark while
  `@media (prefers-color-scheme: dark)` rules don't apply (theme
  `Yaru-blue-dark`, which plain GTK 4 apps can't load), so a stylesheet
  keyed to that media query showed light panels with light text. Task
  Manager's tints come from `currentColor`. File Manager's stylesheet uses
  that media query; worth checking it in the user's session.
- At 18:31:03 on 2026-10-26 the dev install of Patrone Terminal was
  removed (`~/.local/bin/patrone-terminal`,
  `~/.config/autostart/org.patrone.Terminal.desktop` and the launcher in
  `~/.local/share/applications`, within 40 ms), leaving the package's.
  Not Task Manager's doing; noted in case it wasn't meant.

## Suggested order

The user's quick steps first (bash login shell, RDP checks, trying the
compositor, the desktop design). Then the compositor's next features and
milestone 3, the Chromium defaults package, then Patrone's own RDP path.
