# Patrone Remote Desktop

Lets Windows' Remote Desktop Connection (and other RDP clients) open a
Patrone desktop. Accounts in the `remote-desktop` group sign in with their
own Linux username and password, which PAM checks on every connection.
Nothing keeps a copy of the password. Each account gets its own desktop,
started when it signs in, sized to the client's window, and never locked or
blanked while connected. Packaged as `patrone-remote-desktop`.

## Turning it on

It's off after installing. An administrator turns it on, and adds the
accounts that may sign in:

```
sudo patrone-remote-desktop enable
sudo adduser NAME remote-desktop
```

Then connect from Windows with `mstsc /v:HOSTNAME`. Remote Desktop
Connection asks for a username and password: the Linux account's. The first
time, it also asks you to trust the computer's certificate, which is
self-signed. `patrone-remote-desktop status` shows whether it's on and who's
in the group; `patrone-remote-desktop disable` turns it off.

## How it works

```
mstsc ──TLS──▶ freerdp-proxy :3389 ──NLA──▶ the account's GNOME Remote Desktop
               (user patrone-rdp)            :20000+uid, on localhost
                 │  patrone-login plugin
                 ▼
               auth.sock ──▶ auth helper (root): PAM, then prepare-session
```

1. **`freerdp-proxy`** (FreeRDP's proxy, from Ubuntu) listens on port 3389 as
   the unprivileged `patrone-rdp` user (`patrone-remote-desktop.service`,
   config in `/etc/patrone-remote-desktop/proxy.ini`). It speaks TLS without
   NLA to clients, so the client sends the username and password inside the
   encrypted connection.
2. **The `patrone-login` plugin** (`src/patrone-login.c`, loaded by the proxy
   and required: without it the proxy won't start) takes them before the
   proxy connects anywhere, and hands them to the sign-in helper over
   `/run/patrone-remote-desktop/auth.sock`, which only `patrone-rdp` can open.
   It then clears the password from the proxy.
3. **The sign-in helper** (`src/auth.c`, run as root by systemd for each
   request through `patrone-remote-desktop-auth.socket`) checks the account
   with PAM (`/etc/pam.d/patrone-remote-desktop`: in the `remote-desktop`
   group, UID 1000 or above, the account's own password, the account not
   locked or expired). On success it runs `prepare-session`.
4. **`prepare-session`** (`libexec/prepare-session`, root) gets the account's
   desktop ready: its own GNOME Remote Desktop on port 20000 + UID, set up
   through `grdctl --headless`, whose credentials are a random secret kept in
   `/var/lib/patrone-remote-desktop/secrets` (never the account's password);
   then GDM's headless desktop session for the account
   (`gnome-headless-session@USER.service`), unless it's running already. It
   answers with the port and the secret.
5. **The proxy** connects to `127.0.0.1` on that port with NLA, signing in
   with the secret, and passes the session through: display, input,
   clipboard, audio, and resizing with the client's window.

`libexec/no-lock` runs at every GNOME login (from `/etc/xdg/autostart`). In a
session with no seat, which is how these sessions run, it turns off the
screen lock and blanking after saving the account's own settings, and puts
them back at the next login on the machine's own screen.

## Things to know

- **NLA is off towards clients.** To check the password against the account
  when someone connects, the server has to receive it. With NLA it couldn't:
  NLA's first step needs the server to hold a copy of the password (its NT
  hash) already. The password travels inside TLS instead, as it does with
  xrdp. Kerberos would be the way to keep NLA without a copy, for machines
  joined to a domain.
- **One desktop per account.** GNOME runs one desktop session per account,
  so an account signed in on the machine's own screen can't also sign in
  remotely; the proxy refuses it ("busy" in the journal). Log out there
  first.
- **The login keyring stays locked.** Nothing unlocks it when the desktop
  starts, so apps that keep passwords there may ask for it.
- **Each account's GNOME Remote Desktop listens on all interfaces**, on its
  own port, because it has no setting to listen on localhost only. It only
  accepts the account's random secret.
- **Logs:** `journalctl -u patrone-remote-desktop` (the proxy and plugin) and
  `journalctl -t patrone-remote-desktop-auth` (sign-in checks and
  `prepare-session`).

## Building and testing

```
make           # the plugin and the sign-in helper
make check     # tests that need no root and no real password
```

They need `freerdp3-dev`, `libpam0g-dev` and `pkgconf`. `make check` loads
the plugin and plays the proxy's part against a stand-in helper
(`tests/fake-auth.py`): no credentials, a wrong password, a correct one, an
account signed in elsewhere, a garbled answer, and no helper at all. It also
gives the real helper requests it must refuse. Signing in with a real
password, through a real client, needs the package installed.

The plugin is C because FreeRDP's plugin interface is a C struct of
function pointers with fixed slots, which is awkward to match from Rust.
When Patrone Desktop replaces GNOME, the proxy and the sign-in check stay;
only what they connect to changes.
