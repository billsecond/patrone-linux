#!/usr/bin/env python3
"""Renders every icon at 16, 24, 32, 48 and 96 pixels into one PNG, on a light
and a dark background, to check the artwork:

    python3 tools/preview_icons.py data/icons preview.png

Uses GdkPixbuf's SVG loader (librsvg, as GTK does) and Pillow.
"""

import glob
import math
import os
import sys

import gi

gi.require_version("GdkPixbuf", "2.0")
from gi.repository import GdkPixbuf  # noqa: E402
from PIL import Image, ImageDraw, ImageFont  # noqa: E402

SIZES = [16, 24, 32, 48, 96]
CELL_W = sum(SIZES) + 8 * len(SIZES) + 16
CELL_H = 96 + 26
COLUMNS = 4


def render(path, size):
    pixbuf = GdkPixbuf.Pixbuf.new_from_file_at_scale(path, size, size, True)
    if not pixbuf.get_has_alpha():
        pixbuf = pixbuf.add_alpha(False, 0, 0, 0)
    return Image.frombytes(
        "RGBA",
        (pixbuf.get_width(), pixbuf.get_height()),
        pixbuf.get_pixels(),
        "raw",
        "RGBA",
        pixbuf.get_rowstride(),
    )


def main():
    folder, out = sys.argv[1], sys.argv[2]
    files = sorted(glob.glob(os.path.join(folder, "*.svg")))
    app_icon = os.path.join(folder, "..", "org.patrone.FileManager.svg")
    if os.path.exists(app_icon):
        files.append(app_icon)
    rows = math.ceil(len(files) / COLUMNS)
    width = CELL_W * COLUMNS
    height = CELL_H * rows
    sheet = Image.new("RGBA", (width, height * 2))
    draw = ImageDraw.Draw(sheet)
    try:
        font = ImageFont.truetype("/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf", 12)
    except OSError:
        font = ImageFont.load_default()
    for band, (background, text) in enumerate([("#FFFFFF", "#1A1A1A"), ("#1C1C1C", "#E6E6E6")]):
        draw.rectangle([0, band * height, width, (band + 1) * height], fill=background)
        for index, path in enumerate(files):
            x = (index % COLUMNS) * CELL_W + 8
            y0 = band * height + (index // COLUMNS) * CELL_H + 4
            label_x = x
            for size in SIZES:
                icon = render(path, size)
                sheet.alpha_composite(icon, (x, y0 + 96 - icon.height))
                x += size + 8
            draw.text((label_x, y0 + 98), os.path.basename(path)[:-4], fill=text, font=font)
    sheet.save(out)


if __name__ == "__main__":
    main()
