"""Recolors the password prompt's field for the boot splash: Plymouth's
spinner theme draws it (lock.png, the padlock end, and entry.png, the box)
with an orange border, which becomes Patrone Blue.

    python tools/make_prompt_images.py /usr/share/plymouth/themes/spinner system/plymouth/prompt

Only the orange moves: how orange a pixel is comes from how far its red
exceeds its blue, which is about -7 in the box's dark grey fill, 0 in the
padlock's grey and 142 in the border, and that share of the border's color
is swapped for Patrone Blue. The edges, where the border blends into the
fill or into transparency, keep their blend. The other prompt images
(bullet, capslock, keyboard, keymap-render) have no orange and are copied
as they are when the package is built.
"""

import argparse
from pathlib import Path

import numpy as np
from PIL import Image

ORANGE = np.array([239, 134, 97])
PATRONE_BLUE = np.array([0x28, 0x8B, 0xFA])
# Red minus blue: at or below GREY not orange at all, at ORANGE_SPREAD fully
GREY = 2
ORANGE_SPREAD = int(ORANGE[0] - ORANGE[2])


def recolor(image: Image.Image) -> Image.Image:
    rgba = np.asarray(image.convert("RGBA"), dtype=float)
    rgb, alpha = rgba[..., :3], rgba[..., 3:]
    orange = np.clip((rgb[..., 0] - rgb[..., 2] - GREY) / (ORANGE_SPREAD - GREY), 0, 1)[..., None]
    rgb = np.clip(rgb + orange * (PATRONE_BLUE - ORANGE), 0, 255)
    return Image.fromarray(np.dstack([rgb, alpha]).round().astype(np.uint8), "RGBA")


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("spinner", type=Path, help="Plymouth's spinner theme folder")
    parser.add_argument("output", type=Path, help="folder for lock.png and entry.png")
    args = parser.parse_args()

    args.output.mkdir(parents=True, exist_ok=True)
    for name in ("lock.png", "entry.png"):
        recolor(Image.open(args.spinner / name)).save(args.output / name, optimize=True)
        print(args.output / name)


if __name__ == "__main__":
    main()
