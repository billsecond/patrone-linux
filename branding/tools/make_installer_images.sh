#!/usr/bin/env bash
# Makes PatroneOS's pictures for the installer, for installer/build.sh, which
# puts them in place of Ubuntu's (installer/patch_source.py), and for the
# first-login setup (gnome-initial-setup/build.sh): the logo with the P beside
# the wordmark in place of Ubuntu's logo, 66 pixels tall as Ubuntu's is
# (patrone-logo.svg with navy text for the light style, patrone-logo-dark.svg
# with white text for the dark style), and the P in place of Ubuntu's mascot
# and disk icon (patrone-mark.svg, square, with no size of its own). They're
# the vector masters in logos/ (tools/make_logos.py), resized.
#
#   branding/tools/make_installer_images.sh OUT-DIR
set -euo pipefail

branding=$(cd "$(dirname "$0")/.." && pwd)
out=${1:?Usage: $0 OUT-DIR}
mkdir -p "$out"

python3 - "$branding/logos" "$out" <<'EOF'
import re
import sys
from pathlib import Path

logos, out = Path(sys.argv[1]), Path(sys.argv[2])


def resized(source, name, height=None):
    svg = (logos / source).read_text(encoding='utf-8')
    x, y, w, h = map(float, re.search(r'viewBox="([^"]*)"', svg).group(1).split())
    if height:
        size = f' width="{round(w * height / h, 2):g}" height="{height}"'
    else:
        # No size, and the view widened to a square round the P
        side = max(w, h)
        view = f'viewBox="{x - (side - w) / 2:g} {y - (side - h) / 2:g} {side:g} {side:g}"'
        svg = re.sub(r'viewBox="[^"]*"', view, svg, count=1)
        size = ''
    svg = re.sub(r' width="[^"]*" height="[^"]*"', size, svg, count=1)
    (out / name).write_text(svg, encoding='utf-8')
    print(out / name)


resized('patrone-logo-text.svg', 'patrone-logo.svg', 66)
resized('patrone-logo-text-dark.svg', 'patrone-logo-dark.svg', 66)
resized('patrone-mark.svg', 'patrone-mark.svg')
EOF
