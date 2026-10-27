#!/bin/bash
# Builds Patrone Linux's ISO from Ubuntu's desktop ISO.
#
#   sudo iso/build-iso.sh UBUNTU-ISO DEB-DIR OUT-ISO
#
# Ubuntu's disc is made of stacked layers (casper/*.squashfs). This adds two,
# each with Patrone's packages installed (the newest build of each in
# DEB-DIR, as packaging/build.sh leaves them in out/), Ptyxis taken out, and
# a snap seed with Chromium in place of Firefox and App Center:
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
    cp "${newest[@]}" "$root/tmp/patrone-debs/"
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
    # Ubuntu's desktop needs the firefox package, a stub that installs
    # Firefox's snap; the seed has no Firefox, and the hold keeps upgrades
    # from installing it (default-apps/README.md).
    in_root apt-mark hold firefox

    seed_drop "$root/var/lib/snapd/seed" firefox snap-store
    seed_add "$root/var/lib/snapd/seed" chromium
    snap debug validate-seed "$root/var/lib/snapd/seed/seed.yaml"
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
    size=$(du -sbx "$root" | cut -f1)
    echo "$size" >"$work/iso/$name.size"
    unmount_to "$layer_mounts"
    mksquashfs "$layer/upper" "$work/iso/$name.squashfs" -noappend -comp xz -b 131072 -xattrs -quiet
}

step "What the installer installs ($install_layer)"
# Ubuntu's English layer and Patrone's packages: the installer puts no
# language layer of its own on it.
open_layer "$install_layer" minimal.en minimal
close_layer "$install_layer"
install_size=$(cat "$work/iso/$install_layer.size")

step "The live session ($live_layer)"
open_layer "$live_layer" "" minimal.standard.live minimal.standard minimal
# Patrone Terminal opens at login on an installed system; here the
# installer has the screen (Ctrl+Alt+T still opens it).
echo "Hidden=true" >>"$root/etc/xdg/autostart/org.patrone.Terminal.desktop"

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
sed -i -e 's/"Try or Install Ubuntu"/"Try or Install Patrone Linux"/' \
    -e 's/"Ubuntu (safe graphics)"/"Patrone Linux (safe graphics)"/' \
    "$work/iso/grub.cfg" "$work/iso/loopback.cfg"
echo "Patrone Linux 26.04.1, on Ubuntu 26.04.1 LTS \"Resolute Raccoon\" - amd64 ($(date -u +%Y%m%d))" >"$work/iso/info"

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
