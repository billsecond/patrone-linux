#!/usr/bin/env bash
# Makes the icons that put Patrone's P in place of Ubuntu's logo in Yaru's
# icon theme, which every Yaru color and its dark style inherit: start-here
# in each of Yaru's sizes (Yaru's distributor-logo icons are links to it),
# and the one-color symbolic versions of both, traced with potrace.
# patrone-branding diverts Yaru's and installs these in their place.
#
#   branding/tools/make_logo_icons.sh [POTRACE]
#
# Writes branding/icons/, laid out as Yaru's folders. Needs Python's Pillow
# and NumPy, and potrace (POTRACE, default: the one on the PATH).
set -euo pipefail

branding=$(cd "$(dirname "$0")/.." && pwd)
potrace=${1:-potrace}
out=$branding/icons
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

# patrone-mark.png: the P (logos/patrone-logo.png), its edges bled with its
# own blues, so it has no dark rim on light backgrounds
"$branding/tools/make_installer_images.sh" "$work" >/dev/null

rm -rf "$out"
python3 - "$work/patrone-mark.png" "$out" "$work/mark.pbm" <<'EOF'
import sys
from pathlib import Path

import numpy as np
from PIL import Image

mark, out, bitmap = Image.open(sys.argv[1]).convert('RGBA'), Path(sys.argv[2]), sys.argv[3]
# Yaru's sizes; @2x is twice the pixels
for size in (16, 22, 24, 32, 48, 256):
    for scale, folder in ((1, f'{size}x{size}'), (2, f'{size}x{size}@2x')):
        path = out / folder / 'places/start-here.png'
        path.parent.mkdir(parents=True, exist_ok=True)
        mark.resize((size * scale,) * 2, Image.LANCZOS).save(path, optimize=True)
# The shape alone, for tracing
solid = np.asarray(mark.getchannel('A')) >= 128
Image.fromarray(~solid).convert('1').save(bitmap)
EOF

symbolic=$out/scalable/places
mkdir -p "$symbolic"
"$potrace" --svg --flat --turdsize 8 --opttolerance 0.4 -o "$work/mark.svg" "$work/mark.pbm"
# 16 by 16, as symbolic icons are drawn; GTK paints the path in the text's
# color. potrace's own comments go.
python3 - "$work/mark.svg" "$symbolic/start-here-symbolic.svg" <<'EOF'
import re
import sys

svg = open(sys.argv[1], encoding='utf-8').read()
svg = re.sub(r'<\?xml[^>]*\?>\s*|<!DOCTYPE[^>]*>\s*|<metadata>.*?</metadata>\s*', '', svg, flags=re.S)
svg = re.sub(r'width="[^"]*" height="[^"]*"', 'width="16" height="16"', svg, count=1)
svg = re.sub(r'fill="#000000"', 'fill="#bebebe"', svg)
open(sys.argv[2], 'w', encoding='utf-8').write(svg)
EOF
cp "$symbolic/start-here-symbolic.svg" "$symbolic/distributor-logo-symbolic.svg"
find "$out" -type f | sort
