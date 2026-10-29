"""Patrone Sans's drawing engine: pen strokes along skeletons, outlined.

Every letter is drawn as strokes: a centre line (straight lines and cubic
curves through points chosen for each letter) swept by an oval pen, wider
than it is tall, so upright strokes come out a little heavier than
horizontal ones, as in most humanist sans serifs. Each point of a stroke
can scale the pen, which thins a stroke where it meets a stem, tapers a
comma and so on.

A stroke's outline is its two sides, each fitted with one cubic per
segment of the centre line, and its two ends. Every weight is drawn by the
same code with different numbers, so every weight has the same points in
the same order, which is what a variable font needs. Strokes overlap where
they meet; the variable font keeps the overlaps (finish.py marks them, as
TrueType asks).

A stroke is smooth: where a letter has a corner, it's two strokes.
"""

import math

# ---------------------------------------------------------------- vectors


def add(p, q):
    return (p[0] + q[0], p[1] + q[1])


def sub(p, q):
    return (p[0] - q[0], p[1] - q[1])


def mul(p, k):
    return (p[0] * k, p[1] * k)


def dot(p, q):
    return p[0] * q[0] + p[1] * q[1]


def cross(p, q):
    return p[0] * q[1] - p[1] * q[0]


def length(p):
    return math.hypot(p[0], p[1])


def unit(p):
    n = math.hypot(p[0], p[1])
    return (p[0] / n, p[1] / n)


def lerp(p, q, t):
    return (p[0] + (q[0] - p[0]) * t, p[1] + (q[1] - p[1]) * t)


def rotate(v, angle):
    c, s = math.cos(math.radians(angle)), math.sin(math.radians(angle))
    return (v[0] * c - v[1] * s, v[0] * s + v[1] * c)


def direction(angle):
    """unit vector at an angle in degrees, anticlockwise from east"""
    r = math.radians(angle)
    return (math.cos(r), math.sin(r))


# ---------------------------------------------------------------- curves


def bez(c, t):
    p0, p1, p2, p3 = c
    m = 1 - t
    a, b, cc, d = m * m * m, 3 * m * m * t, 3 * m * t * t, t * t * t
    return (
        a * p0[0] + b * p1[0] + cc * p2[0] + d * p3[0],
        a * p0[1] + b * p1[1] + cc * p2[1] + d * p3[1],
    )


def dbez(c, t):
    p0, p1, p2, p3 = c
    m = 1 - t
    a, b, cc = 3 * m * m, 6 * m * t, 3 * t * t
    return (
        a * (p1[0] - p0[0]) + b * (p2[0] - p1[0]) + cc * (p3[0] - p2[0]),
        a * (p1[1] - p0[1]) + b * (p2[1] - p1[1]) + cc * (p3[1] - p2[1]),
    )


def ddbez(c, t):
    p0, p1, p2, p3 = c
    m = 1 - t
    return (
        6 * m * (p2[0] - 2 * p1[0] + p0[0]) + 6 * t * (p3[0] - 2 * p2[0] + p1[0]),
        6 * m * (p2[1] - 2 * p1[1] + p0[1]) + 6 * t * (p3[1] - 2 * p2[1] + p1[1]),
    )


def split(c, t):
    """a cubic cut in two at t (de Casteljau)"""
    p0, p1, p2, p3 = c
    a, b, cc = lerp(p0, p1, t), lerp(p1, p2, t), lerp(p2, p3, t)
    d, e = lerp(a, b, t), lerp(b, cc, t)
    f = lerp(d, e, t)
    return (p0, a, d, f), (f, e, cc, p3)


def seg_tangent(seg, t):
    """unit tangent of a segment, also where a handle sits on its point"""
    if seg[0] == "L":
        return unit(sub(seg[2], seg[1]))
    c = seg[1:]
    d = dbez(c, t)
    if length(d) > 1e-6:
        return unit(d)
    # a handle on its point: the curve leaves towards the next one
    if t < 0.5:
        d = sub(c[2], c[0]) if length(sub(c[2], c[0])) > 1e-6 else sub(c[3], c[0])
    else:
        d = sub(c[3], c[1]) if length(sub(c[3], c[1])) > 1e-6 else sub(c[3], c[0])
    return unit(d)


def seg_point(seg, t):
    if seg[0] == "L":
        return lerp(seg[1], seg[2], t)
    return bez(seg[1:], t)


def smooth(t):
    return t * t * (3 - 2 * t)


# ---------------------------------------------------------------- fitting


def _chord_params(points):
    d = [0.0]
    for p, q in zip(points, points[1:]):
        d.append(d[-1] + length(sub(q, p)))
    total = d[-1] or 1.0
    return [x / total for x in d]


def fit_cubic(points, t0, t1):
    """One cubic through the first and last of points, leaving along t0 and
    arriving along t1, as close as it gets to the rest (least squares on the
    two handles' lengths, with the points' parameters refined by Newton)."""
    q0, q3 = points[0], points[-1]
    chord = length(sub(q3, q0))
    us = _chord_params(points)
    alpha = beta = chord / 3
    for _ in range(5):
        c11 = c12 = c22 = x1 = x2 = 0.0
        for u, p in zip(us, points):
            m = 1 - u
            b0, b1, b2, b3 = m * m * m, 3 * m * m * u, 3 * m * u * u, u * u * u
            base = add(mul(q0, b0 + b1), mul(q3, b2 + b3))
            r = sub(p, base)
            c11 += b1 * b1
            c12 += b1 * b2 * dot(t0, t1)
            c22 += b2 * b2
            x1 += b1 * dot(t0, r)
            x2 += b2 * dot(t1, r)
        det = c11 * c22 - c12 * c12
        if abs(det) > 1e-9:
            alpha = (x1 * c22 - x2 * c12) / det
            beta = (c12 * x1 - c11 * x2) / det
        if alpha <= chord * 0.02 or beta <= chord * 0.02 or alpha > chord * 2 or beta > chord * 2:
            alpha = beta = chord / 3
        c = (q0, add(q0, mul(t0, alpha)), sub(q3, mul(t1, beta)), q3)
        # Newton: move each point's parameter to its nearest place on c
        new = []
        for u, p in zip(us, points):
            d = sub(bez(c, u), p)
            d1, d2 = dbez(c, u), ddbez(c, u)
            den = dot(d1, d1) + dot(d, d2)
            if abs(den) > 1e-9:
                u = min(1.0, max(0.0, u - dot(d, d1) / den))
            new.append(u)
        new[0], new[-1] = 0.0, 1.0
        us = new
    err = max(length(sub(bez(c, u), p)) for u, p in zip(us, points))
    return c, err


# Curves whose outline one cubic can't follow closely are drawn in pieces.
# The same pieces are used in every master, so the masters stay compatible:
# make_ufos.py draws every master once to find them (ERRORS), then again.
PIECES = {}  # (glyph, stroke, segment): how many pieces
ERRORS = {}  # (glyph, stroke, segment): the farthest any master's outline strays
TOLERANCE = 1.5  # units


# ---------------------------------------------------------------- the pen


class Nib:
    """An oval pen: a is half its width (half an upright stroke's weight),
    b half its height (half a horizontal stroke's)."""

    def __init__(self, a, b):
        self.a, self.b = a, b

    def support(self, n):
        """the point of the oval whose outward normal is n"""
        a2, b2 = self.a * self.a, self.b * self.b
        d = math.sqrt(a2 * n[0] * n[0] + b2 * n[1] * n[1])
        return (a2 * n[0] / d, b2 * n[1] / d)

    def half(self, n):
        """half the stroke's weight across a stroke whose normal is n"""
        return math.sqrt(self.a * self.a * n[0] * n[0] + self.b * self.b * n[1] * n[1])

    def scaled(self, k):
        return Nib(self.a * k, self.b * k)


# ---------------------------------------------------------------- strokes


class Stroke:
    """A centre line drawn with a pen. Each point carries a scale for the
    pen (1 is the pen as it is); between two points the scale eases from
    one to the other.

    cap0, cap1: how the start and the end are cut: None squares them off
    across the stroke, an angle (degrees) cuts them along that direction.
    tilt0, tilt1 instead turn the square cut so many degrees towards
    upright (or, below zero, towards level), for terminals cut at a slant.
    """

    def __init__(self, p, nib, s=1.0, cap=None, tilt=None):
        self.nib = nib
        self.points = [tuple(p)]
        self.scales = [s]
        self.segs = []
        self.closed = False
        self.key = None  # (glyph name, stroke number), for PIECES
        self.cap0 = cap
        self.cap1 = None
        self.tilt0 = tilt
        self.tilt1 = None

    @property
    def here(self):
        return self.points[-1]

    def _add(self, seg, p, s):
        self.segs.append(seg)
        self.points.append(tuple(p))
        self.scales.append(self.scales[-1] if s is None else s)
        return self

    def line(self, p, s=None):
        return self._add(("L", self.here, tuple(p)), p, s)

    def curve(self, c1, c2, p, s=None):
        return self._add(("C", self.here, tuple(c1), tuple(c2), tuple(p)), p, s)

    def arc(self, p, start, k=0.56, s=None):
        """A quarter turn to p: leaving horizontally (start "h") or
        vertically ("v"), arriving the other way. k is how full the curve
        is (0.552 is a circle's), one number or one for each end."""
        k0, k1 = k if isinstance(k, tuple) else (k, k)
        x0, y0 = self.here
        x1, y1 = p
        if start == "h":
            c1 = (x0 + (x1 - x0) * k0, y0)
            c2 = (x1, y1 - (y1 - y0) * k1)
        else:
            c1 = (x0, y0 + (y1 - y0) * k0)
            c2 = (x1 - (x1 - x0) * k1, y1)
        return self.curve(c1, c2, p, s)

    def to(self, p, d0, d1, k=0.36, s=None):
        """A curve to p leaving along d0 and arriving along d1 (angles in
        degrees or vectors), its handles k of the distance long (one number,
        or one for each end)."""
        k0, k1 = k if isinstance(k, tuple) else (k, k)
        d0 = direction(d0) if isinstance(d0, (int, float)) else unit(d0)
        d1 = direction(d1) if isinstance(d1, (int, float)) else unit(d1)
        chord = length(sub(p, self.here))
        c1 = add(self.here, mul(d0, chord * k0))
        c2 = sub(p, mul(d1, chord * k1))
        return self.curve(c1, c2, p, s)

    def close(self):
        """Back to the first point; a closed stroke has no ends."""
        if length(sub(self.here, self.points[0])) > 0.5:
            raise ValueError("close() needs the last point on the first")
        self.points[-1] = self.points[0]
        self.segs[-1] = (*self.segs[-1][:-1], self.points[0])
        self.scales[-1] = self.scales[0]
        self.closed = True
        return self

    def end(self, cap=None, tilt=None):
        self.cap1 = cap
        self.tilt1 = tilt
        return self

    # ---- outline

    def _side(self, i, sign):
        """one side of segment i: the centre line moved out by the pen, as a
        line or as cubics (one, or more where PIECES says)"""
        seg = self.segs[i]
        s0, s1 = self.scales[i], self.scales[i + 1]

        def off(t):
            p = seg_point(seg, t)
            tan = seg_tangent(seg, t)
            n = (-tan[1] * sign, tan[0] * sign)
            s = s0 + (s1 - s0) * smooth(t)
            return add(p, mul(self.nib.support(n), s))

        if seg[0] == "L":
            return [["L", off(0.0), off(1.0)]]
        key = self.key and (*self.key, i)
        n = PIECES.get(key, 1)
        out = []
        for j in range(n):
            ta, tb = j / n, (j + 1) / n
            samples = [off(ta + (tb - ta) * k / 24) for k in range(25)]
            # where the scale changes the side isn't parallel to the centre
            # line, so inner ends take the side's own direction
            t0 = seg_tangent(seg, 0.0) if j == 0 else unit(sub(off(ta + 1e-4), off(ta - 1e-4)))
            t1 = seg_tangent(seg, 1.0) if j == n - 1 else unit(sub(off(tb + 1e-4), off(tb - 1e-4)))
            c, err = fit_cubic(samples, t0, t1)
            if key:
                ERRORS[key] = max(ERRORS.get(key, 0.0), err)
            out.append(["C", *c])
        return out

    def _check_smooth(self):
        joints = list(zip(self.segs, self.segs[1:]))
        if self.closed:
            joints.append((self.segs[-1], self.segs[0]))
        for a, b in joints:
            ta, tb = seg_tangent(a, 1.0), seg_tangent(b, 0.0)
            if dot(ta, tb) < math.cos(math.radians(3)):
                raise ValueError(f"{self.key}: corner in a stroke at {a[-1]}: draw it as two strokes")

    def outline(self):
        """the stroke's contours: one for an open stroke, two for a closed one"""
        self._check_smooth()
        left = [p for i in range(len(self.segs)) for p in self._side(i, 1)]
        right = [p for i in range(len(self.segs)) for p in self._side(i, -1)]
        if self.closed:
            return [orient(left, True), orient(right, False)] if area(left) ** 2 > area(right) ** 2 \
                else [orient(right, True), orient(left, False)]
        t_end = seg_tangent(self.segs[-1], 1.0)
        t_start = seg_tangent(self.segs[0], 0.0)
        for which, tan, cap, tilt, centre in ((-1, t_end, self.cap1, self.tilt1, self.points[-1]),
                                              (1, t_start, self.cap0, self.tilt0, self.points[0])):
            if cap is None and tilt is None:
                continue
            cut = direction(cap) if cap is not None else _tilted(tan, tilt)
            for side in (left, right):
                _cut(side[-1] if which == -1 else side[0], which, centre, tan, cut)
        contour = [tuple(g) for g in left]
        contour.append(("L", left[-1][-1], right[-1][-1]))
        contour.extend(reverse_seg(g) for g in reversed(right))
        contour.append(("L", right[0][1], left[0][1]))
        return [orient(contour, True)]


def _tilted(tan, tilt):
    """the square cut across a stroke, turned towards upright by tilt"""
    theta = math.degrees(math.atan2(tan[0], -tan[1])) % 180  # the square cut's angle
    to_upright = 90 - theta
    if tilt >= 0:
        turn = math.copysign(min(tilt, abs(to_upright)), to_upright)
    else:
        turn = -math.copysign(min(-tilt, 90 - abs(to_upright)), to_upright)
    return direction(theta + turn)


def _cut(seg, which, centre, tan, cut):
    """move one end of a side segment along the stroke onto the cut line
    through the centre line's end"""
    p = seg[-1] if which == -1 else seg[1]
    den = cross(tan, cut)
    if abs(den) < 1e-6:
        return
    k = cross(sub(centre, p), cut) / den
    shift = mul(tan, k)
    if which == -1:
        seg[-1] = add(seg[-1], shift)
        if seg[0] == "C":
            seg[-2] = add(seg[-2], shift)
    else:
        seg[1] = add(seg[1], shift)
        if seg[0] == "C":
            seg[2] = add(seg[2], shift)


def reverse_seg(g):
    return (g[0], *reversed(g[1:]))


def area(contour):
    """signed area of a contour's polygon of points, handles included
    (enough for which way it turns)"""
    pts = []
    for g in contour:
        pts.extend(g[1:-1] if g[0] == "C" else ())
        pts.append(g[-1])
    total = 0.0
    for p, q in zip(pts, pts[1:] + pts[:1]):
        total += cross(p, q)
    return total / 2


def orient(contour, outer):
    """outer contours anticlockwise and counters clockwise (as in UFO
    sources; the TrueType build turns them round)"""
    contour = [tuple(g) for g in contour]
    if (area(contour) > 0) != outer:
        contour = [reverse_seg(g) for g in reversed(contour)]
    return contour


# ---------------------------------------------------------------- shapes


def ellipse(cx, cy, rx, ry, k=0.5523):
    """a filled oval (dots), anticlockwise"""
    e = (cx + rx, cy)
    n = (cx, cy + ry)
    w = (cx - rx, cy)
    s = (cx, cy - ry)
    return [
        ("C", e, (cx + rx, cy + ry * k), (cx + rx * k, cy + ry), n),
        ("C", n, (cx - rx * k, cy + ry), (cx - rx, cy + ry * k), w),
        ("C", w, (cx - rx, cy - ry * k), (cx - rx * k, cy - ry), s),
        ("C", s, (cx + rx * k, cy - ry), (cx + rx, cy - ry * k), e),
    ]


def polygon(points):
    """a filled polygon, anticlockwise"""
    pts = [tuple(p) for p in points]
    contour = [("L", p, q) for p, q in zip(pts, pts[1:] + pts[:1])]
    return orient(contour, True)


def transform(contour, f):
    return [(g[0], *(f(p) for p in g[1:])) for g in contour]


def bounds(contours):
    xs, ys = [], []
    for c in contours:
        for g in c:
            if g[0] == "L":
                pts = g[1:]
            else:
                pts = [bez(g[1:], i / 16) for i in range(17)]
            for x, y in pts:
                xs.append(x)
                ys.append(y)
    if not xs:
        return None
    return min(xs), min(ys), max(xs), max(ys)
