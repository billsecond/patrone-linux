#!/usr/bin/env python3
"""Draw a quick proof of Patrone Sans's masters, straight from the drawing
code, to look at the letters while drawing them.

    preview.py OUT.png [--masters Light,Regular,Black] [--text TEXT]...
                       [--big GLYPHS] [--size PX]

Each master is compiled on its own (ufo2ft, overlaps kept) and drawn with
FreeType, light hinting and greyscale smoothing, as GNOME draws text. Big
lines show the x-height, capital height, ascender and descender. Needs
freetype-py and Pillow besides the build's tools.
"""

import argparse
import sys
import tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "source"))

import freetype  # noqa: E402
import ufo2ft  # noqa: E402
from PIL import Image, ImageChops, ImageDraw, ImageOps  # noqa: E402

import glyph as G  # noqa: E402
import make_ufos  # noqa: E402

DEFAULT_TEXT = [
    "nohbdpqaecsrumijltfkvwxyzg",
    "minimum handgloves often quickly",
    "the five boxing wizards jump quickly",
]


def compile_master(master):
    font = make_ufos.master_font(master)
    ttf = ufo2ft.compileTTF(font, removeOverlaps=False, reverseDirection=True)
    path = Path(tempfile.gettempdir()) / f"patrone-preview-{master.name}.ttf"
    ttf.save(path)
    return path


def draw_text(img, face, text, x, y, px, ink=0):
    """draw text with its baseline at y; returns the advance"""
    face.set_pixel_sizes(0, px)
    pen = x * 64
    for ch in text:
        face.load_char(ch, freetype.FT_LOAD_RENDER | freetype.FT_LOAD_TARGET_LIGHT)
        slot = face.glyph
        bm = slot.bitmap
        if bm.width and bm.rows:
            cover = Image.frombytes("L", (bm.width, bm.rows), bytes(bm.buffer), "raw", "L", bm.pitch)
            ink = ImageOps.invert(cover)
            left = pen // 64 + slot.bitmap_left
            top = y - slot.bitmap_top
            box = (left, top, left + bm.width, top + bm.rows)
            img.paste(ImageChops.darker(img.crop(box), ink), box)
        pen += slot.advance.x
    return pen / 64 - x


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("out")
    ap.add_argument("--masters", default="Light,Regular,Black")
    ap.add_argument("--text", action="append")
    ap.add_argument("--big", default=None, help="glyphs to draw big, with metric lines")
    ap.add_argument("--size", type=int, default=150, help="pixels per em of the big line")
    ap.add_argument("--width", type=int, default=2400)
    args = ap.parse_args()
    texts = args.text or DEFAULT_TEXT
    wanted = args.masters.split(",")
    masters = [m for m in G.MASTERS if m.name in wanted]
    big = args.big if args.big is not None else texts[0]

    rows = []
    for m in masters:
        rows.append(("big", m, big))
        for px in (48, 24):
            for t in texts[1:]:
                rows.append(("text", m, t, px))
        for px in (15, 13):
            rows.append(("text", m, " ".join(texts), px))
    heights = {"big": int(args.size * 1.25), "text": None}

    def h(row):
        return heights["big"] if row[0] == "big" else int(row[3] * 1.45)

    total = sum(h(r) for r in rows) + 40
    img = Image.new("L", (args.width, total), 255)
    draw = ImageDraw.Draw(img)
    faces = {m.name: freetype.Face(str(compile_master(m))) for m in masters}
    y = 20
    for row in rows:
        face = faces[row[1].name]
        if row[0] == "big":
            base = y + int(args.size * 0.95)
            s = args.size / G.Master.UPM
            for v, shade in ((0, 150), (G.Master.X, 200), (G.Master.C, 200), (G.Master.A, 220),
                             (G.Master.D, 220)):
                draw.line([(10, base - v * s), (args.width - 10, base - v * s)], fill=shade)
            draw_text(img, face, row[2], 20, base, args.size)
        else:
            px = row[3]
            draw_text(img, face, row[2], 20, y + int(px * 1.05), px)
        y += h(row)
    img.save(args.out)
    print(args.out)


if __name__ == "__main__":
    main()
