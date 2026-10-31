#!/usr/bin/env bash
# Builds Patrone's GNOME Software on the dev VM: Ubuntu's gnome-software with
# Patrone's patch, as packages whose version is Ubuntu's plus "patrone" and
# the revision below (so apt prefers them to the same Ubuntu version, and an
# Ubuntu update of GNOME Software replaces them until this is run again).
#
#   gnome-software/build.sh [UBUNTU_VERSION] [dpkg-buildpackage options]
#
# UBUNTU_VERSION is the Ubuntu version to build from (50.0-1); by default the
# newest for resolute. Fetching Ubuntu's source is the one step that needs
# the network. The packages land in out/: install gnome-software,
# gnome-software-common and the plugins you have, which go together.
# Build-Depends: Ubuntu's, which `dpkg-checkbuilddeps` in the unpacked source
# lists. Upstream's tests are off in Ubuntu's packaging too.
set -euo pipefail

# Patrone's revision of the patch: one more with each change to it, so a new
# build of the same Ubuntu version installs as an upgrade
revision=1
# What changed in it, for the package's changelog
changes="Patrone's build: a Featured tab before Explore, where Software opens, with every app the distribution's list marks with GnomeSoftware::DeploymentFeatured. Patrone maintains this build, with Debian's GNOME team as the original maintainer, and the copyright file names Patrone's patch."

here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/.." && pwd)
version=${1:-resolute}
[[ $# -gt 0 ]] && shift

work="$root/out/build/gnome-software"
rm -rf "$work"
mkdir -p "$work"
cd "$work"
pull-lp-source gnome-software "$version"
cd gnome-software-*/

cp "$here"/patches/*.patch debian/patches/
printf '%s\n' patrone-featured-tab.patch >> debian/patches/series
QUILT_PATCHES=debian/patches quilt push -a

# Patrone maintains this build, so Patrone is its maintainer and Debian's
# GNOME team the original one, as Debian's guidelines for derivatives have it
# (docs/COMPLIANCE.md, item 13), and the copyright file names Patrone's patch
maintainer="William Daugherty <31995458+billsecond@users.noreply.github.com>"
original="Debian GNOME Maintainers <pkg-gnome-maintainers@lists.alioth.debian.org>"
sed -i "s|^Maintainer: $original$|Maintainer: $maintainer\nXSBC-Original-Maintainer: $original|" debian/control
if ! grep -qxF "Maintainer: $maintainer" debian/control ||
    ! grep -qxF "XSBC-Original-Maintainer: $original" debian/control; then
    echo "debian/control's maintainers aren't as this expects: update gnome-software/build.sh" >&2
    exit 1
fi
cat >> debian/copyright << 'EOF'

Files: debian/patches/patrone-featured-tab.patch
       src/gs-featured-page.*
Copyright: 2026 William Daugherty
License: GPL-2+
EOF

DEBFULLNAME="William Daugherty" \
DEBEMAIL="31995458+billsecond@users.noreply.github.com" \
    dch --newversion "$(dpkg-parsechangelog -S Version)patrone$revision" \
    --distribution resolute "$changes"

export DEB_BUILD_OPTIONS="${DEB_BUILD_OPTIONS:-nocheck parallel=$(nproc)}"
dpkg-buildpackage -us -uc -b "$@"

mkdir -p "$root/out"
mv ../*.deb "$root/out/"
ls -l "$root"/out/gnome-software*patrone"$revision"_*.deb
