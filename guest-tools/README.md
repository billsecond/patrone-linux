# Patrone's guest tools (Hyper-V and VirtualBox)

`patrone-guest-tools` gives Patrone Linux, running as a virtual machine,
what a Windows guest gets from Hyper-V's integration services and
VirtualBox's Guest Additions. It's in the ISO (the installer's live session
and every install), and `patrone-desktop` recommends it. Each part runs only
under its own hypervisor (systemd's `ConditionVirtualization=microsoft` or
`oracle`), so a computer that isn't a virtual machine starts none of it.

Build it on the dev VM with `./packaging/build.sh patrone-guest-tools`.

**Source and licences.** The package's own files (units, drop-ins, udev
rule and scripts) are GPL-3.0-or-later, and their source is this folder
and `packaging/patrone-guest-tools/` in Patrone's repository, which isn't
public yet (the ISO's source offer is still to do, `docs/COMPLIANCE.md`).
What it depends on is Ubuntu's, unchanged, with source in Ubuntu's archive:
the Hyper-V daemons from the kernel's source package, `linux` (GPL-2.0), and
VirtualBox's guest utilities from `virtualbox` (GPL-3.0, in multiverse).

## Hyper-V (Generation 2)

| Integration service | What does it | Notes |
| --- | --- | --- |
| Shutdown | the kernel (`hv_utils`) | `Stop-VM` shuts the guest down cleanly. |
| Heartbeat | the kernel (`hv_utils`) | |
| Time Synchronization | the kernel (`hv_utils`), and chrony | The host sets the clock at boot and after a saved VM is restored or checkpointed. The host's clock (`/dev/ptp_hyperv`) is also one of chrony's sources, with the NTS servers (below). |
| Key-Value Pair Exchange | `hv_kvp_daemon` (Ubuntu's `linux-cloud-tools`) | Hyper-V Manager and `Get-VMNetworkAdapter` show the guest's IP addresses, and the host sees its name, "Patrone Linux", version and kernel. |
| VSS | `hv_vss_daemon` (Ubuntu's `linux-cloud-tools`) | Production checkpoints and host backups freeze the file systems first, so they're consistent. |
| Guest Service Interface | `hv_fcopy_uio_daemon`, started by Patrone's `patrone-hv-fcopy.service` | `Copy-VMFile` from the host. |
| Dynamic memory | the kernel (`hv_balloon`) | Memory the host adds is brought online by Ubuntu's `40-vm-hotadd.rules`. |

The drivers are all in `linux-modules-*`, part of every install. The
daemons come from `linux-cloud-tools-generic-hwe-26.04`, which follows the
desktop's kernel (`linux-generic-hwe-26.04`); `linux-cloud-tools-virtual`
follows the GA kernel instead. All of these are in Ubuntu's `main`.

**Any kernel's daemons.** Ubuntu's `/usr/sbin/hv_kvp_daemon` and the others
are wrappers that only run the copy built for the running kernel, so they
fail when that kernel's tools aren't installed: in the installer's live
session, which boots the disc's kernel while the image has the archive's
newer tools, and after a kernel update until the matching tools arrive.
The kernel and each daemon agree on a protocol version when it starts, so
another kernel's copy works too. `hv-tool` runs the running kernel's copy
if it's there, else the newest (and says so in the journal); drop-ins for
`hv-kvp-daemon.service` and `hv-vss-daemon.service` and Patrone's file copy
unit start the daemons through it.

**A note for the ISO build.** apt 3.2 upgrades the other packages built
from the same source as one it installs (`APT::Get::Upgrade-By-Source-Package`,
on by default). Installing the HWE cloud tools from the archive therefore
brings the archive's kernel into the image next to the disc's; installs
boot the newer one, and the live session boots the disc's. Guest tools
work either way.

**File copy.** Linux 6.10 moved the file copy channel from `hv_utils`
(`/dev/vmbus/hv_fcopy`) to a UIO device served by `uio_hv_generic`. Ubuntu
26.04's `hv-fcopy-daemon.service` still waits for the old device
(`ConditionPathExists=` and `BindsTo=`), so it never starts and the host
shows the Guest Service Interface as "No Contact". A drop-in can't take a
`BindsTo=` away, so the package has its own unit, `patrone-hv-fcopy.service`,
started at boot and by `61-patrone-hv-fcopy.rules` when the device appears.
Only one daemon may serve the channel, so a drop-in keeps Ubuntu's unit
off, should Ubuntu fix it. (Not `Conflicts=`: tried first, it let Ubuntu's
unit, queued at boot, cancel this one's start, after which Ubuntu's own
condition skipped it, and file copy stayed off.) `uio_hv_generic` only
takes the channel by itself where it lists it, in Ubuntu's kernels from
7.0.0-38; the disc's 7.0.0-30 doesn't, so the same rule runs
`hv-fcopy-bind`, which hands the channel over (`new_id`) when the driver
left it unbound. On Linux,
`Copy-VMFile -DestinationPath` is a **folder**; the daemon adds the source
file's name:

```powershell
Copy-VMFile -VMName NAME -SourcePath .\notes.txt -DestinationPath /tmp/ -FileSource Host
```

The copy is owned by root, mode 744. `-CreateFullPath` makes missing
folders.

**VSS and the desktops' FUSE mounts.** Before a production checkpoint or a
host backup, `hv_vss_daemon` freezes every file system whose source starts
with `/dev/`, and gives up on the first it can't freeze. FUSE file systems
give their source as `/dev/fuse`, and two of Patrone's own mount one per
signed-in desktop that even root can't open: FreeRDP's clipboard
(`/tmp/com.freerdp.client.cliprdr.*`, from Patrone Remote Desktop's screen
viewer) and GNOME Remote Desktop's (`/run/user/UID/gnome-remote-desktop/cliprdr-*`).
So every production checkpoint failed while someone was signed in ("failed
in the guest VSS Integration Component", and in the guest's journal
"FREEZE of /tmp/com.freerdp.client.cliprdr... failed; error:13"). They hold
nothing to freeze. The drop-in for `hv-vss-daemon.service` runs the daemon
in a mount namespace of its own (`MountFlags=private`), which doesn't
receive mounts made after it starts, through `vss-hide-fuse`, which first
unmounts, in that namespace only, any FUSE mounts already there (for a
restart while someone is signed in). It has to be the daemon's own process:
systemd gives each of a service's commands a new namespace, so an
`ExecStartPre=` doing it unmounted them in one thrown away. The catch: a
disk mounted after the daemon starts (a second virtual disk mounted from
File Manager, say) isn't frozen; disks in `/etc/fstab` are.

**The host's clock.** A Windows guest takes its time from the host. On
Hyper-V, `chrony-hyperv-clock` runs before chronyd (a drop-in,
`chrony.service.d/patrone-hyperv-clock.conf`) and writes
`refclock PHC /dev/ptp_hyperv poll 3 dpoll -2 offset 0 stratum 2`
(Microsoft's settings for Linux guests) to `/run/chrony/patrone/`, which
`/etc/chrony/conf.d/patrone-hyperv-clock.conf` includes. Anywhere else that
folder doesn't exist, which chrony takes as empty (a refclock it can't open
would stop chrony). chrony then picks between the host and the NTS servers
from `patrone-desktop`. `chronyc sources` lists it as `PHC0`.

A guest's time is only as good as the host's, as for a Windows guest. On
the test host (2026-10-28) Windows Time was stopped and Windows' clock was
6 s behind (against time.cloudflare.com with w32tm, and against the
NTS-synced dev VM). After a production checkpoint Hyper-V set the guest's
clock back to the host's (the kernel does that on resume, whatever chrony
thinks), and chrony then kept to `PHC0` for at least ten minutes, setting
the NTS servers aside as unsteady after the jump; after a fresh boot it
chose the NTS servers and set `PHC0` aside instead. With a host that keeps
good time (Windows Time running), they agree. To trust only the NTS
servers on a guest, an empty file
`/etc/systemd/system/chrony.service.d/patrone-hyperv-clock.conf` takes the
place of the drop-in (then restart chrony).

**Protocol versions.** `Get-VMIntegrationService` shows "The protocol version
of the component installed in the virtual machine does not match the
version expected by the hosting system" for Key-Value Pair Exchange and VSS.
Every Linux guest shows it: the kernel speaks an older revision of those two
protocols than Windows 11's Hyper-V expects. Their status is still OK.

### What was checked

On 2026-10-28, from the Windows host, in a new Generation 2 VM
(`PatroneGuestTools`: Secure Boot on, dynamic memory 2 to 8 GB starting at
4, production checkpoints), installed with "Erase disk" from an ISO built
with this package (`iso/build-iso.sh`, sha256 `63eff97a...`):

- **The installer's live session** (the disc's kernel, 7.0.0-30, with the
  image's 7.0.0-42 tools): all six integration services OK in
  `Get-VMIntegrationService`, the guest's IP addresses in
  `Get-VMNetworkAdapter`, and `Copy-VMFile` into `/tmp/` worked (the
  channel handed over by `hv-fcopy-bind`).
- **The installed system** (7.0.0-42), right after boot with nobody signed
  in: all six OK. Then, signed in (with the two FUSE clipboard mounts
  present): `Copy-VMFile` to `/tmp/` and, with `-CreateFullPath`, to a new
  folder; a checkpoint with `CheckpointType ProductionOnly` (which errors
  rather than fall back to a standard one) succeeded, with "op=FREEZE:
  succeeded" and "op=THAW: succeeded" in the guest's journal; the host's
  KVP data said "Patrone Linux", 0.1, 7.0.0-42-generic, the computer's
  name and address; memory grew from 4 GB to the 8 GB maximum;
  `chronyc sources` listed `PHC0`; `Stop-VM` shut it down cleanly in 4 s.
- `patrone-vbox-users`, `virtualbox-guest-utils` and the VBoxClient user
  units stayed off, as they should on Hyper-V.

Found and fixed on the way, in an earlier install of the same VM: file copy
not starting at boot (the `Conflicts=` above), production checkpoints
failing while someone was signed in (the FUSE mounts above), and
`hv-fcopy-bind` for the disc's kernel. Not checked: Enhanced Session
(`patrone-remote-desktop`'s), a backup by real backup software (the checkpoint uses the same
freeze), dynamic memory giving memory back (the guest's demand stayed near
the maximum), and bare metal (nothing there should start; the conditions
were read, not run).

### Screen size and Enhanced Session

In Hyper-V's basic console (VMConnect without Enhanced Session) the screen
doesn't follow the window, for Windows guests too. Settings → Displays offers
the synthetic display's modes, up to 1920×1080 by default (`Set-VMVideo` on
the host changes the list).

Enhanced Session (clipboard, audio, a screen that follows the window) is
Remote Desktop over a Hyper-V socket: VMConnect connects to vsock port 3389
in the guest. `patrone-remote-desktop` 0.5.0 listens there: VMConnect shows
Patrone's login screen, then the account's own desktop
(`remote-desktop/README.md`, "Hyper-V's enhanced session"). Drives can't be
shared. The installer's live session has no Remote Desktop at all. The host
also needs, with the VM off:

```powershell
Set-VM -VMName NAME -EnhancedSessionTransportType HvSocket
```

`hv_sock` loads by itself when something listens on vsock (its alias is the
vsock family, `net-pf-40`).

At login in a Hyper-V VM, Patrone Hyper-V Setup (`patrone-hyperv-setup`,
`hyperv-setup/README.md`) offers to put back whatever of this is missing,
these guest tools included, and shows that command, with the VM's own name,
until Hyper-V has opened an enhanced session with the VM.

## VirtualBox

**Not tried yet.** Nothing on this page about VirtualBox has been seen
working in a VirtualBox VM: VirtualBox isn't installed anywhere Patrone is
built or tested, and the user chose not to install it for this (2026-10-28).
What's known comes from the packages' contents (unpacked and read), their
units and scripts, and VirtualBox's changelogs. The package installs and
its units pass `systemd-analyze verify`; off VirtualBox they don't start
(checked on Hyper-V). To try it, install Patrone in VirtualBox
7.2 with VMSVGA graphics, then check `systemctl status
virtualbox-guest-utils`, `systemctl --user status
'patrone-vboxclient-*'`, `id` (vboxsf, vboxdrmipc), a shared folder with
"Auto-mount" under `/media`, resizing the window, and the clipboard both
ways.

Ubuntu's `virtualbox-guest-utils` and `virtualbox-guest-x11` (VirtualBox
7.2.6), which the package depends on:

| Guest Additions part | What runs it | What it does |
| --- | --- | --- |
| Kernel drivers | the kernel: `vboxguest`, `vboxsf`, and `vmwgfx` for VMSVGA graphics (VirtualBox's default for Linux; `vboxvideo` for VBoxVGA/VBoxSVGA) | In `linux-modules-*`; no DKMS. |
| VBoxService | Ubuntu's `virtualbox-guest-utils.service` | Time sync with the host, guest properties (the host sees the OS, IP addresses and who's signed in), shared folder automounting. |
| VBoxDRMClient | started by `VBoxClient --vmsvga` from the same service | Passes the VM window's size to the screen. |
| VBoxClient, in each session | Patrone's user units `patrone-vboxclient-wayland.service` and `patrone-vboxclient-vmsvga.service` | The shared clipboard (`--wayland`) and the screen layout helper (`--vmsvga-session`). |

Ubuntu's packages start VBoxClient only from `/etc/X11/Xsession.d`, which a
GNOME session on Wayland never runs, so on plain Ubuntu nothing in the
session starts it. Patrone's user units start with `graphical-session.target`
(as Ubuntu's `spice-vdagent.service` does for QEMU's SPICE). They leave out
`VBoxClient --checkhostversion`, which tells you to install the host's
Guest Additions over the system's.

**Shared folders.** VBoxService mounts the host's shared folders under
`/media/sf_NAME` (or the mount point set on the host), readable only by the
`vboxsf` group. `patrone-vbox-users.service` adds every account (UID 1000
to 60000, from `/etc/login.defs`; the live session's account is 1000) to
`vboxsf`, and to `vboxdrmipc`, the group VBoxDRMClient lets its session
helper talk to it with, which Ubuntu's packages don't create
(`patrone-guest-tools` makes it with `sysusers.d`). It runs at boot before
anyone can sign in, and `patrone-vbox-users.path` runs it again when
`/etc/passwd` changes, so new accounts get it at once. A group counts from
the account's next sign-in. It adds an account again if you take it out; to
stop that, `systemctl mask patrone-vbox-users.service patrone-vbox-users.path`.

**On Wayland** (GNOME 50 has no X11 session):

- **Resizing the screen with the window:** VBoxDRMClient tells `vmwgfx` the
  window's size, and the desktop picks the new mode up. Needs VMSVGA
  graphics (VirtualBox's default for Linux).
- **Shared clipboard:** VirtualBox has supported Wayland since 7.1.0, and on
  GNOME `VBoxClient --wayland` works through GTK (GNOME doesn't offer the
  data-control protocol its other helper uses, which 7.2.10 added for KDE
  Plasma). But Ubuntu ships 7.2.6, and 7.2.8 fixed "clipboard sharing was
  not possible in combination of Wayland guest and Windows host" and text
  losing its last character on the way to a Windows host
  ([changelog](https://www.virtualbox.org/wiki/Changelog-7.2)). Expect it
  not to work from a Windows host until Ubuntu updates the package.
- **Drag and drop:** VirtualBox's drag and drop is X11's (XDND); no release
  notes claim it for Wayland. Expect it not to work.
- **Seamless mode:** X11 only.

**Time:** VBoxService corrects the clock when it's more than 100 ms from
the host's (its default), and sets it outright when it's far off, as after
a saved VM is restored; chrony keeps it with the NTS servers, normally well
inside that margin, so they should rarely both act. Ubuntu's unit makes
VBoxService conflict with `systemd-timesyncd`, which Ubuntu 26.04 no longer
uses, but not with chrony. Untried, like the rest of this section.

**Archive section.** `virtualbox-guest-utils` and `virtualbox-guest-x11`
are in Ubuntu's **multiverse** (Debian's `contrib`). Their own copyright file
says why: the VirtualBox source package needs a non-free compiler, Open
Watcom, to build the *host's* BIOS. The guest utilities themselves are
GPL-3.0. Multiverse is on in Ubuntu's (and Patrone's) apt sources by default.
They're Depends, not Recommends: apt 3.2 left `virtualbox-guest-x11` out
when it was only recommended, since it needs an installed package
(`xserver-common`) upgraded, and without it there's no resizing or
clipboard. `patrone-desktop` only recommends `patrone-guest-tools`, so a
system without multiverse still installs the desktop.

**Ubuntu's, unchanged.** Patrone uses Ubuntu's VirtualBox packages as they
are: never rebuilt or renamed (Oracle's FAQ: custom builds may not carry
the VirtualBox name), and never Oracle's Extension Pack, which can't be
redistributed.

**Cost elsewhere.** `virtualbox-guest-x11` depends on Xorg's server
(`xserver-xorg-core`, about 4 MB), which a Wayland-only desktop doesn't
otherwise have. It never runs: GDM 50 offers no X11 session, and nothing
else starts it.
