#!/usr/bin/env python3
"""Draw Patrone Sans's masters and write them as UFO sources, with the
designspace fontmake builds the variable font from.

    make_ufos.py OUT_DIR
"""

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import ufoLib2  # noqa: E402

import engine  # noqa: E402
import glyph as G  # noqa: E402
import lower  # noqa: E402,F401  (each module registers its glyphs)
import upper  # noqa: E402,F401
import figures  # noqa: E402,F401
import punct  # noqa: E402,F401
import marks  # noqa: E402,F401
import composites  # noqa: E402

composites.add_composites()
MARKS = set()  # the combining marks, for GDEF


def plan_pieces():
    """draw every master until every outline follows its centre line
    closely enough, splitting the curves that don't (engine.PIECES)"""
    if getattr(plan_pieces, "done", False):
        return
    for _ in range(4):
        engine.ERRORS.clear()
        for master in G.MASTERS:
            _draw(master)
        more = {k for k, e in engine.ERRORS.items() if e > engine.TOLERANCE}
        if not more:
            break
        for key in more:
            engine.PIECES[key] = engine.PIECES.get(key, 1) + 1
    plan_pieces.done = True


def drawn(master):
    plan_pieces()
    return _draw(master)


def _draw(master):
    """every glyph drawn for one master: name -> (unicodes, contours,
    components, advance, anchors)"""
    out = {}
    for name, (unicodes, upper, f) in G.GLYPHS.items():
        g = G.Draw(master, name, upper=upper)
        g.glyphs = out
        f(g)
        if getattr(g, "mark", False):
            MARKS.add(name)
        contours, advance, anchors, _ = g.finish()
        out[name] = (unicodes, contours, g.components, advance, anchors)
    return out


def write_glyph(font, name, unicodes, contours, components, advance, anchors):
    gl = font.newGlyph(name)
    gl.width = round(advance)
    gl.unicodes = unicodes
    pen = gl.getPen()
    r = lambda p: (round(p[0]), round(p[1]))  # noqa: E731
    for contour in contours:
        start = r(contour[0][1])
        pen.moveTo(start)
        for i, seg in enumerate(contour):
            end = start if i == len(contour) - 1 else r(seg[-1])
            if seg[0] == "L":
                pen.lineTo(end)
            else:
                pen.curveTo(r(seg[2]), r(seg[3]), end)
        pen.closePath()
    for base, dx, dy in components:
        pen.addComponent(base, (1, 0, 0, 1, round(dx), round(dy)))
    for key, (x, y) in anchors.items():
        gl.appendAnchor({"name": key, "x": round(x), "y": round(y)})
    return gl


VERSION = (2, 0)
COPYRIGHT = "Copyright 2026 William Daugherty (https://github.com/billsecond/patroneos)"
DESCRIPTION = (
    "Patrone Sans is PatroneOS's interface typeface, a humanist sans drawn from scratch: "
    "every letter is a pen stroke along a skeleton, outlined by Patrone's own code "
    "(fonts/source), with AI help. No outline or measurement comes from another font."
)
LICENSE = (
    "This Font Software is licensed under the SIL Open Font License, Version 1.1. "
    "This license is available with a FAQ at: https://openfontlicense.org"
)
INSTANCES = [("Light", 300), ("Regular", 400), ("Medium", 500), ("SemiBold", 600), ("Bold", 700),
             ("Black", 900)]
# the line spacing of Patrone Sans 1, so that changing fonts moves nothing
LINE = {"ascender": 1069, "descender": -293, "gap": 0}


def win_metrics(all_drawn):
    top = bottom = 0
    for drawn_master in all_drawn:
        for unicodes, contours, components, advance, anchors in drawn_master.values():
            box = engine.bounds(contours)
            if box:
                bottom, top = min(bottom, box[1]), max(top, box[3])
    return max(LINE["ascender"], round(top) + 10), max(-LINE["descender"], round(-bottom) + 10)


def master_font(master, win=None):
    font = ufoLib2.Font()
    info = font.info
    info.familyName = "Patrone Sans"
    info.styleName = master.name
    info.versionMajor, info.versionMinor = VERSION
    info.copyright = COPYRIGHT
    info.unitsPerEm = G.Master.UPM
    info.xHeight = G.Master.X
    info.capHeight = G.Master.C
    info.ascender = G.Master.A
    info.descender = G.Master.D
    info.openTypeHheaAscender = LINE["ascender"]
    info.openTypeHheaDescender = LINE["descender"]
    info.openTypeHheaLineGap = LINE["gap"]
    info.openTypeOS2TypoAscender = LINE["ascender"]
    info.openTypeOS2TypoDescender = LINE["descender"]
    info.openTypeOS2TypoLineGap = LINE["gap"]
    if win:
        info.openTypeOS2WinAscent, info.openTypeOS2WinDescent = win
    info.openTypeOS2Selection = [7]  # use the typo metrics
    info.openTypeOS2Type = []  # installable
    info.openTypeOS2VendorID = "PTRN"
    info.openTypeOS2WeightClass = master.wght
    info.openTypeOS2StrikeoutPosition = round(G.Master.X * 0.5 + 10)
    info.openTypeOS2StrikeoutSize = 50
    info.postscriptUnderlinePosition = -100
    info.postscriptUnderlineThickness = 50
    info.openTypeNameDesigner = "William Daugherty"
    info.openTypeNameManufacturer = "PatroneOS"
    info.openTypeNameManufacturerURL = "https://github.com/billsecond/patroneos"
    info.openTypeNameDescription = DESCRIPTION
    info.openTypeNameLicense = LICENSE
    info.openTypeNameLicenseURL = "https://openfontlicense.org"
    drawn_glyphs = drawn(master)
    for name, (unicodes, contours, components, advance, anchors) in drawn_glyphs.items():
        write_glyph(font, name, unicodes, contours, components, advance, anchors)
    font.glyphOrder = list(G.GLYPHS)
    font.lib["public.openTypeCategories"] = {
        name: ("mark" if name in MARKS else "base") for name in G.GLYPHS
    }
    font.features.text = features()
    import kerning
    kerning.apply(font, master)
    return font


def features():
    """dotless i and j under accents, and the capitals' accents after
    capitals (mark and kern come from anchors and kerning)"""
    above = sorted(n for n in MARKS if n + ".case" in G.GLYPHS)
    upper = [n for n, (u, up, f) in G.GLYPHS.items() if up and u and chr(u[0]).isalpha()]
    return f"""languagesystem DFLT dflt;
languagesystem latn dflt;

@MarkAbove = [{" ".join(above)}];
@MarkAboveCase = [{" ".join(n + ".case" for n in above)}];
@Upper = [{" ".join(upper)}];

feature ccmp {{
    lookup dotless {{
        sub i' @MarkAbove by dotlessi;
        sub j' @MarkAbove by dotlessj;
    }} dotless;
    lookup case_marks {{
        sub @Upper @MarkAbove' by @MarkAboveCase;
    }} case_marks;
}} ccmp;
"""


def designspace(out):
    from fontTools.designspaceLib import AxisDescriptor, DesignSpaceDocument, InstanceDescriptor, SourceDescriptor
    doc = DesignSpaceDocument()
    axis = AxisDescriptor()
    axis.tag, axis.name = "wght", "Weight"
    axis.minimum, axis.default, axis.maximum = 300, 400, 900
    doc.addAxis(axis)
    for master in G.MASTERS:
        src = SourceDescriptor()
        src.filename = f"PatroneSans-{master.name}.ufo"
        src.familyName, src.styleName = "Patrone Sans", master.name
        src.location = {"Weight": master.wght}
        doc.addSource(src)
    for name, wght in INSTANCES:
        inst = InstanceDescriptor()
        inst.familyName, inst.styleName = "Patrone Sans", name
        inst.location = {"Weight": wght}
        doc.addInstance(inst)
    doc.write(out / "PatroneSans.designspace")


def main():
    out = Path(sys.argv[1])
    out.mkdir(parents=True, exist_ok=True)
    plan_pieces()
    win = win_metrics([_draw(m) for m in G.MASTERS])
    for master in G.MASTERS:
        master_font(master, win).save(out / f"PatroneSans-{master.name}.ufo", overwrite=True)
    designspace(out)
    print(f"{out}: {len(G.GLYPHS)} glyphs in {len(G.MASTERS)} masters, win metrics {win}")


if __name__ == "__main__":
    main()
