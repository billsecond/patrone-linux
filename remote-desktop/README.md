# Patrone Remote Desktop: Windows-style sessions

Packaged as `patrone-remote-desktop`. It makes desktops work the way they do
on Windows:

- **Each account has one desktop**, which keeps running whether or not
  anyone is looking at it.
- **The machine's own screen and Remote Desktop are both views of it.**
  Signing in on the screen (the "Patrone Desktop" login session) shows it
  there; signing in with Windows' Remote Desktop Connection shows the same
  desktop, with the same windows, there.
- **Whoever connects last has it.** A Remote Desktop sign-in takes the
  machine's screen back to the login screen; signing in on the screen
  disconnects the Remote Desktop client. Disconnecting leaves the desktop
  running for next time; logging out ends it.
- **Remote Desktop asks once, for the account's own password**, which PAM
  checks on every connection. Nothing keeps a copy of it. Remote sessions
  never lock or blank the screen.

## Turning on Remote Desktop

The screen part works as soon as the package is installed: choose "Patrone
Desktop" from the gear menu on the login screen (it's the default for
accounts that haven't picked a session). Remote Desktop is off until an
administrator turns it on and adds the accounts that may use it:

```
sudo patrone-remote-desktop enable
sudo adduser NAME remote-desktop
```

Then connect from Windows with `mstsc /v:HOSTNAME`, and give the Linux
account's username and password. The first time, Remote Desktop Connection
also asks you to trust the computer's certificate, which is self-signed.
`patrone-remote-desktop status` shows whether it's on and who's in the group;
`patrone-remote-desktop disable` turns it off.

## How it works

```
mstsc ──TLS──▶ freerdp-proxy :3389 ──NLA──▶ ┐
               (user patrone-rdp)           │ the account's GNOME Remote
                 │ patrone-login plugin     │ Desktop, :20000+uid, serving
                 ▼                          │ its one headless GNOME desktop
               auth.sock ─▶ auth (root):    │ (gnome-headless-session@USER)
                 PAM, then prepare-session  │
                                            │
screen: GDM ─▶ "Patrone Desktop" session ───┘
               console: cage + sdl-freerdp, after console.sock ─▶
               auth --console (root): prepare-session
```

1. **The desktop** is GDM's headless GNOME session for the account
   (`gnome-headless-session@USER.service`), served by the account's own
   GNOME Remote Desktop in headless mode on port 20000 + UID. Its
   credentials are a random secret kept in
   `/var/lib/patrone-remote-desktop/secrets`, never the account's password.
   In headless mode GNOME Remote Desktop accepts a new client and then stops
   the one before it, which is the whole "whoever connects last" behavior.
2. **`prepare-session`** (`libexec/prepare-session`, root) gets it ready:
   sets up the account's GNOME Remote Desktop through `grdctl --headless`,
   starts the session unless it's running, and answers with the port and
   the secret. It answers "busy" if the account is signed in on the screen
   with Ubuntu's plain GNOME session instead, which GNOME can't run beside
   this one.
3. **The screen**: the "Patrone Desktop" login session
   (`/usr/share/wayland-sessions/patrone-desktop.desktop`) runs
   `libexec/console` as the account. It asks for its desktop over
   `/run/patrone-remote-desktop/console.sock`, where `auth --console` takes
   the account from the connection itself (`SO_PEERCRED`), and shows it full
   screen with FreeRDP's SDL client inside `cage`, a one-app compositor.
   When the client is disconnected, the session ends and GDM shows the login
   screen.
4. **Remote Desktop**: FreeRDP's proxy listens on port 3389 as the
   unprivileged `patrone-rdp` user (`patrone-remote-desktop.service`, config
   in `/etc/patrone-remote-desktop/proxy.ini`). It speaks TLS without NLA to
   clients, so the client sends the username and password inside the
   encrypted connection. The `patrone-login` plugin (`src/patrone-login.c`,
   required: without it the proxy won't start) hands them to `auth` over
   `/run/patrone-remote-desktop/auth.sock`, which only `patrone-rdp` can
   open, and clears the password from the proxy. `auth` (`src/auth.c`, root)
   checks them with PAM (`/etc/pam.d/patrone-remote-desktop`: in the
   `remote-desktop` group, UID 1000 or above, the account's own password,
   the account not locked or expired), then runs `prepare-session`. The
   proxy connects to `127.0.0.1` on the answered port with NLA, signing in
   with the secret, and passes the session through: display, input,
   clipboard, audio, and resizing with the client's window.

`libexec/no-lock` runs at every GNOME login (from `/etc/xdg/autostart`). In a
session with no seat, which is how these desktops run, it turns off GNOME's
screen lock and blanking, after saving the account's own settings, and puts
them back at the next login in a plain GNOME session on the screen.

## Things to know

- **The screen goes through Remote Desktop too.** Showing the desktop on the
  machine's own screen means encoding and decoding it locally, so video and
  games are slower than in a plain GNOME session. Patrone's own compositor
  will move a session between the screen and Remote Desktop without that.
  Ubuntu's plain session stays in the gear menu, but it can't be shared.
- **NLA is off towards clients.** To check the password against the account
  when someone connects, the server has to receive it; NLA's first step
  needs the server to hold a copy of the password (its NT hash) already. The
  password travels inside TLS instead, as it does with xrdp. Kerberos would
  keep NLA without a copy, on machines joined to a domain.
- **The login keyring stays locked.** Nothing unlocks it when the desktop
  starts, so apps that keep passwords there may ask for it.
- **The screen doesn't lock by itself yet.** Leaving the machine, sign out
  of the screen's session, or connect from elsewhere.
- **Each account's GNOME Remote Desktop listens on all interfaces**, on its
  own port, because it has no setting to listen on localhost only. It only
  accepts the account's random secret.
- **Logs:** `journalctl -u patrone-remote-desktop` (the proxy and plugin),
  `journalctl -t patrone-remote-desktop-auth` (Remote Desktop sign-ins and
  `prepare-session`), `journalctl -t patrone-remote-desktop-console` (the
  screen's requests).

## Building and testing

```
make           # the plugin and the helper
make check     # tests that need no root and no real password
```

They need `freerdp3-dev`, `libpam0g-dev`, `pkgconf` and `python3`, and
`freerdp-proxy` for the config check. `make check` loads the plugin and
plays the proxy's part against a stand-in helper (`tests/fake-auth.py`): no
credentials, a wrong password, a correct one, an account signed in
elsewhere, a garbled answer, and no helper at all. It also gives the real
helper requests it must refuse, and runs the real proxy on the shipped
config. Signing in for real, on the screen or through a client, needs the
package installed.

The plugin and helper are C because FreeRDP's plugin interface is a C struct
of function pointers with fixed slots, which is awkward to match from Rust.
