"""Accented letters, built from a letter and an accent by their anchors,
and the letters drawn from others (æ, ø, ł, đ, ß and the like)."""

import unicodedata

from engine import bounds
from glyph import GLYPHS, glyph
from parts import bar, bottom_line, edges, place, quarter, ring, stem, through, top_line, x_at
from punct import comma_shape, dot_r

COMBINING = {
    0x300: "gravecomb", 0x301: "acutecomb", 0x302: "circumflexcomb", 0x303: "tildecomb",
    0x304: "macroncomb", 0x306: "brevecomb", 0x307: "dotaccentcomb", 0x308: "dieresiscomb",
    0x30A: "ringcomb", 0x30B: "hungarumlautcomb", 0x30C: "caroncomb", 0x326: "commaaccentcomb",
    0x327: "cedillacomb", 0x328: "ogonekcomb",
}
BELOW = {"commaaccentcomb": "bottom", "cedillacomb": "bottom", "ogonekcomb": "ogonek"}

# Latvian's and Romanian's commas, which Unicode spells with a cedilla
COMMA_NOT_CEDILLA = {0x122, 0x123, 0x136, 0x137, 0x13B, 0x13C, 0x145, 0x146, 0x156, 0x157}
# ď ľ ť Ľ: a comma beside the letter, not a caron over it
CARON_BESIDE = {0x10F, 0x13D, 0x13E, 0x165}


def drawn_outline(g, name):
    """the drawn glyph's (contours, advance, anchors), already spaced"""
    return g.glyphs[name]


def accented(code):
    """(base glyph, mark glyph, anchor) for a precomposed letter"""
    parts = unicodedata.decomposition(chr(code)).split()
    if len(parts) != 2:
        return None
    base, mark = int(parts[0], 16), int(parts[1], 16)
    if mark not in COMBINING:
        return None
    mark_name = COMBINING[mark]
    if code in COMMA_NOT_CEDILLA:
        mark_name = "commaturnedabovecomb" if code == 0x123 else "commaaccentcomb"
    base_name = glyph_name(base)
    upper = chr(base).isupper()
    anchor = BELOW.get(mark_name, "top")
    if anchor == "top":
        if base_name == "i":
            base_name = "dotlessi"
        if base_name == "j":
            base_name = "dotlessj"
        if upper and mark_name != "commaturnedabovecomb":
            mark_name += ".case"
    return base_name, mark_name, anchor


NAMES = {}


def glyph_name(code):
    for name, (unicodes, _, _) in GLYPHS.items():
        if code in unicodes:
            return name
    raise KeyError(hex(code))


def composite(code):
    def f(g):
        base, mark, anchor = accented(code)
        _, _, _, advance, anchors = g.glyphs[base]
        m = g.glyphs[mark]
        mark_anchors = m[4]
        if anchor == "ogonek" and anchor not in anchors:
            # under the letter's right foot
            x1 = bounds(g.glyphs[base][1])[2]
            pen = g.P.unib if base[0].isupper() else g.P.nib
            anchors = dict(anchors, ogonek=(x1 - pen.a * (1.9 if base in ("e", "E") else 1.0), 0))
        bx, by = anchors[anchor]
        mx, my = mark_anchors["_" + anchor]
        g.component(base)
        g.component(mark, bx - mx, by - my)
        g.advance = advance
        # an accented letter takes another accent above its own
        for k, (x, y) in anchors.items():
            if k != anchor:
                g.anchor(k, x, y)
        if anchor == "top" and "top" in mark_anchors:
            tx, ty = mark_anchors["top"]
            g.anchor("top", tx + bx - mx, ty + by - my)
    return f


def beside(code):
    """ď ľ ť Ľ: the letter with a comma at its top right"""
    def f(g):
        P = g.P
        base = glyph_name(int(unicodedata.decomposition(chr(code)).split()[0], 16))
        contours, comps, advance, anchors = g.glyphs[base][1:]
        x0, y0, x1, y1 = bounds(contours)
        g.component(base)
        scale = P.t(0.9, 0.9, 0.7)
        r = dot_r(P) * scale
        x, top, extra = {
            "d": (x1 + r * 1.0, P.A, r * 2.2),
            "l": (x0 + P.stem + r * 1.3, P.A, 0),
            "t": (x0 + P.stem + P.cw(66) + r * 1.4, P.X + P.t(150, 150, 160) + r * 0.6, 0),
            "L": (x0 + P.ustem + r * 1.6, P.C, 0),
        }[base]
        mark = g.glyphs["caroncomb.alt"]
        mx, my = mark[4]["_topright"]
        g.component("caroncomb.alt", x - mx, top - my)
        g.advance = advance + extra
    return f


@glyph("caroncomb.alt")
def caroncomb_alt(g):
    """the comma ď ľ ť Ľ have for a caron, beside the letter"""
    P = g.P
    scale = P.t(0.9, 0.9, 0.7)
    r = dot_r(P) * scale
    comma_shape(g, 0, -r, scale)
    g.anchor("_topright", 0, 0)
    g.fixed(0)
    g.keep_place = True
    g.mark = True


# ---------------------------------------------------------------- letters drawn from others


def slash_through(g, x0, y0, x1, y1, s=0.9, nib=None):
    through(g, (x0, y0), (x1, y1), s0=s, s1=s, nib=nib or g.nib, cap0=None, cap1=None)


@glyph("oslash", 0xF8)
def oslash(g):
    P = g.P
    g.draw("o")
    from lower import o_width
    w = o_width(P)
    slash_through(g, -w * 0.04, -P.X * 0.08, w * 1.04, P.X * 1.08, nib=P.nib.scaled(0.9))
    g.anchor("top", w / 2, P.X)
    g.space(0.5, 0.5)


@glyph("Oslash", 0xD8, upper=True)
def Oslash(g):
    P = g.P
    g.draw("O")
    from upper import o_width
    w = o_width(P)
    slash_through(g, w * 0.02, -P.C * 0.06, w * 0.98, P.C * 1.06, nib=P.unib.scaled(0.9))
    g.anchor("top", w / 2, P.C)
    g.space(0.6, 0.6)


@glyph("lslash", 0x142)
def lslash(g):
    P = g.P
    g.draw("l")
    y = P.A * 0.55
    slash_through(g, P.a - P.cw(105), y - P.cw(80), P.a + P.cw(105), y + P.cw(80), nib=P.nib.scaled(0.9))
    g.space(0.2, 0.3)


@glyph("Lslash", 0x141, upper=True)
def Lslash(g):
    P = g.P
    g.draw("L")
    y = P.C * 0.5
    slash_through(g, P.ua - P.cw(110), y - P.cw(85), P.ua + P.cw(120), y + P.cw(90), nib=P.unib.scaled(0.9))
    g.space(0.25, 0.3)


@glyph("ldot", 0x140)
def ldot(g):
    P = g.P
    g.draw("l")
    r = dot_r(P)
    g.dot(P.stem + P.cw(150) + r, P.X * 0.62, r)
    g.space(1.0, 0.8)


@glyph("Ldot", 0x13F, upper=True)
def Ldot(g):
    P = g.P
    g.draw("L")
    r = dot_r(P)
    g.dot(P.ustem + P.cw(190) + r, P.C * 0.5, r)
    g.space(1.0, 0.3)


@glyph("Eth", 0xD0, upper=True)
def Eth(g):
    P = g.P
    g.draw("D")
    bar(g, -P.cw(70), P.ustem + P.cw(140), P.C * 0.5, s=0.9)
    g.anchor("top", P.ustem + P.cw(230), P.C)
    g.space(0.3, 0.6)


@glyph("Dcroat", 0x110, upper=True)
def Dcroat(g):
    g.copy_of("Eth")


@glyph("dcroat", 0x111)
def dcroat(g):
    P = g.P
    g.draw("d")
    xo = P.a * P.rnd
    xs = xo + P.a * P.rnd + P.cw(292) + P.a
    bar(g, xs - P.a - P.cw(130), xs + P.a + P.cw(70), P.A - P.t(130, 130, 150), s=0.88)
    g.space(0.6, 0.2)


@glyph("hbar", 0x127)
def hbar(g):
    P = g.P
    g.draw("h")
    bar(g, -P.cw(70), P.stem + P.cw(150), P.A - P.t(130, 130, 150), s=0.88)
    g.space(0.2, 0.88)


@glyph("Hbar", 0x126, upper=True)
def Hbar(g):
    P = g.P
    g.draw("H")
    xr = P.ustem + P.cw(410) + P.ua
    bar(g, -P.cw(60), xr + P.ua + P.cw(60), P.C * 0.78, s=0.86)
    g.space(0.4, 0.4)


@glyph("tbar", 0x167)
def tbar(g):
    P = g.P
    g.draw("t")
    xs = P.a + P.cw(66)
    bar(g, xs - P.a - P.cw(60), xs + P.a + P.cw(70), P.X * 0.46, s=0.86)
    g.space(0.2, 0.3)


@glyph("Tbar", 0x166, upper=True)
def Tbar(g):
    P = g.P
    g.draw("T")
    w = P.ustem + P.cw(470)
    bar(g, w / 2 - P.ua - P.cw(110), w / 2 + P.ua + P.cw(110), P.C * 0.48, s=0.88)
    g.space(0.25, 0.25)


@glyph("Thorn", 0xDE, upper=True)
def Thorn(g):
    P = g.P
    xs = P.ua
    xr = xs + P.ua + P.cw(330) + P.ua * P.rnd
    stem(g, xs, 0, P.C)
    from upper import bowl_right
    bowl_right(g, xs, xr, P.C * 0.18 + P.ub, P.C * 0.80 - P.ub, k=(0.6, 0.62))
    g.space(1.0, 0.5)


@glyph("thorn", 0xFE)
def thorn(g):
    P = g.P
    from parts import bowl
    xs = P.a
    xo = xs + P.a + P.cw(292) + P.a * P.rnd
    stem(g, xs, P.D, P.A)
    bowl(g, xs, xo)
    g.space(1.0, 0.6)


@glyph("kgreenlandic", 0x138)
def kgreenlandic(g):
    P = g.P
    xs = P.a
    w = P.stem + P.cw(310)
    stem(g, xs, 0, P.X)
    nib = P.nib.scaled(0.96)
    c0, c1 = place(nib, (xs, P.X * 0.36), (w - P.cw(20), P.X), 0, 1)
    through(g, c0, c1, nib=nib, cap0=90, cap1=0)
    t = 0.42
    lx, ly = c0[0] + (c1[0] - c0[0]) * t, c0[1] + (c1[1] - c0[1]) * t
    d0, d1 = place(nib, (w, 0), (lx - P.a * 0.4, ly + P.b * 0.2), 1, 0, 1.0, 0.9)
    through(g, d0, d1, s0=1.0, s1=0.9, nib=nib, cap0=0, cap1=0)
    g.space(1.0, 0.12)


@glyph("eng", 0x14B)
def eng(g):
    P = g.P
    from parts import arch
    xl = P.a
    xr = P.stem + P.cw(290) + P.a
    stem(g, xl, 0, P.X)
    arch(g, xl, xr, down_to=P.D * 0.36)
    yb = bottom_line(P, base=P.D)
    s = g.stroke((xr, P.D * 0.36 + 20))  # overlapping the arch's stem a little
    s.line((xr, P.D * 0.36))
    s.arc((xr - P.a - P.cw(70), yb), "v", k=(0.6, 0.6))
    s.line((xr - P.a - P.cw(140), yb), s=0.96)
    g.space(1.0, 0.88)


@glyph("Eng", 0x14A, upper=True)
def Eng(g):
    P = g.P
    from parts import arch
    xl, xr = P.ua, P.ustem + P.cw(410) + P.ua
    stem(g, xl, 0, P.C)
    yt = top_line(P, True)
    arch(g, xl, xr, down_to=-P.C * 0.06, top=yt, join=P.C - P.t(160, 170, 230), shoulder=P.C * 0.62, upper=True)
    yb = bottom_line(P, True, base=P.D * 0.8)
    s = g.stroke((xr, -P.C * 0.06 + 20))
    s.line((xr, -P.C * 0.06))
    s.arc((xr - P.ua - P.cw(80), yb), "v", k=(0.6, 0.6))
    s.line((xr - P.ua - P.cw(160), yb), s=0.96)
    g.space(1.0, 1.0)


@glyph("longs", 0x17F)
def longs(g):
    P = g.P
    xs = P.a + P.cw(72)
    yt = top_line(P) + (P.A - P.X)
    s = g.stroke((xs, 0))
    s.line((xs, P.A - P.t(170, 180, 230)))
    s.arc((xs + P.a + P.cw(66), yt), "v", k=(0.6, 0.58))
    s.to((xs + P.a + P.cw(P.t(156, 156, 190)), P.A - P.t(40, 44, 96)), 0, P.t(-30, -30, -36), k=0.36, s=0.95)
    s.end(tilt=16)
    g.space(1.0, 0.0)


@glyph("germandbls", 0xDF)
def germandbls(g):
    """ß: a stem rising into an arch, a small upper bowl down to a waist,
    and a larger lower bowl"""
    P = g.P
    xs = P.a
    w = P.stem * 2 + P.cw(250)
    yt = top_line(P) + (P.A - P.X)
    yb = bottom_line(P)
    xr1, xr2 = w - P.a - P.cw(30), w - P.a
    xm1 = xs + (xr1 - xs) * 0.5
    y1 = P.t(600, 600, 570)
    xw, yw = xs + P.a + P.cw(70), P.X * P.t(0.92, 0.92, 0.84)
    s = g.stroke((xs, 0))
    s.line((xs, P.A - P.t(220, 220, 300)))
    s.arc((xm1, yt), "v", k=(0.6, 0.58))
    s.arc((xr1, y1), "h", k=0.58, s=1.02)
    s.arc((xw, yw), "v", k=0.58, s=0.94)
    t = g.stroke((xw - P.cw(10), yw), s=0.94)
    t.line((xw + P.cw(40), yw), s=0.94)
    t.arc((xr2, P.X * 0.33), "h", k=(0.58, 0.6), s=1.03)
    t.arc((xs + (xr2 - xs) * 0.5, yb), "v", k=(0.6, 0.58), s=1.0)
    t.line((xs + P.a + P.cw(40), yb), s=0.95)
    g.anchor("top", (xs + xr2) / 2, P.A)
    g.space(1.0, 0.6)


@glyph("eth", 0xF0)
def eth(g):
    P = g.P
    from lower import o_width
    w = o_width(P)
    top = P.X * 1.06
    ring(g, 0, w, bottom_line(P), top + P.ov - P.b)
    s = g.stroke((w - P.a * P.rnd, (top + P.ov - P.b + bottom_line(P)) / 2), s=P.rnd)
    s.to((w * 0.30, P.A + P.ov - P.b), 90, 150, k=(0.5, 0.36), s=0.96)
    s.end()
    c0, c1 = (w * 0.30, P.A * 0.74), (w * 0.82, P.A * 0.96)
    through(g, c0, c1, s0=0.84, s1=0.84, nib=P.nib.scaled(0.92), cap0=None, cap1=None)
    g.space(0.6, 0.6)


@glyph("ae", 0xE6)
def ae(g):
    P = g.P
    from lower import a_parts
    xs = a_parts(g, stem_down=False)
    g.draw("e", dx=xs - P.a * P.rnd)
    g.space(0.62, 0.5)


@glyph("oe", 0x153)
def oe(g):
    P = g.P
    from lower import o_width
    w = o_width(P)
    g.draw("o")
    g.draw("e", dx=w - P.stem * P.rnd * 1.0)
    g.space(0.6, 0.5)


@glyph("AE", 0xC6, upper=True)
def AE(g):
    P = g.P
    nib = P.unib.scaled(0.96)
    xs = P.ustem * 0.8 + P.cw(340)
    w = xs + P.ua + P.cw(270)
    hb = P.ub * 0.96
    stem(g, xs, 0, P.C)
    bar(g, xs - P.ua - P.cw(70), w, P.C - hb, s=0.96)
    bar(g, xs, w - P.cw(30), P.C * 0.5, s=0.92)
    bar(g, xs, w + P.cw(10), hb, s=0.96)
    c0, c1 = place(nib, (0, 0), (xs - P.ua - P.cw(70), P.C), -1, -1, 1.0, 0.92)
    through(g, c0, c1, s0=1.0, s1=0.92, nib=nib)
    yb = P.C * 0.30
    left = edges(nib, c0, c1, 1.0, 0.92)[1]
    bar(g, x_at(left, yb) - P.ua * 0.5, xs, yb, s=0.9)
    g.anchor("top", xs, P.C)
    g.space(0.1, 0.5)


@glyph("OE", 0x152, upper=True)
def OE(g):
    P = g.P
    from upper import o_width
    w = o_width(P) * 0.9
    ring(g, 0, w, bottom_line(P, True), top_line(P, True))
    xs = w - P.ua * P.rnd
    stem(g, xs, 0, P.C)
    hb = P.ub * 0.96
    we = xs + P.ua + P.cw(260)
    bar(g, xs - P.cw(120), we, P.C - hb, s=0.96)
    bar(g, xs, we - P.cw(30), P.C * 0.5, s=0.92)
    bar(g, xs - P.cw(120), we + P.cw(10), hb, s=0.96)
    g.space(0.6, 0.5)


@glyph("ij", 0x133)
def ij(g):
    P = g.P
    i_adv = g.glyphs["i"][3]
    g.component("i")
    g.component("j", i_adv)
    g.advance = i_adv + g.glyphs["j"][3]


@glyph("IJ", 0x132, upper=True)
def IJ(g):
    P = g.P
    i_adv = g.glyphs["I"][3]
    g.component("I")
    g.component("J", i_adv)
    g.advance = i_adv + g.glyphs["J"][3]


@glyph("napostrophe", 0x149)
def napostrophe(g):
    q = g.glyphs["quoteright"][3]
    g.component("quoteright")
    g.component("n", q)
    g.advance = q + g.glyphs["n"][3]


def add_composites():
    """every precomposed Latin letter Patrone Sans covers, from Unicode's
    decompositions"""
    codes = list(range(0xC0, 0x180)) + [0x218, 0x219, 0x21A, 0x21B]
    have = {c for unicodes, _, _ in GLYPHS.values() for c in unicodes}
    for code in codes:
        if code in have:
            continue
        if code in CARON_BESIDE:
            glyph(gname(code), code, upper=chr(code).isupper())(beside(code))
        elif accented(code):
            glyph(gname(code), code, upper=chr(code).isupper())(composite(code))


def gname(code):
    """the glyph's name: the AGL's for Latin-1 and Latin Extended-A, else uniXXXX"""
    from fontTools import agl
    name = agl.UV2AGL.get(code)
    return name or f"uni{code:04X}"
