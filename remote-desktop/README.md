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
- **Remote Desktop asks once, for the account's own password**, in Remote
  Desktop Connection's own dialog (NLA, as on Windows), and PAM checks it on
  every connection. Remote sessions never lock or blank the screen.

## Turning on Remote Desktop

The screen part works as soon as the package is installed and GDM has
restarted (a reboot): "Patrone Desktop" is the login screen's only session.
Remote Desktop is off until an administrator turns it on and adds the
accounts that may use it.

**In Settings** (Patrone's build of GNOME Settings, `gnome-settings/`):
System › Remote Desktop, whose first tab is Remote Desktop, has one switch,
**Remote Desktop**. Turning it on asks for an administrator's password,
then turns Remote Desktop on and lets the account using Settings sign in.
Then it asks for that account's own password once, which it checks and
records what NLA checks, so the account can connect right away. The tab
then shows what to connect to: the computer's name and IP addresses, the
username, and a connection file to save. Turning it off (an
administrator's password again) turns Remote Desktop off for every
account; the accounts stay in the group, so turning it on again needs no
new password. Under "Other Ways to Connect", the tab says what Desktop
Sharing, Secure Shell (SSH) and GNOME's Remote Login do.

**From a terminal:**

```
sudo patrone-remote-desktop enable
sudo adduser NAME remote-desktop
```

An account added this way can connect once it has signed in on the
computer with its password after joining the group (on the login screen,
with `sudo`, over SSH), or changed its password, or confirmed it in
Settings' Remote Desktop tab: that's when Patrone records what NLA checks.

Then connect from Windows with `mstsc /v:HOSTNAME`, and give the Linux
account's username and password in Remote Desktop Connection's dialog. The
first time, it also asks you to trust the computer's certificate, which is
self-signed. A connection file with the computer and username filled in,
full screen, and resizing with the window:

```
patrone-remote-desktop rdp-file > Patrone.rdp     # copy it to Windows, double-click
```
`patrone-remote-desktop status` shows whether it's on and who's in the group;
`patrone-remote-desktop disable` turns it off.

## How it works

```
mstsc ──NLA──▶ freerdp-proxy :3389 ──NLA──▶ ┐
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
   (`/usr/share/wayland-sessions/gnome.desktop`, see 6) runs `libexec/session`,
   which on a seat runs `libexec/console` as the account. It asks for its desktop over
   `/run/patrone-remote-desktop/console.sock`, where `auth --console` takes
   the account from the connection itself (`SO_PEERCRED`), and shows it full
   screen with FreeRDP's SDL client inside `cage`, a one-app compositor.
   When the client is disconnected, the session ends and GDM shows the login
   screen.
4. **Remote Desktop**: FreeRDP's proxy listens on port 3389 as the
   unprivileged `patrone-rdp` user (`patrone-remote-desktop.service`, config
   in `/etc/patrone-remote-desktop/proxy.ini`). It requires NLA from
   clients, as Windows does, and checks it against
   `/var/lib/patrone-remote-desktop/sam` (root's, readable by `patrone-rdp`),
   which holds the NT hash of each `remote-desktop` member's password. A
   wrong password fails right there, in Remote Desktop Connection's own
   "logon attempt failed". NLA then hands the server the password itself,
   and the `patrone-login` plugin (`src/patrone-login.c`, required: without
   it the proxy won't start) takes it before anything else in the
   connection can overwrite it. Before the proxy connects anywhere, the
   plugin hands the username and password to `auth` over
   `/run/patrone-remote-desktop/auth.sock`, which only `patrone-rdp` can
   open, and forgets the password. `auth` (`src/auth.c`, root) checks them
   with PAM (`/etc/pam.d/patrone-remote-desktop`: in the `remote-desktop`
   group, UID 1000 or above, the account's own password, the account not
   locked or expired), so a hash that still matches doesn't let in an
   account that's been locked or taken out of the group. Then it runs
   `prepare-session`. The proxy connects to `127.0.0.1` on the answered port
   with NLA, signing in with the secret, and passes the session through:
   display, input, clipboard, audio, and resizing with the client's window.
5. **Keeping the hashes current**: a `pam-auth-update` profile
   (`/usr/share/pam-configs/patrone-remote-desktop`) adds `pam_exec` to
   `common-auth` and `common-password`, after the password check succeeds.
   It runs `auth --remember` as root with the password, which writes the
   account's NT hash to the SAM file if the account is in the
   `remote-desktop` group (and drops it if not). That happens at every
   password sign-in (login screen, `sudo`, SSH, Remote Desktop) and every
   password change, so the hash always matches the current password. Screen
   unlocks run as the account itself, not root, and are skipped. The same
   profile runs `auth --screen-sign-in` after a sign-in on the login screen
   (`gdm-password`, `-fingerprint`, `-smartcard`), which gets the account's
   desktop ready before GDM opens the screen's session: GDM refuses to start
   a background desktop for an account that has a graphical session open,
   and the screen's own session would be one.

6. **The login screen**: GNOME's login screen treats any other running
   desktop of the account as a conflict, and offers to end it. Here that
   would end the very desktop the screen is about to join, so the
   `patrone-login@patrone-linux` extension (`login-screen/`, for the login
   screen only, switched on by `/usr/share/gdm/dconf/96-patrone-remote-desktop`)
   leaves desktops without a seat out of that check. Ubuntu's own session
   (`ubuntu.desktop`) is moved aside, since a plain GNOME session started on
   the screen would end the account's desktop. Patrone Desktop is
   `gnome.desktop`, because `gnome` is the session GDM falls back to for an
   account whose own choice isn't valid (like a remembered "ubuntu"); without
   it, GDM falls back to the first session file by name, which is GDM's own
   `gnome-greeter` (a bare GNOME session). The same session runs the
   background desktops (`X-GDM-CanRunHeadless`): `libexec/session` runs the
   viewer when the session has a seat, and Ubuntu's GNOME session
   (`gnome-session --session=ubuntu`) when it has none. A `gnome.desktop`
   from any other package is moved aside too.

7. **Permissions**: polkit, which decides what a session may do without an
   administrator's password, counts a session as local only when it has a
   seat. These desktops have none, so polkit treated them like SSH logins:
   the quick settings' network menu wouldn't open, and shutting down asked
   for a password. `data/patrone-desktops.rules`
   (`/usr/share/polkit-1/rules.d/45-patrone-desktops.rules`) gives a
   graphical session with no seat what a desktop at the machine's own screen
   gets without a password: network control (and, for administrators,
   system-wide connections, as Ubuntu allows), shutting down, restarting
   and sleeping, and mounting drives. It asks logind for the session's type,
   so SSH logins and other text sessions keep the defaults, as does every
   other action. GNOME 50 runs the desktop's shell and services as units of
   the account's own service manager (`org.gnome.Shell@ubuntu.service`, in
   its `session.slice`), outside the login session, so polkit gives them no
   session at all and, until 0.4.4, the rule never matched them: the Wired
   menu still wouldn't open. A process with no session now counts when its
   cgroup is in the account's own `session.slice` and the account's display
   session is a desktop as above. (Another of the account's processes could
   move itself there, but only one the same account already runs.) Since
   0.4.6 so does one in its `app.slice`, where apps opened from the desktop
   and Patrone Software's background service (`gnome-software.service`)
   run, and the rules cover what Patrone Software does: refreshing and
   updating Flatpak apps and apt's lists. Without them a new account's
   first login asked for a password to "update information about software"
   (2026-10-27). Since 0.4.7, installing, removing and updating software
   (Flatpak apps and bundles, apt packages through Patrone Software, snaps)
   asks for a password once, which is remembered for a few minutes and
   covers what the install needs, such as an app's runtimes, as at the
   screen (`auth_admin_keep`). polkit had given each step its own password:
   installing MailCraft asked at least 30 times (2026-10-28). 0.4.6 let
   administrators install with no password, as Flatpak's and Ubuntu's own
   rules do at the screen; the user asked for one.

8. **Settings' switch**: Patrone's GNOME Settings shows its own Remote
   Desktop tab (`gnome-settings/remote-login/`) first in System › Remote
   Desktop wherever `/usr/libexec/patrone-remote-desktop/settings` is
   installed, in place of GNOME's Remote Login tab, whose system daemon
   `enable` turns off (both would listen on port 3389). The tab runs that
   helper through `pkexec`; the helper acts
   only for pkexec's caller (`PKEXEC_UID`), never for an account it's
   given. `data/org.patrone.remote-desktop.policy` picks polkit's action by
   the helper's first argument: `status` and `password` ask nothing,
   `enable` and `disable` need an administrator (`auth_admin_keep`). Each
   allows the same for `any` as for `active`, because polkit counts
   Patrone's seatless desktops as `any`. `status`, `enable` and `disable`
   answer with `on=`, `member=`, `password=` (the SAM file has the
   account's hash) and `others=` lines. `password` hands the password from
   Settings to `auth --confirm`, which checks it with PAM through
   `/etc/pam.d/patrone-remote-desktop`, as a Remote Desktop sign-in does
   (so only a member's password counts), and writes its hash. That's the
   same check `su` makes for an account's own password, which is why it
   needs no administrator. The administrator's password in polkit's own
   dialog can't do it: polkit checks it before `enable` adds the account to
   the group, and the PAM hook keeps hashes for members only.

`libexec/no-lock` runs at every GNOME login (from `/etc/xdg/autostart`). In a
session with no seat, which is how these desktops run, it turns off GNOME's
screen lock and blanking, after saving the account's own settings, and puts
them back at the next login in a plain GNOME session on the screen.

## Things to know

- **The screen goes through Remote Desktop too.** Showing the desktop on the
  machine's own screen means encoding and decoding it locally, so video and
  games are slower than in a plain GNOME session. Patrone's own compositor
  will move a session between the screen and Remote Desktop without that.
  Removing the package brings Ubuntu's own session back to the login
  screen.
- **The SAM file holds password hashes**, as Windows' own account database
  does: NLA's NTLM step can't work without the NT hash. It's root's, readable
  only by the proxy's user, and only for `remote-desktop` members. Like
  Windows' hashes, anyone who gets root can use them for NLA logins
  ("pass the hash"); PAM's check of the real password at every connection
  still applies. Kerberos would avoid the hash, on machines joined to a
  domain. Without NLA, Remote Desktop Connection sends no password at all
  (it expects a sign-in screen from the server), which is why Patrone uses
  NLA.
- **A brand-new member can't connect until a password sign-in** records the
  hash, or the account confirms its password in Settings (which asks right
  after turning Remote Desktop on). Changing a password with `usermod -p` or
  by editing `/etc/shadow` skips PAM, so the next password sign-in brings
  the hash up to date.
- **Ubuntu bug 2171951:** GDM's headless session unit runs as a
  DynamicUser that the system bus can't look up, so on Ubuntu 26.04 it never
  starts ("Failed: The connection is closed"). The package's drop-in
  (`gnome-headless-session@.service.d/patrone-static-user.conf`) runs it as
  the `gdm` account; drop it once Ubuntu fixes the unit.
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
helper requests it must refuse (Settings' password check among them),
checks that Settings' helper won't run without pkexec and that each polkit
action names it and one of its commands, runs the real proxy on the
shipped config, and runs the polkit rule under gjs with polkit stood in
for (`tests/polkit-rule-test.js`). Signing in for real, on the screen or
through a client, needs the package installed.

0.4.5's Settings helper was tried on the dev VM (2026-10-27), installed, as
root the way pkexec runs it, for a throwaway account with a random
password: `enable` added it to the group, a wrong password was refused
and left no hash, the right one wrote exactly one SAM line, and `disable`
then `enable` stopped and started the proxy. Through `pkexec`, `status`
and `password` ran without a prompt while `enable` asked for
authentication.

The plugin and helper are C because FreeRDP's plugin interface is a C struct
of function pointers with fixed slots, which is awkward to match from Rust.
