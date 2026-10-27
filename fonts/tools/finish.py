#!/usr/bin/env python3
"""Finish Patrone Sans's fonts after fontmake and gftools gen-stat.

- Adds a gasp table that asks for smoothing at every size. The fonts carry
  no TrueType instructions on purpose: FreeType's autohinter then hints
  them as they're drawn, which is what GNOME's default ("slight") does
  with any TrueType font, Noto Sans's instructions included.
- Drops the Macintosh names some feature names come with (every system
  reads the Windows ones).
- Checks that no name in the font says Noto, except where the notices
  credit Noto Sans.

    finish.py FONT.ttf [FONT.ttf...]
"""

import sys

from fontTools.ttLib import TTFont, newTable

# Copyright, designer and description name Noto Sans on purpose
CREDITS = {0, 9, 10}


def main():
    status = 0
    for path in sys.argv[1:]:
        font = TTFont(path)
        for instructions in ("fpgm", "prep", "cvt "):
            if instructions in font:
                del font[instructions]
        gasp = newTable("gasp")
        gasp.version = 1
        gasp.gaspRange = {0xFFFF: 0x000F}
        font["gasp"] = gasp
        font["name"].names = [r for r in font["name"].names if r.platformID != 1]
        for record in font["name"].names:
            if record.nameID not in CREDITS and "Noto" in record.toUnicode():
                print(f"{path}: name {record.nameID} says {record.toUnicode()!r}")
                status = 1
        font.save(path)
        print(f"{path}: finished")
    sys.exit(status)


if __name__ == "__main__":
    main()
