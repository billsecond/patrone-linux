#!/usr/bin/env bash
# Copies GNOME's own pages of GNOME Help, in every language GNOME has them,
# for the ones Ubuntu takes out of its gnome-user-docs package and replaces
# in ubuntu-docs ("Ubuntu Desktop Guide", with Ubuntu's logo): the front
# page and the pages about the guide and getting help. patrone-branding
# installs them in ubuntu-docs' place, so Help is GNOME Help.
#
#   branding/tools/make_help.sh
#
# Writes branding/help/LANGUAGE/gnome-help/. Needs curl and a network: it
# fetches GNOME's release of the version Ubuntu has (gnome-user-docs 50.0),
# checked against its published SHA-256.
set -euo pipefail

version=50.0
sha256=e8e23324184c7e985c504f05f6d9c63420c9a9d1f64efde5da2a811236072f78
# The pages in GNOME's release that Ubuntu's gnome-user-docs 50.0-0ubuntu1
# leaves out
pages="about-this-guide get-involved help-irc help-matrix index more-help
shell-apps-auto-start translate"

branding=$(cd "$(dirname "$0")/.." && pwd)
out=$branding/help
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

tarball=gnome-user-docs-$version.tar.xz
curl -fsSL -o "$work/$tarball" "https://download.gnome.org/sources/gnome-user-docs/${version%%.*}/$tarball"
echo "$sha256  $work/$tarball" | sha256sum -c --quiet
tar -C "$work" -xf "$work/$tarball"
help=$work/gnome-user-docs-$version/gnome-help

rm -rf "$out"
for dir in "$help"/*/; do
    lang=$(basename "$dir")
    [ -f "$dir/index.page" ] || continue
    mkdir -p "$out/$lang/gnome-help"
    for page in $pages; do
        cp "$dir/$page.page" "$out/$lang/gnome-help/"
    done
done
echo "$(find "$out" -name '*.page' | wc -l) pages in $(ls "$out" | wc -l) languages"
