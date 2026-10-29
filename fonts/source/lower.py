"""The lowercase."""

from glyph import glyph
from parts import (arch, bar, bottom_line, bowl, edge_line, edges, part_of, place, point_of,
                   quarter, ring, stem, through, top_line, x_at, zigzag)
from engine import unit


def marks(g, x, top=None, bottom=0):
    """where accents go: above the x-height (or top) and under the baseline"""
    g.anchor("top", x, g.P.X if top is None else top)
    g.anchor("bottom", x, bottom)


def dot_radius(P):
    return P.stem * 0.6


def i_dot(g, x):
    P = g.P
    r = dot_radius(P)
    g.dot(x, P.A - r - P.t(4, 4, 0), r)


# ---------------------------------------------------------------- stems and arches


@glyph("n", 0x6E)
def n(g):
    P = g.P
    xl = P.a
    xr = P.stem + P.cw(290) + P.a
    stem(g, xl, 0, P.X)
    arch(g, xl, xr)
    marks(g, (xl + xr) / 2)
    g.space(1.0, 0.88)


@glyph("h", 0x68)
def h(g):
    P = g.P
    xl = P.a
    xr = P.stem + P.cw(290) + P.a
    stem(g, xl, 0, P.A)
    arch(g, xl, xr)
    marks(g, (xl + xr) / 2, top=P.A)
    g.space(1.0, 0.88)


@glyph("m", 0x6D)
def m(g):
    P = g.P
    c = P.cw(236)
    x1 = P.a
    x2 = x1 + P.stem + c
    x3 = x2 + P.stem + c
    stem(g, x1, 0, P.X)
    arch(g, x1, x2, apex=0.52)
    arch(g, x2, x3, apex=0.52)
    marks(g, x2)
    g.space(1.0, 0.88)


@glyph("u", 0x75)
def u(g):
    P = g.P
    xl = P.a
    xr = P.stem + P.cw(290) + P.a
    stem(g, xr, 0, P.X)
    yb = bottom_line(P)
    s = g.stroke((xl, P.X))
    s.line((xl, P.X * 0.44))
    s.arc(((xl + xr) / 2, yb), "v", k=(0.60, 0.62))
    s.to((xr, P.t(140, 150, 205)), 0, 60, k=(0.56, 0.40), s=P.join)
    marks(g, (xl + xr) / 2)
    g.space(0.88, 1.0)


@glyph("r", 0x72)
def r(g):
    P = g.P
    xl = P.a
    stem(g, xl, 0, P.X)
    yt = top_line(P)
    yj = P.X - P.t(140, 150, 205)
    xt = xl + P.a + P.cw(118)
    s = g.stroke((xl, yj), s=P.join)
    s.to((xt, yt), 60, 0, k=(0.40, 0.56), s=1.0)
    s.to((xt + P.cw(84) + P.a * 0.4, yt - P.t(22, 24, 34)), 0, -28, k=0.4, s=0.96)
    s.end(tilt=18)
    marks(g, xl + P.a + P.cw(60))
    g.space(1.0, 0.22)


# ---------------------------------------------------------------- rounds


def o_width(P):
    return P.cw(300) + 2 * P.stem * P.rnd


@glyph("o", 0x6F)
def o(g):
    P = g.P
    w = o_width(P)
    ring(g, 0, w, bottom_line(P), top_line(P))
    marks(g, w / 2)
    g.space(0.6, 0.6)


def c_curves(P, w, mid=None):
    """the four quarters of the round c and e are cut from"""
    xl, xr = P.a * P.rnd, w - P.a * P.rnd
    cx = w / 2
    cy = P.X * 0.5 if mid is None else mid
    yt, yb = top_line(P), bottom_line(P)
    k = P.k
    return (
        quarter((xr, cy), (cx, yt), "v", k),
        quarter((cx, yt), (xl, cy), "h", k),
        quarter((xl, cy), (cx, yb), "v", k),
        quarter((cx, yb), (xr, cy), "h", k),
    )


@glyph("c", 0x63)
def c(g):
    P = g.P
    w = P.cw(268) + 2 * P.stem * P.rnd
    q1, q2, q3, q4 = c_curves(P, w)
    t0, t1 = P.t(0.42, 0.40, 0.36), P.t(0.62, 0.62, 0.66)
    s = g.stroke(point_of(q1, t0), s=1.0, tilt=24)
    part_of(s, q1, t0, 1.0, s=1.0)
    s.curve(q2[1], q2[2], q2[3], s=P.rnd)
    s.curve(q3[1], q3[2], q3[3], s=1.0)
    part_of(s, q4, 0.0, t1, s=1.0)
    s.end(tilt=22)
    marks(g, w / 2 + P.a * 0.3)
    g.space(0.6, 0.42)


@glyph("e", 0x65)
def e(g):
    P = g.P
    w = P.cw(292) + 2 * P.stem * P.rnd
    xl, xr = P.a * P.rnd, w - P.a * P.rnd
    ybar = P.X * P.t(0.53, 0.52, 0.50)
    q1, q2, q3, q4 = c_curves(P, w)
    # the top from the bar's end round to the bottom
    top = quarter((xr, ybar), q1[3], "v", P.k)
    s = g.stroke((xr, ybar), s=P.rnd)
    s.curve(top[1], top[2], top[3], s=1.0)
    s.curve(q2[1], q2[2], q2[3], s=P.rnd)
    s.curve(q3[1], q3[2], q3[3], s=1.0)
    part_of(s, q4, 0.0, P.t(0.64, 0.64, 0.50), s=1.0)
    s.end(tilt=P.t(22, 22, 10))
    bar(g, xl, xr + P.a * P.rnd, ybar, s=P.t(0.92, 0.9, 0.82))
    marks(g, w / 2)
    g.space(0.6, 0.5)


def a_parts(g, stem_down=True):
    """a's hook and bowl; returns the stem's centre line (æ joins its e
    there)"""
    P = g.P
    w = P.cw(250) + 2 * P.stem
    xs = w - P.a
    yt = top_line(P)
    # the hook over the top, into the stem
    s = g.stroke((P.a * 1.1 + P.cw(20), P.X * P.t(0.80, 0.80, 0.78)), s=1.0, tilt=P.t(22, 22, 12))
    s.to((w * 0.50, yt), P.t(58, 58, 34), 0, k=(0.42, 0.52))
    s.arc((xs, P.X * 0.56), "h", k=(0.62, 0.58))
    s.line((xs, 0 if stem_down else P.X * 0.40))
    # the bowl, from the stem round and back
    ybt = P.X * P.t(0.57, 0.56, 0.61) - P.b * 0.85
    yb = bottom_line(P)
    xbl = P.a * P.rnd
    xm = xbl + (xs - xbl) * 0.42
    b = g.stroke((xs, ybt), s=0.86)
    b.line((xs - (xs - xbl) * 0.42, ybt))
    side = P.t(P.rnd, P.rnd, 0.92)
    b.arc((xbl, (ybt + yb) * 0.5), "h", k=(0.60, 0.62), s=side)
    b.arc((xm, yb), "v", k=(0.62, 0.60), s=1.0)
    b.to((xs, P.t(100, 106, 150)), 0, 30, k=(0.5, 0.4), s=P.join)
    return xs


@glyph("a", 0x61)
def a(g):
    xs = a_parts(g)
    w = xs + g.P.a
    marks(g, w * 0.5)
    g.space(0.62, 1.0)


@glyph("b", 0x62)
def b(g):
    P = g.P
    xs = P.a
    xo = xs + P.a + P.cw(292) + P.a * P.rnd
    stem(g, xs, 0, P.A)
    bowl(g, xs, xo)
    marks(g, (xs + xo) / 2 + P.a * 0.3, top=P.A)
    g.space(1.0, 0.6)


@glyph("d", 0x64)
def d(g):
    P = g.P
    xo = P.a * P.rnd
    xs = xo + P.a * P.rnd + P.cw(292) + P.a
    stem(g, xs, 0, P.A)
    bowl(g, xs, xo, flip=True)
    marks(g, (xs + xo) / 2 - P.a * 0.3, top=P.A)
    g.space(0.6, 1.0)


@glyph("p", 0x70)
def p(g):
    P = g.P
    xs = P.a
    xo = xs + P.a + P.cw(292) + P.a * P.rnd
    stem(g, xs, P.D, P.X)
    bowl(g, xs, xo)
    marks(g, (xs + xo) / 2 + P.a * 0.3)
    g.space(1.0, 0.6)


@glyph("q", 0x71)
def q(g):
    P = g.P
    xo = P.a * P.rnd
    xs = xo + P.a * P.rnd + P.cw(292) + P.a
    stem(g, xs, P.D, P.X)
    bowl(g, xs, xo, flip=True)
    marks(g, (xs + xo) / 2 - P.a * 0.3)
    g.space(0.6, 1.0)


@glyph("g", 0x67)
def g_(g):
    P = g.P
    xo = P.a * P.rnd
    xs = xo + P.a * P.rnd + P.cw(286) + P.a
    bowl(g, xs, xo, flip=True)
    yb = bottom_line(P, base=P.D)
    s = g.stroke((xs, P.X))
    s.line((xs, P.D * 0.30))
    s.arc(((xs + xo) / 2 + P.a * 0.2, yb), "v", k=(0.62, 0.58))
    s.line((xo + P.a * 0.2 + P.cw(30), yb), s=0.96)
    marks(g, (xs + xo) / 2 - P.a * 0.3, bottom=P.D)
    g.space(0.6, 1.0)


@glyph("s", 0x73)
def s_(g):
    P = g.P
    w = P.cw(226) + 2 * P.stem
    # heavy weights thin the s's level strokes, or its counters close
    flat = P.t(1.0, 1.0, 0.84)
    yt = P.X + P.ov - P.b * flat
    yb = -P.ov + P.b * flat
    a = P.a
    cx = w / 2
    # the upper bowl a little narrower than the lower; in heavy weights the
    # bowls' sides come nearer the middle, so their curves stay rounder
    # than the pen
    xul, xur = a * 1.02 + w * 0.03, w - a - w * 0.02
    yu = P.X * P.t(0.75, 0.75, 0.726)
    xll, xlr = a + w * 0.01, w - a * 1.04
    yl = P.X * P.t(0.26, 0.26, 0.274)
    up_right = quarter((xur, yu), (cx + w * 0.01, yt), "v", 0.6)
    up_left = quarter((cx + w * 0.01, yt), (xul, yu), "h", (0.6, 0.62))
    lo_right = quarter((xlr, yl), (cx - w * 0.01, yb), "v", (0.62, 0.6))
    lo_left = quarter((cx - w * 0.01, yb), (xll, yl), "h", 0.6)
    t0, t1 = P.t(0.40, 0.42, 0.46), P.t(0.56, 0.54, 0.50)
    side = P.t(1.02, 1.02, 0.92)
    st = g.stroke(point_of(up_right, t0), s=0.98 * flat, tilt=P.t(22, 22, 10))
    part_of(st, up_right, t0, 1.0, s=flat)
    st.curve(up_left[1], up_left[2], up_left[3], s=side)
    # the spine: in heavy weights flatter and lighter, the one way its
    # turns stay rounder than the pen in the room there is
    spine = P.t(-26, -26, -20)
    k = P.t((0.44, 0.5), (0.44, 0.5), (0.5, 0.25))
    st.to((cx, P.X * 0.505), 270, spine, k=k, s=P.t(1.08, 1.10, 0.84))
    st.to((xlr, yl), spine, 270, k=k[::-1], s=side)
    st.curve(lo_right[1], lo_right[2], lo_right[3], s=flat)
    part_of(st, lo_left, 0.0, t1, s=0.98 * flat)
    st.end(tilt=P.t(22, 22, 10))
    marks(g, cx)
    g.space(0.62, 0.62)


# ---------------------------------------------------------------- i j l t f


@glyph("space", 0x20)
def space(g):
    g.fixed(g.P.t(250, 250, 270))


@glyph("dotlessi", 0x131)
def dotlessi(g):
    P = g.P
    stem(g, P.a, 0, P.X)
    marks(g, P.a)
    g.space(1.0, 1.0)


@glyph("i", 0x69)
def i(g):
    P = g.P
    stem(g, P.a, 0, P.X)
    i_dot(g, P.a)
    marks(g, P.a)
    g.space(1.0, 1.0)


def j_body(g):
    P = g.P
    xs = P.a + P.cw(150)
    yb = bottom_line(P, base=P.D)
    s = g.stroke((xs, P.X))
    s.line((xs, P.D * 0.36))
    s.arc((xs - P.a - P.cw(70), yb), "v", k=(0.6, 0.6))
    s.line((xs - P.a - P.cw(140), yb), s=0.96)
    marks(g, xs, bottom=P.D)
    g.space(-0.5, 1.0)
    return xs


@glyph("dotlessj", 0x237)
def dotlessj(g):
    j_body(g)


@glyph("j", 0x6A)
def j(g):
    xs = j_body(g)
    i_dot(g, xs)


@glyph("l", 0x6C)
def l(g):
    P = g.P
    xs = P.a
    yb = bottom_line(P)
    s = g.stroke((xs, P.A))
    s.line((xs, P.t(150, 160, 210)))
    s.arc((xs + P.a + P.cw(52), yb), "v", k=(0.58, 0.6))
    s.line((xs + P.a + P.cw(92), yb), s=0.95)
    marks(g, xs + P.a * 0.4, top=P.A)
    g.space(1.0, 0.3)


@glyph("t", 0x74)
def t(g):
    P = g.P
    xs = P.a + P.cw(66)
    yb = bottom_line(P)
    top = P.X + P.t(140, 140, 150)
    s = g.stroke((xs, top), cap=24)
    s.line((xs, P.t(150, 160, 210)))
    s.arc((xs + P.a + P.cw(56), yb), "v", k=(0.58, 0.6))
    s.line((xs + P.a + P.cw(112), yb), s=0.95)
    bar(g, xs - P.a - P.cw(66), xs + P.a + P.cw(118), P.X - P.b * 0.9, s=0.9)
    marks(g, xs, top=top)
    g.space(0.2, 0.3)


@glyph("f", 0x66)
def f(g):
    P = g.P
    xs = P.a + P.cw(72)
    yt = top_line(P) + (P.A - P.X)
    s = g.stroke((xs, 0))
    s.line((xs, P.A - P.t(170, 180, 230)))
    s.arc((xs + P.a + P.cw(66), yt), "v", k=(0.6, 0.58))
    s.to((xs + P.a + P.cw(P.t(156, 156, 190)), P.A - P.t(40, 44, 96)), 0, P.t(-30, -30, -36), k=0.36, s=0.95)
    s.end(tilt=16)
    bar(g, xs - P.a - P.cw(70), xs + P.a + P.cw(120), P.X - P.b * 0.9, s=0.9)
    marks(g, xs, top=P.A)
    g.space(0.2, 0.0)


# ---------------------------------------------------------------- diagonals


@glyph("k", 0x6B)
def k(g):
    P = g.P
    xs = P.a
    w = P.stem + P.cw(330)
    stem(g, xs, 0, P.A)
    nib = P.nib.scaled(0.96)
    yj = P.X * 0.32
    c0, c1 = place(nib, (xs, yj), (w - P.cw(20), P.X), 0, 1)
    through(g, c0, c1, nib=nib, cap0=90, cap1=0)
    # the leg leaves the arm and runs to the baseline
    t = 0.40
    lx, ly = c0[0] + (c1[0] - c0[0]) * t, c0[1] + (c1[1] - c0[1]) * t
    d0, d1 = place(nib, (w, 0), (lx - P.a * 0.4, ly + P.b * 0.2), 1, 0, 1.0, 0.9)
    through(g, d0, d1, s0=1.0, s1=0.9, nib=nib, cap0=0, cap1=0)
    marks(g, w * 0.5, top=P.A)
    g.space(1.0, 0.12)


@glyph("v", 0x76)
def v(g):
    P = g.P
    w = P.stem * 1.8 + P.cw(326)
    nib = g.nib.scaled(0.95)
    zigzag(g, [(0, P.X), (w / 2, 0, P.stem * 0.5), (w, P.X)], [(1.0, 0.9), (0.9, 1.0)], nib)
    marks(g, w / 2)
    g.space(0.14, 0.14)


@glyph("w", 0x77)
def w(g):
    P = g.P
    wd = P.stem * 3.2 + P.cw(560)
    nib = g.nib.scaled(0.9)
    flat = P.stem * 0.44
    q = wd / 4
    zigzag(g, [(0, P.X), (q + q * 0.06, 0, flat), (wd / 2, P.X * 0.93, flat * 0.6),
               (3 * q - q * 0.06, 0, flat), (wd, P.X)],
           [(1.0, 0.9), (0.9, 0.84), (0.84, 0.9), (0.9, 1.0)], nib)
    marks(g, wd / 2)
    g.space(0.12, 0.12)


@glyph("x", 0x78)
def x(g):
    P = g.P
    w = P.stem * 1.6 + P.cw(310)
    nib = g.nib.scaled(0.95)
    edge_line(g, (w * 0.03, P.X), (w, 0), -1, 1, nib=nib)
    edge_line(g, (w * 0.97, P.X), (0, 0), 1, -1, nib=nib)
    marks(g, w / 2)
    g.space(0.1, 0.1)


@glyph("y", 0x79)
def y(g):
    P = g.P
    w = P.stem * 1.8 + P.cw(326)
    nib = g.nib.scaled(0.95)
    xm = w / 2 + P.a * 0.25
    # the right stroke runs straight on into the descender
    c0, c1 = place(nib, (xm + P.stem * 0.25, 0), (w, P.X), 1, 1)
    d = unit((c1[0] - c0[0], c1[1] - c0[1]))
    tail = (c0[0] + d[0] * (P.D / d[1]), P.D)
    through(g, tail, c1, nib=nib)
    # the left one ends where its left edge meets the right one's
    meet_x = x_at(edges(nib, c0, c1)[0], 0)
    a0, a1 = place(nib, (0, P.X), (meet_x, 0), -1, -1, 1.0, 0.88)
    through(g, a1, a0, s0=0.88, s1=1.0, nib=nib)
    marks(g, w / 2, bottom=P.D)
    g.space(0.14, 0.14)


@glyph("z", 0x7A)
def z(g):
    P = g.P
    w = P.stem * 1.2 + P.cw(300)
    hb = P.b * 0.96
    xr = w - P.a * 0.4
    bar(g, P.cw(16), xr, P.X - hb, s=0.96)
    bar(g, 0, w, hb, s=0.96)
    # the diagonal meets the top bar's lower corner and the bottom one's upper
    edge_line(g, (xr, P.X - 2 * hb), (0, 2 * hb), 1, -1, nib=g.nib.scaled(1.02))
    marks(g, w / 2)
    g.space(0.4, 0.4)
