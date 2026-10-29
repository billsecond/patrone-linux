"""PatroneOS's logo as geometry: the P and the PATRONEOS wordmark, drawn from
straight lines and arcs, written out as SVG and drawn into pictures.

Every logo file in logos/ and icons/, and the logo in the wallpapers, comes
from the shapes here (make_logos.py, make_wallpapers.py), so they all match
and stay sharp at any size. The shapes were measured from the user's Neon
Horizon pictures (wallpapers/patrone-back-02.png), at 1672x941, and tidied:
one slant for every slanted edge, one stroke weight for the letters.

Units: the mark is 1000 tall; the wordmark's capitals are 100 tall. Colors
are Patrone's palette (branding/README.md), unchanged.
"""
import math

import cv2
import numpy as np

# Patrone's palette
BLUE = "#288BFA"        # Patrone Blue
HIGHLIGHT = "#4DA0FC"
MID = "#2372DD"
DEEP = "#1851A8"
SHADOW = "#134394"
WHITE = "#FFFFFF"
NAVY = "#14223A"        # the wordmark on light backgrounds

# --- Paths -------------------------------------------------------------------
# A contour is a start point and segments: ("L", x, y), a straight line to
# (x, y), or ("E", cx, cy, rx, ry, t0, t1), an elliptical arc from angle t0
# to t1 round (cx, cy). Angles are in radians with y pointing down, so a
# growing angle turns clockwise on screen, as SVG's sweep flag 1 does.


def _end(seg):
    if seg[0] == "L":
        return seg[1], seg[2]
    _, cx, cy, rx, ry, t0, t1 = seg
    return cx + rx * math.cos(t1), cy + ry * math.sin(t1)


class Contour:
    def __init__(self, start, segments):
        self.start = start
        self.segments = segments

    def transformed(self, k, dx, dy):
        def p(x, y):
            return x * k + dx, y * k + dy
        segs = []
        for s in self.segments:
            if s[0] == "L":
                segs.append(("L",) + p(s[1], s[2]))
            else:
                _, cx, cy, rx, ry, t0, t1 = s
                segs.append(("E",) + p(cx, cy) + (rx * k, ry * k, t0, t1))
        return Contour(p(*self.start), segs)

    def svg(self):
        f = _num
        d = [f"M{f(self.start[0])} {f(self.start[1])}"]
        for s in self.segments:
            if s[0] == "L":
                d.append(f"L{f(s[1])} {f(s[2])}")
            else:
                _, cx, cy, rx, ry, t0, t1 = s
                x, y = _end(s)
                large = 1 if abs(t1 - t0) > math.pi else 0
                sweep = 1 if t1 > t0 else 0
                d.append(f"A{f(rx)} {f(ry)} 0 {large} {sweep} {f(x)} {f(y)}")
        return "".join(d) + "Z"

    def points(self, k):
        """The outline as a polyline, at k pixels a unit."""
        pts = [self.start]
        for s in self.segments:
            if s[0] == "L":
                pts.append((s[1], s[2]))
            else:
                _, cx, cy, rx, ry, t0, t1 = s
                n = max(4, int(abs(t1 - t0) * max(rx, ry) * k / 0.6))
                for t in np.linspace(t0, t1, n + 1)[1:]:
                    pts.append((cx + rx * math.cos(t), cy + ry * math.sin(t)))
        return np.array(pts, np.float64)


def _num(v):
    s = f"{v:.2f}".rstrip("0").rstrip(".")
    return "0" if s == "-0" else s


def rounded(points, radii, hole=False):
    """A polygon with each corner rounded by its radius (0: sharp), clockwise
    on screen, or anticlockwise for a hole."""
    pts = [np.array(p, float) for p in points]
    area = sum(a[0] * b[1] - b[0] * a[1] for a, b in zip(pts, pts[1:] + pts[:1]))
    if (area < 0) != hole:
        pts, radii = pts[::-1], radii[::-1]
    n = len(pts)
    corners = []
    for i in range(n):
        p, prev, nxt, r = pts[i], pts[i - 1], pts[(i + 1) % n], radii[i]
        u, v = prev - p, nxt - p
        lu, lv = np.linalg.norm(u), np.linalg.norm(v)
        u, v = u / lu, v / lv
        half = math.acos(max(-1.0, min(1.0, float(u @ v)))) / 2
        if r <= 0 or half < 1e-6:
            corners.append((p, p, None))
            continue
        cut = r / math.tan(half)
        a, b = p + u * cut, p + v * cut
        bis = (u + v) / np.linalg.norm(u + v)
        c = p + bis * (r / math.sin(half))
        t0 = math.atan2(a[1] - c[1], a[0] - c[0])
        t1 = math.atan2(b[1] - c[1], b[0] - c[0])
        # the short way round
        while t1 - t0 > math.pi:
            t1 -= 2 * math.pi
        while t0 - t1 > math.pi:
            t1 += 2 * math.pi
        corners.append((a, b, ("E", c[0], c[1], r, r, t0, t1)))
    start = corners[0][1]
    segs = []
    for a, b, arc in corners[1:] + corners[:1]:
        segs.append(("L", a[0], a[1]))
        if arc:
            segs.append(arc)
    return Contour((start[0], start[1]), segs)


# --- Paints ------------------------------------------------------------------

def rgb(hexcolor):
    v = int(hexcolor.lstrip("#"), 16)
    return np.array([v >> 16, (v >> 8) & 255, v & 255], np.float64) / 255.0


class Linear:
    """A linear gradient from p0 to p1 with stops [(offset, color)],
    interpolated in sRGB as SVG does by default."""

    def __init__(self, p0, p1, stops):
        self.p0, self.p1, self.stops = p0, p1, stops

    def transformed(self, k, dx, dy):
        return Linear((self.p0[0] * k + dx, self.p0[1] * k + dy),
                      (self.p1[0] * k + dx, self.p1[1] * k + dy), self.stops)

    def at(self, x, y):
        (x0, y0), (x1, y1) = self.p0, self.p1
        ex, ey = x1 - x0, y1 - y0
        t = np.clip(((x - x0) * ex + (y - y0) * ey) / (ex * ex + ey * ey), 0, 1)
        offs = np.array([o for o, _ in self.stops])
        cols = np.array([rgb(c) for _, c in self.stops])
        return np.stack([np.interp(t, offs, cols[:, i]) for i in range(3)], axis=-1)


class Solid:
    def __init__(self, color):
        self.color = color

    def transformed(self, k, dx, dy):
        return self

    def at(self, x, y):
        return np.broadcast_to(rgb(self.color), x.shape + (3,))


class Piece:
    """Contours filled (nonzero) with one paint."""

    def __init__(self, contours, paint):
        self.contours, self.paint = contours, paint

    def transformed(self, k, dx, dy):
        return Piece([c.transformed(k, dx, dy) for c in self.contours],
                     self.paint.transformed(k, dx, dy))


def transformed(pieces, k, dx=0.0, dy=0.0):
    return [p.transformed(k, dx, dy) for p in pieces]


def bounds(pieces):
    pts = np.concatenate([c.points(4) for p in pieces for c in p.contours])
    return pts[:, 0].min(), pts[:, 1].min(), pts[:, 0].max(), pts[:, 1].max()


# --- The mark ----------------------------------------------------------------
# A slanted P in four pieces: a band across the top that folds down round
# the bowl and tapers to a point, the lower half of the bowl, and the stem,
# split by a facet into a lit upper part and a shaded lower one. Every
# slanted edge leans SLANT (dx/dy, about 57 degrees from level): the stem and
# the bowl's left edge one way, the band's end and the facet the other.

SLANT = 0.66
BAND_TOP, BAND_BOTTOM = 0.0, 300.0       # the band; under it, the slit
LOWER_TOP, LOWER_BOTTOM = 398.0, 718.0   # the bowl's lower half and the stem's top
STEM_BOTTOM = 1000.0
BAND_LEFT = 230.0                        # where the band's slanted end meets its top
STEM_WIDTH = 394.0                       # across, level
STEM_GAP = 100.0                         # between the stem and the bowl, level
BOWL_FLAT = 1170.0                       # where the bowl's top and bottom start to curve
BOWL_RX, BOWL_RY = 255.0, 337.0          # its corners, elliptical
FOLD_X, FOLD_R = 1117.0, 294.0           # the fold: the band's underside curving down
FACET = (360.7, 751.8)                   # a point on the stem's facet


def _stem_left(y):
    return SLANT * (STEM_BOTTOM - y)


def _bowl_point(t, lower):
    cy = (BAND_TOP + LOWER_BOTTOM) / 2 + (22.0 if lower else -22.0)
    return BOWL_FLAT + BOWL_RX * math.cos(t), cy + BOWL_RY * math.sin(t)


def _fold_meets_bowl():
    """Where the fold's circle crosses the bowl's lower corner: the band's tip."""
    cx, cy = FOLD_X, BAND_BOTTOM + FOLD_R
    lo, hi = 0.0, math.pi / 2
    for _ in range(60):
        t = (lo + hi) / 2
        x, y = _bowl_point(t, True)
        if math.hypot(x - cx, y - cy) > FOLD_R:
            lo = t
        else:
            hi = t
    return lo


def mark(mono=None):
    """The P, 1425 by 1000 units. mono: one color for the whole mark."""
    cy_mid = (BAND_TOP + LOWER_BOTTOM) / 2
    top_c, low_c = cy_mid - 22.0, cy_mid + 22.0
    t_tip = _fold_meets_bowl()
    tip = _bowl_point(t_tip, True)
    fold_c = (FOLD_X, BAND_BOTTOM + FOLD_R)
    a_tip = math.atan2(tip[1] - fold_c[1], tip[0] - fold_c[0])
    band = Contour((BAND_LEFT, BAND_TOP), [
        ("L", BOWL_FLAT, BAND_TOP),
        ("E", BOWL_FLAT, top_c, BOWL_RX, BOWL_RY, -math.pi / 2, 0.0),
        ("L", BOWL_FLAT + BOWL_RX, low_c),
        ("E", BOWL_FLAT, low_c, BOWL_RX, BOWL_RY, 0.0, t_tip),
        ("E", fold_c[0], fold_c[1], FOLD_R, FOLD_R, a_tip, -math.pi / 2),
        ("L", BAND_LEFT + SLANT * (BAND_BOTTOM - BAND_TOP), BAND_BOTTOM),
    ])
    # The lower half runs under the band's tip, which is drawn over it, so
    # no gap shows where they meet.
    lower_left = _stem_left(LOWER_TOP) + STEM_WIDTH + STEM_GAP
    t_top = math.asin((LOWER_TOP - low_c) / BOWL_RY)
    lower = Contour((lower_left, LOWER_TOP), [
        ("L",) + _bowl_point(t_top, True),
        ("E", BOWL_FLAT, low_c, BOWL_RX, BOWL_RY, t_top, math.pi / 2),
        ("L", lower_left - SLANT * (LOWER_BOTTOM - LOWER_TOP), LOWER_BOTTOM),
    ])
    stem_pts = [(_stem_left(LOWER_TOP), LOWER_TOP), (_stem_left(LOWER_TOP) + STEM_WIDTH, LOWER_TOP),
                (STEM_WIDTH, STEM_BOTTOM), (0.0, STEM_BOTTOM)]
    stem = rounded(stem_pts, [0] * 4)
    # The facet leans the band's way, through FACET
    fx, fy = FACET

    def facet_meets(x0):  # x0 + -SLANT*(y - LOWER_TOP) edge, against the facet
        y = (x0 + SLANT * LOWER_TOP - fx + SLANT * fy) / (2 * SLANT)
        return x0 - SLANT * (y - LOWER_TOP), y
    upper = rounded([stem_pts[0], stem_pts[1], facet_meets(stem_pts[1][0]),
                     facet_meets(stem_pts[0][0])], [0] * 4)
    if mono:
        paint = Solid(mono)
        return [Piece([lower], paint), Piece([stem], paint), Piece([band], paint)]
    return [
        Piece([lower], Linear((700, 718), (1400, 400), [(0, DEEP), (0.55, MID), (1, BLUE)])),
        Piece([stem], Linear((450, 700), (100, 1000), [(0, DEEP), (1, SHADOW)])),
        Piece([upper], Linear((700, 398), (300, 850), [(0, BLUE), (1, MID)])),
        Piece([band], Linear((250, 300), (1350, 150), [(0, BLUE), (1, HIGHLIGHT)])),
    ]


MARK_SIZE = (1425.0, 1000.0)

# --- The wordmark ------------------------------------------------------------
# PATRONE and OS in wide, squared capitals: rounded outer corners, no
# crossbar on the A, and a notch under the top bar of P, R and E that echoes
# the slit in the mark. Capitals are 100 tall.

H_STROKE = 22.3          # horizontal strokes
V_STROKE = 23.5          # vertical strokes
MID_BAR = (39.4, 59.9)   # E's and S's middle bar
BOWL_BAR = (43.4, 66.0)  # P's and R's
NOTCH = (22.3, 29.3)     # under the top bar, from the left edge
NOTCH_DEPTH = 20.0
R_OUT, R_TURN, R_IN, R_SQ = 26.5, 22.0, 4.0, 1.5


def _left_edge_with_notch(x, top_y_after):
    """The points down a stem's left edge from the bottom, with the notch:
    as part of a clockwise outline going up the left side."""
    a, b = NOTCH
    return [(x, b), (x + NOTCH_DEPTH, b), (x + NOTCH_DEPTH, a), (x, a)], [1.0, 1.0, 1.0, 1.0]


def _p_like(x0, x1):
    """P's outline and counter (R's too, without the leg)."""
    side = x1 - 25.0
    a, b = BOWL_BAR
    notch, nr = _left_edge_with_notch(x0, 0)
    outline = rounded(
        [(x0, 0), (x1, 0), (x1, b), (x0 + V_STROKE, b), (x0 + V_STROKE, 100), (x0, 100)] + notch,
        [R_SQ, R_OUT, R_OUT, R_IN, R_SQ, R_SQ] + nr)
    counter = rounded([(x0 + V_STROKE, H_STROKE), (side, H_STROKE), (side, a), (x0 + V_STROKE, a)],
                      [R_IN] * 4, hole=True)
    return [outline, counter]


def _letters():
    """{letter: [contours]} for P A T R O N E O S, in place."""
    L = {}
    L["P"] = _p_like(0.0, 129.0)
    # A: two legs meeting in a flat top, leaning SLANT-ish like the mark
    lean = 0.634
    x0, top_l, top_r, x1 = 149.8, 213.2, 237.7, 301.1
    leg = 29.1
    apex_x = (x0 + leg + x1 - leg) / 2
    apex_y = 100 - (apex_x - (x0 + leg)) / lean
    L["A"] = [rounded([(x0, 100), (top_l, 0), (top_r, 0), (x1, 100), (x1 - leg, 100), (apex_x, apex_y),
                       (x0 + leg, 100)], [R_SQ] * 7)]
    # T
    tx0, tx1 = 313.6, 429.9
    sx = (tx0 + tx1) / 2 - V_STROKE / 2
    L["T"] = [rounded([(tx0, 0), (tx1, 0), (tx1, H_STROKE), (sx + V_STROKE, H_STROKE), (sx + V_STROKE, 100),
                       (sx, 100), (sx, H_STROKE), (tx0, H_STROKE)], [R_SQ] * 8)]
    # R: P's shape and a leg from the bowl's bar to the right foot
    rx0, rx1 = 458.8, 583.6
    r = _p_like(rx0, rx1)
    foot_r, foot_l = 584.5, 584.5 - 30.6
    lean_r = 0.88
    up = 100 - BOWL_BAR[0] - 6
    r.append(rounded([(foot_l - lean_r * up, 100 - up), (foot_r - lean_r * up, 100 - up), (foot_r, 100),
                      (foot_l, 100)], [0, 0, R_SQ, R_SQ]))
    L["R"] = r

    def o(x0, x1):
        return [rounded([(x0, 0), (x1, 0), (x1, 100), (x0, 100)], [R_OUT] * 4),
                rounded([(x0 + V_STROKE, H_STROKE), (x1 - V_STROKE, H_STROKE), (x1 - V_STROKE, 100 - H_STROKE),
                         (x0 + V_STROKE, 100 - H_STROKE)], [R_IN] * 4, hole=True)]
    L["O"] = o(612.6, 736.1)
    # N: the diagonal runs from the left stem's top to the right stem's foot
    n0, n3 = 767.2, 889.9
    n1, n2 = n0 + V_STROKE, n3 - V_STROKE
    diag = 1.07
    L["N"] = [rounded([(n0, 0), (n1, 0), (n2, (n2 - n1) / diag), (n2, 0), (n3, 0), (n3, 100), (n2, 100),
                       (n1, 100 - (n2 - n1) / diag), (n1, 100), (n0, 100)],
                      [R_SQ, R_SQ, R_SQ, R_SQ, R_SQ, R_SQ, R_SQ, R_SQ, R_SQ, R_SQ])]
    # E
    e0, e1, em = 923.4, 1034.8, 1028.3
    es = e0 + V_STROKE
    a, b = MID_BAR
    notch, nr = _left_edge_with_notch(e0, 0)
    L["E"] = [rounded([(e0, 0), (e1, 0), (e1, H_STROKE), (es, H_STROKE), (es, a), (em, a), (em, b), (es, b),
                       (es, 100 - H_STROKE), (e1, 100 - H_STROKE), (e1, 100), (e0, 100)] + notch,
                      [R_SQ, R_SQ, R_SQ, R_IN, R_IN, R_SQ, R_SQ, R_IN, R_IN, R_SQ, R_SQ, R_SQ] + nr)]
    L["O2"] = o(1065.4, 1188.9)
    # S: square ends, rounded turns
    s0, s3 = 1216.8, 1342.7
    s1, s2 = s0 + V_STROKE, s3 - V_STROKE
    L["S"] = [rounded([(s0, 0), (s3 - 2.7, 0), (s3 - 2.7, H_STROKE), (s1, H_STROKE), (s1, a), (s3, a),
                       (s3, 100), (s0 + 2.7, 100), (s0 + 2.7, 100 - H_STROKE), (s2, 100 - H_STROKE), (s2, b),
                       (s0, b)],
                      [R_OUT, R_SQ, R_SQ, R_IN, R_IN, R_TURN, R_OUT, R_SQ, R_SQ, R_IN, R_IN, R_TURN])]
    return L


WORDMARK_WIDTH = 1342.7


def wordmark(dark=True, mono=None):
    """PATRONE in white (navy with dark=False) and OS in Patrone Blue, capitals
    100 tall, 1342.7 wide."""
    L = _letters()
    name = [c for k in "PATRONE" for c in L[k]]
    os_ = L["O2"] + L["S"]
    if mono:
        return [Piece(name + os_, Solid(mono))]
    text = Solid(WHITE if dark else NAVY)
    blue = Linear((0, 0), (0, 100), [(0, HIGHLIGHT), (1, BLUE)] if dark else [(0, BLUE), (1, MID)])
    return [Piece(name, text), Piece(os_, blue)]


# --- Lockups -----------------------------------------------------------------
# Stacked, as in the Neon Horizon pictures: the mark over the wordmark, set a
# little left of centre (the bowl is the heavier side). In the wordmark's
# units: the mark is 396.8 tall, then a gap of 48.4, then the capitals.

STACK_MARK = 396.8 / 1000.0
STACK_GAP = 48.4
STACK_SHIFT = -12.9
# Side by side: the mark 2.4 times the capitals' height, centred on them
SIDE_MARK = 240.0 / 1000.0
SIDE_GAP = 62.0


def stacked(dark=True, mono=None):
    w = MARK_SIZE[0] * STACK_MARK
    mx = WORDMARK_WIDTH / 2 + STACK_SHIFT - w / 2
    top = MARK_SIZE[1] * STACK_MARK + STACK_GAP
    return (transformed(mark(mono), STACK_MARK, mx, 0)
            + transformed(wordmark(dark, mono), 1.0, 0, top))


def side_by_side(dark=True, mono=None):
    w, h = MARK_SIZE[0] * SIDE_MARK, MARK_SIZE[1] * SIDE_MARK
    return (transformed(mark(mono), SIDE_MARK, 0, 50 - h / 2)
            + transformed(wordmark(dark, mono), 1.0, w + SIDE_GAP, 0))


# --- Output ------------------------------------------------------------------

def svg(pieces, size=None, pad=0.0, title=None):
    """An SVG of the pieces, its viewBox their bounds plus pad; size: its
    width and height in pixels, or None for the viewBox's own."""
    x0, y0, x1, y1 = bounds(pieces)
    x0, y0, x1, y1 = x0 - pad, y0 - pad, x1 + pad, y1 + pad
    vw, vh = x1 - x0, y1 - y0
    w, h = size if size else (vw, vh)
    out = [f'<svg xmlns="http://www.w3.org/2000/svg" width="{_num(w)}" height="{_num(h)}" '
           f'viewBox="{_num(x0)} {_num(y0)} {_num(vw)} {_num(vh)}">']
    if title:
        out.append(f"<title>{title}</title>")
    defs, body = [], []
    for i, p in enumerate(pieces):
        d = "".join(c.svg() for c in p.contours)
        if isinstance(p.paint, Linear):
            g = p.paint
            stops = "".join(f'<stop offset="{_num(o)}" stop-color="{c}"/>' for o, c in g.stops)
            defs.append(f'<linearGradient id="g{i}" gradientUnits="userSpaceOnUse" x1="{_num(g.p0[0])}" '
                        f'y1="{_num(g.p0[1])}" x2="{_num(g.p1[0])}" y2="{_num(g.p1[1])}">{stops}</linearGradient>')
            fill = f"url(#g{i})"
        else:
            fill = p.paint.color
        body.append(f'<path fill="{fill}" d="{d}"/>')
    if defs:
        out.append("<defs>" + "".join(defs) + "</defs>")
    out += body
    out.append("</svg>")
    return "\n".join(out) + "\n"


def coverage(contours, W, H, k, dx, dy, ss=8, y_range=None):
    """Nonzero coverage (0 to 1) of contours at k pixels a unit, shifted by
    (dx, dy) pixels, on a W x H grid, sampled ss x ss times a pixel."""
    ya, yb = y_range or (0, H)
    shift = 4
    acc = np.zeros(((yb - ya) * ss, W * ss), np.int16)
    one = np.zeros_like(acc, np.uint8)
    for c in contours:
        pts = c.points(k * ss)
        x = (pts[:, 0] * k + dx) * ss
        y = (pts[:, 1] * k + dy - ya) * ss
        area = np.sum(x * np.roll(y, -1) - np.roll(x, -1) * y)
        poly = np.round(np.stack([x - 0.5, y - 0.5], 1) * (1 << shift)).astype(np.int32)
        one[:] = 0
        cv2.fillPoly(one, [poly], 1, cv2.LINE_8, shift)
        acc += one if area > 0 else -one.astype(np.int16)
    cov = (acc != 0).astype(np.float32)
    return cov.reshape(yb - ya, ss, W, ss).mean(axis=(1, 3))


def draw(pieces, W, H, k=1.0, dx=0.0, dy=0.0, ss=8, strip=256):
    """The pieces drawn on a transparent W x H canvas at k pixels a unit,
    shifted by (dx, dy) pixels: premultiplied RGB and alpha, floats."""
    color = np.zeros((H, W, 3), np.float32)
    alpha = np.zeros((H, W), np.float32)
    for p in pieces:
        x0, y0, x1, y1 = bounds([p])
        ya = max(0, int(math.floor(y0 * k + dy)) - 1)
        yb = min(H, int(math.ceil(y1 * k + dy)) + 2)
        for s in range(ya, yb, strip):
            e = min(yb, s + strip)
            a = coverage(p.contours, W, H, k, dx, dy, ss, (s, e))
            ys, xs = np.mgrid[s:e, 0:W].astype(np.float64) + 0.5
            c = p.paint.at((xs - dx) / k, (ys - dy) / k).astype(np.float32)
            color[s:e] = color[s:e] * (1 - a[..., None]) + c * a[..., None]
            alpha[s:e] = alpha[s:e] * (1 - a) + a
    return color, alpha


def png_rgba(pieces, width=None, height=None, pad=0.0, ss=8):
    """The pieces as an RGBA uint8 array (straight alpha) fitted into width
    or height pixels, plus pad pixels all round."""
    x0, y0, x1, y1 = bounds(pieces)
    k = (width - 2 * pad) / (x1 - x0) if width else (height - 2 * pad) / (y1 - y0)
    W = width or int(round((x1 - x0) * k + 2 * pad))
    H = height or int(round((y1 - y0) * k + 2 * pad))
    color, alpha = draw(pieces, W, H, k, pad - x0 * k + (W - 2 * pad - (x1 - x0) * k) / 2,
                        pad - y0 * k + (H - 2 * pad - (y1 - y0) * k) / 2, ss)
    straight = color / np.maximum(alpha, 1e-6)[..., None]
    out = np.dstack([straight, alpha])
    return (np.clip(out, 0, 1) * 255 + 0.5).astype(np.uint8)
