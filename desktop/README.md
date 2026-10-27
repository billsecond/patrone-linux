# Patrone's desktop

`patrone-desktop` (`packaging/patrone-desktop`) is the package that makes an
Ubuntu install Patrone's desktop. It depends on Ubuntu's desktop
(`ubuntu-desktop-minimal`) and `patrone-branding`, recommends Patrone's apps,
and leaves out everything that reports to Canonical. It also recommends a
few tools everyone gets (the user asked for them, 2026-10-26): `fastfetch`,
`nano` and `vim` (Ubuntu installs only `vim-tiny`), `btop` and `htop`,
`tree`, `ripgrep` (`rg`), `fzf`, `bat` (Ubuntu names its command `batcat`),
`ncdu` and `7zip` (`7z`). Its app store is GNOME Software, with Patrone's
picks (`patrone-software`, `software/README.md`), in place of Canonical's App
Center.

| File | Installed as | What it does |
| --- | --- | --- |
| `gnome-initial-setup/vendor.conf` | `/etc/gnome-initial-setup/vendor.conf` | First-login setup skips Ubuntu's "Help improve Ubuntu" page (`ubuntu_insights`), which asks to send system and error reports to Canonical, and its Ubuntu Pro page (`ubuntu_pro`) |
| `systemd/ubuntu-pro-only.conf` | `/usr/lib/systemd/system/{apt-news,esm-cache}.service.d/` | Ubuntu Pro's apt news and ESM check, which fetch Canonical's news and offers at every `apt update`, run only on a machine attached to Ubuntu Pro |
| `chrony/patrone-nts.sources` | `/etc/chrony/sources.d/` | The clock's servers: Cloudflare (`time.cloudflare.com`), Netnod (`nts.netnod.se`), PTB (`ptbtime1.ptb.de`) and SIDN Labs (`ntppool1.time.nl`), each authenticated with NTS, in place of Ubuntu's `ntp.ubuntu.com` pools |
| `chrony/patrone-nts.conf` | `/etc/chrony/conf.d/` | `nocerttimecheck 1`: a computer whose clock starts far off can still use NTS (below) |
| `networkmanager/30-patrone-connectivity.conf` | `/usr/lib/NetworkManager/conf.d/` | NetworkManager's check for Wi-Fi sign-in pages asks GNOME's server (`nmcheck.gnome.org`) in place of `connectivity-check.ubuntu.com`. It comes after Ubuntu's `20-connectivity-ubuntu.conf`, so it wins |
| `update-manager/patrone.cfg` | `/etc/update-manager/release-upgrades.d/` | `Prompt=never`: nothing offers to upgrade to Ubuntu's next release, or asks `changelogs.ubuntu.com` whether there is one. Patrone moves to a new Ubuntu base itself |
| `session/make-session-mode` | `/usr/libexec/patrone-desktop/` | Writes `/usr/share/gnome-shell/modes/ubuntu.json`: Ubuntu's session mode (moved aside to `ubuntu.json.ubuntu` by preinst) with Patrone's taskbar among the session's own extensions (below). Run by postinst, and again by a trigger whenever `gnome-shell-common` brings a new one; if Ubuntu's file changes shape, or the result isn't valid JSON, Ubuntu's is used as it is |

### The taskbar is part of the session

For the first minute after GNOME Shell starts, a crash makes GNOME turn
`disable-user-extensions` on (`extensionSystem.js`: the
`gnome-shell-disable-extensions` file in the runtime folder), which turns off
every extension but the session mode's own. Patrone's taskbar was one of the
others, so after the crashes of 2026-10-27 GNOME's top bar came back, and
stayed (the user: "I should never see that bar"). As one of the session's
own extensions (0.1.5) it always loads. The other side of that: a taskbar
that crashed GNOME Shell at sign-in would keep doing it, with no safety
switch. `disabled-extensions` still turns it off, over SSH if need be:
`gsettings set org.gnome.shell disabled-extensions "['patrone-taskbar@patrone-linux']"`
in the account's session.

Ubuntu's GNOME Shell doesn't load a session-mode extension from an account's
own folder, so a copy installed there (to try unreleased changes) is ignored:
`shell/gnome/tests/headless.sh` mounts Ubuntu's own mode file in its
throwaway shell so that its copy runs.

### The clock

Ubuntu's chrony keeps its pools in `ubuntu-ntp-pools.sources`, which no
package owns: chrony's postinst writes it with ucf when its debconf question
`chrony/configure_ubuntu_pools_in_sourcesd` is yes (the default), and removes
it when the answer is no. `patrone-desktop`'s postinst answers no and removes
the file the same way; its postrm answers yes and puts the file back.

NTS checks each server's certificate against the clock, so a computer whose
clock starts far off (a dead clock battery) can't set it. Ubuntu gets around
that with `ntp-bootstrap.ubuntu.com`, whose certificate is valid for decades.
Patrone has no such server, so `nocerttimecheck 1` skips the certificates'
dates until the clock is first set. chrony's manual calls that a last resort:
for that first update, an attacker holding an expired certificate's key for
one of these servers could feed it a wrong time.

## What reports to Canonical, and what happens to it

| Package | What it does | Patrone |
| --- | --- | --- |
| `whoopsie` | Sends crash reports to errors.ubuntu.com | Removed (`Conflicts`) |
| `ubuntu-insights` | Collects and uploads system reports, from a weekly timer | Removed |
| `ubuntu-report` | Sends a system report after a release upgrade | Removed |
| `apport-gtk` | The crash dialog whose Send button hands reports to whoopsie | Removed |
| `apport` | Writes crash reports to `/var/crash` | Kept, and recommended so that removing `apport-gtk` doesn't leave it to `apt autoremove`: the reports stay on the machine for debugging, and nothing sends them |
| `whoopsie-preferences`, `libinsights0` | Record the choices in Settings and first-login setup | Kept: Settings and first-login setup depend on them. They write a setting and never upload anything |

All four removed packages are only recommended by `ubuntu-desktop-minimal`,
so removing them leaves Ubuntu's desktop installed.

Settings → Privacy & Security → Telemetry still shows "Send error reports to
Canonical" and the system-report switches: Ubuntu builds that page into
Settings itself, and nothing on the system hides it at run time. With the
programs that send reports gone, the switches do nothing. Hiding the page
takes a Patrone build of Settings.

## What still reaches Canonical's servers

None of these sends reports about the machine. They fetch what the system
needs:

- **Updates:** Ubuntu's archive (`archive.ubuntu.com`, `security.ubuntu.com`).
- **Snaps:** the Snap Store, which keeps Chromium up to date.
- **Ubuntu Pro:** Settings and `pro attach` contact Canonical only when
  someone attaches the machine to Ubuntu Pro.

Ubuntu's daily news for the login message (`motd-news`) is already off:
its script runs only when `/etc/default/motd-news` sets `ENABLED=1`, and that
file isn't there.
