"""Punctuation, symbols and spaces."""

import math

from engine import direction, ellipse, polygon
from figures import small, tab
from glyph import glyph
from parts import (bar, bottom_line, edges, part_of, place, point_of, quarter, ring, stem, through, top_line,
                   x_at, zigzag)


def dot_r(P):
    return P.stem * 0.64


def axis(P):
    """the height math signs and dashes centre on"""
    return P.X * 0.54


# ---------------------------------------------------------------- spaces


def space_glyph(name, code, width):
    @glyph(name, code)
    def f(g):
        g.fixed(width(g.P))
    return f


space_glyph("nbspace", 0xA0, lambda P: P.t(250, 250, 270))
space_glyph("enspace", 0x2002, lambda P: 500)
space_glyph("emspace", 0x2003, lambda P: 1000)
space_glyph("figurespace", 0x2007, tab)
space_glyph("punctuationspace", 0x2008, lambda P: P.t(220, 230, 300))
space_glyph("thinspace", 0x2009, lambda P: 200)
space_glyph("hairspace", 0x200A, lambda P: 100)
space_glyph("narrownbspace", 0x202F, lambda P: 200)


# ---------------------------------------------------------------- dots and commas


@glyph("period", 0x2E)
def period(g):
    r = dot_r(g.P)
    g.dot(r, r, r)
    g.space(1.1, 1.1)


def comma_shape(g, x, y, scale=1.0):
    """a comma whose round head sits with its centre at (x, y)"""
    P = g.P
    r = dot_r(P) * scale
    g.dot(x, y, r)
    s = g.stroke((x + r * 0.38, y - r * 0.1), s=1.04 * scale, nib=P.nib)
    s.to((x - r * 0.62, y - r * 3.0), 268, 236, k=(0.42, 0.36), s=0.5 * scale)


@glyph("comma", 0x2C)
def comma(g):
    comma_shape(g, 0, dot_r(g.P))
    g.space(1.1, 1.1)


@glyph("colon", 0x3A)
def colon(g):
    P = g.P
    r = dot_r(P)
    g.dot(r, r, r)
    g.dot(r, P.X - r, r)
    g.space(1.1, 1.1)


@glyph("semicolon", 0x3B)
def semicolon(g):
    P = g.P
    r = dot_r(P)
    comma_shape(g, r, r)
    g.dot(r, P.X - r, r)
    g.space(1.1, 1.1)


@glyph("exclam", 0x21)
def exclam(g):
    P = g.P
    r = dot_r(P)
    s = g.stroke((r, P.C), s=1.06)
    s.line((r, P.C * 0.27), s=0.86)
    g.dot(r, r, r)
    g.space(1.2, 1.2)


@glyph("question", 0x3F)
def question(g):
    P = g.P
    r = dot_r(P)
    w = P.cw(230) + 2 * P.stem
    xs = w * 0.46
    yt = P.C + P.uov - P.b
    s = g.stroke((P.a * 1.05, P.C * 0.80), tilt=18)
    s.to((w * 0.52, yt), 64, 0, k=(0.40, 0.55))
    s.arc((w - P.a, P.C * 0.72), "h", k=0.58)
    s.to((xs, P.C * 0.42), 270, 240, k=(0.36, 0.34), s=0.96)
    s.to((xs, P.C * 0.27), 240, 270, k=0.3, s=0.9)
    g.dot(xs, r, r)
    g.space(0.6, 0.6)


def turned(g, name, cy):
    """another glyph turned half round about height cy (¡ ¿)"""
    g.draw(name)
    g.transform_all(lambda p: (-p[0], 2 * cy - p[1]))


@glyph("exclamdown", 0xA1)
def exclamdown(g):
    turned(g, "exclam", g.P.X / 2)
    g.space(1.2, 1.2)


@glyph("questiondown", 0xBF)
def questiondown(g):
    turned(g, "question", g.P.X / 2)
    g.space(0.6, 0.6)


@glyph("periodcentered", 0xB7)
def periodcentered(g):
    r = dot_r(g.P)
    g.dot(r, axis(g.P), r)
    g.space(1.1, 1.1)


@glyph("ellipsis", 0x2026)
def ellipsis(g):
    P = g.P
    r = dot_r(P)
    gap = P.t(160, 166, 200) + 2 * r
    for i in range(3):
        g.dot(r + i * gap, r, r)
    g.space(1.1, 1.1)


@glyph("bullet", 0x2022)
def bullet(g):
    P = g.P
    r = P.stem * 1.05 + 20
    g.dot(r, axis(P), r)
    g.space(1.1, 1.1)


@glyph("blackcircle", 0x25CF)
def blackcircle(g):
    P = g.P
    r = 150 + P.stem * 0.3
    g.dot(r, axis(P), r)
    g.space(1.4, 1.4)


@glyph("ratio", 0x2236)
def ratio(g):
    P = g.P
    r = dot_r(P)
    g.dot(r, P.C * 0.5 - P.C * 0.22, r)
    g.dot(r, P.C * 0.5 + P.C * 0.22, r)
    g.space(1.1, 1.1)


# ---------------------------------------------------------------- quotes


@glyph("quotesingle", 0x27)
def quotesingle(g):
    P = g.P
    s = g.stroke((P.a, P.C + 20), s=1.05)
    s.line((P.a, P.C - P.C * 0.32), s=0.8)
    g.space(1.1, 1.1)


@glyph("quotedbl", 0x22)
def quotedbl(g):
    P = g.P
    gap = P.stem + P.t(110, 110, 120)
    for x in (P.a, P.a + gap):
        s = g.stroke((x, P.C + 20), s=1.05)
        s.line((x, P.C - P.C * 0.32), s=0.8)
    g.space(1.1, 1.1)


def quote(g, n, up, high=True):
    """comma-shaped quotes: n of them, up (turned, as ‘ and “) or not"""
    P = g.P
    r = dot_r(P)
    gap = 2 * r + P.t(90, 92, 110)
    y = P.C - r + 6 if high else r
    for i in range(n):
        x = i * gap
        sub = len(g.strokes), len(g.contours)
        comma_shape(g, x, y)
        if up:
            # turned half round about its own middle
            cy = y - r * 1.1
            g.transform_all(lambda p, x=x: (2 * x - p[0], 2 * cy - p[1]), start=sub[0], contours_from=sub[1])
    g.space(1.1, 1.1)


@glyph("quoteright", 0x2019)
def quoteright(g):
    quote(g, 1, False)


@glyph("quoteleft", 0x2018)
def quoteleft(g):
    quote(g, 1, True)


@glyph("quotesinglbase", 0x201A)
def quotesinglbase(g):
    quote(g, 1, False, high=False)


@glyph("quotedblright", 0x201D)
def quotedblright(g):
    quote(g, 2, False)


@glyph("quotedblleft", 0x201C)
def quotedblleft(g):
    quote(g, 2, True)


@glyph("quotedblbase", 0x201E)
def quotedblbase(g):
    quote(g, 2, False, high=False)


@glyph("minute", 0x2032)
def minute(g):
    P = g.P
    through(g, (0, P.C * 0.62), (P.cw(70), P.C + 10), s0=0.7, s1=1.0, cap0=None, cap1=0)
    g.space(1.0, 1.0)


@glyph("second", 0x2033)
def second(g):
    P = g.P
    for x in (0, P.stem + P.cw(60)):
        through(g, (x, P.C * 0.62), (x + P.cw(70), P.C + 10), s0=0.7, s1=1.0, cap0=None, cap1=0)
    g.space(1.0, 1.0)


def angle_quote(g, x, flip, n=1):
    """‹ and ›, « and »: two strokes meeting at a point"""
    P = g.P
    h = P.X * 0.24
    w = P.cw(170)
    y = P.X * 0.5
    gap = P.cw(150) + P.stem * 0.8
    nib = g.nib.scaled(0.96)
    for i in range(n):
        x0 = x + i * gap
        tip, end = (x0, y), (x0 + w, y)
        if flip:
            tip, end = (x0 + w, y), (x0, y)
        # the two cuts at the tip lean a little apart, so they don't coincide
        through(g, tip, (end[0], y + h), nib=nib, cap0=88, cap1=None)
        through(g, tip, (end[0], y - h), nib=nib, cap0=92, cap1=None)


@glyph("guilsinglleft", 0x2039)
def guilsinglleft(g):
    angle_quote(g, 0, False)
    g.space(0.7, 0.7)


@glyph("guilsinglright", 0x203A)
def guilsinglright(g):
    angle_quote(g, 0, True)
    g.space(0.7, 0.7)


@glyph("guillemetleft", 0xAB)
def guillemetleft(g):
    angle_quote(g, 0, False, 2)
    g.space(0.7, 0.7)


@glyph("guillemetright", 0xBB)
def guillemetright(g):
    angle_quote(g, 0, True, 2)
    g.space(0.7, 0.7)


# ---------------------------------------------------------------- dashes and lines


@glyph("hyphen", 0x2D)
def hyphen(g):
    P = g.P
    bar(g, 0, P.cw(190) + P.stem, P.X * 0.48, s=0.98)
    g.space(0.7, 0.7)


def hyphen_copy(name, code):
    @glyph(name, code)
    def f(g):
        g.copy_of("hyphen")
    return f


# no soft hyphen (U+00AD): it marks where a word may break, and text
# engines draw the hyphen there themselves
hyphen_copy("hyphentwo", 0x2010)
hyphen_copy("nonbreakinghyphen", 0x2011)


@glyph("endash", 0x2013)
def endash(g):
    bar(g, 0, 500, g.P.X * 0.48, s=0.98)
    g.fixed(500)


@glyph("emdash", 0x2014)
def emdash(g):
    bar(g, 0, 1000, g.P.X * 0.48, s=0.98)
    g.fixed(1000)


@glyph("figuredash", 0x2012)
def figuredash(g):
    bar(g, 0, tab(g.P) - 2 * g.P.sb * 0.5, g.P.X * 0.48, s=0.98)
    g.fixed(tab(g.P))


@glyph("horizontalbar", 0x2015)
def horizontalbar(g):
    bar(g, 0, 1000, g.P.X * 0.48, s=0.98)
    g.fixed(1000)


@glyph("underscore", 0x5F)
def underscore(g):
    P = g.P
    bar(g, 0, 500, -P.t(120, 125, 140), s=1.0)
    g.fixed(500)


@glyph("slash", 0x2F)
def slash(g):
    P = g.P
    c0, c1 = place(g.nib, (0, P.D * 0.6), (P.cw(300), P.A), -1, 1)
    through(g, c0, c1)
    g.space(0.1, 0.1)


@glyph("backslash", 0x5C)
def backslash(g):
    P = g.P
    c0, c1 = place(g.nib, (P.cw(300), P.D * 0.6), (0, P.A), 1, -1)
    through(g, c0, c1)
    g.space(0.1, 0.1)


@glyph("fraction", 0x2044)
def fraction(g):
    P = g.P
    c0, c1 = place(g.nib.scaled(0.9), (0, -10), (P.cw(240), P.C + 10), -1, 1)
    through(g, c0, c1, nib=g.nib.scaled(0.9))
    g.space(-1.2, -1.2)


@glyph("bar", 0x7C)
def bar_(g):
    P = g.P
    stem(g, P.a, P.D, P.A + 30)
    g.space(1.6, 1.6)


@glyph("brokenbar", 0xA6)
def brokenbar(g):
    P = g.P
    stem(g, P.a, P.D, P.X * 0.32)
    stem(g, P.a, P.X * 0.62, P.A + 30)
    g.space(1.6, 1.6)


# ---------------------------------------------------------------- brackets


def paren(g, flip):
    P = g.P
    top, bottom = P.A + 40, P.D + 60
    w = P.cw(150)
    mid = (top + bottom) / 2
    s = g.stroke((w, top))
    s.to((0, mid), 240, 270, k=(0.38, 0.44))
    s.to((w, bottom), 270, 300, k=(0.44, 0.38))
    if flip:
        g.transform_all(lambda p: (w - p[0], p[1]))


@glyph("parenleft", 0x28)
def parenleft(g):
    paren(g, False)
    g.space(1.0, 0.5)


@glyph("parenright", 0x29)
def parenright(g):
    paren(g, True)
    g.space(0.5, 1.0)


def bracket(g, flip):
    P = g.P
    top, bottom = P.A + 40, P.D + 60
    w = P.stem + P.cw(110)
    hb = P.b * 0.96
    stem(g, P.a, bottom, top)
    bar(g, P.a, w, top - hb, s=0.96)
    bar(g, P.a, w, bottom + hb, s=0.96)
    if flip:
        g.transform_all(lambda p: (w - p[0], p[1]))


@glyph("bracketleft", 0x5B)
def bracketleft(g):
    bracket(g, False)
    g.space(1.0, 0.4)


@glyph("bracketright", 0x5D)
def bracketright(g):
    bracket(g, True)
    g.space(0.4, 1.0)


def brace(g, flip):
    P = g.P
    top, bottom = P.A + 40, P.D + 60
    mid = (top + bottom) / 2
    x0, x1, x2 = 0, P.cw(110) + P.a, P.cw(110) + P.a + P.cw(120)
    nib = g.nib.scaled(0.96)
    for sign in (1, -1):
        y_end = top if sign == 1 else bottom
        s = g.stroke((x2, y_end - sign * P.b * 0.96), nib=nib, s=0.96)
        s.arc((x1, y_end - sign * (P.b + P.cw(110))), "h", k=0.56, s=1.0)
        s.line((x1, mid + sign * P.cw(130)))
        s.to((x0, mid), 270 if sign == 1 else 90, 180, k=(0.42, 0.44), s=0.92)
        s.end(cap=87 if sign == 1 else 93)  # the halves' cuts at the point lean apart
    if flip:
        g.transform_all(lambda p: (x2 - p[0], p[1]))


@glyph("braceleft", 0x7B)
def braceleft(g):
    brace(g, False)
    g.space(0.6, 0.4)


@glyph("braceright", 0x7D)
def braceright(g):
    brace(g, True)
    g.space(0.4, 0.6)


# ---------------------------------------------------------------- math


def math_width(P):
    return tab(P) - 2 * P.t(56, 54, 46)


@glyph("plus", 0x2B)
def plus(g):
    P = g.P
    w = math_width(P)
    y = axis(P)
    bar(g, 0, w, y, s=0.94)
    stem(g, w / 2, y - w / 2, y + w / 2, s=0.94)
    g.fixed(tab(P))


@glyph("minus", 0x2212)
def minus(g):
    P = g.P
    bar(g, 0, math_width(P), axis(P), s=0.94)
    g.fixed(tab(P))


@glyph("equal", 0x3D)
def equal(g):
    P = g.P
    w = math_width(P)
    gap = P.t(76, 80, 96) + P.b
    bar(g, 0, w, axis(P) + gap, s=0.94)
    bar(g, 0, w, axis(P) - gap, s=0.94)
    g.fixed(tab(P))


@glyph("notequal", 0x2260)
def notequal(g):
    P = g.P
    w = math_width(P)
    g.draw("equal")
    y = axis(P)
    through(g, (w * 0.28, y - w * 0.46), (w * 0.72, y + w * 0.46), s0=0.9, s1=0.9, cap0=None, cap1=None)
    g.fixed(tab(P))


def less_shape(g, flip, y=None, h=None):
    P = g.P
    w = math_width(P)
    y = axis(P) if y is None else y
    h = w * 0.40 if h is None else h
    nib = g.nib.scaled(0.92)
    tip, end = (0, y), (w, y)
    if flip:
        tip, end = (w, y), (0, y)
    through(g, tip, (end[0], y + h), nib=nib, cap0=88, cap1=None)
    through(g, tip, (end[0], y - h), nib=nib, cap0=92, cap1=None)


@glyph("less", 0x3C)
def less(g):
    less_shape(g, False)
    g.fixed(tab(g.P))


@glyph("greater", 0x3E)
def greater(g):
    less_shape(g, True)
    g.fixed(tab(g.P))


@glyph("lessequal", 0x2264)
def lessequal(g):
    P = g.P
    w = math_width(P)
    less_shape(g, False, y=axis(P) + w * 0.14, h=w * 0.34)
    bar(g, 0, w, axis(P) - w * 0.44, s=0.92)
    g.fixed(tab(P))


@glyph("greaterequal", 0x2265)
def greaterequal(g):
    P = g.P
    w = math_width(P)
    less_shape(g, True, y=axis(P) + w * 0.14, h=w * 0.34)
    bar(g, 0, w, axis(P) - w * 0.44, s=0.92)
    g.fixed(tab(P))


@glyph("multiply", 0xD7)
def multiply(g):
    P = g.P
    w = math_width(P) * 0.78
    y = axis(P)
    through(g, (0, y - w / 2), (w, y + w / 2), s0=0.92, s1=0.92, cap0=None, cap1=None)
    through(g, (0, y + w / 2), (w, y - w / 2), s0=0.92, s1=0.92, cap0=None, cap1=None)
    g.fixed(tab(P))


@glyph("divide", 0xF7)
def divide(g):
    P = g.P
    w = math_width(P)
    y = axis(P)
    r = dot_r(P) * 0.95
    bar(g, 0, w, y, s=0.94)
    g.dot(w / 2, y + w * 0.36, r)
    g.dot(w / 2, y - w * 0.36, r)
    g.fixed(tab(P))


@glyph("plusminus", 0xB1)
def plusminus(g):
    P = g.P
    w = math_width(P)
    y = axis(P) + w * 0.18
    bar(g, 0, w, y, s=0.94)
    stem(g, w / 2, y - w * 0.42, y + w * 0.42, s=0.94)
    bar(g, 0, w, y - w * 0.66, s=0.94)
    g.fixed(tab(P))


@glyph("logicalnot", 0xAC)
def logicalnot(g):
    P = g.P
    w = math_width(P)
    y = axis(P)
    hb = P.b * 0.94
    bar(g, 0, w, y + hb, s=0.94)
    stem(g, w - P.a * 0.94, y - w * 0.32, y + 2 * hb, s=0.94)
    g.fixed(tab(P))


@glyph("asciicircum", 0x5E)
def asciicircum(g):
    P = g.P
    w = math_width(P) * 0.86
    zigzag(g, [(0, P.C * 0.42), (w / 2, P.C, P.stem * 0.5), (w, P.C * 0.42)],
           [(1.0, 0.9), (0.9, 1.0)], g.nib.scaled(0.9))
    g.fixed(tab(P))


@glyph("asciitilde", 0x7E)
def asciitilde(g):
    P = g.P
    w = math_width(P)
    y = axis(P)
    h = w * 0.11
    s = g.stroke((0, y - h * 0.6), s=0.92)
    s.to((w * 0.3, y + h), 64, 0, k=(0.36, 0.4), s=0.98)
    s.to((w * 0.7, y - h), 0, 0, k=(0.44, 0.44), s=1.02)
    s.to((w, y + h * 0.6), 0, 64, k=(0.4, 0.36), s=0.92)
    g.fixed(tab(P))


@glyph("approxequal", 0x2248)
def approxequal(g):
    P = g.P
    w = math_width(P)
    for dy in (P.t(84, 90, 110), -P.t(84, 90, 110)):
        k = len(g.strokes)
        g.draw("asciitilde", dy=dy)
    g.fixed(tab(P))


@glyph("infinity", 0x221E)
def infinity(g):
    P = g.P
    w = math_width(P) * 1.2
    y = axis(P)
    r = w / 4
    h = w * 0.2
    nib = g.nib.scaled(0.92)
    cx = w / 2
    # each loop runs on a little past the crossing, into the other, so
    # their ends don't meet edge to edge
    e = r * 0.16
    s = g.stroke((cx + e, y - e), nib=nib, s=0.9)
    s.line((cx, y), s=0.9)
    s.to((cx - r, y + h), 135, 180, k=(0.4, 0.36), s=1.0)
    s.arc((cx - 2 * r, y), "h", k=0.6, s=1.0)
    s.arc((cx - r, y - h), "v", k=0.6, s=1.0)
    s.to((cx, y), 0, 45, k=(0.36, 0.4), s=0.9)
    s.line((cx + e, y + e), s=0.9)
    t = g.stroke((cx - e, y - e), nib=nib, s=0.9)
    t.line((cx, y), s=0.9)
    t.to((cx + r, y + h), 45, 0, k=(0.4, 0.36), s=1.0)
    t.arc((cx + 2 * r, y), "h", k=0.6, s=1.0)
    t.arc((cx + r, y - h), "v", k=0.6, s=1.0)
    t.to((cx, y), 180, 135, k=(0.36, 0.4), s=0.9)
    t.line((cx - e, y + e), s=0.9)
    g.space(0.6, 0.6)


@glyph("percent", 0x25)
def percent(g):
    P = g.P
    nib = g.nib.scaled(0.86)
    rw = P.cw(130) + P.stem * 1.2
    rh = rw * 1.16
    w = rw * 2 + P.cw(110)
    _ring_with(g, 0, rw, P.C - rh, P.C + 6, nib)
    _ring_with(g, w - rw, w, -6, rh, nib)
    c0, c1 = place(nib, (rw * 0.55, -10), (w - rw * 0.55, P.C + 10), -1, 1)
    through(g, c0, c1, nib=nib)
    g.space(0.6, 0.6)


def _ring_with(g, x0, x1, y0, y1, nib, s=1.0):
    """a ring with its own pen: outer edges x0..x1, y0..y1. A small ring
    lightens its pen until its curves stay rounder than the pen, or its
    counter would pinch"""
    from engine import cross, dbez, ddbez, length
    for _ in range(8):
        rx = (x1 - x0) / 2 - nib.a * s
        ry = (y1 - y0) / 2 - nib.b * s
        q = quarter((rx, 0), (0, ry), "v", 0.58)
        tight = 9.0
        for i in range(21):
            t = i / 20
            d1, d2 = dbez(q, t), ddbez(q, t)
            radius = length(d1) ** 3 / max(abs(cross(d1, d2)), 1e-9)
            n = (d1[1] / length(d1), -d1[0] / length(d1))
            tight = min(tight, radius * 0.62 / (nib.half(n) * s))
        if tight >= 1:
            break
        s *= max(0.85, tight ** 0.5)
    xl, xr = x0 + nib.a * s, x1 - nib.a * s
    yb, yt = y0 + nib.b * s, y1 - nib.b * s
    cx, cy = (xl + xr) / 2, (yb + yt) / 2
    st = g.stroke((xr, cy), nib=nib, s=s)
    st.arc((cx, yt), "v", k=0.58, s=s)
    st.arc((xl, cy), "h", k=0.58, s=s)
    st.arc((cx, yb), "v", k=0.58, s=s)
    st.arc((xr, cy), "h", k=0.58, s=s)
    st.close()
    return st


@glyph("perthousand", 0x2030)
def perthousand(g):
    P = g.P
    nib = g.nib.scaled(0.82)
    rw = P.cw(118) + P.stem * 1.1
    rh = rw * 1.16
    w = rw * 2 + P.cw(100)
    _ring_with(g, 0, rw, P.C - rh, P.C + 6, nib)
    _ring_with(g, w - rw, w, -6, rh, nib)
    _ring_with(g, w + P.cw(40), w + P.cw(40) + rw, -6, rh, nib)
    c0, c1 = place(nib, (rw * 0.55, -10), (w - rw * 0.55, P.C + 10), -1, 1)
    through(g, c0, c1, nib=nib)
    g.space(0.6, 0.6)


@glyph("degree", 0xB0)
def degree(g):
    P = g.P
    d = P.cw(150) + P.stem * 1.4
    _ring_with(g, 0, d, P.C - d, P.C + 4, g.nib.scaled(0.84))
    g.space(0.8, 0.8)


@glyph("numbersign", 0x23)
def numbersign(g):
    P = g.P
    w = P.cw(400) + P.stem
    h = P.C * 0.96
    nib = g.nib.scaled(0.9)
    slant = h * 0.12
    for x in (w * 0.30, w * 0.70):
        through(g, (x - slant / 2, 0), (x + slant / 2, h), nib=nib)
    for y in (h * 0.34, h * 0.66):
        bar(g, 0, w, y, s=0.9, nib=nib)
    g.space(0.4, 0.4)


@glyph("asterisk", 0x2A)
def asterisk(g):
    P = g.P
    r = P.cw(170) + P.stem * 0.5
    cx, cy = r, P.C - r * 0.95
    nib = g.nib.scaled(0.86)
    for ang in (90, 30, -30):
        d = direction(ang)
        through(g, (cx - d[0] * r, cy - d[1] * r), (cx + d[0] * r, cy + d[1] * r), s0=0.84, s1=0.84,
                nib=nib, cap0=None, cap1=None, mid=1.0)
    g.space(0.5, 0.5)


@glyph("dagger", 0x2020)
def dagger(g):
    P = g.P
    w = P.cw(300) + P.stem
    stem(g, w / 2, P.D * 0.7, P.C)
    bar(g, 0, w, P.C * 0.66, s=0.94)
    g.space(0.5, 0.5)


@glyph("daggerdbl", 0x2021)
def daggerdbl(g):
    P = g.P
    w = P.cw(300) + P.stem
    stem(g, w / 2, P.D * 0.7, P.C)
    bar(g, 0, w, P.C * 0.66, s=0.94)
    bar(g, 0, w, P.C * 0.10, s=0.94)
    g.space(0.5, 0.5)


# ---------------------------------------------------------------- signs


@glyph("ampersand", 0x26)
def ampersand(g):
    P = g.P
    nib = g.nib.scaled(1.02)
    w = P.cw(470) + P.stem * 2
    a = nib.a
    yt = P.C + P.uov - nib.b
    yb = -P.uov + nib.b
    s = g.stroke((w, 0), nib=nib, s=1.0, cap=0)
    # the leg, up to the loop
    s.to((w * 0.24 + a * 0.2, P.C * 0.74), 128, 90, k=(0.46, 0.28), s=1.0)
    s.to((w * 0.46, yt), 90, 0, k=(0.5, 0.54), s=0.96)
    s.to((w * 0.66 - a * 0.2, P.C * 0.76), 0, 270, k=(0.52, 0.48), s=1.0)
    s.to((a * 1.04 + w * 0.02, P.C * 0.24), 270, 270, k=(0.42, 0.42), s=1.04)
    s.arc((w * 0.38, yb), "v", k=0.58, s=0.96)
    s.to((w * 0.86, P.C * 0.40), 0, 66, k=(0.5, 0.32), s=1.0)
    s.end(tilt=10)
    g.space(0.6, 0.12)


@glyph("at", 0x40)
def at(g):
    P = g.P
    nib = g.nib.scaled(P.t(0.84, 0.84, 0.62))
    R = P.C * 0.56
    cx, cy = R, P.C * 0.42
    ri = R * 0.44
    xs = cx + ri * 0.95
    _ring_with(g, cx - ri * 1.1, xs + nib.a * 0.9, cy - ri, cy + ri, nib)
    s = g.stroke((xs, cy + ri - nib.b), nib=nib)
    s.line((xs, cy - ri * 0.55))
    s.arc((xs + R * 0.22, cy - ri * 0.95), "v", k=0.6)
    s.arc((cx + R - nib.a, cy), "h", k=0.58)
    s.arc((cx, cy + R - nib.b), "v", k=0.58)
    s.arc((cx - R + nib.a, cy), "h", k=0.58)
    s.arc((cx, cy - R + nib.b), "v", k=0.58)
    s.to((cx + R * 0.66, cy - R * 0.80), 0, 30, k=0.3)
    g.space(0.6, 0.6)


@glyph("section", 0xA7)
def section(g):
    P = g.P
    m = small(P, 0.74, 0.9)
    g.draw("s", master=m, dy=P.C * 0.44)
    g.draw("s", master=m, dy=-P.C * 0.06)
    g.space(0.6, 0.6)


@glyph("paragraph", 0xB6)
def paragraph(g):
    P = g.P
    w = P.cw(330) + P.stem * 2
    x1, x2 = w * 0.55, w - P.a
    stem(g, x1, P.D * 0.7, P.C)
    stem(g, x2, P.D * 0.7, P.C)
    bar(g, x1, x2 + P.a, P.C - P.b, s=1.0)
    y0, y1 = P.C * 0.36, P.C
    r = (y1 - y0) / 2
    k = 0.56
    cy = (y0 + y1) / 2
    g.shape([
        ("L", (x1, y1), (x1 - r * 0.2, y1)),
        ("C", (x1 - r * 0.2, y1), (0 + r * (1 - k) - 0, y1), (0, cy + r * k), (0, cy)),
        ("C", (0, cy), (0, cy - r * k), (r * (1 - k), y0), (x1 - r * 0.2, y0)),
        ("L", (x1 - r * 0.2, y0), (x1, y0)),
        ("L", (x1, y0), (x1, y1)),
    ])
    g.space(0.6, 1.0)


def circled(g, inner, upper=True, scale=0.5):
    P = g.P
    d = P.C + 2 * P.uov
    nib = g.nib.scaled(0.72)
    _ring_with(g, 0, d, -P.uov, P.C + P.uov, nib)
    m = small(P, scale, 0.72)
    sub = g.draw(inner, master=m, upper=upper)
    # centre what was drawn inside the ring
    from engine import bounds
    k0 = len(g.strokes) - len(sub.strokes)
    cs = [c for st in g.strokes[k0:] for c in st.outline()]
    x0, y0, x1, y1 = bounds(cs)
    dx, dy = d / 2 - (x0 + x1) / 2, P.C / 2 - (y0 + y1) / 2
    g.transform_all(lambda p: (p[0] + dx, p[1] + dy), start=k0, contours_from=len(g.contours))
    g.space(0.6, 0.6)


@glyph("copyright", 0xA9, upper=True)
def copyright(g):
    circled(g, "C")


@glyph("registered", 0xAE, upper=True)
def registered(g):
    circled(g, "R")


@glyph("trademark", 0x2122, upper=True)
def trademark(g):
    P = g.P
    m = small(P, 0.46, 0.7)
    t = g.draw("T", master=m, dy=P.C * 0.54)
    from engine import bounds
    tw = bounds([c for st in t.strokes for c in st.outline()])[2]
    g.draw("M", master=m, dx=tw + P.cw(50), dy=P.C * 0.54)
    g.space(0.6, 0.6)


# ---------------------------------------------------------------- currency


@glyph("dollar", 0x24, upper=True)
def dollar(g):
    P = g.P
    m = small(P, 0.92, 0.98)
    sub = g.draw("S", master=m, upper=True, dy=P.C * 0.04)
    from engine import bounds
    k0 = len(g.strokes) - len(sub.strokes)
    x0, _, x1, _ = bounds([c for st in g.strokes[k0:] for c in st.outline()])
    stem(g, (x0 + x1) / 2, -P.C * 0.12, P.C * 1.10, s=0.84)
    g.fixed(tab(P))


@glyph("cent", 0xA2)
def cent(g):
    P = g.P
    sub = g.draw("c", dy=P.C * 0.06)
    from engine import bounds
    x0, _, x1, _ = bounds([c for st in g.strokes for c in st.outline()])
    stem(g, (x0 + x1) / 2 + P.a * 0.2, -P.X * 0.1, P.X * 1.1 + P.C * 0.06, s=0.84)
    g.fixed(tab(P))


@glyph("sterling", 0xA3, upper=True)
def sterling(g):
    P = g.P
    nib = g.nib.scaled(P.t(0.97, 0.97, 0.84))
    w = P.cw(330) + P.ustem * 2
    a, b = nib.a, nib.b
    xs = w * 0.30
    yt = P.C + P.uov - b
    s = g.stroke((w - a * 1.1, P.C * 0.80), nib=nib, tilt=16)
    s.to((xs + P.cw(150), yt), 112, 180, k=(0.36, 0.5))
    s.arc((xs, P.C * 0.68), "h", k=0.58)
    s.line((xs, b * 0.96))
    bar(g, 0, w, b * 0.96, s=0.96, nib=nib)
    bar(g, P.cw(10), w * 0.68, P.C * 0.44, s=0.9, nib=nib)
    g.fixed(tab(P))


@glyph("yen", 0xA5, upper=True)
def yen(g):
    P = g.P
    w = tab(P) - 2 * P.t(40, 40, 30)
    nib = g.nib.scaled(0.94)
    yj = P.C * 0.44
    zigzag(g, [(0, P.C), (w / 2, yj, P.ustem * 0.94), (w, P.C)], [(1.0, 0.92), (0.92, 1.0)], nib)
    stem(g, w / 2, 0, yj + P.ub, nib=nib)
    for y in (P.C * 0.30, P.C * 0.10 + P.ub):
        bar(g, w * 0.16, w * 0.84, y + P.C * 0.04, s=0.86, nib=nib)
    g.fixed(tab(P))


@glyph("euro", 0x20AC, upper=True)
def euro(g):
    P = g.P
    m = small(P, 0.92, 0.98)
    sub = g.draw("C", master=m, upper=True, dx=P.cw(70), dy=P.C * 0.04)
    nib = g.nib.scaled(0.86)
    for y in (P.C * 0.60, P.C * 0.40):
        bar(g, 0, P.cw(360), y, s=0.86, nib=nib)
    g.fixed(tab(P))


@glyph("currency", 0xA4)
def currency(g):
    P = g.P
    w = math_width(P) * 0.9
    y = axis(P)
    nib = g.nib.scaled(0.86)
    r = w * 0.32
    _ring_with(g, w / 2 - r, w / 2 + r, y - r, y + r, nib)
    for ang in (45, 135, 225, 315):
        d = direction(ang)
        p0 = (w / 2 + d[0] * r * 0.8, y + d[1] * r * 0.8)
        p1 = (w / 2 + d[0] * w * 0.62, y + d[1] * w * 0.62)
        through(g, p0, p1, nib=nib, cap0=None, cap1=None)
    g.fixed(tab(P))


@glyph("florin", 0x192)
def florin(g):
    P = g.P
    xs = P.a + P.cw(150)
    yt = top_line(P) + (P.A - P.X)
    yb = bottom_line(P, base=P.D)
    s = g.stroke((xs - P.a - P.cw(120), yb), s=0.95)
    s.line((xs - P.a - P.cw(70), yb))
    s.arc((xs, P.D * 0.2), "h", k=(0.6, 0.6))
    s.line((xs, P.A - P.t(170, 180, 230)))
    s.arc((xs + P.a + P.cw(66), yt), "v", k=(0.6, 0.58))
    s.line((xs + P.a + P.cw(120), yt), s=0.95)
    bar(g, xs - P.a - P.cw(70), xs + P.a + P.cw(110), P.X - P.b * 0.9, s=0.9)
    g.space(0.1, 0.1)


@glyph("mu", 0xB5)
def mu(g):
    g.draw("u")
    P = g.P
    stem(g, P.a, P.D, P.X * 0.5)
    g.space(1.0, 1.0)


# ---------------------------------------------------------------- small figures and letters


def superior(g, name, scale=0.6, base=None, upper=True):
    P = g.P
    m = small(P, scale, 0.8)
    lift = P.C - P.C * scale if base is None else base
    g.draw(name, master=m, dy=lift, upper=upper)


@glyph("onesuperior", 0xB9, upper=True)
def onesuperior(g):
    superior(g, "one")
    g.fixed(tab(g.P) * 0.6)


@glyph("twosuperior", 0xB2, upper=True)
def twosuperior(g):
    superior(g, "two")
    g.fixed(tab(g.P) * 0.6)


@glyph("threesuperior", 0xB3, upper=True)
def threesuperior(g):
    superior(g, "three")
    g.fixed(tab(g.P) * 0.6)


def fraction_glyph(name, code, top, bottom):
    @glyph(name, code, upper=True)
    def f(g):
        from engine import bounds
        P = g.P
        m = small(P, 0.52, 0.76)
        gap = P.C * 0.05
        k0 = len(g.strokes)
        g.draw(top, master=m, dy=P.C - P.C * 0.52)
        x1t = bounds([c for st in g.strokes[k0:] for c in st.outline()])[2]
        # the slash clears the upper figure at its foot, and the lower
        # figure clears the slash at its top
        nib = g.nib.scaled(0.66)
        run = P.C * 0.42
        xa = x1t + gap - run * (P.C * 0.48 + 10) / (P.C + 20)
        c0, c1 = place(nib, (xa, -10), (xa + run, P.C + 10), -1, -1)  # its left edge
        through(g, c0, c1, nib=nib)
        right = edges(nib, c0, c1)[1]
        k1 = len(g.strokes)
        g.draw(bottom, master=m)
        x0b = bounds([c for st in g.strokes[k1:] for c in st.outline()])[0]
        dx = x_at(right, P.C * 0.52) + gap - x0b
        g.transform_all(lambda p: (p[0] + dx, p[1]), start=k1, contours_from=len(g.contours))
        g.space(0.5, 0.5)
    return f


fraction_glyph("onequarter", 0xBC, "one", "four")
fraction_glyph("onehalf", 0xBD, "one", "two")
fraction_glyph("threequarters", 0xBE, "three", "four")


@glyph("ordfeminine", 0xAA)
def ordfeminine(g):
    P = g.P
    m = small(P, 0.62, 0.8)
    g.draw("a", master=m, dy=P.C - P.X * 0.62)
    from engine import bounds
    x0, _, x1, _ = bounds([c for st in g.strokes for c in st.outline()])
    bar(g, x0, x1, P.C - P.X * 0.62 - P.C * 0.12, s=0.8)
    g.space(0.6, 0.6)


@glyph("ordmasculine", 0xBA)
def ordmasculine(g):
    P = g.P
    m = small(P, 0.62, 0.8)
    g.draw("o", master=m, dy=P.C - P.X * 0.62)
    from engine import bounds
    x0, _, x1, _ = bounds([c for st in g.strokes for c in st.outline()])
    bar(g, x0, x1, P.C - P.X * 0.62 - P.C * 0.12, s=0.8)
    g.space(0.6, 0.6)


# ---------------------------------------------------------------- arrows


def arrow(g, angle, both=False):
    P = g.P
    L = P.t(700, 700, 720) if angle in (0, 180) else P.t(640, 640, 660)
    nib = g.nib.scaled(0.92)
    head = P.t(190, 200, 230)
    y = axis(P) if angle in (0, 180) else P.C * 0.5 - L / 2 + 0
    d = direction(angle)
    # the shaft from tail to tip, its middle on the axis
    cx, cy = (L / 2, axis(P)) if angle in (0, 180) else (L * 0.32, P.C * 0.5)
    tail = (cx - d[0] * L / 2, cy - d[1] * L / 2)
    tip = (cx + d[0] * L / 2, cy + d[1] * L / 2)
    across = (angle + 90) % 180  # the tip is cut square to the arrow
    through(g, tail, tip, nib=nib, cap0=across if both else None, cap1=across)
    ends = [(tip, d)] + ([(tail, (-d[0], -d[1]))] if both else [])
    for (tx, ty), (dx, dy) in ends:
        for side in (1, -1):
            back = math.radians(math.degrees(math.atan2(dy, dx)) + 180 + side * 42)
            end = (tx + math.cos(back) * head, ty + math.sin(back) * head)
            through(g, (tx, ty), end, nib=nib, cap0=across + 2 * side, cap1=None)
    g.space(0.5, 0.5)


for _name, _code, _angle, _both in (("arrowleft", 0x2190, 180, False), ("arrowup", 0x2191, 90, False),
                                    ("arrowright", 0x2192, 0, False), ("arrowdown", 0x2193, 270, False),
                                    ("arrowboth", 0x2194, 0, True), ("arrowupdn", 0x2195, 90, True)):
    glyph(_name, _code)(lambda g, a=_angle, b=_both: arrow(g, a, b))
