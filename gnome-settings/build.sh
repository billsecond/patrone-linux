#!/usr/bin/env bash
# Builds Patrone's GNOME Settings on the dev VM: Ubuntu's gnome-control-center
# with Patrone's page and patches, as packages whose version is Ubuntu's
# plus "patrone" and the revision below (so apt prefers them to the same
# Ubuntu version, and an Ubuntu update of Settings replaces them until this
# is run again).
#
#   gnome-settings/build.sh [UBUNTU_VERSION] [dpkg-buildpackage options]
#
# UBUNTU_VERSION is the Ubuntu version to build from (1:50.3-0ubuntu0.2);
# by default the newest for resolute. Fetching Ubuntu's source is the one
# step that needs the network. The packages land in out/: install
# gnome-control-center and gnome-control-center-data, which go together.
# Build-Depends: Ubuntu's, which `dpkg-checkbuilddeps` in the unpacked
# source lists. Tests are skipped unless DEB_BUILD_OPTIONS says otherwise.
set -euo pipefail

# Patrone's revision of the page and patches: one more with each change to
# them, so a new build of the same Ubuntu version installs as an upgrade
revision=14
# What changed in it, for the package's changelog
changes="Patrone's build: a Patrone page (taskbar size, layout, Start button, color and pin icons, desktop bubble, Recycle Bin and the terminal at sign-in, title bar button position and style, theme, font), the desktop bubble's and Recycle Bin's switches on the Desktop page in place of Ubuntu's desktop icons and dock settings, no Telemetry page, Patrone's logo on the About page, and Patrone Remote Desktop first in System > Remote Desktop: a \"Remote Desktop\" switch, how to connect, and what Desktop Sharing, Secure Shell and GNOME's Remote Login do. Patrone maintains this build, with Ubuntu as the original maintainer, and the copyright file names Patrone's pages."

here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/.." && pwd)
version=${1:-resolute}
[[ $# -gt 0 ]] && shift

work="$root/out/build/gnome-settings"
rm -rf "$work"
mkdir -p "$work"
cd "$work"
pull-lp-source gnome-control-center "$version"
cd gnome-control-center-*/

# Patrone's page, as a patch that adds its files, then the patches to
# Ubuntu's own files
added=patrone-add-patrone-panel.patch
{
    printf 'Description: Add Patrone'\''s page: the taskbar, title bar buttons and theme\n'
    printf ' From gnome-settings/panels/patrone in Patrone'\''s repository.\n'
    printf 'Author: William Daugherty <31995458+billsecond@users.noreply.github.com>\n'
    printf 'Forwarded: not-needed\n---\n'
    for file in "$here"/panels/patrone/*; do
        name=panels/patrone/$(basename "$file")
        diff -u --label /dev/null --label "b/$name" /dev/null "$file" || true
    done
} > "debian/patches/$added"
# Remote Login's Patrone Remote Desktop page, the same way
added_remote_login=patrone-add-remote-login.patch
{
    printf 'Description: Add Patrone Remote Desktop'\''s part of the Remote Login page\n'
    printf ' From gnome-settings/remote-login in Patrone'\''s repository.\n'
    printf 'Author: William Daugherty <31995458+billsecond@users.noreply.github.com>\n'
    printf 'Forwarded: not-needed\n---\n'
    for file in "$here"/remote-login/*; do
        name=panels/system/remote-desktop/$(basename "$file")
        diff -u --label /dev/null --label "b/$name" /dev/null "$file" || true
    done
} > "debian/patches/$added_remote_login"
cp "$here"/patches/*.patch debian/patches/
printf '%s\n' "$added" patrone-register-patrone-panel.patch patrone-hide-telemetry.patch \
    "$added_remote_login" patrone-remote-login.patch patrone-desktop-switches.patch \
    >> debian/patches/series
QUILT_PATCHES=debian/patches quilt push -a

# The About page's logo: Ubuntu's build sets it to Ubuntu's own logo files
# (distributor_logo in debian/rules), so os-release's LOGO is never read and
# the page showed Ubuntu's (the user found it, 2026-10-27). Patrone's, from
# patrone-branding, take their place.
sed -i \
    -e 's|^\(\s*DISTRIBUTOR_LOGO := \)/usr/share/pixmaps/ubuntu-logo-text\.svg$|\1/usr/share/pixmaps/patrone-logo-text.svg|' \
    -e 's|^\(\s*DISTRIBUTOR_LOGO_DARK := \)/usr/share/pixmaps/ubuntu-logo-text-dark\.svg$|\1/usr/share/pixmaps/patrone-logo-text-dark.svg|' \
    debian/rules
if ! grep -q 'patrone-logo-text\.svg$' debian/rules || ! grep -q 'patrone-logo-text-dark\.svg$' debian/rules; then
    echo "debian/rules no longer names Ubuntu's logo as this expects: update gnome-settings/build.sh" >&2
    exit 1
fi

# Patrone maintains this build, so Patrone is its maintainer and Ubuntu the
# original one, as Debian's guidelines for derivatives have it, and the
# copyright file names Patrone's own files (docs/COMPLIANCE.md, item 13).
maintainer="William Daugherty <31995458+billsecond@users.noreply.github.com>"
sed -i \
    -e "s|^Maintainer: Ubuntu Developers <ubuntu-devel-discuss@lists.ubuntu.com>$|Maintainer: $maintainer|" \
    -e 's|^XSBC-Original-Maintainer: .*$|XSBC-Original-Maintainer: Ubuntu Developers <ubuntu-devel-discuss@lists.ubuntu.com>|' \
    debian/control
if ! grep -qxF "Maintainer: $maintainer" debian/control ||
    ! grep -qx 'XSBC-Original-Maintainer: Ubuntu Developers <.*>' debian/control; then
    echo "debian/control's maintainers aren't as this expects: update gnome-settings/build.sh" >&2
    exit 1
fi
# The stanza goes with the other Files paragraphs, before the license texts
stanza="Files: panels/patrone/*
 panels/system/remote-desktop/cc-patrone-remote-login.*
Copyright: 2026 William Daugherty
License: GPL-2+
Comment: Patrone Linux's own pages, added by Patrone's build (gnome-settings/
 in https://github.com/billsecond/patrone-linux)."
if ! stanza=$stanza awk '!done && previous == "" && /^License:/ { print ENVIRON["stanza"]; print ""; done = 1 }
        { print; previous = $0 }
        END { exit !done }' debian/copyright > debian/copyright.patrone; then
    echo "debian/copyright has no license texts where this expects: update gnome-settings/build.sh" >&2
    exit 1
fi
mv debian/copyright.patrone debian/copyright

DEBFULLNAME="William Daugherty" \
DEBEMAIL="31995458+billsecond@users.noreply.github.com" \
    dch --newversion "$(dpkg-parsechangelog -S Version)patrone$revision" \
    --distribution resolute "$changes"

export DEB_BUILD_OPTIONS="${DEB_BUILD_OPTIONS:-nocheck parallel=$(nproc)}"
dpkg-buildpackage -us -uc -b "$@"

mkdir -p "$root/out"
mv ../gnome-control-center*_*.deb "$root/out/"
ls -l "$root"/out/gnome-control-center*patrone"$revision"_*.deb
