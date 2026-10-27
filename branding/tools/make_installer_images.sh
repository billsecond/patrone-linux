#!/usr/bin/env bash
# Makes Patrone's pictures for the installer, for installer/build.sh, which
# puts them in place of Ubuntu's (installer/patch_source.py): Patrone's
# wordmark in place of Ubuntu's logo, 66 pixels tall as Ubuntu's is
# (patrone-logo.svg with dark text for the light style, patrone-logo-dark.svg
# with light text for the dark style), and the P in place of Ubuntu's mascot
# and disk icon (patrone-mark.svg, square). Each is the PNG inside an SVG, as
# make_about_logos.sh makes them.
#
#   branding/tools/make_installer_images.sh OUT-DIR
set -euo pipefail

branding=$(cd "$(dirname "$0")/.." && pwd)
out=${1:?Usage: $0 OUT-DIR}
mkdir -p "$out"

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

# The P (logos/patrone-logo.png) is drawn for Patrone's near-black: its
# edges fade to black, a dark rim on the installer's white pages. Its own
# blues, spread into the edges, keep them smooth on any background.
python3 - "$branding/logos/patrone-logo.png" "$out/patrone-mark.png" <<'EOF'
import sys
import numpy as np
from PIL import Image


def blur(channel, radius):
    kernel = np.ones(2 * radius + 1) / (2 * radius + 1)
    for axis in (0, 1):
        channel = np.apply_along_axis(np.convolve, axis, channel, kernel, mode='same')
    return channel


image = Image.open(sys.argv[1]).convert('RGBA')
pixels = np.asarray(image, dtype=float)
rgb, alpha = pixels[..., :3], pixels[..., 3]
solid = (alpha >= 250).astype(float)
bled = rgb.copy()
todo = alpha < 250
for radius in (1, 2, 4, 8):
    weight = blur(solid, radius)
    spread = np.stack([blur(rgb[..., c] * solid, radius) for c in range(3)], axis=-1)
    fill = todo & (weight > 0.01)
    bled[fill] = (spread / np.maximum(weight, 1e-6)[..., None])[fill]
    todo &= ~fill
out = np.dstack([bled, alpha]).round().clip(0, 255).astype(np.uint8)
Image.fromarray(out, 'RGBA').save(sys.argv[2])
EOF

wrap "$branding/logos/patrone-logo-text.png" "$out/patrone-logo.svg" 66
wrap "$branding/logos/patrone-logo-text-dark.png" "$out/patrone-logo-dark.svg" 66
wrap "$out/patrone-mark.png" "$out/patrone-mark.svg" 0
