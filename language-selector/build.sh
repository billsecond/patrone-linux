#!/usr/bin/env bash
# Builds Patrone's Language Support on the dev VM: Ubuntu's language-selector
# without its words that point at Ubuntu (README.md here), as packages
# whose version is Ubuntu's plus "patrone" and the revision below, as
# gnome-initial-setup/build.sh does for first-login setup (so apt prefers
# them to the same Ubuntu version, and an Ubuntu update replaces them until
# this is run again).
#
#   language-selector/build.sh [UBUNTU_VERSION] [dpkg-buildpackage options]
#
# UBUNTU_VERSION is the Ubuntu version to build from (0.228build1); by
# default the newest for resolute. Fetching Ubuntu's source is the one step
# that needs the network. The packages land in out/: install
# language-selector-common and language-selector-gnome, which go together.
# Build-Depends: Ubuntu's (`sudo apt-get build-dep ./` in the unpacked
# source).
set -euo pipefail

# Patrone's revision of the changes: one more with each change to them
revision=2
# What changed in it, for the package's changelog
changes="Patrone's build: the input method's tip, the error message and the help name PatroneOS, or nobody, where they named Ubuntu or Launchpad. Patrone maintains this build, with Ubuntu as the original maintainer."

here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/.." && pwd)
version=${1:-resolute}
[[ $# -gt 0 ]] && shift

work="$root/out/build/language-selector"
rm -rf "$work"
mkdir -p "$work"
cd "$work"
pull-lp-source language-selector "$version"
cd language-selector-*/

# edit FILE OLD NEW: OLD, which must be there, replaced by NEW everywhere
edit() {
    OLD=$2 NEW=$3 perl -0777 -pi -e 's/\Q$ENV{OLD}\E/$ENV{NEW}/g or
        die "$ARGV no longer has \"$ENV{OLD}\": update language-selector/build.sh\n"' "$1"
}

# The keyboard input method's tip ("The recommended value for Ubuntu is
# IBus"): IBus is what Patrone's desktop uses too (docs/COMPLIANCE.md,
# item 18)
edit data/LanguageSelector.ui 'The recommended value for Ubuntu is "IBus".' \
    'The recommended value is "IBus".'
# A failed install asked for a bug report in Launchpad, Ubuntu's bug tracker
edit LanguageSelector/gtk/GtkLanguageSelector.py \
    '_("This is perhaps a bug of this application. Please "
                         "file a bug report at "
                         "https://bugs.launchpad.net/ubuntu/+source/language-selector/+filebug")' \
    '_("This is perhaps a bug of this application.")'
# The help described Ubuntu. What it says is as true of PatroneOS, which
# is built on Ubuntu; its credits and licence (CC BY-SA 2.5) stay, with a
# line on what Patrone changed, as the licence asks.
help=help/C/index.docbook
edit "$help" 'Ubuntu has been translated' 'PatroneOS has been translated'
edit "$help" 'most Ubuntu users' 'most users'
edit "$help" '<para>Ubuntu uses the ' '<para>PatroneOS uses the '
edit "$help" 'Ubuntu with GNOME' 'PatroneOS'
edit "$help" '		  <para>This documentation is distributed in the hope' \
    '		  <para>PatroneOS changed this guide to name PatroneOS where it named Ubuntu.</para>

		  <para>This documentation is distributed in the hope'
if grep -n 'Ubuntu' "$help" | grep -v -e 'Ubuntu Documentation Contributors' \
    -e 'the Ubuntu documentation source code' -e 'PatroneOS changed this guide' >&2; then
    echo "$help names Ubuntu where this doesn't expect it: update language-selector/build.sh" >&2
    exit 1
fi

# Patrone maintains this build, so Patrone is its maintainer and Ubuntu the
# original one, as Debian's guidelines for derivatives have it
# (docs/COMPLIANCE.md, item 13)
maintainer="William Daugherty <31995458+billsecond@users.noreply.github.com>"
sed -i \
    -e "s|^Maintainer: Ubuntu Developers <ubuntu-devel-discuss@lists.ubuntu.com>$|Maintainer: $maintainer\nXSBC-Original-Maintainer: Ubuntu Developers <ubuntu-devel-discuss@lists.ubuntu.com>|" \
    debian/control
if ! grep -qxF "Maintainer: $maintainer" debian/control ||
    [ "$(grep -c '^XSBC-Original-Maintainer: Ubuntu Developers <.*>$' debian/control)" != 1 ]; then
    echo "debian/control's maintainers aren't as this expects: update language-selector/build.sh" >&2
    exit 1
fi

DEBFULLNAME="William Daugherty" \
DEBEMAIL="31995458+billsecond@users.noreply.github.com" \
    dch --newversion "$(dpkg-parsechangelog -S Version)patrone$revision" \
    --distribution resolute "$changes"

export DEB_BUILD_OPTIONS="${DEB_BUILD_OPTIONS:-nocheck}"
dpkg-buildpackage -us -uc -b "$@"

mkdir -p "$root/out"
mv ../language-selector-*_*.deb "$root/out/"
ls -l "$root"/out/language-selector-*patrone"$revision"_*.deb
