"""Accents: combining marks (no width, placed by anchors), flatter ones
for capitals (.case), and the spacing accents."""

from glyph import glyph
from parts import bar, quarter, through, zigzag
from punct import comma_shape, dot_r


def mark_glyph(name, code, draw, case=True, anchor="top"):
    """a combining mark (and its capital version), drawn by draw(g, x, y, h)
    with x its middle, y its foot and h its height"""

    def make(capital):
        def f(g):
            P = g.P
            if anchor == "top":
                base = P.C if capital else P.X
                gap = P.t(64, 64, 52) if capital else P.t(92, 92, 78)
                h = P.t(118, 120, 130) if capital else P.t(160, 164, 178)
                draw(g, 0, base + gap, h)
                g.anchor("_top", 0, base)
                g.anchor("top", 0, base + gap + h)
            else:
                draw(g, 0, 0, 0)
                g.anchor("_" + anchor, 0, 0)
            g.fixed(0)
            g.mark = True
        return f

    glyph(name, code)(make(False))
    if case and anchor == "top":
        glyph(name + ".case")(make(True))


def acute(g, x, y, h, flip=False):
    P = g.P
    w = h * 0.72
    p0, p1 = (x - w / 2, y), (x + w / 2, y + h)
    if flip:
        p0, p1 = (x + w / 2, y), (x - w / 2, y + h)
    through(g, p0, p1, s0=0.78, s1=1.04, nib=g.P.nib.scaled(0.96), cap0=0, cap1=None)


def grave(g, x, y, h):
    acute(g, x, y, h, flip=True)


def hungarumlaut(g, x, y, h):
    P = g.P
    gap = P.stem * 0.7 + 60
    for dx in (-gap / 2, gap / 2):
        w = h * 0.5
        through(g, (x + dx - w / 2, y), (x + dx + w / 2, y + h), s0=0.74, s1=1.0,
                nib=P.nib.scaled(0.92), cap0=0, cap1=None)


def circumflex(g, x, y, h, down=False):
    P = g.P
    w = h * P.t(1.9, 1.9, 2.1)
    nib = P.nib.scaled(P.t(0.86, 0.86, 0.62))
    if down:
        zigzag(g, [(x - w / 2, y + h), (x, y, P.stem * 0.5), (x + w / 2, y + h)], [(1.0, 0.92), (0.92, 1.0)], nib)
    else:
        zigzag(g, [(x - w / 2, y), (x, y + h, P.stem * 0.5), (x + w / 2, y)], [(1.0, 0.92), (0.92, 1.0)], nib)


def caron(g, x, y, h):
    circumflex(g, x, y, h, down=True)


def breve(g, x, y, h):
    P = g.P
    w = h * P.t(1.7, 1.7, 1.9)
    nib = P.nib.scaled(P.t(0.84, 0.84, 0.6))
    yb = y + nib.b * 0.9
    s = g.stroke((x - w / 2 + nib.a, y + h), nib=nib, s=0.9)
    s.arc((x, yb), "v", k=0.6, s=1.0)
    s.arc((x + w / 2 - nib.a, y + h), "h", k=0.6, s=0.9)


def dieresis(g, x, y, h):
    P = g.P
    r = dot_r(P) * 0.94
    gap = P.stem * 1.0 + P.t(150, 150, 140)
    for dx in (-gap / 2, gap / 2):
        g.dot(x + dx, y + r, r)


def dotaccent(g, x, y, h):
    r = dot_r(g.P) * 0.96
    g.dot(x, y + r, r)


def ring_mark(g, x, y, h):
    from punct import _ring_with
    P = g.P
    d = h * 1.15
    _ring_with(g, x - d / 2, x + d / 2, y, y + d, P.nib.scaled(0.8))


def macron(g, x, y, h):
    P = g.P
    w = h * 1.9
    bar(g, x - w / 2, x + w / 2, y + P.b, s=0.92)


def tilde(g, x, y, h):
    P = g.P
    nib = P.nib.scaled(P.t(0.88, 0.88, 0.54))
    w = h * P.t(1.9, 1.9, 2.1)
    # its waves no tighter than the pen, in the capitals' lower tilde too
    a = max(h * P.t(0.32, 0.32, 0.40), nib.b * 1.5)
    yc = y + h * 0.42
    s = g.stroke((x - w / 2, yc - a * 0.6), nib=nib, s=0.9)
    s.to((x - w * 0.2, yc + a), 60, 0, k=(0.36, 0.5), s=1.0)
    s.to((x + w * 0.2, yc - a), 0, 0, k=(0.56, 0.56), s=1.0)
    s.to((x + w / 2, yc + a * 0.6), 0, 60, k=(0.5, 0.36), s=0.9)


def turned_comma(g, x, y, h):
    """ģ's comma, turned, above"""
    P = g.P
    scale = P.t(0.8, 0.8, 0.56)
    r = dot_r(P) * scale
    k0, c0 = len(g.strokes), len(g.contours)
    comma_shape(g, x, y + r * 3.2, scale)
    cy = y + r * 1.75
    g.transform_all(lambda p: (2 * x - p[0], 2 * cy - p[1]), start=k0, contours_from=c0)


def comma_below(g, x, y, h):
    P = g.P
    scale = P.t(0.82, 0.82, 0.58)
    r = dot_r(P) * scale
    comma_shape(g, x, -P.t(70, 70, 60) - r, scale)


def cedilla(g, x, y, h):
    P = g.P
    nib = P.nib.scaled(P.t(0.78, 0.78, 0.6))
    w = P.t(150, 156, 200)
    stem_bottom = -P.t(70, 72, 70)
    s = g.stroke((x, P.t(30, 30, 40)), nib=nib)
    s.line((x, stem_bottom))
    t = g.stroke((x - nib.a * 0.6, stem_bottom - nib.b * 0.9 + 4), nib=nib, s=0.96)
    t.line((x + w * 0.2, stem_bottom - nib.b * 0.9 + 4), s=0.96)
    t.arc((x + w * 0.5, stem_bottom - P.t(70, 72, 84)), "h", k=0.6, s=1.0)
    t.arc((x + w * 0.1, stem_bottom - P.t(150, 154, 176) + nib.b * 0.9), "v", k=0.6, s=0.96)
    t.line((x - w * 0.42, stem_bottom - P.t(150, 154, 176) + nib.b * 0.9), s=0.9)


def ogonek(g, x, y, h):
    P = g.P
    nib = P.nib.scaled(P.t(0.9, 0.9, 0.66))
    yb = -P.t(196, 200, 214) + nib.b
    s = g.stroke((x, P.t(20, 20, 30)), nib=nib)
    s.to((x - P.t(64, 66, 84), -P.t(120, 120, 120)), 240, 270, k=(0.4, 0.4), s=1.0)
    s.arc((x + P.t(10, 10, 20), yb), "v", k=0.6, s=0.96)
    s.line((x + P.t(80, 84, 100), yb), s=0.86)


MARKS = [
    ("gravecomb", 0x300, grave), ("acutecomb", 0x301, acute), ("circumflexcomb", 0x302, circumflex),
    ("tildecomb", 0x303, tilde), ("macroncomb", 0x304, macron), ("brevecomb", 0x306, breve),
    ("dotaccentcomb", 0x307, dotaccent), ("dieresiscomb", 0x308, dieresis), ("ringcomb", 0x30A, ring_mark),
    ("hungarumlautcomb", 0x30B, hungarumlaut), ("caroncomb", 0x30C, caron),
]
for _name, _code, _draw in MARKS:
    mark_glyph(_name, _code, _draw)
mark_glyph("commaturnedabovecomb", 0x312, turned_comma, case=False)
mark_glyph("commaaccentcomb", 0x326, comma_below, anchor="bottom")
mark_glyph("cedillacomb", 0x327, cedilla, anchor="bottom")
mark_glyph("ogonekcomb", 0x328, ogonek, anchor="ogonek")


# the spacing accents: the marks, in a width of their own
SPACING = [
    ("grave", 0x60, "gravecomb"), ("acute", 0xB4, "acutecomb"), ("circumflex", 0x2C6, "circumflexcomb"),
    ("caron", 0x2C7, "caroncomb"), ("breve", 0x2D8, "brevecomb"), ("dotaccent", 0x2D9, "dotaccentcomb"),
    ("ring", 0x2DA, "ringcomb"), ("tilde", 0x2DC, "tildecomb"), ("hungarumlaut", 0x2DD, "hungarumlautcomb"),
    ("dieresis", 0xA8, "dieresiscomb"), ("macron", 0xAF, "macroncomb"), ("cedilla", 0xB8, "cedillacomb"),
    ("ogonek", 0x2DB, "ogonekcomb"),
]


def spacing(mark):
    def f(g):
        g.draw(mark)
        g.space(0.8, 0.8)
    return f


for _name, _code, _mark in SPACING:
    glyph(_name, _code)(spacing(_mark))
