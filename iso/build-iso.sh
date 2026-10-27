#!/bin/bash
# Builds Patrone Linux's ISO from Ubuntu's desktop ISO.
#
#   sudo iso/build-iso.sh UBUNTU-ISO DEB-DIR OUT-ISO
#
# Ubuntu's installer puts the new system together from layers: the base
# (casper/minimal.squashfs) and a language layer on top (minimal.en...).
# This adds one, minimal.patrone.squashfs: Ubuntu's English layer with
# Patrone's packages installed (the newest build of each in DEB-DIR, as
# packaging/build.sh leaves them in out/), and makes it the installer's only
# choice, "Patrone Linux". Its snap seed has Chromium in place of Firefox and
# App Center. Ubuntu's live session and installer stay as they are. Run on
# Ubuntu 26.04 as root; needs squashfs-tools, xorriso, snapd and a network
# (apt and the Snap Store). WORK (default /var/tmp/patrone-iso) is emptied.
set -euo pipefail

[ "$(id -u)" = 0 ] || { echo "build-iso.sh: run it as root" >&2; exit 1; }
[ $# = 3 ] || { sed -n '4p' "$0" >&2; exit 2; }
ubuntu_iso=$(realpath "$1")
debs=$(realpath "$2")
out=$(realpath -m "$3")
work=${WORK:-/var/tmp/patrone-iso}
layer=minimal.patrone

mounts=()
mnt() { mount "$@"; mounts+=("${*: -1}"); }
unmount_all() {
    local i
    for ((i = ${#mounts[@]} - 1; i >= 0; i--)); do
        umount "${mounts[i]}" || umount -l "${mounts[i]}"
    done
    mounts=()
}
trap unmount_all EXIT

step() { echo "== $*"; }

# A run that was killed can leave this machine's /dev mounted in WORK.
if findmnt -rn -o TARGET | grep -q "^$work/"; then
    echo "build-iso.sh: something is still mounted in $work; unmount it first" >&2
    exit 1
fi
rm -rf --one-file-system "$work"
mkdir -p "$work"/{cd,minimal,upper,ovl,root,iso,snaps}
mnt -o loop,ro "$ubuntu_iso" "$work/cd"
cp "$work/cd/boot/grub/grub.cfg" "$work/cd/boot/grub/loopback.cfg" \
    "$work/cd/md5sum.txt" "$work/cd/casper/install-sources.yaml" "$work/iso/"
casper_files=$(ls "$work/cd/casper")

step "Patrone's layer: Ubuntu's English layer, on the base"
# Its files and the whiteouts that hide what it removes from the base. The
# installer puts no language layer of its own on Patrone's.
unsquashfs -q -f -d "$work/upper" "$work/cd/casper/minimal.en.squashfs"
mnt -o loop,ro "$work/cd/casper/minimal.squashfs" "$work/minimal"
mnt -t overlay overlay -o "lowerdir=$work/minimal,upperdir=$work/upper,workdir=$work/ovl,redirect_dir=off,metacopy=off,index=off,xino=off" "$work/root"

root=$work/root
mnt -t proc proc "$root/proc"
mnt -t sysfs sysfs "$root/sys"
mnt --bind /dev "$root/dev"
mnt --bind /dev/pts "$root/dev/pts"
# Not this machine's /run: without snapd's socket, packages that install a
# snap (chromium-browser) leave it to the seed, below.
mnt -t tmpfs tmpfs "$root/run"
mkdir -p "$root/run/systemd/resolve"
cp -L /etc/resolv.conf "$root/run/systemd/resolve/stub-resolv.conf"
# The image's apt sources include the disc (file:/cdrom), as the installer
# sees it.
mkdir -p "$root/cdrom"
mnt --bind "$work/cd" "$root/cdrom"
in_root() { chroot "$root" env DEBIAN_FRONTEND=noninteractive LC_ALL=C.UTF-8 "$@"; }

# No services start in the image, and the boot menu is the installer's job
# (update-grub can't work out an image's disks).
printf '#!/bin/sh\nexit 101\n' >"$root/usr/sbin/policy-rc.d"
chmod 755 "$root/usr/sbin/policy-rc.d"
in_root dpkg-divert --local --rename --add /usr/sbin/update-grub >/dev/null
ln -s /bin/true "$root/usr/sbin/update-grub"

step "Patrone's packages"
declare -A newest
for deb in "$debs"/*.deb; do
    name=$(dpkg-deb -f "$deb" Package)
    case $name in *-dev | *-dbgsym) continue ;; esac
    if [ -z "${newest[$name]:-}" ] || dpkg --compare-versions \
        "$(dpkg-deb -f "$deb" Version)" gt "$(dpkg-deb -f "${newest[$name]}" Version)"; then
        newest[$name]=$deb
    fi
done
mkdir -p "$root/tmp/patrone-debs"
cp "${newest[@]}" "$root/tmp/patrone-debs/"
install=()
for deb in "$root"/tmp/patrone-debs/*.deb; do
    install+=("/tmp/patrone-debs/${deb##*/}")
    echo "${deb##*/}"
done
in_root apt-get update
# A new image has no edits of anyone's to keep, and no one to answer dpkg's
# questions: patrone-branding's /etc/issue, which takes the place of Ubuntu's
# (moved aside), otherwise asks whether to keep Ubuntu's "deleted" one.
in_root apt-get install -y -o Dpkg::Options::=--force-confnew "${install[@]}"
# Ubuntu's desktop needs the firefox package, a stub that installs Firefox's
# snap; Patrone's seed has no Firefox, and the hold keeps upgrades from
# installing it (default-apps/README.md).
in_root apt-mark hold firefox
in_root dpkg-query -W -f '${Package}\t${Version}\n' >"$work/iso/$layer.manifest"

step "The snap seed: Chromium in place of Firefox and App Center"
seed=$root/var/lib/snapd/seed
awk '/^  -/ {if (n) print n "\t" c "\t" f; n = c = f = ""}
     /name:/ {n = $2} /channel:/ {c = $2} /file:/ {f = $2}
     END {if (n) print n "\t" c "\t" f}' "$seed/seed.yaml" >"$work/seed.tsv"
for snap in firefox snap-store; do
    rm -f "$seed/snaps/${snap}_"*.snap "$seed/assertions/${snap}_"*.assert
    sed -i "/^$snap\t/d" "$work/seed.tsv"
done
seed_snap() {
    local name=$1 file need
    grep -q "^$name"$'\t' "$work/seed.tsv" && return
    snap download --channel=stable --target-directory="$work/snaps" "$name" >/dev/null
    file=$(basename "$(ls "$work/snaps/${name}_"*.snap | head -1)")
    cp "$work/snaps/$file" "$seed/snaps/"
    cp "$work/snaps/${file%.snap}.assert" "$seed/assertions/"
    printf '%s\tstable\t%s\n' "$name" "$file" >>"$work/seed.tsv"
    echo "seeded $file"
    # Its base and the snaps it names as default providers come too.
    for need in $(unsquashfs -cat "$work/snaps/$file" meta/snap.yaml |
        sed -nE 's/^base: *([a-z0-9-]+).*/\1/p; s/.*default-provider: *([a-z0-9-]+).*/\1/p' | sort -u); do
        seed_snap "$need"
    done
}
seed_snap chromium
{
    echo "snaps:"
    while IFS=$'\t' read -r name channel file; do
        printf '  -\n    name: %s\n    channel: %s\n    file: %s\n' "$name" "$channel" "$file"
    done <"$work/seed.tsv"
} >"$seed/seed.yaml"
snap debug validate-seed "$seed/seed.yaml"

step "Tidying the image"
rm -rf "$root/tmp/patrone-debs" "$root/usr/sbin/policy-rc.d" "$root/usr/sbin/update-grub"
in_root dpkg-divert --local --rename --remove /usr/sbin/update-grub >/dev/null
in_root apt-get clean
find "$root/var/lib/apt/lists" -maxdepth 1 -type f -delete
rm -f "$root"/var/cache/apt/*.bin
size=$(du -sbx "$root" | cut -f1)
unmount_all

step "Compressing Patrone's layer"
mksquashfs "$work/upper" "$work/iso/$layer.squashfs" -noappend -comp xz -b 131072 -xattrs -quiet
echo "$size" >"$work/iso/$layer.size"

step "The ISO"
# The ID is Ubuntu's default choice's, which the installer's page of app
# choices knows; the name is what it shows.
cat >"$work/iso/install-sources.yaml" <<EOF
$(grep -A1 '^kernel:' "$work/iso/install-sources.yaml")
sources:
- default: true
  description:
    en: Patrone Linux's desktop, on Ubuntu 26.04 LTS.
  id: ubuntu-desktop-minimal
  locale_support: langpack
  name:
    en: Patrone Linux
  path: $layer.squashfs
  size: $size
  type: fsimage-layered
  variant: desktop
  variations:
    patrone:
      path: $layer.squashfs
      size: $size
version: 2
EOF
sed -i -e 's/"Try or Install Ubuntu"/"Try or Install Patrone Linux"/' \
    -e 's/"Ubuntu (safe graphics)"/"Patrone Linux (safe graphics)"/' \
    "$work/iso/grub.cfg" "$work/iso/loopback.cfg"
echo "Patrone Linux 26.04.1, on Ubuntu 26.04.1 LTS \"Resolute Raccoon\" - amd64 ($(date -u +%Y%m%d))" >"$work/iso/info"

# Ubuntu's layers that nothing installs any more go; the live session keeps
# the base, the standard layer and its own.
remove=()
for f in $casper_files; do
    case $f in
        initrd | vmlinuz | install-sources.yaml) ;;
        minimal.squashfs | minimal.standard.squashfs | minimal.standard.live.squashfs) ;;
        minimal.manifest* | minimal.size | minimal.standard.manifest* | minimal.standard.size) ;;
        minimal.standard.live.*) ;;
        *) remove+=("/casper/$f") ;;
    esac
done

# The disc's checksums, which the live session checks
(cd "$work/iso" && md5sum "$layer.squashfs" "$layer.manifest" "$layer.size" install-sources.yaml grub.cfg loopback.cfg info) >"$work/new.md5"
awk -v rm="$(printf '.%s\n' "${remove[@]}")" '
    BEGIN {split(rm, r, "\n"); for (i in r) gone[r[i]] = 1}
    !($2 in gone) && $2 !~ /^\.\/(casper\/install-sources\.yaml|boot\/grub\/(grub|loopback)\.cfg|\.disk\/info)$/' \
    "$work/iso/md5sum.txt" >"$work/md5sum.txt"
sed -e "s|  $layer|  ./casper/$layer|" -e 's|  install-sources.yaml|  ./casper/install-sources.yaml|' \
    -e 's|  grub.cfg|  ./boot/grub/grub.cfg|' -e 's|  loopback.cfg|  ./boot/grub/loopback.cfg|' \
    -e 's|  info|  ./.disk/info|' "$work/new.md5" >>"$work/md5sum.txt"

volume_date=$(xorriso -indev "$ubuntu_iso" -pvd_info 2>/dev/null | sed -n 's/^Modif. Time *: *//p')
rm -f "$out"
xorriso -indev "$ubuntu_iso" -outdev "$out" -boot_image any replay \
    -volid "Patrone Linux 26.04.1 amd64" -volume_date uuid "$volume_date" \
    -rm "${remove[@]}" -- \
    -map "$work/iso/$layer.squashfs" "/casper/$layer.squashfs" \
    -map "$work/iso/$layer.manifest" "/casper/$layer.manifest" \
    -map "$work/iso/$layer.size" "/casper/$layer.size" \
    -map "$work/iso/install-sources.yaml" /casper/install-sources.yaml \
    -map "$work/iso/grub.cfg" /boot/grub/grub.cfg \
    -map "$work/iso/loopback.cfg" /boot/grub/loopback.cfg \
    -map "$work/iso/info" /.disk/info \
    -map "$work/md5sum.txt" /md5sum.txt

step "On the disc"
xorriso -indev "$out" -ls /casper 2>/dev/null
ls -l "$out"
