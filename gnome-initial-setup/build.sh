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
revision=3
# What changed in it, for the package's changelog
changes="Patrone's build: Patrone Linux in place of Ubuntu, Patrone Software in place of App Center, and Patrone's pictures, in Patrone's blue. No password prompts from the crash reporter's settings, which Patrone doesn't have. Patrone maintains this build, with Ubuntu as the original maintainer, and the copyright file names Patrone's pictures and the Craft apps' icons in one of them."

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

# Patrone maintains this build, so Patrone is its maintainer and Ubuntu the
# original one, as Debian's guidelines for derivatives have it, and the
# copyright file names Patrone's pictures and the Craft apps' icons in
# patrone-software.svg, whose MIT licence asks for its notice
# (docs/COMPLIANCE.md, item 13), as gnome-settings/build.sh does for Settings
maintainer="William Daugherty <31995458+billsecond@users.noreply.github.com>"
sed -i \
    -e "s|^Maintainer: Ubuntu Developers <ubuntu-devel-discuss@lists.ubuntu.com>$|Maintainer: $maintainer|" \
    -e 's|^XSBC-Original-Maintainer: .*$|XSBC-Original-Maintainer: Ubuntu Developers <ubuntu-devel-discuss@lists.ubuntu.com>|' \
    debian/control
if ! grep -qxF "Maintainer: $maintainer" debian/control ||
    ! grep -qx 'XSBC-Original-Maintainer: Ubuntu Developers <.*>' debian/control; then
    echo "debian/control's maintainers aren't as this expects: update gnome-initial-setup/build.sh" >&2
    exit 1
fi
cat >> debian/copyright << 'EOF'

Files: gnome-initial-setup/pages/language/distributor-logo.svg
 gnome-initial-setup/pages/welcome/ubuntu-mascot-light.svg
 gnome-initial-setup/pages/welcome/ubuntu-mascot-dark.svg
Copyright: 2026 William Daugherty, as far as these files are protected by
 copyright
Comment: Patrone's P, in place of Ubuntu's logo and mascot, put in by
 Patrone's build (gnome-initial-setup/patrone.py in
 https://github.com/billsecond/patrone-linux), which also changes the
 setup's words and recolors its other pictures from Ubuntu's orange to
 Patrone's blue. The P comes from Patrone's logo, which was made with an AI
 image generator, so it's offered under GPL-3+ only as far as anyone holds
 copyright in it (patrone-branding's copyright file says more).
License: GPL-3+
 This program is free software: you can redistribute it and/or modify
 it under the terms of the GNU General Public License as published by
 the Free Software Foundation, either version 3 of the License, or
 (at your option) any later version.
 .
 On Debian systems, the complete text of the GNU General Public License
 version 3 can be found in "/usr/share/common-licenses/GPL-3".

Files: gnome-initial-setup/pages/apps/app-center.svg
Copyright: 2026 William Daugherty
 2026 ArtCraft Team and the PhotoCraft, PdfCraft, FilmCraft, WordCraft,
 LightCraft, VectorCraft, GridCraft, DeckCraft and SoundCraft contributors
 2026 the MailCraft contributors
 2007-2016 Richard Hughes and the GNOME Software contributors
Comment: Patrone's picture, in place of Ubuntu's App Center one
 (patrone-software.svg, drawn by gnome-initial-setup/make_software_picture.py
 in https://github.com/billsecond/patrone-linux): GNOME Software's icon
 (GPL-2+, as GNOME Software), with ten Craft apps' icons round it. The
 Craft apps' makers license them under MIT OR Apache-2.0; they're used here
 under the MIT licence, below.
License: GPL-2+ and MIT
 GNOME Software's icon: as gnome-initial-setup, above (GPL-2+).
 .
 The Craft apps' icons:
 .
 Permission is hereby granted, free of charge, to any person obtaining a
 copy of this software and associated documentation files (the
 "Software"), to deal in the Software without restriction, including
 without limitation the rights to use, copy, modify, merge, publish,
 distribute, sublicense, and/or sell copies of the Software, and to permit
 persons to whom the Software is furnished to do so, subject to the
 following conditions:
 .
 The above copyright notice and this permission notice shall be included
 in all copies or substantial portions of the Software.
 .
 THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS
 OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
 MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN
 NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM,
 DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR
 OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE
 USE OR OTHER DEALINGS IN THE SOFTWARE.
EOF

DEBFULLNAME="William Daugherty" \
DEBEMAIL="31995458+billsecond@users.noreply.github.com" \
    dch --newversion "$(dpkg-parsechangelog -S Version)patrone$revision" \
    --distribution resolute "$changes"

export DEB_BUILD_OPTIONS="${DEB_BUILD_OPTIONS:-nocheck parallel=$(nproc)}"
dpkg-buildpackage -us -uc -b "$@"

mkdir -p "$root/out"
mv ../gnome-initial-setup*_*.deb "$root/out/"
ls -l "$root"/out/gnome-initial-setup*patrone"$revision"_*.deb
