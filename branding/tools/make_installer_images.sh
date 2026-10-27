#!/usr/bin/env bash
# Makes the installer's images (system/desktop-provision/, see
# whitelabel.yaml there): Patrone's wordmark in place of Ubuntu's logo, 66
# pixels tall as Ubuntu's is (patrone-logo.svg with dark text for the light
# style, patrone-logo-dark.svg with light text for the dark style), and the
# P in place of Ubuntu's mascot (patrone-mark.svg, square). Each is the PNG
# inside an SVG, as make_about_logos.sh makes them.
#
#   branding/tools/make_installer_images.sh
set -euo pipefail

branding=$(cd "$(dirname "$0")/.." && pwd)
out=$branding/system/desktop-provision

wrap() { # PNG OUT HEIGHT (0: no size, only a viewBox)
    local size w h width
    size=$(file "$1" | grep -o '[0-9]* x [0-9]*')
    w=${size% x *}
    h=${size#* x }
    {
        printf '<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" '
        if [ "$3" != 0 ]; then
            width=$(( (w * $3 + h / 2) / h ))
            printf 'width="%s" height="%s" ' "$width" "$3"
        fi
        printf 'viewBox="0 0 %s %s">' "$w" "$h"
        printf '<image width="%s" height="%s" xlink:href="data:image/png;base64,' "$w" "$h"
        base64 -w0 "$1"
        printf '"/></svg>\n'
    } >"$2"
    echo "$2"
}

wrap "$branding/logos/patrone-logo-text.png" "$out/patrone-logo.svg" 66
wrap "$branding/logos/patrone-logo-text-dark.png" "$out/patrone-logo-dark.svg" 66
wrap "$branding/logos/patrone-logo.png" "$out/patrone-mark.svg" 0
