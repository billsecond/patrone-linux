"""The capitals."""

from engine import unit
from glyph import glyph
from parts import (bar, bottom_line, edges, part_of, place, point_of, quarter, ring, stem, through,
                   top_line, x_at, zigzag)


def marks(g, x, top=None, bottom=0):
    g.anchor("top", x, g.P.C if top is None else top)
    g.anchor("bottom", x, bottom)


def U(name, *unicodes):
    return glyph(name, *unicodes, upper=True)


# ---------------------------------------------------------------- straight


@U("H", 0x48)
def H(g):
    P = g.P
    xl, xr = P.ua, P.ustem + P.cw(410) + P.ua
    stem(g, xl, 0, P.C)
    stem(g, xr, 0, P.C)
    bar(g, xl, xr, P.C * 0.5, s=0.94)
    marks(g, (xl + xr) / 2)
    g.space(1.0, 1.0)


@U("I", 0x49)
def I(g):
    P = g.P
    stem(g, P.ua, 0, P.C)
    marks(g, P.ua)
    g.space(1.0, 1.0)


def e_bars(g, top=True, middle=True, bottom=True, w=None, mid=0.5):
    P = g.P
    w = P.ustem + P.cw(300) if w is None else w
    stem(g, P.ua, 0, P.C)
    hb = P.ub * 0.96
    if top:
        bar(g, P.ua, w, P.C - hb, s=0.96)
    if middle:
        bar(g, P.ua, w - P.cw(30), P.C * mid, s=0.92)
    if bottom:
        bar(g, P.ua, w + P.cw(10), hb, s=0.96)
    return w


@U("E", 0x45)
def E(g):
    w = e_bars(g)
    marks(g, w / 2 + g.P.ua * 0.5)
    g.space(1.0, 0.5)


@U("F", 0x46)
def F(g):
    w = e_bars(g, bottom=False, mid=0.47)
    marks(g, w / 2 + g.P.ua * 0.5)
    g.space(1.0, 0.4)


@U("L", 0x4C)
def L(g):
    w = e_bars(g, top=False, middle=False)
    marks(g, g.P.ua * 1.2)
    g.space(1.0, 0.3)


@U("T", 0x54)
def T(g):
    P = g.P
    w = P.ustem + P.cw(470)
    hb = P.ub * 0.96
    bar(g, 0, w, P.C - hb, s=0.96)
    stem(g, w / 2, 0, P.C - hb)
    marks(g, w / 2)
    g.space(0.25, 0.25)


@U("J", 0x4A)
def J(g):
    P = g.P
    xs = P.ua + P.cw(180)
    yb = bottom_line(P, True)
    s = g.stroke((xs, P.C))
    s.line((xs, P.C * 0.22))
    s.arc((xs - P.ua - P.cw(64), yb), "v", k=(0.6, 0.6))
    s.line((xs - P.ua - P.cw(150), yb), s=0.96)
    marks(g, xs - P.cw(40))
    g.space(0.3, 1.0)


@U("U", 0x55)
def U_(g):
    P = g.P
    xl, xr = P.ua, P.ustem + P.cw(410) + P.ua
    yb = bottom_line(P, True)
    s = g.stroke((xl, P.C))
    s.line((xl, P.C * 0.36))
    s.arc(((xl + xr) / 2, yb), "v", k=(0.6, 0.6))
    s.arc((xr, P.C * 0.36), "h", k=(0.6, 0.6))
    s.line((xr, P.C))
    marks(g, (xl + xr) / 2)
    g.space(0.96, 0.96)


@U("Z", 0x5A)
def Z(g):
    P = g.P
    w = P.ustem * 1.1 + P.cw(430)
    hb = P.ub * 0.96
    xr = w - P.ua * 0.3
    bar(g, P.cw(24), xr, P.C - hb, s=0.96)
    bar(g, 0, w, hb, s=0.96)
    from parts import edge_line
    edge_line(g, (xr, P.C - 2 * hb), (0, 2 * hb), 1, -1, nib=g.nib.scaled(1.02))
    marks(g, w / 2)
    g.space(0.4, 0.4)


# ---------------------------------------------------------------- diagonals


@U("V", 0x56)
def V(g):
    P = g.P
    w = P.ustem * 1.6 + P.cw(500)
    zigzag(g, [(0, P.C), (w / 2, 0, P.ustem * 0.5), (w, P.C)], [(1.0, 0.9), (0.9, 1.0)],
           g.nib.scaled(0.96))
    marks(g, w / 2)
    g.space(0.1, 0.1)


@U("A", 0x41)
def A(g):
    P = g.P
    w = P.ustem * 1.6 + P.cw(520)
    nib = g.nib.scaled(0.96)
    zigzag(g, [(0, 0), (w / 2, P.C, P.ustem * 0.5), (w, 0)], [(1.0, 0.9), (0.9, 1.0)], nib)
    # the bar, from edge to edge
    yb = P.C * 0.30
    left = place(nib, (0, 0), (w / 2 - P.ustem * 0.25, P.C), -1, -1, 1.0, 0.9)
    right = place(nib, (w / 2 + P.ustem * 0.25, P.C), (w, 0), 1, 1, 0.9, 1.0)
    xa = x_at(left, yb)
    xb = x_at(right, yb)
    bar(g, xa, xb, yb, s=0.9)
    marks(g, w / 2)
    g.space(0.1, 0.1)


@U("W", 0x57)
def W(g):
    P = g.P
    wd = P.ustem * 2.8 + P.cw(820)
    flat = P.ustem * 0.44
    q = wd / 4
    zigzag(g, [(0, P.C), (q + q * 0.08, 0, flat), (wd / 2, P.C, flat * 0.7),
               (3 * q - q * 0.08, 0, flat), (wd, P.C)],
           [(1.0, 0.9), (0.9, 0.84), (0.84, 0.9), (0.9, 1.0)], g.nib.scaled(0.9))
    marks(g, wd / 2)
    g.space(0.1, 0.1)


@U("M", 0x4D)
def M(g):
    P = g.P
    w = P.ustem * 2 + P.cw(530)
    sn = 0.9
    xl, xr = P.ua * sn, w - P.ua * sn
    stem(g, xl, 0, P.C, s=sn)
    stem(g, xr, 0, P.C, s=sn)
    zigzag(g, [(0, P.C), (w / 2, P.C * 0.05, P.ustem * 0.46), (w, P.C)], [(1.0, 0.9), (0.9, 1.0)],
           g.nib.scaled(0.96))
    marks(g, w / 2)
    g.space(0.9, 0.9)


@U("N", 0x4E)
def N(g):
    P = g.P
    w = P.ustem * 2 + P.cw(420)
    sn = 0.9
    xl, xr = P.ua * sn, w - P.ua * sn
    stem(g, xl, 0, P.C, s=sn)
    stem(g, xr, 0, P.C, s=sn)
    from parts import edge_line
    edge_line(g, (0, P.C), (w, 0), -1, 1, nib=g.nib.scaled(1.0))
    marks(g, w / 2)
    g.space(0.9, 0.9)


@U("X", 0x58)
def X(g):
    P = g.P
    w = P.ustem * 1.4 + P.cw(470)
    nib = g.nib.scaled(0.96)
    from parts import edge_line
    edge_line(g, (w * 0.02, P.C), (w, 0), -1, 1, nib=nib)
    edge_line(g, (w * 0.98, P.C), (0, 0), 1, -1, nib=nib)
    marks(g, w / 2)
    g.space(0.1, 0.1)


@U("Y", 0x59)
def Y(g):
    P = g.P
    w = P.ustem * 1.4 + P.cw(480)
    yj = P.C * 0.42
    zigzag(g, [(0, P.C), (w / 2, yj, P.ustem), (w, P.C)], [(1.0, 0.92), (0.92, 1.0)],
           g.nib.scaled(0.96))
    stem(g, w / 2, 0, yj + P.ub)
    marks(g, w / 2)
    g.space(0.1, 0.1)


@U("K", 0x4B)
def K(g):
    P = g.P
    xs = P.ua
    w = P.ustem + P.cw(440)
    stem(g, xs, 0, P.C)
    nib = g.nib.scaled(0.96)
    c0, c1 = place(nib, (xs, P.C * 0.30), (w - P.cw(20), P.C), 0, 1)
    through(g, c0, c1, nib=nib, cap0=90, cap1=0)
    t = 0.42
    lx, ly = c0[0] + (c1[0] - c0[0]) * t, c0[1] + (c1[1] - c0[1]) * t
    d0, d1 = place(nib, (w, 0), (lx - P.ua * 0.4, ly + P.ub * 0.2), 1, 0, 1.0, 0.9)
    through(g, d0, d1, s0=1.0, s1=0.9, nib=nib, cap0=0, cap1=0)
    marks(g, w * 0.5)
    g.space(1.0, 0.1)


# ---------------------------------------------------------------- rounds


def o_width(P):
    return P.cw(555) + 2 * P.ustem * P.rnd


@U("O", 0x4F)
def O(g):
    P = g.P
    w = o_width(P)
    ring(g, 0, w, bottom_line(P, True), top_line(P, True))
    marks(g, w / 2)
    g.space(0.6, 0.6)


@U("Q", 0x51)
def Q(g):
    P = g.P
    w = o_width(P)
    ring(g, 0, w, bottom_line(P, True), top_line(P, True))
    nib = g.nib.scaled(1.0)
    through(g, (w * 0.62, P.C * 0.10), (w * 0.98, -P.C * 0.17), s0=0.86, s1=1.0, nib=nib,
            cap0=None, cap1=None)
    marks(g, w / 2)
    g.space(0.6, 0.6)


def big_c(P, w, cut0, cut1):
    a = P.ua
    xl, xr = a * P.rnd, w - a * P.rnd
    cx, cy = w / 2, P.C / 2
    yt, yb = top_line(P, True), bottom_line(P, True)
    k = P.k
    return (quarter((xr, cy), (cx, yt), "v", k), quarter((cx, yt), (xl, cy), "h", k),
            quarter((xl, cy), (cx, yb), "v", k), quarter((cx, yb), (xr, cy), "h", k))


@U("C", 0x43)
def C(g):
    P = g.P
    w = P.cw(470) + 2 * P.ustem * P.rnd
    q1, q2, q3, q4 = big_c(P, w, 0, 0)
    t0, t1 = P.t(0.44, 0.42, 0.40), P.t(0.58, 0.58, 0.62)
    s = g.stroke(point_of(q1, t0), tilt=20)
    part_of(s, q1, t0, 1.0)
    s.curve(q2[1], q2[2], q2[3], s=P.rnd)
    s.curve(q3[1], q3[2], q3[3], s=1.0)
    part_of(s, q4, 0.0, t1)
    s.end(tilt=20)
    marks(g, w / 2 + P.ua * 0.3)
    g.space(0.6, 0.4)


@U("G", 0x47)
def G_(g):
    P = g.P
    w = P.cw(500) + 2 * P.ustem * P.rnd
    q1, q2, q3, q4 = big_c(P, w, 0, 0)
    xr = w - P.ua * P.rnd
    yg = P.C * 0.44
    t0 = P.t(0.44, 0.42, 0.40)
    s = g.stroke(point_of(q1, t0), tilt=20)
    part_of(s, q1, t0, 1.0)
    s.curve(q2[1], q2[2], q2[3], s=P.rnd)
    s.curve(q3[1], q3[2], q3[3], s=1.0)
    # the lower right runs up to the bar as a short stem
    s.arc((xr, P.C * 0.30), "h", k=P.k, s=1.0)
    s.line((xr, yg))
    bar(g, xr - P.ua - P.cw(150), xr + P.ua, yg - P.ub * 0.92, s=0.92)
    marks(g, w / 2)
    g.space(0.6, 0.9)


def bowl_right(g, x0, xr, y0, y1, k=None, join=1.0):
    """a capital's bowl from the stem at x0 out to xr (centre lines) between
    the centre lines y0 and y1, its straight parts level"""
    P = g.P
    kk = P.k if k is None else k
    r = (y1 - y0) / 2
    xq = xr - r * 0.95
    s = g.stroke((x0, y1), s=join)
    s.line((xq, y1), s=1.0)
    s.arc((xr, (y0 + y1) / 2), "h", k=kk, s=P.rnd)
    s.arc((xq, y0), "v", k=kk, s=1.0)
    s.line((x0, y0), s=join)
    return s


@U("D", 0x44)
def D(g):
    P = g.P
    xs = P.ua
    xr = xs + P.ua + P.cw(470) + P.ua * P.rnd
    stem(g, xs, 0, P.C)
    s = g.stroke((xs, P.C - P.ub))
    xq = xs + (xr - xs) * 0.38
    s.line((xq, P.C - P.ub))
    s.arc((xr, P.C / 2), "h", k=(0.62, 0.6), s=P.rnd)
    s.arc((xq, P.ub), "v", k=(0.6, 0.62), s=1.0)
    s.line((xs, P.ub))
    marks(g, (xs + xr) / 2)
    g.space(1.0, 0.6)


@U("P", 0x50)
def P_(g):
    P = g.P
    xs = P.ua
    xr = xs + P.ua + P.cw(330) + P.ua * P.rnd
    stem(g, xs, 0, P.C)
    bowl_right(g, xs, xr, P.C * 0.42, P.C - P.ub * 0.96, k=(0.6, 0.62))
    marks(g, (xs + xr) / 2 - P.ua * 0.3)
    g.space(1.0, 0.5)


@U("R", 0x52)
def R(g):
    P = g.P
    xs = P.ua
    xr = xs + P.ua + P.cw(340) + P.ua * P.rnd
    stem(g, xs, 0, P.C)
    ym = P.C * 0.44
    bowl_right(g, xs, xr, ym, P.C - P.ub * 0.96, k=(0.6, 0.62))
    nib = g.nib.scaled(0.98)
    c0, c1 = place(nib, (xr + P.ua * 1.1, 0), (xr - P.cw(120), ym), 1, 0, 1.0, 0.9)
    through(g, c0, c1, s1=0.9, nib=nib, cap0=0, cap1=0)
    marks(g, (xs + xr) / 2 - P.ua * 0.3)
    g.space(1.0, 0.15)


@U("B", 0x42)
def B(g):
    P = g.P
    xs = P.ua
    ym = P.C * 0.53
    xr1 = xs + P.ua + P.cw(300) + P.ua * P.rnd
    xr2 = xs + P.ua + P.cw(350) + P.ua * P.rnd
    stem(g, xs, 0, P.C)
    bowl_right(g, xs + P.ua * 0.3, xr1, ym, P.C - P.ub * 0.96, k=(0.6, 0.62))
    bowl_right(g, xs, xr2, P.ub * 0.96, ym, k=(0.6, 0.62))
    marks(g, (xs + xr2) / 2 - P.ua * 0.3)
    g.space(1.0, 0.55)


@U("S", 0x53)
def S(g):
    P = g.P
    w = P.cw(340) + 2 * P.ustem
    flat = P.t(1.0, 1.0, 0.86)
    a, b = P.ua, P.ub
    yt = P.C + P.uov - b * flat
    yb = -P.uov + b * flat
    cx = w / 2
    xul, xur = a * 1.02 + w * 0.03, w - a - w * 0.02
    yu = P.C * P.t(0.76, 0.76, 0.74)
    xll, xlr = a + w * 0.01, w - a * 1.04
    yl = P.C * P.t(0.25, 0.25, 0.26)
    up_right = quarter((xur, yu), (cx + w * 0.01, yt), "v", 0.6)
    up_left = quarter((cx + w * 0.01, yt), (xul, yu), "h", (0.6, 0.62))
    lo_right = quarter((xlr, yl), (cx - w * 0.01, yb), "v", (0.62, 0.6))
    lo_left = quarter((cx - w * 0.01, yb), (xll, yl), "h", 0.6)
    t0, t1 = P.t(0.42, 0.42, 0.46), P.t(0.56, 0.56, 0.52)
    side = P.t(1.02, 1.02, 0.94)
    st = g.stroke(point_of(up_right, t0), s=0.98 * flat, tilt=P.t(20, 20, 10))
    part_of(st, up_right, t0, 1.0, s=flat)
    st.curve(up_left[1], up_left[2], up_left[3], s=side)
    spine = P.t(-30, -30, -26)
    k = P.t((0.44, 0.5), (0.44, 0.5), (0.46, 0.34))
    st.to((cx, P.C * 0.505), 270, spine, k=k, s=P.t(1.06, 1.08, 0.9))
    st.to((xlr, yl), spine, 270, k=k[::-1], s=side)
    st.curve(lo_right[1], lo_right[2], lo_right[3], s=flat)
    part_of(st, lo_left, 0.0, t1, s=0.98 * flat)
    st.end(tilt=P.t(20, 20, 10))
    marks(g, cx)
    g.space(0.6, 0.6)
