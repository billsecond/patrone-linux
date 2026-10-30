"""Enlarges the user's gallery pictures into wallpapers for 4K screens.

usage: make_gallery_wallpapers.py OUT_DIR [--sheet FILE] [--size W H] [NAME...]

Reads OUT_DIR/originals/NAME.png, the 1672x941 pictures as the user supplied
them, and writes OUT_DIR/patrone-NAME.jpg, 3840x2160 unless --size says
otherwise (all of them unless names are given). With --sheet, a contact sheet
of all of them, and of the abstract designs beside them in ../generated
(make_abstract_wallpapers.py).

  1. The picture is smoothed where it's flat, edges kept: image generators
     leave a faint two-pixel mesh in dark areas, which enlarging would make
     plain to see.
  2. It's enlarged with Lanczos and its edges given a little definition.
  3. Fine grain goes into its flat dark areas. JPEG drops a dither's fine
     detail, which leaves blotches in dark gradients, as it does with no
     dither at all; grain a little coarser than a pixel survives it.
  4. It's saved as a JPEG: PNGs of these are 6 to 12 MB, these 1 to 3.

Every "random" choice is a hash of integer coordinates, as in
make_generated_wallpapers.py, so the same picture comes out the same.
"""
import argparse
import os

import cv2
import numpy as np
from PIL import Image, ImageDraw, ImageFont

from make_generated_wallpapers import label_font, unit

# Groups, by the first word of their names, as Settings sorts them.
# NAME: name in Settings.
GALLERY = {
    "sapling-light": "Sapling in the Light",
    "sapling-cliff": "Sapling on the Cliff",
    "sapling-ledge": "Sapling on the Ledge",
    "spheres-gold": "Spheres in Gold",
    "spheres-charcoal": "Spheres in Charcoal",
    "spheres-shadow": "Spheres in Shadow",
    "storm-hill": "Storm Over the Hill",
    "storm-sunburst": "Storm Sunburst",
    "storm-clearing": "Storm Clearing",
    "woodland-path": "Woodland Path",
    "woodland-bend": "Woodland Bend",
    "woodland-sunbeams": "Woodland Sunbeams",
    "woodland-hollow": "Woodland Hollow",
}
# The abstract designs (make_abstract_wallpapers.py), on the same sheet.
# NAME: name in Settings.
ABSTRACT = {
    "silk": "Patrone Silk",
    "liquid": "Patrone Liquid",
    "nebula": "Patrone Nebula",
    "shallows": "Patrone Shallows",
    "marble": "Patrone Marble",
    "ink": "Patrone Ink",
}
QUALITY = 92
GRAIN = 1.2         # levels of 255
GRAIN_SEED = 2026


def enlarged(picture, W, H):
    flat = cv2.bilateralFilter(picture, 9, 0.05, 2.5)
    big = cv2.resize(flat, (W, H), interpolation=cv2.INTER_LANCZOS4)
    return big + 0.5 * (big - cv2.GaussianBlur(big, (0, 0), 1.5 * W / 3840))


def grain(W, H):
    """Fine noise of standard deviation 1, a little coarser than a pixel."""
    y, x = np.mgrid[0:H, 0:W]
    noise = (unit(x, y, GRAIN_SEED) + unit(x, y, GRAIN_SEED + 1) - 1).astype(np.float32)
    noise = cv2.GaussianBlur(noise, (0, 0), 0.7)
    return noise / noise.std()


def grained(img, noise):
    """img with noise in its dark, flat areas."""
    luma = img @ np.array([0.114, 0.587, 0.299], np.float32)
    busy = np.abs(img - cv2.GaussianBlur(img, (0, 0), 4)).max(axis=2)
    flat = 1 - np.clip(cv2.GaussianBlur(busy, (0, 0), 6) * 40, 0, 1)
    dark = np.clip((0.35 - luma) / 0.2, 0, 1)
    return img + (noise * dark * flat * GRAIN / 255)[..., None]


def save(path, img):
    rgb = (np.clip(img, 0, 1) * 255 + 0.5).astype(np.uint8)[..., ::-1]
    Image.fromarray(rgb).save(path, "JPEG", quality=QUALITY, subsampling=0, optimize=True, progressive=True)


def sheet(out_dir, path, tile=(480, 270), pad=12, label=26, columns=4):
    """Every wallpaper with its name, a row to each group of the gallery,
    then the abstract designs."""
    groups = {}
    for name in GALLERY:
        groups.setdefault(name.split("-")[0], []).append((out_dir, name, GALLERY[name]))
    generated = os.path.join(out_dir, os.pardir, "generated")
    designs = [(generated, name, title) for name, title in ABSTRACT.items()]
    rows = list(groups.values()) + [designs[i:i + columns] for i in range(0, len(designs), columns)]
    w, h = tile
    canvas = Image.new("RGB", (columns * (w + pad) + pad, len(rows) * (h + label + pad) + pad), (243, 246, 251))
    draw = ImageDraw.Draw(canvas)
    font = label_font(15) or ImageFont.load_default(15)
    for row, items in enumerate(rows):
        for col, (folder, name, title) in enumerate(items):
            x, y = pad + col * (w + pad), pad + row * (h + label + pad)
            with Image.open(os.path.join(folder, f"patrone-{name}.jpg")) as img:
                canvas.paste(img.convert("RGB").resize(tile, Image.LANCZOS), (x, y))
            draw.text((x, y + h + 5), title, fill=(20, 34, 58), font=font)
    canvas.save(path, "JPEG", quality=88, optimize=True)


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("out_dir")
    ap.add_argument("names", nargs="*")
    ap.add_argument("--size", nargs=2, type=int, default=(3840, 2160), metavar=("W", "H"))
    ap.add_argument("--sheet")
    args = ap.parse_args()
    W, H = args.size
    for name in args.names:
        if name not in GALLERY:
            ap.error(f"unknown wallpaper {name}; known: {', '.join(GALLERY)}")
    noise = grain(W, H)
    for name in args.names or GALLERY:
        src = os.path.join(args.out_dir, "originals", f"{name}.png")
        picture = cv2.imread(src, cv2.IMREAD_COLOR).astype(np.float32) / 255
        path = os.path.join(args.out_dir, f"patrone-{name}.jpg")
        save(path, grained(enlarged(picture, W, H), noise))
        print(path, f"{os.path.getsize(path) / 1e6:.1f} MB", flush=True)
    if args.sheet:
        sheet(args.out_dir, args.sheet)
        print(args.sheet)


if __name__ == "__main__":
    main()
