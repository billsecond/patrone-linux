# Patrone's desktop

`patrone-desktop` (`packaging/patrone-desktop`) is the package that makes an
Ubuntu install Patrone's desktop. It depends on Ubuntu's desktop
(`ubuntu-desktop-minimal`) and `patrone-branding`, recommends Patrone's apps,
and leaves out everything that reports to Canonical.

| File | Installed as | What it does |
| --- | --- | --- |
| `gnome-initial-setup/vendor.conf` | `/etc/gnome-initial-setup/vendor.conf` | First-login setup skips Ubuntu's "Help improve Ubuntu" page (`ubuntu_insights`), which asks to send system and error reports to Canonical |
| `systemd/ubuntu-pro-only.conf` | `/usr/lib/systemd/system/{apt-news,esm-cache}.service.d/` | Ubuntu Pro's apt news and ESM check, which fetch Canonical's news and offers at every `apt update`, run only on a machine attached to Ubuntu Pro |

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
- **The network check:** NetworkManager loads `connectivity-check.ubuntu.com`
  every few minutes to spot hotel and airport Wi-Fi sign-in pages
  (`network-manager-config-connectivity-ubuntu`). Settings → Privacy &
  Security → Connectivity turns it off.
- **The clock:** chrony sets the time from `ntp.ubuntu.com`, authenticated with
  NTS (`/etc/chrony/sources.d/ubuntu-ntp-pools.sources`).
- **Release upgrades:** update-notifier checks `changelogs.ubuntu.com` for the
  next Ubuntu LTS and offers to upgrade to it (`Prompt=lts` in
  `/etc/update-manager/release-upgrades`).
- **Ubuntu Pro:** first-login setup's Ubuntu Pro page, and Settings, contact
  Canonical only when someone attaches the machine to Ubuntu Pro.

Ubuntu's daily news for the login message (`motd-news`) is already off:
its script runs only when `/etc/default/motd-news` sets `ENABLED=1`, and that
file isn't there.
