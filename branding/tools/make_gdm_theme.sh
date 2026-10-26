#!/bin/sh
# Builds Patrone's login-screen theme: Yaru's GNOME Shell theme with
# system/gdm/patrone-gdm.css added to the end of its login-screen stylesheet.
# The package build runs it, so the result follows the Yaru it was built
# against: rebuild patrone-branding when Yaru's shell theme changes.
#
#   tools/make_gdm_theme.sh YARU_GRESOURCE PATRONE_CSS OUTPUT_GRESOURCE
set -eu

yaru=$1
css=$2
out=$3

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

{
    echo '<?xml version="1.0" encoding="UTF-8"?>'
    echo '<gresources><gresource prefix="/">'
    for resource in $(gresource list "$yaru"); do
        mkdir -p "$work/files$(dirname "$resource")"
        gresource extract "$yaru" "$resource" > "$work/files$resource"
        echo "<file>${resource#/}</file>"
    done
    echo '</gresource></gresources>'
} > "$work/theme.gresource.xml"

stylesheet="$work/files/org/gnome/shell/theme/gdm.css"
if [ ! -f "$stylesheet" ]; then
    echo "$yaru has no /org/gnome/shell/theme/gdm.css" >&2
    exit 1
fi
cat "$css" >> "$stylesheet"

mkdir -p "$(dirname "$out")"
glib-compile-resources --sourcedir="$work/files" --target="$out" "$work/theme.gresource.xml"
