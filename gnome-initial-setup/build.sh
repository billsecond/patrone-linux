#!/usr/bin/env bash
# Builds Patrone's first-login setup on the dev VM: Ubuntu's
# gnome-initial-setup with Patrone's name, Patrone Software and Patrone's
# pictures (patrone.py), as packages whose version is Ubuntu's plus "patrone"
# and the revision below, as gnome-settings/build.sh does for Settings (so
# apt prefers them to the same Ubuntu version, and an Ubuntu update replaces
# them until this is run again).
#
#   gnome-initial-setup/build.sh [UBUNTU_VERSION] [dpkg-buildpackage options]
#
# UBUNTU_VERSION is the Ubuntu version to build from (50.0-0ubuntu7.1); by
# default the newest for resolute. Fetching Ubuntu's source is the one step
# that needs the network. The package lands in out/. Build-Depends:
# Ubuntu's (`sudo apt-get build-dep ./` in the unpacked source).
set -euo pipefail

# Patrone's revision of the changes: one more with each change to them
revision=1
# What changed in it, for the package's changelog
changes="Patrone's build: Patrone Linux in place of Ubuntu, Patrone Software in place of App Center, and Patrone's pictures, in Patrone's blue."

here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/.." && pwd)
version=${1:-resolute}
[[ $# -gt 0 ]] && shift

work="$root/out/build/gnome-initial-setup"
rm -rf "$work"
mkdir -p "$work"
cd "$work"
pull-lp-source gnome-initial-setup "$version"
cd gnome-initial-setup-*/

"$root/branding/tools/make_installer_images.sh" "$work/images" >/dev/null
python3 -I "$here/patrone.py" . "$work/images" "$here/patrone-software.svg"

DEBFULLNAME="William Daugherty" \
DEBEMAIL="31995458+billsecond@users.noreply.github.com" \
    dch --newversion "$(dpkg-parsechangelog -S Version)patrone$revision" \
    --distribution resolute "$changes"

export DEB_BUILD_OPTIONS="${DEB_BUILD_OPTIONS:-nocheck parallel=$(nproc)}"
dpkg-buildpackage -us -uc -b "$@"

mkdir -p "$root/out"
mv ../gnome-initial-setup*_*.deb "$root/out/"
ls -l "$root"/out/gnome-initial-setup*patrone"$revision"_*.deb
