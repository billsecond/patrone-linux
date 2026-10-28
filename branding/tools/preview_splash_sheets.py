"""Turns preview_splash.sh's screen dumps (SIZE-MODE.xwd.gz) into PNGs and
one contact sheet per screen size, every mode labeled.

    python tools/preview_splash_sheets.py FOLDER

Xvfb writes its screen as an XWD file; this reads the 32-bit kind it writes.
"""

import gzip
import struct
import sys
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw

MODES = ["boot", "boot-message", "boot-password", "boot-question", "shutdown", "reboot",
         "updates", "system-upgrade", "firmware-upgrade", "system-reset"]
TILE_WIDTH = 640


def read_xwd(path: Path) -> Image.Image:
    data = gzip.open(path).read()
    fields = struct.unpack(">25I", data[:100])
    header_size, width, height = fields[0], fields[4], fields[5]
    byte_order, bits_per_pixel, bytes_per_line = fields[7], fields[11], fields[12]
    masks = fields[14:17]
    colormap_entries = fields[19]
    if bits_per_pixel != 32:
        raise SystemExit(f"{path}: {bits_per_pixel} bits per pixel; only 32 is read")
    start = header_size + colormap_entries * 12
    dtype = "<u4" if byte_order == 0 else ">u4"
    rows = np.frombuffer(data[start:start + bytes_per_line * height], dtype=dtype)
    pixels = rows.reshape(height, -1)[:, :width]
    channels = [((pixels & mask) >> ((mask & -mask).bit_length() - 1)).astype(np.uint8) for mask in masks]
    return Image.fromarray(np.dstack(channels), "RGB")


def main():
    folder = Path(sys.argv[1])
    sizes = sorted({p.name.split("-")[0] for p in folder.glob("*.xwd.gz")},
                   key=lambda size: [int(n) for n in size.split("x")])
    for size in sizes:
        tiles = []
        for mode in MODES:
            path = folder / f"{size}-{mode}.xwd.gz"
            if path.exists():
                image = read_xwd(path)
                image.save(folder / f"{size}-{mode}.png")
                tiles.append((mode, image))
        width, height = tiles[0][1].size
        tile_height = round(height * TILE_WIDTH / width)
        rows = (len(tiles) + 1) // 2
        sheet = Image.new("RGB", (TILE_WIDTH * 2 + 30, (tile_height + 28) * rows + 10), (40, 40, 40))
        draw = ImageDraw.Draw(sheet)
        for i, (mode, image) in enumerate(tiles):
            x, y = 10 + (i % 2) * (TILE_WIDTH + 10), 10 + (i // 2) * (tile_height + 28)
            draw.text((x, y), f"{size}  {mode}", fill=(220, 220, 220))
            sheet.paste(image.resize((TILE_WIDTH, tile_height), Image.LANCZOS), (x, y + 16))
        sheet.save(folder / f"sheet-{size}.png")
        print(folder / f"sheet-{size}.png")


if __name__ == "__main__":
    main()
