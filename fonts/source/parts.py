"""Parts many letters share: arches, bowls, rounds, diagonals."""

import math

from engine import direction, dot, split, sub, unit


def mirror_angle(a, flip):
    return 180 - a if flip else a


def top_line(P, upper=False):
    """the centre line of a round's top, so its edge overshoots the height"""
    return (P.C + P.uov - P.ub) if upper else (P.X + P.ov - P.b)


def bottom_line(P, upper=False, base=0):
    return base - (P.uov if upper else P.ov) + (P.ub if upper else P.b)


def quarter(p0, p1, start, k):
    """the cubic of a quarter turn from p0 to p1 (see Stroke.arc)"""
    k0, k1 = k if isinstance(k, tuple) else (k, k)
    (x0, y0), (x1, y1) = p0, p1
    if start == "h":
        return (p0, (x0 + (x1 - x0) * k0, y0), (x1, y1 - (y1 - y0) * k1), p1)
    return (p0, (x0, y0 + (y1 - y0) * k0), (x1 - (x1 - x0) * k1, y1), p1)


def part_of(stroke, cubic, t0=0.0, t1=1.0, s=None):
    """draw the part of a cubic from t0 to t1 (from the stroke's point)"""
    c = cubic
    if t1 < 1.0:
        c = split(c, t1)[0]
        t0 = t0 / t1
    if t0 > 0.0:
        c = split(c, t0)[1]
    stroke.curve(c[1], c[2], c[3], s)
    return c


def point_of(cubic, t):
    return split(cubic, t)[0][3] if 0 < t < 1 else (cubic[0] if t <= 0 else cubic[3])


def arch(g, xl, xr, down_to=0, apex=0.5, top=None, join=None, shoulder=None, upper=False):
    """an n's arch: out of the stem at xl (its centre line), over, and down
    as the stem at xr to down_to"""
    P = g.P
    yt = top_line(P, upper) if top is None else top
    yj = P.X - P.t(140, 150, 205) if join is None else join
    ys = P.X * 0.56 if shoulder is None else shoulder
    xt = xl + (xr - xl) * apex
    s = g.stroke((xl, yj), s=P.join)
    s.to((xt, yt), 60, 0, k=(0.40, 0.56), s=1.0)
    s.arc((xr, ys), "h", k=(0.62, 0.60), s=1.0)
    s.line((xr, down_to))
    return s


def bowl(g, xs, xo, flip=False, top_join=None, bottom_join=None, apex=0.47, bottom_apex=0.5,
         mid=None, k=None):
    """b's bowl, from the stem at xs (centre line) round to xo (the far
    side's centre line) and back into the stem; flip for d's"""
    P = g.P
    yt, yb = top_line(P), bottom_line(P)
    yjt = P.X - P.t(140, 150, 205) if top_join is None else top_join
    yjb = P.t(100, 104, 140) if bottom_join is None else bottom_join
    cy = P.X * 0.5 if mid is None else mid
    xt = xs + (xo - xs) * apex
    xb = xs + (xo - xs) * bottom_apex
    kk = P.k if k is None else k
    s = g.stroke((xs, yjt), s=P.join)
    s.to((xt, yt), mirror_angle(60, flip), mirror_angle(0, flip), k=(0.40, 0.56), s=1.0)
    s.arc((xo, cy), "h", k=kk, s=P.rnd)
    s.arc((xb, yb), "v", k=kk, s=1.0)
    s.to((xs, yjb), mirror_angle(180, flip), mirror_angle(150, flip), k=(0.52, 0.40), s=P.join)
    return s


def ring(g, x0, x1, y0, y1, k=None, side=None, flat=None, upper=False):
    """a round's closed stroke: x0, x1 its outer left and right edges,
    y0, y1 its centre line's bottom and top"""
    P = g.P
    nib = g.nib
    sd = P.rnd if side is None else side
    xl, xr = x0 + nib.a * sd, x1 - nib.a * sd
    cx, cy = (xl + xr) / 2, (y0 + y1) / 2
    kk = P.k if k is None else k
    s = g.stroke((xr, cy), s=sd)
    s.arc((cx, y1), "v", k=kk, s=1.0)
    s.arc((xl, cy), "h", k=kk, s=sd)
    s.arc((cx, y0), "v", k=kk, s=1.0)
    s.arc((xr, cy), "h", k=kk, s=sd)
    s.close()
    return s


def half_width(nib, d, s=1.0):
    """half a straight stroke's width along a horizontal line"""
    n = (-d[1], d[0])
    return nib.half(n) * s / abs(n[0])


def edge_line(g, top, bottom, top_side, bottom_side, s0=1.0, s1=1.0, nib=None, cap=0):
    """A straight stroke placed by its edges where it's cut: top and bottom
    are points (x, y) on its left edge (side -1) or right edge (+1) at the
    cut lines, which are horizontal (cap 0) unless cap says otherwise.
    Returns the stroke."""
    nib = nib or g.nib
    (xt, yt), (xb, yb) = top, bottom
    ct, cb = xt, xb
    for _ in range(6):
        d = unit(sub((ct, yt), (cb, yb)))
        ct = xt - top_side * half_width(nib, d, s0)
        cb = xb - bottom_side * half_width(nib, d, s1)
    st = g.stroke((cb, yb), s=s1, nib=nib, cap=cap)
    st.line((ct, yt), s=s0)
    st.end(cap)
    return st


def through(g, p0, p1, s0=1.0, s1=1.0, nib=None, cap0=0, cap1=0, mid=None):
    """a straight stroke along its centre line, cut horizontally"""
    st = g.stroke(p0, s=s0, nib=nib or g.nib, cap=cap0)
    if mid is not None:
        pm = ((p0[0] + p1[0]) / 2, (p0[1] + p1[1]) / 2)
        st.line(pm, s=mid)
    st.line(p1, s=s1)
    st.end(cap1)
    return st


def stem(g, x, y0, y1, s=1.0, nib=None):
    st = g.stroke((x, y0), s=s, nib=nib)
    st.line((x, y1))
    return st


def bar(g, x0, x1, y, s=1.0, nib=None):
    st = g.stroke((x0, y), s=s, nib=nib)
    st.line((x1, y))
    return st


def angle_of(p, q):
    return math.degrees(math.atan2(q[1] - p[1], q[0] - p[0]))


# ---------------------------------------------------------------- straight strokes that meet


def place(nib, p0, p1, side0, side1, s0=1.0, s1=1.0):
    """the centre line of a straight stroke whose edge on side0 (-1 its
    left edge along a level line, +1 its right, 0 its centre) passes
    through p0 and whose edge on side1 passes through p1"""
    (x0, y0), (x1, y1) = p0, p1
    c0, c1 = x0, x1
    for _ in range(8):
        d = unit(sub((c1, y1), (c0, y0)))
        c0 = x0 - side0 * half_width(nib, d, s0)
        c1 = x1 - side1 * half_width(nib, d, s1)
    return (c0, y0), (c1, y1)


def edges(nib, c0, c1, s0=1.0, s1=1.0):
    """a straight stroke's two edges, as lines: (its left, its right)
    along level lines"""
    d = unit(sub(c1, c0))
    n = (-d[1], d[0])
    e0, e1 = nib.support(n), nib.support(n)
    a = ((c0[0] + e0[0] * s0, c0[1] + e0[1] * s0), (c1[0] + e1[0] * s1, c1[1] + e1[1] * s1))
    b = ((c0[0] - e0[0] * s0, c0[1] - e0[1] * s0), (c1[0] - e1[0] * s1, c1[1] - e1[1] * s1))
    mid = lambda l: (l[0][0] + l[1][0]) / 2  # noqa: E731
    return (a, b) if mid(a) < mid(b) else (b, a)


def x_at(line, y):
    (x0, y0), (x1, y1) = line
    return x0 + (x1 - x0) * (y - y0) / (y1 - y0)


def meet(l1, l2):
    """where two lines cross"""
    (x1, y1), (x2, y2) = l1
    (x3, y3), (x4, y4) = l2
    den = (x1 - x2) * (y3 - y4) - (y1 - y2) * (x3 - x4)
    t = ((x1 - x3) * (y3 - y4) - (y1 - y3) * (x3 - x4)) / den
    return (x1 + t * (x2 - x1), y1 + t * (y2 - y1))


def at_height(c0, c1, s0, s1, y):
    """the point of a straight centre line at height y, and its scale"""
    u = (y - c0[1]) / (c1[1] - c0[1])
    return (c0[0] + (c1[0] - c0[0]) * u, y), s0 + (s1 - s0) * u


def zigzag(g, points, scales=None, nib=None):
    """Straight strokes from point to point, left to right, as in v, w, A
    and V. The first and last points are where the outer edges end, cut
    level: (x, y). The others are the points where two strokes meet, cut
    flat: (x, y, flat). Each stroke is cut where it meets the next, and the
    point between them is filled, so no edge pokes out past the other
    stroke. scales: (start, end) for each stroke."""
    from engine import polygon

    nib = nib or g.nib
    n = len(points) - 1
    scales = scales or [(1.0, 1.0)] * n
    lines = []
    for i in range(n):
        p, q = points[i], points[i + 1]
        # where this stroke's edges must pass: the left edge at a stroke's
        # start (the first point, or a meeting's left corner), the right
        # edge at its end
        if i == 0:
            r0, side0 = (p[0], p[1]), -1
        else:
            r0, side0 = (p[0] + p[2] / 2, p[1]), 1
        if i == n - 1:
            r1, side1 = (q[0], q[1]), 1
        else:
            r1, side1 = (q[0] - q[2] / 2, q[1]), -1
        s0, s1 = scales[i]
        c0, c1 = place(nib, r0, r1, side0, side1, s0, s1)
        lines.append([c0, c1, s0, s1])
    cuts = [[lines[i][0], lines[i][1]] for i in range(n)]  # centre-line ends after cutting
    cut_scales = [[lines[i][2], lines[i][3]] for i in range(n)]
    for i in range(1, n):
        x, y, flat = points[i]
        a, b = lines[i - 1], lines[i]
        a_left, a_right = edges(nib, *a)
        b_left, b_right = edges(nib, *b)
        crotch = meet(a_right, b_left)
        up = 1 if crotch[1] > y else -1  # a meeting at the bottom, or at the top
        yc = crotch[1]
        pa, sa = at_height(a[0], a[1], a[2], a[3], yc)
        pb, sb = at_height(b[0], b[1], b[2], b[3], yc)
        cuts[i - 1][1], cut_scales[i - 1][1] = pa, sa
        cuts[i][0], cut_scales[i][0] = pb, sb
        yd = yc + up * 6
        g.shape(polygon([
            (x_at(a_left, yd), yd), (x - flat / 2, y), (x + flat / 2, y),
            (x_at(b_right, yd), yd), crotch,
        ]))
    for (c0, c1), (s0, s1) in zip(cuts, cut_scales):
        st = g.stroke(c0, s=s0, nib=nib, cap=0)
        st.line(c1, s=s1)
        st.end(0)
