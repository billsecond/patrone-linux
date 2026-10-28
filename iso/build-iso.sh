#!/bin/bash
# Builds Patrone Linux's ISO from Ubuntu's desktop ISO.
#
#   sudo iso/build-iso.sh UBUNTU-ISO DEB-DIR OUT-ISO
#
# Ubuntu's disc is made of stacked layers (casper/*.squashfs). This adds two,
# each with Patrone's packages installed (the newest build of each in
# DEB-DIR, as packaging/build.sh leaves them in out/), Ptyxis taken out, and
# a snap seed without Firefox and App Center. There's no web browser: each
# account picks its first one (patrone-browser-chooser).
#
# - minimal.patrone, what the installer installs: Ubuntu's English layer
#   with Patrone's packages, on the base. It's the installer's only choice,
#   "Patrone Linux".
# - minimal.standard.live.patrone, on Ubuntu's live session, which the disc
#   now boots: the live desktop, its splash and the installer (whose name,
#   colors and pictures patrone-branding sets) are Patrone's.
#
# Run on Ubuntu 26.04 as root; needs squashfs-tools, xorriso, cpio,
# initramfs-tools-core, snapd and a network (apt and the Snap Store). WORK
# (default /var/tmp/patrone-iso) is emptied.
set -euo pipefail

[ "$(id -u)" = 0 ] || { echo "build-iso.sh: run it as root" >&2; exit 1; }
# The build mounts this machine's /dev and /dev/pts into each layer. It runs
# in a mount namespace of its own, sharing nothing back, so unmounting them
# there can't take this machine's away: with mounts shared, as systemd sets
# them up, three builds at once left the dev VM without /dev/pts, and new
# terminals and SSH logins failed (2026-10-28). The build's mounts also go
# when it ends, however it ends.
if [ -z "${PATRONE_ISO_NAMESPACE:-}" ]; then
    exec env PATRONE_ISO_NAMESPACE=1 unshare --mount --propagation private -- "$BASH" "$0" "$@"
fi
[ $# = 3 ] || { sed -n '4p' "$0" >&2; exit 2; }
ubuntu_iso=$(realpath "$1")
debs=$(realpath "$2")
out=$(realpath -m "$3")
work=${WORK:-/var/tmp/patrone-iso}
install_layer=minimal.patrone
live_layer=minimal.standard.live.patrone

mounts=()
mnt() { mount "$@"; mounts+=("${*: -1}"); }
unmount_to() { # unmounts all but the first $1 mounts
    local i
    for ((i = ${#mounts[@]} - 1; i >= $1; i--)); do
        umount "${mounts[i]}" || umount -l "${mounts[i]}"
        unset 'mounts[i]'
    done
}
trap 'unmount_to 0' EXIT

step() { echo "== $*"; }

# A run that was killed can leave this machine's /dev mounted in WORK.
if findmnt -rn -o TARGET | grep -q "^$work/"; then
    echo "build-iso.sh: something is still mounted in $work; unmount it first" >&2
    exit 1
fi
rm -rf --one-file-system "$work"
mkdir -p "$work"/{cd,layers,iso,snaps}
mnt -o loop,ro "$ubuntu_iso" "$work/cd"
cp "$work/cd/boot/grub/grub.cfg" "$work/cd/boot/grub/loopback.cfg" "$work/cd/md5sum.txt" "$work/iso/"
kernel=$(grep -A1 '^kernel:' "$work/cd/casper/install-sources.yaml")
casper_files=$(ls "$work/cd/casper")

declare -A newest
for deb in "$debs"/*.deb; do
    name=$(dpkg-deb -f "$deb" Package)
    case $name in *-dev | *-dbgsym) continue ;; esac
    if [ -z "${newest[$name]:-}" ] || dpkg --compare-versions \
        "$(dpkg-deb -f "$deb" Version)" gt "$(dpkg-deb -f "${newest[$name]}" Version)"; then
        newest[$name]=$deb
    fi
done
printf '%s\n' "${newest[@]##*/}" | sort

in_root() { chroot "$root" env DEBIAN_FRONTEND=noninteractive LC_ALL=C.UTF-8 "$@"; }

# seed_drop SEED NAME...: takes snaps out of a snap seed (the snaps a new
# system sets up at first boot), leaving every other entry as it is.
seed_drop() {
    local seed=$1 name
    shift
    awk -v names=" $* " '
        function flush() { if (blk != "" && !drop) printf "%s", blk; blk = ""; drop = 0 }
        /^  -/ { flush(); blk = $0 "\n"; next }
        blk != "" { blk = blk $0 "\n"; if ($1 == "name:" && index(names, " " $2 " ")) drop = 1; next }
        { print }
        END { flush() }' "$seed/seed.yaml" >"$seed/seed.yaml.new"
    mv "$seed/seed.yaml.new" "$seed/seed.yaml"
    for name; do
        rm -f "$seed/snaps/${name}_"*.snap "$seed/assertions/${name}_"*.assert
    done
}

# seed_add SEED NAME: adds a snap from the Snap Store, with its base and the
# snaps it names as default providers.
seed_add() {
    local seed=$1 name=$2 file need
    grep -qx "    name: $name" "$seed/seed.yaml" && return
    if ! ls "$work/snaps/${name}_"*.snap >/dev/null 2>&1; then
        snap download --channel=stable --target-directory="$work/snaps" "$name" >/dev/null
    fi
    file=$(basename "$(ls "$work/snaps/${name}_"*.snap | head -1)")
    cp "$work/snaps/$file" "$seed/snaps/"
    cp "$work/snaps/${file%.snap}.assert" "$seed/assertions/"
    printf '  -\n    name: %s\n    channel: stable\n    file: %s\n' "$name" "$file" >>"$seed/seed.yaml"
    echo "seeded $file"
    for need in $(unsquashfs -cat "$work/snaps/$file" meta/snap.yaml |
        sed -nE 's/^base: *([a-z0-9-]+).*/\1/p; s/.*default-provider: *([a-z0-9-]+).*/\1/p' | sort -u); do
        seed_add "$seed" "$need"
    done
}

# open_layer NAME START LOWER...: makes layer NAME on Ubuntu's layers LOWER
# (the topmost first), starting from a copy of Ubuntu's layer START (or
# empty), with Patrone's packages installed, and leaves it open as $root.
open_layer() {
    local name=$1 start=$2 lowers=() l deb install=()
    shift 2
    for l; do
        if [ ! -d "$work/layers/$l" ]; then
            mkdir -p "$work/layers/$l"
            mnt -o loop,ro "$work/cd/casper/$l.squashfs" "$work/layers/$l"
        fi
        lowers+=("$work/layers/$l")
    done
    layer=$work/$name
    mkdir -p "$layer"/{upper,ovl,root}
    if [ -n "$start" ]; then
        # Its files and the whiteouts that hide what it removes below it
        unsquashfs -q -f -d "$layer/upper" "$work/cd/casper/$start.squashfs"
    fi
    root=$layer/root
    layer_mounts=${#mounts[@]}
    mnt -t overlay overlay -o "lowerdir=$(IFS=:; echo "${lowers[*]}"),upperdir=$layer/upper,workdir=$layer/ovl,redirect_dir=off,metacopy=off,index=off,xino=off" "$root"
    # Ubuntu's layers come with their seed's snaps set up already
    # ("preseeded": App Center, Firefox and Security Center installed, and
    # the seed's later changes ignored). That's undone here, and done again
    # with Patrone's seed when the layer is closed.
    /usr/lib/snapd/snap-preseed --reset "$root"
    mnt -t proc proc "$root/proc"
    mnt -t sysfs sysfs "$root/sys"
    mnt --bind /dev "$root/dev"
    mnt --bind /dev/pts "$root/dev/pts"
    # Not this machine's /run: without snapd's socket, packages that install
    # a snap (chromium-browser) leave it to the seed.
    mnt -t tmpfs tmpfs "$root/run"
    mkdir -p "$root/run/systemd/resolve"
    cp -L /etc/resolv.conf "$root/run/systemd/resolve/stub-resolv.conf"
    # The image's apt sources include the disc (file:/cdrom), as the
    # installer sees it.
    mkdir -p "$root/cdrom"
    mnt --bind "$work/cd" "$root/cdrom"

    # No services start in the image, and the boot menu is the installer's
    # job (update-grub can't work out an image's disks).
    printf '#!/bin/sh\nexit 101\n' >"$root/usr/sbin/policy-rc.d"
    chmod 755 "$root/usr/sbin/policy-rc.d"
    in_root dpkg-divert --local --rename --add /usr/sbin/update-grub >/dev/null
    ln -s /bin/true "$root/usr/sbin/update-grub"

    mkdir -p "$root/tmp/patrone-debs"
    for deb in "${newest[@]}"; do
        [[ " $leave_out " == *" $(dpkg-deb -f "$deb" Package) "* ]] || cp "$deb" "$root/tmp/patrone-debs/"
    done
    for deb in "$root"/tmp/patrone-debs/*.deb; do
        install+=("/tmp/patrone-debs/${deb##*/}")
    done
    in_root apt-get update
    # A new image has no edits of anyone's to keep, and no one to answer
    # dpkg's questions: patrone-branding's /etc/issue, which takes the place
    # of Ubuntu's (moved aside), otherwise asks whether to keep Ubuntu's
    # "deleted" one.
    in_root apt-get install -y -o Dpkg::Options::=--force-confnew "${install[@]}"
    # Patrone Terminal is the terminal (the user: "the other terminal
    # shouldn't even show up"). Ubuntu's desktop only recommends Ptyxis.
    in_root apt-get purge -y ptyxis
    # No web browser: the first time one's needed, Patrone Browser Chooser
    # asks which to install. Ubuntu's firefox package is a stub that
    # installs Firefox's snap whenever it's installed or upgraded, and
    # Ubuntu's desktop only recommends it. A hold left it to every updater
    # to honour; an update to the stub is in resolute-updates
    # (1snap1-0ubuntu9.1). chromium-browser is the same for Chromium's snap,
    # which patrone-default-apps depended on before 0.2.0.
    in_root apt-get purge -y firefox chromium-browser
    # The settings left by packages Patrone's removed (ubuntu-report's
    # autostart entry; update-notifier's, which said "Ubuntu Pro is
    # available" at login)
    leftovers=$(in_root dpkg-query -W -f '${db:Status-Abbrev} ${Package}\n' | awk '$1 == "rc" {print $2}')
    [ -z "$leftovers" ] || in_root apt-get purge -y $leftovers

    # Security Center is Ubuntu Pro's and the snaps' settings
    seed_drop "$root/var/lib/snapd/seed" firefox chromium snap-store desktop-security-center
    snap debug validate-seed "$root/var/lib/snapd/seed/seed.yaml"
    check_no_browser "$name"
}

# check_no_browser NAME: stops the build if the open layer has a web
# browser, as a package, a launcher or a snap in its seed, so none comes
# back unasked.
check_no_browser() {
    local found
    found=$(
        in_root dpkg-query -W -f '${db:Status-Abbrev} ${Package}: ${Provides}\n' |
            awk '$1 == "ii" && /(x|gnome)-www-browser/ {print "package " $2}'
        grep -l -E '^Categories=(.*;)?WebBrowser(;|$)' "$root"/usr/share/applications/*.desktop 2>/dev/null |
            sed 's/^/launcher /'
        grep -E '^    name: (firefox|chromium|brave|opera|vivaldi|librewolf|epiphany)' \
            "$root/var/lib/snapd/seed/seed.yaml" | sed 's/^ *name: /snap /'
    ) || true
    if [ -n "$found" ]; then
        echo "build-iso.sh: $1 has a web browser:" >&2
        echo "$found" >&2
        exit 1
    fi
}

# close_layer NAME: tidies the open layer, closes it and compresses it.
close_layer() {
    local name=$1 size
    rm -rf "$root/tmp/patrone-debs" "$root/usr/sbin/policy-rc.d" "$root/usr/sbin/update-grub"
    in_root dpkg-divert --local --rename --remove /usr/sbin/update-grub >/dev/null
    in_root apt-get clean
    find "$root/var/lib/apt/lists" -maxdepth 1 -type f -delete
    rm -f "$root"/var/cache/apt/*.bin
    in_root dpkg-query -W -f '${Package}\t${Version}\n' >"$work/iso/$name.manifest"
    # The seed's snaps set up in the image, as Ubuntu's are, so a new system
    # (and the live session's installer, whose back end is a snap) doesn't
    # wait for them at first boot. snap-preseed needs the layer's /dev, /proc
    # and /sys/kernel/security mounted. If it fails, they're set up at first
    # boot instead.
    mnt -t securityfs securityfs "$root/sys/kernel/security"
    if ! /usr/lib/snapd/snap-preseed "$root"; then
        echo "build-iso.sh: preseeding $name's snaps failed; they're set up at first boot" >&2
        /usr/lib/snapd/snap-preseed --reset "$root"
    fi
    size=$(du -sbx "$root" | cut -f1)
    echo "$size" >"$work/iso/$name.size"
    unmount_to "$layer_mounts"
    # The disc's mount point, made for the build: the installer makes /cdrom
    # in the new system itself, and stops if it's there already.
    rmdir "$layer/upper/cdrom"
    mksquashfs "$layer/upper" "$work/iso/$name.squashfs" -noappend -comp xz -b 131072 -xattrs -quiet
}

step "What the installer installs ($install_layer)"
# Ubuntu's English layer and Patrone's packages: the installer puts no
# language layer of its own on it.
# Patrone's installer is for the live session only.
leave_out=patrone-installer
open_layer "$install_layer" minimal.en minimal
close_layer "$install_layer"
install_size=$(cat "$work/iso/$install_layer.size")

step "The live session ($live_layer)"
# Without Patrone Remote Desktop, whose desktop on the screen starts at the
# login screen: the live session logs in by itself, and GDM then refuses
# the account's desktop ("There's already an opened session").
leave_out=patrone-remote-desktop
open_layer "$live_layer" "" minimal.standard.live minimal.standard minimal
# Patrone Terminal opens at login on an installed system; here the
# installer has the screen (Ctrl+Alt+T still opens it).
echo "Hidden=true" >>"$root/etc/xdg/autostart/org.patrone.Terminal.desktop"
# Ubuntu's live layer turns off only Ubuntu's desktop icons
# (livecd-rootfs.gschema.override, which comes after Patrone's 90_patrone
# and so wins), which brought Ubuntu's dock back. Patrone's taskbar is the
# panel here too.
cat >"$root/usr/share/glib-2.0/schemas/zz_patrone-live.gschema.override" <<'EOF'
[org.gnome.shell]
disabled-extensions=['ubuntu-dock@ubuntu.com', 'ding@rastersoft.com']
EOF
in_root glib-compile-schemas /usr/share/glib-2.0/schemas

step "The live session's boot image"
# Ubuntu's own, which knows the disc (conf/uuid.conf) and how to start the
# live session, with Patrone's boot splash and Patrone's live layer.
initrd=$work/initrd
unmkinitramfs "$work/cd/casper/initrd" "$initrd"
if [ -d "$initrd/main" ]; then
    echo "build-iso.sh: casper/initrd has more than one archive; only one is handled" >&2
    exit 1
fi
themes=$initrd/usr/share/plymouth/themes
rm -rf "$themes/bgrt" "$themes/ubuntu-text"
cp -a "$root/usr/share/plymouth/themes/patrone" "$root/usr/share/plymouth/themes/patrone-text" "$themes/"
ln -sfn /usr/share/plymouth/themes/patrone/patrone.plymouth "$themes/default.plymouth"
ln -sfn /usr/share/plymouth/themes/patrone-text/patrone-text.plymouth "$themes/text.plymouth"
echo "LAYERFS_PATH=$live_layer.squashfs" >"$initrd/conf/conf.d/default-layer.conf"
(cd "$initrd" && find . -print0 | sort -z | cpio --null -o -H newc -R 0:0 --quiet) >"$work/iso/initrd"
close_layer "$live_layer"

step "The ISO"
# The ID is Ubuntu's default choice's, which the installer's page of app
# choices knows; the name is what it shows.
cat >"$work/iso/install-sources.yaml" <<EOF
$kernel
sources:
- default: true
  description:
    en: Patrone Linux's desktop, on Ubuntu 26.04 LTS.
  id: ubuntu-desktop-minimal
  locale_support: langpack
  name:
    en: Patrone Linux
  path: $install_layer.squashfs
  size: $install_size
  type: fsimage-layered
  variant: desktop
  variations:
    patrone:
      path: $install_layer.squashfs
      size: $install_size
version: 2
EOF
# The boot menu (the user, 2026-10-27: "installer only"): first "Install
# Patrone Linux", the installer alone, full screen (patrone-installer's
# session for patrone-install-only); then "Try Patrone Linux", the live
# desktop with the installer on it; then the installer with safe graphics.
# Ubuntu's entries for the firmware stay, and it counts down 10 seconds, not
# 30.
boot_menu() { # MORE-KERNEL-ARGUMENTS
    local entry
    for entry in "Install Patrone Linux|patrone-install-only" "Try Patrone Linux|" \
        "Install Patrone Linux (safe graphics)|nomodeset patrone-install-only"; do
        printf 'menuentry "%s" {\n    set gfxpayload=keep\n    linux  /casper/vmlinuz %s %s --- quiet splash\n    initrd /casper/initrd\n}\n' \
            "${entry%%|*}" "${entry#*|}" "$1"
    done
}
grep -q '^set timeout=30$' "$work/iso/grub.cfg" && grep -q '^grub_platform' "$work/iso/grub.cfg"
{
    sed -e '/^menuentry /,$d' -e 's/^set timeout=30$/set timeout=10/' "$work/iso/grub.cfg"
    boot_menu ""
    sed -n '/^grub_platform/,$p' "$work/iso/grub.cfg"
} >"$work/iso/grub.cfg.new"
mv "$work/iso/grub.cfg.new" "$work/iso/grub.cfg"
boot_menu 'iso-scan/filename=${iso_path}' >"$work/iso/loopback.cfg"
# Ubuntu's form; the installer shows what comes before the quote as the new
# system's name.
echo "Patrone Linux 26.04.1 \"Patrone Linux\" - Release amd64 ($(date -u +%Y%m%d))" >"$work/iso/info"

# What's new on the disc
declare -A put=([/casper/initrd]=initrd [/casper/install-sources.yaml]=install-sources.yaml
    [/boot/grub/grub.cfg]=grub.cfg [/boot/grub/loopback.cfg]=loopback.cfg [/.disk/info]=info)
for l in "$install_layer" "$live_layer"; do
    for ext in squashfs manifest size; do
        put[/casper/$l.$ext]=$l.$ext
    done
done
# Ubuntu's layers that nothing uses any more go: the live session still
# stands on the base, the standard layer and Ubuntu's live layer.
remove=()
for f in $casper_files; do
    case $f in
        initrd | vmlinuz | install-sources.yaml) ;;
        minimal.squashfs | minimal.manifest* | minimal.size) ;;
        minimal.standard.squashfs | minimal.standard.manifest* | minimal.standard.size) ;;
        minimal.standard.live.*) ;;
        *) remove+=("/casper/$f") ;;
    esac
done
# Ubuntu's disc has a link at its root, "ubuntu", back to the disc itself (a
# Debian-era convention). Nothing on Patrone's uses that path (apt's disc
# source and the installer use file:///cdrom, and no checksum goes through
# it), and Windows shows it as a file named UBUNTU (the user, 2026-10-28).
remove+=(/ubuntu)

# The disc's checksums, which the live session checks
{
    awk -v gone="$(printf '.%s\n' "${remove[@]}" "${!put[@]}")" '
        BEGIN { n = split(gone, g, "\n"); for (i = 1; i <= n; i++) skip[g[i]] = 1 }
        !($2 in skip)' "$work/iso/md5sum.txt"
    for path in "${!put[@]}"; do
        echo "$(md5sum <"$work/iso/${put[$path]}" | cut -d' ' -f1)  .$path"
    done
} >"$work/md5sum.txt"
maps=(-map "$work/md5sum.txt" /md5sum.txt)
for path in "${!put[@]}"; do
    maps+=(-map "$work/iso/${put[$path]}" "$path")
done

volume_date=$(xorriso -indev "$ubuntu_iso" -pvd_info 2>/dev/null | sed -n 's/^Modif. Time *: *//p')
rm -f "$out"
xorriso -indev "$ubuntu_iso" -outdev "$out" -boot_image any replay \
    -volid "Patrone Linux 26.04.1 amd64" -volume_date uuid "$volume_date" \
    -rm "${remove[@]}" -- "${maps[@]}"

step "On the disc"
xorriso -indev "$out" -ls /casper 2>/dev/null
ls -l "$out"
