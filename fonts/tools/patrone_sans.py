#!/usr/bin/env python3
"""Make Patrone Sans's sources from Noto Sans's.

Takes the designspace and master UFOs that glyphsLib makes from Noto Sans's
Glyphs sources (roman or italic) and turns them into Patrone Sans's, in
place:

- keeps the normal width only, so the font has one axis, weight;
- gives l a tail, drawn from Noto Sans's own t in every master, so that
  I, l and 1 can't be mistaken for one another (lslash follows it);
- makes Noto Sans's slashed zero the default, so 0 and O can't be either;
- renames the family and sets its notices (OFL 1.1, Noto's copyright kept).

Nothing else changes: the other glyphs, the spacing and kerning, the
vertical metrics (so that switching between the two fonts doesn't move
anything on screen), the features and the language support are Noto Sans's.
See FONTLOG.txt for the reasons.

    patrone_sans.py SOURCES.designspace
"""

import argparse
import sys
from pathlib import Path

import ufoLib2
from fontTools.designspaceLib import DesignSpaceDocument
from ufoLib2.objects import Contour, Point

FAMILY = "Patrone Sans"
PS_FAMILY = "PatroneSans"
VERSION = (1, 0)

# How much of t's foot, to the right of the stem, l's tail keeps
TAIL = 0.62
# How much of t's right side bearing, after its foot, l keeps after its tail
TAIL_SIDE_BEARING = 0.8

NOTO_COPYRIGHT = (
    "Copyright 2022 The Noto Project Authors "
    "(https://github.com/notofonts/latin-greek-cyrillic)"
)
COPYRIGHT = f"{NOTO_COPYRIGHT}. Patrone Sans changes copyright 2026 William Daugherty."
DESCRIPTION = (
    "Patrone Sans is Noto Sans 2.015 changed for Patrone Linux: a tailed l and "
    "a slashed zero tell I, l, 1, O and 0 apart. Noto Sans was designed by the "
    "Monotype design team and Irene Vlachou."
)
LICENSE = (
    "This Font Software is licensed under the SIL Open Font License, Version "
    "1.1. This license is available with a FAQ at: https://openfontlicense.org"
)


def die(message):
    sys.exit(f"patrone_sans.py: {message}")


# --- Geometry ---------------------------------------------------------------


def on_curve(point):
    return point.type is not None


def x_at(p, q, y):
    """x on the line through p and q, at height y"""
    if q[1] == p[1]:
        return p[0]
    return p[0] + (q[0] - p[0]) * (y - p[1]) / (q[1] - p[1])


def stem_edges(contour):
    """The left and right edges of a plain stem (l's four-point contour), as
    two (bottom, top) pairs of points"""
    points = sorted(((p.x, p.y) for p in contour.points), key=lambda p: (p[1], p[0]))
    if len(points) != 4:
        die(f"expected a stem of 4 points, got {len(points)}")
    bottom, top = points[:2], points[2:]
    return (bottom[0], top[0]), (bottom[1], top[1])


def foot_of_t(glyph, x_height):
    """The points of t's foot, in contour order: from where the left stem
    turns into the bottom curve, around to the end of the tail, and back up
    the inner curve to the right stem. Also the indices of the end of the
    tail (its two corners) within that list."""
    if len(glyph.contours) != 1 or glyph.components:
        die("t is expected to be one contour")
    points = glyph.contours[0].points
    n = len(points)
    low = [i for i, p in enumerate(points) if on_curve(p) and p.y < 0.4 * x_height]
    low.sort(key=lambda i: points[i].x, reverse=True)
    bottom, top = sorted(low[:2], key=lambda i: points[i].y)
    if (bottom + 1) % n != top:
        die("the corners of t's tail aren't next to each other")

    # Up the inner curve to the first point with a straight line above it
    end = top
    while True:
        end = (end + 1) % n
        after = points[(end + 1) % n]
        if on_curve(points[end]) and on_curve(after) and after.y > 0.5 * x_height:
            break
    # Back along the bottom curve to the first point with a straight line above
    start = bottom
    while True:
        start = (start - 1) % n
        before = points[(start - 1) % n]
        if on_curve(points[start]) and on_curve(before) and before.y > 0.5 * x_height:
            break

    indices = []
    i = start
    while True:
        indices.append(i)
        if i == end:
            break
        i = (i + 1) % n
    foot = [points[i] for i in indices]
    stem_left = (points[(start - 1) % n], points[start])
    stem_right = (points[end], points[(end + 1) % n])
    return foot, indices.index(bottom), indices.index(top), stem_left, stem_right


def tailed_l(l_glyph, t_glyph, x_height):
    """l's contour with t's foot for a tail, and its new advance width"""
    if len(l_glyph.contours) != 1 or l_glyph.components:
        die("l is expected to be one contour")
    left, right = stem_edges(l_glyph.contours[0])
    top_y = left[1][1]
    foot, bottom, top, t_left, t_right = foot_of_t(t_glyph, x_height)

    as_xy = lambda p: (p.x, p.y)
    # Line t's stem up with l's: the left edges meet where the foot starts...
    start = foot[0]
    dx = x_at(*left, start.y) - x_at(as_xy(t_left[0]), as_xy(t_left[1]), start.y)
    # ...and l keeps its own stem width, by moving the inner curve sideways
    end = foot[-1]
    stem_change = x_at(*right, end.y) - (
        x_at(as_xy(t_right[0]), as_xy(t_right[1]), end.y) + dx
    )
    tail_end_x = foot[top].x

    points = [Point(round(x_at(*left, top_y)), top_y, "line")]
    for i, p in enumerate(foot):
        x = p.x + dx
        if i > top:
            # Fade the stem's change out towards the end of the tail
            weight = (tail_end_x - p.x) / (tail_end_x - end.x)
            x += stem_change * min(1, max(0, weight))
        # A shorter tail than t's: l has no crossbar to balance it
        edge = x_at(*right, p.y)
        if x > edge:
            x = edge + (x - edge) * TAIL
        kind = "line" if i == 0 else p.type
        points.append(Point(round(x), p.y, kind, smooth=p.smooth if i else True))
    points.append(Point(round(x_at(*right, top_y)), top_y, "line"))

    t_side_bearing = t_glyph.width - max(foot[bottom].x, foot[top].x)
    tail_x = max(points[1 + bottom].x, points[1 + top].x)
    width = max(l_glyph.width, round(tail_x + t_side_bearing * TAIL_SIDE_BEARING))
    return Contour(points=points), width


def interpolate(a, b, f):
    """A contour between two compatible ones"""
    return Contour(
        points=[
            Point(
                round(p.x + (q.x - p.x) * f),
                round(p.y + (q.y - p.y) * f),
                p.type,
                smooth=p.smooth,
            )
            for p, q in zip(a.points, b.points)
        ]
    )


def lslash_bar(glyph):
    """The bar of lslash: its four points outside the stem, as a contour"""
    if len(glyph.contours) != 1 or glyph.components:
        die("lslash is expected to be one contour")
    points = glyph.contours[0].points
    ys = sorted(p.y for p in points)
    bottom = sorted((p.x, p.y) for p in points if p.y == ys[0])
    top = sorted((p.x, p.y) for p in points if p.y == ys[-1])
    left, right = (bottom[0], top[0]), (bottom[-1], top[-1])
    bar = [
        p
        for p in points
        if p.x < x_at(*left, p.y) - 1 or p.x > x_at(*right, p.y) + 1
    ]
    if len(bar) != 4:
        die(f"expected lslash's bar to have 4 points, got {len(bar)}")
    return Contour(points=[Point(p.x, p.y, "line") for p in bar]), left


# --- The changes --------------------------------------------------------------


def keep_normal_width(doc):
    """Drop the width axis, with every master and instance that isn't of
    normal width"""
    width = next((a for a in doc.axes if a.tag == "wdth"), None)
    if width is None:
        return
    normal = width.map_forward(100) if width.map else 100
    doc.sources = [s for s in doc.sources if s.location.get(width.name) == normal]
    doc.instances = [
        i for i in doc.instances if i.designLocation.get(width.name, normal) == normal
    ]
    for item in [*doc.sources, *doc.instances]:
        item.location.pop(width.name, None)
        if hasattr(item, "userLocation"):
            item.userLocation.pop(width.name, None)
    doc.axes.remove(width)
    if doc.rules:
        die("the sources have substitution rules; check them before dropping width")


def rename(doc):
    for source in doc.sources:
        if source.familyName:
            source.familyName = FAMILY
        if source.name:
            source.name = source.name.replace("Noto Sans", FAMILY)
    for instance in doc.instances:
        style = instance.styleName
        instance.familyName = FAMILY
        instance.name = f"{FAMILY} {style}"
        ps_style = style.replace(" ", "")
        instance.postScriptFontName = f"{PS_FAMILY}-{ps_style}"
        instance.filename = f"instance_ufos/{PS_FAMILY}-{ps_style}.ufo"
        if instance.styleMapFamilyName:
            instance.styleMapFamilyName = FAMILY


def set_info(font):
    info = font.info
    info.familyName = FAMILY
    info.styleMapFamilyName = None
    info.openTypeNamePreferredFamilyName = None
    info.postscriptFontName = None
    info.postscriptFullName = None
    info.openTypeNameUniqueID = None
    info.openTypeNameVersion = None
    info.copyright = COPYRIGHT
    info.trademark = None
    info.versionMajor, info.versionMinor = VERSION
    info.openTypeNameDesigner = "Monotype Design Team, Irene Vlachou (Noto Sans)"
    info.openTypeNameDesignerURL = None
    info.openTypeNameManufacturer = "Patrone Linux"
    info.openTypeNameManufacturerURL = None
    info.openTypeNameDescription = DESCRIPTION
    info.openTypeNameLicense = LICENSE
    info.openTypeNameLicenseURL = "https://openfontlicense.org"
    info.openTypeOS2VendorID = "PTRN"
    info.openTypeNameRecords = None


def give_l_a_tail(fonts, doc):
    """l gets a tail in every master, and lslash with it"""
    weight = next(a for a in doc.axes if a.tag == "wght")
    # Per master: the tailed l, how much wider it is, and where the old l's
    # left edge met the baseline (lslash's stem stands a little left of it)
    tailed = {}
    for source in doc.sources:
        if source.layerName:
            continue
        font = fonts[source.path]
        layer = font.layers.defaultLayer
        l_glyph = layer["l"]
        left, _ = stem_edges(l_glyph.contours[0])
        old_width = l_glyph.width
        contour, width = tailed_l(l_glyph, layer["t"], font.info.xHeight)
        l_glyph.contours = [contour]
        l_glyph.width = width
        at = source.location[weight.name]
        tailed[at] = (contour, width - old_width, x_at(*left, 0))
        print(f"  {Path(source.path).name}: l {old_width} -> {width}")

    for source in doc.sources:
        font = fonts[source.path]
        layer = font.layers[source.layerName] if source.layerName else font.layers.defaultLayer
        at = source.location[weight.name]
        if at in tailed:
            contour, wider, l_left = tailed[at]
            old_width = layer["l"].width - wider
        else:
            # An intermediate master: l there is between its masters
            below = max(w for w in tailed if w <= at)
            above = min(w for w in tailed if w >= at)
            f = (at - below) / (above - below)
            (c0, w0, x0), (c1, w1, x1) = tailed[below], tailed[above]
            contour = interpolate(c0, c1, f)
            wider = round(w0 + (w1 - w0) * f)
            l_left = x0 + (x1 - x0) * f
            low, high = (masters_layer(fonts, doc, w) for w in (below, above))
            old_width = round(
                (low["l"].width - w0) + ((high["l"].width - w1) - (low["l"].width - w0)) * f
            )
        where = f" {source.layerName}" if source.layerName else ""
        widened = widen_composites(layer, font.layers.defaultLayer, wider, old_width)
        if widened:
            print(f"  {Path(source.path).name}{where}: {wider:+} for {', '.join(widened)}")
        if "lslash" not in layer:
            continue
        lslash = layer["lslash"]
        bar, lslash_left = lslash_bar(lslash)
        shift = round(x_at(*lslash_left, 0) - l_left)
        lslash.contours = [shifted(contour, shift), bar]
        lslash.width += wider
        print(f"  {Path(source.path).name}{where}: lslash {wider:+}")


def masters_layer(fonts, doc, at):
    weight = next(a for a in doc.axes if a.tag == "wght")
    source = next(
        s for s in doc.sources if not s.layerName and s.location[weight.name] == at
    )
    return fonts[source.path].layers.defaultLayer


def widen_composites(layer, default_layer, wider, old_width):
    """Glyphs made with l (ľ, ŀ, ﬂ, lj and the like) get wider with it, and
    what follows the l in them (lj's j) moves over. Marks stay where l's
    anchors put them."""
    widened = []
    for glyph in layer:
        if glyph.name == "l":
            continue
        ls = [c for c in glyph.components if c.baseGlyph == "l"]
        if not ls:
            continue
        changed = False
        for l_component in ls:
            xx, xy, yx, yy, dx, dy = l_component.transformation
            if xy or yx or xx <= 0:
                continue
            l_end = dx + old_width * xx
            for other in glyph.components:
                if other is l_component:
                    continue
                base = layer if other.baseGlyph in layer else default_layer
                o = other.transformation
                if base[other.baseGlyph].width > 0 and o[4] >= l_end - 1:
                    other.transformation = o._replace(dx=o[4] + round(wider * xx))
            if glyph.width >= l_end - 5:
                glyph.width += round(wider * xx)
                changed = True
        if changed:
            widened.append(glyph.name)
    return widened


def shifted(contour, dx):
    return Contour(
        points=[Point(p.x + dx, p.y, p.type, smooth=p.smooth) for p in contour.points]
    )


def slash_the_zero(fonts, doc):
    """The slashed zero (Noto Sans's zero.slash) becomes the default zero,
    and the proportional zero gets the same slash"""
    for source in doc.sources:
        font = fonts[source.path]
        layer = font.layers[source.layerName] if source.layerName else font.layers.defaultLayer
        if "zero.slash" not in layer or "zero" not in layer:
            continue
        slash = layer["zero.slash"]
        bars = list(slash.contours)
        if len(bars) != 1 or [c.baseGlyph for c in slash.components] != ["zero"]:
            die("zero.slash is expected to be zero and one more contour")
        zero = layer["zero"]
        if zero.components:
            die("zero is expected to be contours only")
        zero.contours.append(bars[0])
        slash.contours = []  # it was zero with a slash; zero now has it
        name = Path(source.path).name + (f" {source.layerName}" if source.layerName else "")
        if "zero.lf" in layer and not source.layerName:
            lf = layer["zero.lf"]
            if lf.components:
                print(f"  {name}: zero.lf follows zero (a component)")
            else:
                dx = lf.getBounds(layer).xMin - zero_bounds_without(zero, layer)
                lf.contours.append(shifted(bars[0], dx))
                print(f"  {name}: zero.lf slashed, {dx:+} units over")
        print(f"  {name}: zero slashed")


def zero_bounds_without(zero, layer):
    """zero's left edge, without the slash just added"""
    plain = zero.contours[:-1]
    return min(p.x for c in plain for p in c.points)


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("designspace", type=Path)
    args = parser.parse_args()

    doc = DesignSpaceDocument.fromfile(args.designspace)
    keep_normal_width(doc)
    rename(doc)
    fonts = {}
    for source in doc.sources:
        if source.path not in fonts:
            fonts[source.path] = ufoLib2.Font.open(source.path)

    print(f"{args.designspace.name}:")
    give_l_a_tail(fonts, doc)
    slash_the_zero(fonts, doc)

    for path, font in fonts.items():
        set_info(font)
        new_name = Path(path).name.replace("NotoSans", PS_FAMILY)
        font.save(Path(path).with_name(new_name), overwrite=True)
    for source in doc.sources:
        source.filename = Path(source.filename).name.replace("NotoSans", PS_FAMILY)
        source.path = None
    out = args.designspace.with_name(args.designspace.name.replace("NotoSans", PS_FAMILY))
    doc.write(out)
    print(f"  wrote {out.name}, {len(doc.sources)} sources, {len(doc.instances)} instances")


if __name__ == "__main__":
    main()
