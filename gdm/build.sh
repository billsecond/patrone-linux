#!/usr/bin/env bash
# Builds Patrone's GDM on the dev VM: Ubuntu's gdm3 with Patrone's patches,
# as packages whose version is Ubuntu's plus "patrone" and the revision below
# (so apt prefers them to the same Ubuntu version, and an Ubuntu update of
# GDM replaces them until this is run again).
#
#   gdm/build.sh [UBUNTU_VERSION] [dpkg-buildpackage options]
#
# UBUNTU_VERSION is the Ubuntu version to build from (50.1-0ubuntu0.1); by
# default the newest for resolute. Fetching Ubuntu's source is the one step
# that needs the network. The packages land in out/: install gdm3, libgdm1
# and gir1.2-gdm-1.0, which go together. Build-Depends: Ubuntu's, which
# `dpkg-checkbuilddeps` in the unpacked source lists. Tests are skipped
# unless DEB_BUILD_OPTIONS says otherwise.
set -euo pipefail

# Patrone's revision of the patches: one more with each change to them, so a
# new build of the same Ubuntu version installs as an upgrade
revision=1
# What changed in it, for the package's changelog
changes="Patrone's build: Hyper-V's synthetic video counts as the boot display, so on a generation 2 VM the login screen no longer waits 10 seconds for a primary GPU."

here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/.." && pwd)
version=${1:-resolute}
[[ $# -gt 0 ]] && shift

work="$root/out/build/gdm"
rm -rf "$work"
mkdir -p "$work"
cd "$work"
pull-lp-source gdm3 "$version"
cd gdm3-*/

cp "$here"/patches/*.patch debian/patches/
printf '%s\n' patrone-hyperv-boot-display.patch >> debian/patches/series
QUILT_PATCHES=debian/patches quilt push -a

DEBFULLNAME="William Daugherty" \
DEBEMAIL="31995458+billsecond@users.noreply.github.com" \
    dch --newversion "$(dpkg-parsechangelog -S Version)patrone$revision" \
    --distribution resolute "$changes"

export DEB_BUILD_OPTIONS="${DEB_BUILD_OPTIONS:-nocheck parallel=$(nproc)}"
dpkg-buildpackage -us -uc -b "$@"

mkdir -p "$root/out"
mv ../*.deb "$root/out/"
ls -l "$root"/out/*gdm*patrone"$revision"_*.deb
