# Patrone's GDM

Patrone builds Ubuntu's GDM (`gdm3`) again with one change, so the login
screen doesn't keep a stalled boot splash on screen for 10 seconds on a
Hyper-V VM.

The user saw it on the dev VM (2026-10-27): the boot splash's spinner turned
for only a few seconds, then the logo stood still until the login screen.
The 10:07 boot's journal, on the monotonic clock:

| Seconds after boot | |
| --- | --- |
| 2.4 | `hyperv_drm`, Hyper-V's display driver, starts (from the initramfs) |
| 4.5 | GDM starts and runs `plymouth deactivate`: the splash stays on screen but stops animating |
| 15.0 | GDM: "It appears that your system does not have a primary GPU! Proceeding with any GPU" |
| 15.5 | The login screen's GNOME Shell starts |

(The wall-clock times looked like 15 seconds because chrony stepped the clock
4.7 seconds forward in between.)

GDM 50.1 waits up to 10 seconds at startup (`SEAT0_GRAPHICS_CHECK_TIMEOUT`
in `daemon/gdm-local-display-factory.c`) for the display the computer booted
on, and in `udev_is_settled()` it only recognizes one that's a platform
device or a PCI or DRM device marked by the firmware (`boot_vga` or
`boot_display`). A generation 2 Hyper-V VM's only display, Hyper-V's
synthetic video, is on the VMBus, so GDM never recognizes it and always waits
the full 10 seconds.

| File | |
| --- | --- |
| `patches/patrone-hyperv-boot-display.patch` | Counts a display on the VMBus as the boot display: `hyperv_drm` takes over the framebuffer the VM's firmware booted on. Other computers are checked as before |
| `build.sh` | Fetches Ubuntu's source, adds the patch, versions it Ubuntu's version plus `patrone` and the revision, and builds |

GDM still deactivates the splash as it starts: Plymouth has to let go of the
screen before GNOME Shell can take it. Without the wait, the splash stands
still only while the login screen starts, about a second.

## Building

On the dev VM, with Ubuntu's build dependencies for `gdm3` installed
(`dpkg-checkbuilddeps` in the unpacked source lists any that are missing):

```
./gdm/build.sh
```

It fetches the newest resolute source (`pull-lp-source`, the one step that
needs the network), or a given version (`build.sh 50.1-0ubuntu0.1`). Install
the three packages that go together:

```
sudo apt install ./out/gdm3_*patrone1_amd64.deb ./out/libgdm1_*patrone1_amd64.deb ./out/gir1.2-gdm-1.0_*patrone1_amd64.deb
```

GDM loads it when it next starts, which is at the next boot. An Ubuntu
update of GDM replaces these packages until `build.sh` is run again.

## How it was checked

`50.1-0ubuntu0.1patrone1`, built on the dev VM (2026-10-27): the patch
applied cleanly, the patched file compiled with no warnings, and the built
`/usr/sbin/gdm3` has the new check's message in it. Before building, GDM's
check was run on the VM's real display through the same libgudev calls
`udev_is_settled()` makes: as Ubuntu builds it, it found no primary GPU;
with the patch's rule, it accepted `card1` (`hyperv_drm`) at once. Not yet
checked: a boot with it installed. After one, `journalctl -b -o
short-monotonic | grep -E 'gdm.service|primary GPU|greeter'` should show no
"primary GPU" warning and the login screen's session about a second after
GDM starts.
