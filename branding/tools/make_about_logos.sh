#!/usr/bin/env bash
# Makes the logos for Settings → About: logos/patrone-logo-text.svg (dark
# text, for the light style) and patrone-logo-text-dark.svg (light text, for
# the dark style). Each is the full-size wordmark PNG inside an SVG that's
# 72 pixels tall, the height Ubuntu's logo has there: the About page shows
# its logo at its own size, and the PNGs are 248 tall. The PNG inside keeps
# it sharp on HiDPI screens.
#
#   branding/tools/make_about_logos.sh
set -euo pipefail

logos=$(cd "$(dirname "$0")/../logos" && pwd)
height=72
for name in patrone-logo-text patrone-logo-text-dark; do
    png=$logos/$name.png
    size=$(file "$png" | grep -o '[0-9]* x [0-9]*')
    w=${size% x *}
    h=${size#* x }
    width=$(( (w * height + h / 2) / h ))
    {
        printf '<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" '
        printf 'width="%s" height="%s" viewBox="0 0 %s %s">' "$width" "$height" "$w" "$h"
        printf '<image width="%s" height="%s" xlink:href="data:image/png;base64,' "$w" "$h"
        base64 -w0 "$png"
        printf '"/></svg>\n'
    } > "$logos/$name.svg"
    echo "$logos/$name.svg: ${width}x$height"
done
