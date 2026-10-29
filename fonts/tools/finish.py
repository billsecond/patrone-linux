#!/usr/bin/env python3
"""Finish Patrone Sans's variable font after fontmake.

- Marks every glyph's outlines as overlapping (OVERLAP_SIMPLE, and
  OVERLAP_COMPOUND on accented letters): Patrone Sans's strokes overlap
  where they meet, as variable fonts may, and the flag tells rasterizers
  (FreeType, from 2.10.3) to smooth such outlines carefully.
- Adds a STAT table naming the six weights, Regular the one a name may
  leave out.
- Adds a gasp table that asks for smoothing at every size, and a prep
  program that only turns on dropout control (for rasterizers that run
  TrueType programs). The font carries no hinting on purpose: FreeType's
  autohinter hints it as drawn, which is what GNOME's default ("slight")
  does with any TrueType font.
- Adds an avar table that maps the weight axis as it is.
- Drops the Macintosh names (every system reads the Windows ones), and
  checks that no name says Noto, or Linux (the system was renamed).

    finish.py FONT.ttf
"""

import sys

from fontTools.otlLib.builder import buildStatTable
from fontTools.ttLib import TTFont, newTable
from fontTools.ttLib.tables import ttProgram
from fontTools.ttLib.tables._g_l_y_f import OVERLAP_COMPOUND, flagOverlapSimple

WEIGHTS = [(300, "Light"), (400, "Regular"), (500, "Medium"), (600, "SemiBold"), (700, "Bold"),
           (900, "Black")]


def main():
    status = 0
    for path in sys.argv[1:]:
        font = TTFont(path)
        for instructions in ("fpgm", "cvt "):
            if instructions in font:
                del font[instructions]
        glyf = font["glyf"]
        for name in font.getGlyphOrder():
            g = glyf[name]
            if g.isComposite():
                g.components[0].flags |= OVERLAP_COMPOUND
            elif g.numberOfContours > 0:
                g.flags[0] |= flagOverlapSimple
        values = []
        for value, label in WEIGHTS:
            v = {"value": value, "name": label}
            if value == 400:
                v["flags"] = 0x2  # elidable
                v["linkedValue"] = 700
            values.append(v)
        buildStatTable(font, [{"tag": "wght", "name": "Weight", "values": values}],
                       elidedFallbackName="Regular")
        prep = newTable("prep")
        prep.program = ttProgram.Program()
        prep.program.fromAssembly(["PUSHW[]", "511", "SCANCTRL[]", "PUSHB[]", "4", "SCANTYPE[]"])
        font["prep"] = prep
        if "avar" not in font:
            avar = newTable("avar")
            avar.segments = {"wght": {-1.0: -1.0, 0.0: 0.0, 1.0: 1.0}}
            font["avar"] = avar
        gasp = newTable("gasp")
        gasp.version = 1
        gasp.gaspRange = {0xFFFF: 0x000F}
        font["gasp"] = gasp
        font["name"].names = [r for r in font["name"].names if r.platformID != 1]
        for record in font["name"].names:
            text = record.toUnicode()
            if "Noto" in text or "Linux" in text:
                print(f"{path}: name {record.nameID} says {text!r}")
                status = 1
        font.save(path)
        print(f"{path}: finished")
    sys.exit(status)


if __name__ == "__main__":
    main()
