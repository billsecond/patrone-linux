"""Draw Patrone's generated wallpapers: abstract designs in the brand's blues,
each in a light and a dark version, made from nothing but this script (no
photos, no downloads, no image models), so Patrone owns them outright.

    python tools/make_generated_wallpapers.py wallpapers/generated --sheet wallpapers/generated/contact-sheet.jpg

usage: make_generated_wallpapers.py OUT_DIR [--size W H] [--sheet FILE] [--crops FILE] [NAME...]

Writes OUT_DIR/patrone-NAME.png (dark) and OUT_DIR/patrone-NAME-light.png for
each design in DESIGNS (all of them unless names are given), 3840x2160 unless
--size says otherwise. --sheet writes a contact sheet of every one, light and
dark; --crops a sheet of the dark ones as GNOME's "zoom" shows them on 16:10,
4:3 and 21:9 screens.

They're deterministic: every "random" choice is a hash of a design's seed and
integer coordinates, with no random-number generator (whose streams can
change between NumPy versions), so the same command gives the same pictures.
Don't edit the images by hand; change the script and run it again.

Coordinates: u across and v down, both in units of the canvas height, with
(0, 0) in the middle, so v runs from -0.5 to 0.5 and u from -0.89 to 0.89 at
16:9. GNOME's "zoom" fills the screen and crops what's over: a 16:10 screen
shows |u| < 0.8, a 4:3 one |u| < 0.67 and a 21:9 ultrawide |v| < 0.37, so
everything that matters stays inside those. Sizes in these units keep a
design the same at any resolution.
"""
import argparse
import math
import os

import cv2
import numpy as np

from wallpaper_layers import dithered8

M32 = 0xFFFFFFFF


def rgb(hexcolor):
    """'#rrggbb' as a float RGB triple."""
    v = int(hexcolor.lstrip("#"), 16)
    return np.array([v >> 16, (v >> 8) & 255, v & 255], np.float32) / 255.0


def mix(a, b, t):
    return a + (b - a) * t


def smoothstep(a, b, x):
    t = np.clip((x - a) / (b - a), 0, 1)
    return t * t * (3 - 2 * t)


def ramp(t, stops):
    """Colours along t: stops is [(position, '#rrggbb'), ...]."""
    xs = [p for p, _ in stops]
    cols = np.array([rgb(h) for _, h in stops])
    return np.stack([np.interp(t, xs, cols[:, k]).astype(np.float32) for k in range(3)], axis=-1)


# Hashing: the only source of randomness.

def _mix32(h):
    """Bijective 32-bit integer mixing (lowbias32) of a uint32 array."""
    h = h ^ (h >> np.uint32(16))
    h = h * np.uint32(0x7FEB352D)
    h = h ^ (h >> np.uint32(15))
    h = h * np.uint32(0x846CA68B)
    return h ^ (h >> np.uint32(16))


def hash2(i, j, seed):
    """A uint32 hash of integer lattice coordinates i, j (arrays) and a seed."""
    i = np.asarray(i).astype(np.int64)
    j = np.asarray(j).astype(np.int64)
    h = (i * 0x27D4EB2D + j * 0x165667B1 + seed * 0x9E3779B1) & M32
    return _mix32(_mix32(h.astype(np.uint32)))


def unit(i, j, seed):
    """hash2 as floats in [0, 1)."""
    return hash2(i, j, seed).astype(np.float64) / 2.0**32


def rand(seed, *k):
    """A number in [0, 1) from a seed and integers k."""
    h = np.array([seed & M32], np.uint32)
    for x in k:
        h = _mix32(h ^ np.uint32((x * 0x9E3779B1) & M32))
    return float(h[0]) / 2.0**32


def noise(x, y, seed):
    """Gradient (Perlin) noise, roughly -1 to 1, one feature per unit."""
    x, y = np.broadcast_arrays(np.asarray(x, np.float32), np.asarray(y, np.float32))
    x0, y0 = np.floor(x), np.floor(y)
    fx, fy = x - x0, y - y0
    ix, iy = x0.astype(np.int64), y0.astype(np.int64)

    def dot(i, j, dx, dy):
        a = (hash2(i, j, seed) >> np.uint32(8)).astype(np.float32) * np.float32(2 * math.pi / 2**24)
        return np.cos(a) * dx + np.sin(a) * dy

    sx = fx * fx * fx * (fx * (fx * 6 - 15) + 10)
    sy = fy * fy * fy * (fy * (fy * 6 - 15) + 10)
    n0 = mix(dot(ix, iy, fx, fy), dot(ix + 1, iy, fx - 1, fy), sx)
    n1 = mix(dot(ix, iy + 1, fx, fy - 1), dot(ix + 1, iy + 1, fx - 1, fy - 1), sx)
    return mix(n0, n1, sy) * np.float32(1.4)


def fbm(x, y, seed, octaves=4, lacunarity=2.0, gain=0.5):
    """Noise summed over octaves, each finer and fainter: natural-looking detail."""
    total, amp, norm, f = 0.0, 1.0, 0.0, 1.0
    for o in range(octaves):
        total = total + amp * noise(x * f + 17.13 * o, y * f - 9.71 * o, seed + 101 * o)
        norm += amp
        amp *= gain
        f *= lacunarity
    return total / norm


class Canvas:
    def __init__(self, W, H):
        self.W, self.H = W, H
        self.px = 1.0 / H  # one pixel, in units of the height

    def uv(self, step=1):
        """u (1, w) and v (h, 1) at the centres of step x step blocks of pixels."""
        w, h = -(-self.W // step), -(-self.H // step)
        u = ((np.arange(w, dtype=np.float32) + 0.5) * step - self.W / 2) / self.H
        v = ((np.arange(h, dtype=np.float32) + 0.5) * step - self.H / 2) / self.H
        return u[None, :], v[:, None]

    def full(self, a):
        """A smooth field drawn at a lower resolution, enlarged to full size."""
        if a.shape[:2] == (self.H, self.W):
            return a
        return cv2.resize(np.ascontiguousarray(a), (self.W, self.H), interpolation=cv2.INTER_CUBIC)

    def blur(self, a, sigma, step=1):
        """Gaussian blur, sigma in units, of a field drawn at 1/step of full size."""
        return cv2.GaussianBlur(a, (0, 0), sigma * self.H / step)

    def fill(self, a, shape):
        return np.broadcast_to(a, shape).astype(np.float32)


def cover(c, d):
    """Coverage of a shape at signed distance d (units, negative inside),
    antialiased over one pixel."""
    return np.clip(0.5 - d / c.px, 0, 1).astype(np.float32)


def over(img, colour, alpha):
    """Paint colour (a triple or an image) over img with coverage alpha, in place."""
    a = alpha[..., None] if alpha.ndim == 2 else alpha
    img *= 1 - a
    img += colour * a


def shade(img, amount, tint):
    """Darken img towards tint (black in the dark; a pale blue in the light,
    so shadows don't turn grey), in place."""
    img *= 1 - amount[..., None] * (1 - rgb(tint))


def vertical(c, top, bottom, step=4, ease=(-0.5, 0.5)):
    """A top-to-bottom gradient at 1/step of full size."""
    u, v = c.uv(step)
    t = smoothstep(ease[0], ease[1], v)[..., None]
    return c.fill(mix(rgb(top), rgb(bottom), t), (v.shape[0], u.shape[1], 3))


def curve_cover(c, y, v):
    """Coverage of the area below the curve v = y(u), y sampled at every pixel
    column: the distance to the curve is the vertical gap times its cosine."""
    slope = np.gradient(y, axis=1) / c.px
    d = (y - v) / np.sqrt(1 + slope * slope)
    return cover(c, d), d


# The designs. Each takes a canvas and "dark" or "light" and returns a float
# RGB image, 0 to 1. The contact sheet shows them in this order; Settings
# sorts them by name.

DESIGNS = {}


def design(title):
    def register(fn):
        DESIGNS[fn.__name__] = (title, fn)
        return fn
    return register


@design("Patrone Focus")
def focus(c, mode):
    """Calm: an almost flat field with a low, soft band of light, for working."""
    u, v = c.uv(4)
    if mode == "dark":
        img = vertical(c, "#0b1322", "#05070b")
        glow, glow_a, edge, edge_a = "#2372dd", 0.16, "#020305", 0.55
    else:
        img = vertical(c, "#f7f9fd", "#e9f0f9")
        glow, glow_a, edge, edge_a = "#a9cbf8", 0.45, "#dbe5f2", 0.5
    g = np.exp(-((u / 0.8) ** 2 + ((v - 0.28) / 0.12) ** 2))
    over(img, rgb(glow), glow_a * g)
    vignette = smoothstep(0.55, 1.15, np.hypot(u / 1.25, v / 0.85))
    over(img, rgb(edge), edge_a * vignette)
    return c.full(img)


@design("Patrone Aurora")
def aurora(c, mode):
    """Curtains of light hanging across the sky."""
    S = 101
    dark = mode == "dark"
    u, v = c.uv(4)
    img = vertical(c, "#03050c", "#081224") if dark else vertical(c, "#eaf1fb", "#f7f9fd")
    # centre height, swing, frequency, colour in the dark and in the light, strength
    ribbons = [
        (0.04, 0.07, 2.1, "#3fc6ff", "#2a8cf0", 1.0),
        (-0.09, 0.09, 1.5, "#288bfa", "#3d74dc", 0.9),
        (-0.20, 0.06, 2.7, "#6f5cff", "#7c6cf0", 0.65),
    ]
    total = np.zeros_like(img)
    for i, (y0, swing, f, cd, cl, k) in enumerate(ribbons):
        phase = 2 * math.pi * rand(S, i)
        cy = y0 + swing * np.sin(f * u + phase) + 0.05 * fbm(u * 1.2, i * 7.3, S + i, 3)
        dv = cy - v  # above the ribbon's lower edge when positive
        rise = 0.65 * np.exp(-np.maximum(dv, 0) / 0.11) + 0.35 * np.exp(-np.maximum(dv, 0) / 0.025)
        edge = np.exp(-(np.minimum(dv, 0) / 0.016) ** 2)
        band = np.where(dv >= 0, rise, edge)
        rays = 0.84 + 0.16 * fbm(u * 24, i * 5.1, S + 10 + i, 2)
        along = smoothstep(-0.25, 0.55, fbm(u * 0.9 + i * 3.1, 1.7 * i, S + 20 + i, 2) + 0.2)
        I = (k * band * rays * along).astype(np.float32)
        if dark:
            total += I[..., None] * rgb(cd)
        else:
            over(img, rgb(cl), 0.5 * (1 - np.exp(-1.6 * I)))
    if dark:
        img = 1 - (1 - img) * np.exp(-1.15 * total)
    return c.full(img)


@design("Patrone Bloom")
def bloom(c, mode):
    """A bold, flowing gradient of blues with a touch of violet."""
    S = 303
    u, v = c.uv(8)
    wu = u + 0.2 * fbm(u * 0.8, v * 0.8, S, 3)
    wv = v + 0.2 * fbm(u * 0.8 + 5.2, v * 0.8 + 1.3, S + 1, 3)
    if mode == "dark":
        blobs = [(-0.6, -0.3, "#288bfa"), (0.55, 0.3, "#0f2a66"), (0.2, -0.38, "#5a46e8"),
                 (-0.3, 0.38, "#071028"), (0.8, -0.3, "#34b8ff"), (0.0, 0.05, "#173e9a"),
                 (-0.85, 0.2, "#0a1a44")]
    else:
        blobs = [(-0.6, -0.3, "#9ccaff"), (0.55, 0.3, "#d9e8fd"), (0.2, -0.38, "#c8c0ff"),
                 (-0.3, 0.38, "#f5f8fd"), (0.8, -0.3, "#94dcff"), (0.0, 0.05, "#b7d3fb"),
                 (-0.85, 0.2, "#e6effc")]
    num = 0
    den = 0
    for x, y, col in blobs:
        w = np.exp(-((wu - x) ** 2 + (wv - y) ** 2) / (2 * 0.28**2)) + 1e-6
        num = num + w[..., None] * rgb(col)
        den = den + w[..., None]
    return c.full((num / den).astype(np.float32))


@design("Patrone Ring")
def ring(c, mode):
    """The boot spinner's ring, large: a comet of light running round it."""
    dark = mode == "dark"
    cx, cy, R, w = 0.31, 0.0, 0.29, 0.011
    head = math.radians(-35)  # upper right; angles run clockwise on screen
    length = 0.62 * 2 * math.pi
    if dark:
        track_col, track_a = "#134394", 0.45
        stops = [(0, "#e4f1ff"), (0.1, "#4da0fc"), (0.4, "#288bfa"), (0.85, "#2372dd"), (1, "#2372dd")]
        bg_in, bg_out = "#0d1a33", "#030303"
    else:
        track_col, track_a = "#c6d8f1", 0.9
        stops = [(0, "#0e3480"), (0.1, "#1851a8"), (0.45, "#288bfa"), (0.85, "#78b2f9"), (1, "#78b2f9")]
        bg_in, bg_out = "#e3ecf8", "#f4f7fb"

    def comet(u, v, px):
        du, dv = u - cx, v - cy
        r = np.hypot(du, dv)
        s = ((head - np.arctan2(dv, du)) % (2 * math.pi)) / length  # 0 at the head
        fade = np.where(s <= 1, 1 - smoothstep(0.45, 1.0, s), 0).astype(np.float32)
        band = np.clip(0.5 - (np.abs(r - R) - w * 0.65) / px, 0, 1)
        hx, hy = cx + R * math.cos(head), cy + R * math.sin(head)
        cap = np.clip(0.5 - (np.hypot(u - hx, v - hy) - w * 0.65) / px, 0, 1)
        # Past the tail's end is the gap before the head: colour it as the head
        return np.maximum(band * fade, cap).astype(np.float32), np.where(s > 1, 0, s), r

    # Background and glow, drawn small
    u4, v4 = c.uv(4)
    rr = np.hypot(u4 - cx, v4 - cy)
    img = c.fill(mix(rgb(bg_in), rgb(bg_out), smoothstep(0, 0.95, rr)[..., None]), (v4.shape[0], u4.shape[1], 3))
    a4, s4, _ = comet(u4, v4, 4 * c.px)
    col4 = ramp(s4, stops)
    glow = c.blur(a4[..., None] * col4, 0.02, 4) * 1.1 + c.blur(a4, 0.07, 4)[..., None] * rgb("#288bfa") * 1.3
    if dark:
        img = 1 - (1 - img) * np.exp(-glow)
    else:
        over(img, rgb("#a8c9f6"), np.clip(c.blur(a4, 0.05, 4) * 0.5, 0, 1))
    img = c.full(img)

    u, v = c.uv()
    a, s, r = comet(u, v, c.px)
    track = cover(c, np.abs(r - R) - w / 2)
    over(img, rgb(track_col), track * track_a)
    over(img, ramp(s, stops), a)
    return img


@design("Patrone Waves")
def waves(c, mode):
    """Bold: layered waves of blue rolling across the lower screen."""
    S = 505
    dark = mode == "dark"
    if dark:
        img = vertical(c, "#020409", "#0c1c3e", ease=(-0.5, 0.1))
        cols = ["#2f8ff8", "#2372dd", "#1a5bbf", "#144499", "#0e3170", "#0a2050"]
        depth_a, tint, lit, lit_a = 0.45, "#000000", "#9ccaff", 0.5
    else:
        img = vertical(c, "#f7f9fd", "#e3eefb", ease=(-0.5, 0.1))
        cols = ["#d2e4fb", "#a8ccf9", "#76b0f7", "#4795f1", "#2a77dc", "#1a58b5"]
        depth_a, tint, lit, lit_a = 0.3, "#9fbde6", "#ffffff", 0.7
    img = c.full(img)
    u, v = c.uv()
    bases = [-0.07, 0.03, 0.12, 0.20, 0.28, 0.35]
    for i, (base, col) in enumerate(zip(bases, cols)):
        y = np.full_like(u, base)
        for k, (amp, f) in enumerate([(0.055, 1.6 - 0.12 * i), (0.02, 4.2 - 0.3 * i)]):
            y = y + amp * np.sin(f * u + 2 * math.pi * rand(S, i, k))
        y = y + 0.008 * fbm(u * 3, i * 2.3, S + i, 2)
        a, d = curve_cover(c, y, v)
        # The layer's shadow on the ones behind it, just above its crest
        shade(img, depth_a * np.exp(-np.maximum(d, 0) / 0.03) * (1 - a), tint)
        depth = np.exp(-np.maximum(v - y, 0) / 0.14)[..., None]
        fill = mix(rgb(col) * (0.8 if dark else 0.9), rgb(col), depth)
        over(img, fill, a)
        crest = np.clip(1 - np.abs(-d - 2.5 * c.px) / (1.5 * c.px), 0, 1)
        over(img, rgb(lit), lit_a * crest * a)
    return img


@design("Patrone Horizon")
def horizon(c, mode):
    """Calm: ridges fading into a lit horizon, under stars in the dark."""
    S = 707
    dark = mode == "dark"
    u4, v4 = c.uv(4)
    if dark:
        sky = vertical(c, "#02040a", "#12295a", ease=(-0.5, 0.05))
        glow, glow_a = "#2f7be8", 0.7
        cols = ["#1b3d7c", "#142f63", "#0e234b", "#091833", "#050e20"]
        mist = "#2c5fb0"
    else:
        sky = vertical(c, "#dfeafa", "#fbfcfe", ease=(-0.5, 0.05))
        glow, glow_a = "#ffffff", 0.7
        cols = ["#c3d8f4", "#9dbdea", "#779fdb", "#507fc8", "#2c5aa8"]
        mist = "#eef4fc"
    g = np.exp(-(((u4 + 0.3) / 0.6) ** 2 + ((v4 - 0.03) / 0.14) ** 2))
    over(sky, rgb(glow), glow_a * g)
    img = c.full(sky)
    u, v = c.uv()

    if dark:
        # Stars: at most one in each cell of a grid, brighter higher up
        cell = 0.018
        ci, cj = np.floor(u / cell), np.floor(v / cell)
        has = unit(ci, cj, S + 1) < 0.12
        sx = (ci + 0.15 + 0.7 * unit(ci, cj, S + 2)) * cell
        sy = (cj + 0.15 + 0.7 * unit(ci, cj, S + 3)) * cell
        size = 0.0004 + 0.0007 * unit(ci, cj, S + 4) ** 3
        star = cover(c, np.hypot(u - sx, v - sy) - size) * has
        star *= (0.35 + 0.65 * unit(ci, cj, S + 5)).astype(np.float32) * smoothstep(0.05, -0.25, v)
        over(img, rgb("#dcebff"), star.astype(np.float32))

    bases = [0.02, 0.10, 0.18, 0.27, 0.37]
    for i, (base, col) in enumerate(zip(bases, cols)):
        f = 2.2 - 0.3 * i
        x = u * f + 10 * i
        ridge = 0.0
        amp = 1.0
        for o in range(3):
            ridge = ridge + amp * (1 - np.abs(noise(x * 2**o, 3.7 * i + o, S + 10 * i + o))) ** 2
            amp *= 0.32
        y = base - (0.03 + 0.016 * i) * ridge
        a, d = curve_cover(c, y, v)
        haze = np.exp(-np.maximum(v - y, 0) / 0.05)[..., None] * (0.55 - 0.1 * i)
        over(img, mix(rgb(col), rgb(mist), haze), a)
    return img


@design("Patrone Slant")
def slant(c, mode):
    """Bold: panes of blue glass at the slant of the P's stem."""
    dark = mode == "dark"
    th = math.radians(62)  # the P's stem, from the horizontal
    sn, cs = math.sin(th), math.cos(th)
    if dark:
        img = vertical(c, "#050a18", "#02040a")
        u4, v4 = c.uv(4)
        over(img, rgb("#0f2350"), 0.8 * np.exp(-(((u4 - 0.45) / 0.7) ** 2 + ((v4 - 0.2) / 0.6) ** 2)))
        edge, edge_a = "#9fd0ff", 0.8
    else:
        img = vertical(c, "#f6f8fc", "#e8f0fa")
        edge, edge_a = "#ffffff", 0.9
    img = c.full(img)
    u, v = c.uv()
    t = u * sn + v * cs  # across the panes
    s = u * cs - v * sn  # along them, up and to the right
    # across from, to, colour in the dark and in the light, opacity
    panes = [
        (-0.42, -0.36, "#134394", "#a9cbf8", 0.6),
        (0.02, 0.27, "#1851a8", "#9cc6fa", 0.75),
        (0.20, 0.36, "#288bfa", "#4da0fc", 0.8),
        (0.40, 0.49, "#2372dd", "#2a83ef", 0.7),
        (0.53, 0.98, "#134394", "#1851a8", 0.65),
        (0.62, 0.70, "#4da0fc", "#78b6fb", 0.55),
    ]
    for t0, t1, cd, cl, alpha in panes:
        d = np.maximum(t0 - t, t - t1)
        a = cover(c, d)
        # Each pane brightens towards its top, like the P
        glass = rgb(cd if dark else cl) * (0.75 + 0.35 * smoothstep(-0.9, 0.9, s))[..., None]
        shadow = np.exp(-np.maximum(t - t1, 0) / 0.04) * (t > t1)
        shade(img, (0.35 if dark else 0.25) * shadow, "#000000" if dark else "#a9c4ea")
        over(img, glass, alpha * a)
        lit = np.clip(1 - np.abs(t - t0 - 2 * c.px) / (1.5 * c.px), 0, 1)
        over(img, rgb(edge), edge_a * alpha * lit * smoothstep(-0.8, 0.6, s))
    return img


@design("Patrone Facets")
def facets(c, mode):
    """Bold: low-poly facets of blue, lit from the top left."""
    S = 909
    dark = mode == "dark"
    cell = 0.09
    half_w = c.W / c.H / 2
    nx = int(math.ceil(2 * half_w / cell)) + 3
    ny = int(math.ceil(1 / cell)) + 3
    gi, gj = np.meshgrid(np.arange(nx), np.arange(ny))
    x = (gi - 1) * cell - half_w + (unit(gi, gj, S) - 0.5) * 0.75 * cell
    y = (gj - 1) * cell - 0.5 + (unit(gi, gj, S + 1) - 0.5) * 0.75 * cell
    z = 0.06 * fbm(x * 2, y * 2, S + 2, 2) + 0.025 * unit(gi, gj, S + 3)

    tris = []
    for j in range(ny - 1):
        for i in range(nx - 1):
            a, b, cc, d = (j, i), (j, i + 1), (j + 1, i), (j + 1, i + 1)
            if rand(S + 4, i, j) < 0.5:
                tris += [(a, b, d), (a, d, cc)]
            else:
                tris += [(a, b, cc), (b, d, cc)]
    P = np.array([[(x[p], y[p], z[p]) for p in t] for t in tris])  # (n, 3, 3)
    n = np.cross(P[:, 1] - P[:, 0], P[:, 2] - P[:, 0])
    n /= np.linalg.norm(n, axis=1, keepdims=True)
    n *= np.sign(n[:, 2:3])
    light = np.array([-0.5, -0.6, 0.62])
    light /= np.linalg.norm(light)
    lit = n @ light
    lit = (lit - lit.mean()) / (lit.std() + 1e-6)
    cu, cv = P[:, :, 0].mean(1), P[:, :, 1].mean(1)
    diag = smoothstep(-0.9, 0.9, cu * 0.75 + cv * 0.65)
    band = np.exp(-(((cu * 0.6 - cv * 0.8) - 0.05) / 0.3) ** 2)
    jitter = np.array([rand(S + 5, k) for k in range(len(tris))]) - 0.5
    if dark:
        col = mix(rgb("#050b1a"), rgb("#1851a8"), diag[:, None])
        col = mix(col, rgb("#288bfa"), 0.45 * band[:, None] * diag[:, None])
        col *= (1 + 0.1 * lit + 0.05 * jitter)[:, None]
    else:
        col = mix(rgb("#f6f9fd"), rgb("#7fb3f5"), diag[:, None])
        col = mix(col, rgb("#4f98f2"), 0.35 * band[:, None] * diag[:, None])
        col = 1 - (1 - col) * (1 - 0.18 * lit - 0.08 * jitter)[:, None]

    # Draw each triangle's number at twice the size, then average the colours
    k, shift = 2, 4
    ids = np.zeros((c.H * k, c.W * k), np.int32)
    to_px = c.H * k * (1 << shift)
    for idx, t in enumerate(P):
        pts = np.round(np.stack([(t[:, 0] + half_w) * to_px, (t[:, 1] + 0.5) * to_px], 1)).astype(np.int32)
        cv2.fillConvexPoly(ids, pts, int(idx), lineType=cv2.LINE_8, shift=shift)
    out = np.empty((c.H, c.W, 3), np.float32)
    for ch in range(3):
        out[..., ch] = col[:, ch].astype(np.float32)[ids].reshape(c.H, k, c.W, k).mean(axis=(1, 3))
    return out


@design("Patrone Contours")
def contours(c, mode):
    """Calm: contour lines of an imaginary landscape, fading in from a corner."""
    S = 1111
    dark = mode == "dark"
    u2, v2 = c.uv(2)
    f = c.full((fbm(u2 * 0.9, v2 * 0.9, S, 3) + 0.3 * u2 - 0.15 * v2).astype(np.float32))
    u, v = c.uv()
    gy, gx = np.gradient(f)
    grad = np.hypot(gx, gy) * c.H + 1e-6  # change per unit
    step = 0.03
    k = f / step
    d = np.abs(k - np.round(k)) * step / grad  # distance to the nearest line, units
    major = (np.round(k).astype(np.int64) % 5 == 0)
    width = np.where(major, 2.4, 1.4) * c.px
    spacing = step / grad / c.px  # pixels between lines
    a = cover(c, d - width / 2) * smoothstep(3, 9, spacing)
    reveal = smoothstep(-1.0, 0.6, u * 0.8 + v * 0.6)
    if dark:
        img = vertical(c, "#04070d", "#071226")
        lines = ramp(np.clip(f * 0.8 + 0.5, 0, 1), [(0, "#1851a8"), (0.5, "#288bfa"), (1, "#7ec0ff")])
        alpha = np.where(major, 0.85, 0.5) * reveal
    else:
        img = vertical(c, "#f7f9fd", "#eaf1fa")
        lines = ramp(np.clip(f * 0.8 + 0.5, 0, 1), [(0, "#6fa8ef"), (0.5, "#2a83ef"), (1, "#1851a8")])
        alpha = np.where(major, 0.7, 0.38) * reveal
    img = c.full(img)
    over(img, lines, (a * alpha).astype(np.float32))
    return img


@design("Patrone Halftone")
def halftone(c, mode):
    """A field of dots that swell towards one corner, like a print or a terminal."""
    S = 1313
    dark = mode == "dark"
    a = 0.017  # dot spacing
    u, v = c.uv()
    best_d = None
    best = None
    for ox, oy in [(0, 0), (0.5, 0.5)]:  # a hexagonal grid is two rectangular ones
        gx = np.round(u / a - ox) + ox
        gy = np.round(v / (a * math.sqrt(3)) - oy) + oy
        cxp, cyp = gx * a, gy * a * math.sqrt(3)
        dist = np.hypot(u - cxp, v - cyp)
        if best_d is None:
            best_d, best = dist, (cxp, cyp)
        else:
            closer = dist < best_d
            best = (np.where(closer, cxp, best[0]), np.where(closer, cyp, best[1]))
            best_d = np.minimum(dist, best_d)
    cxp, cyp = best
    m = smoothstep(-0.55, 0.95, cxp * 0.7 + cyp * 0.75 + 0.15 * fbm(cxp * 1.5, cyp * 1.5, S, 3))
    radius = a * 0.47 * m
    dots = cover(c, best_d - radius) * (radius > c.px * 0.6)
    t = np.clip(m, 0, 1)
    if dark:
        img = c.full(vertical(c, "#04070e", "#060c1a"))
        col = ramp(t, [(0, "#134394"), (0.5, "#2372dd"), (1, "#4da0fc")])
    else:
        img = c.full(vertical(c, "#f7f9fd", "#eef3fa"))
        col = ramp(t, [(0, "#a9cbf8"), (0.5, "#4795f1"), (1, "#1851a8")])
    over(img, col, dots.astype(np.float32))
    return img


@design("Patrone Orbs")
def orbs(c, mode):
    """Soft geometric shapes: overlapping discs of tinted glass."""
    dark = mode == "dark"
    if dark:
        img = c.full(vertical(c, "#060a16", "#03050a"))
        discs = [(0.40, 0.10, 0.31, "#1851a8", "#288bfa"), (0.05, 0.22, 0.2, "#4a3fd0", "#2a2f8a"),
                 (0.60, -0.17, 0.15, "#4da0fc", "#2372dd"), (-0.5, -0.2, 0.11, "#2372dd", "#134394"),
                 (0.2, -0.2, 0.08, "#9fd0ff", "#4da0fc")]
        depth_a, tint, rim = 0.5, "#000000", "#cfe6ff"
    else:
        img = c.full(vertical(c, "#f7f9fd", "#ebf1fa"))
        discs = [(0.40, 0.10, 0.31, "#a9cdfa", "#5c9ef2"), (0.05, 0.22, 0.2, "#c9c2ff", "#8f86ef"),
                 (0.60, -0.17, 0.15, "#7fb8fb", "#288bfa"), (-0.5, -0.2, 0.11, "#b6d4f8", "#6fa4ea"),
                 (0.2, -0.2, 0.08, "#e3efff", "#9cc6fa")]
        depth_a, tint, rim = 0.35, "#b4cbea", "#ffffff"
    u4, v4 = c.uv(4)
    u, v = c.uv()
    for x, y, r, top, bottom in discs:
        # A soft shadow, below and to the right
        sh = smoothstep(r + 0.06, r - 0.04, np.hypot(u4 - x - 0.015, v4 - y - 0.03))
        shade(img, depth_a * c.full(sh.astype(np.float32)), tint)
        d = np.hypot(u - x, v - y) - r
        a = cover(c, d)
        towards = ((u - x) * 0.5 + (v - y) * 0.87) / r  # -1 at the top left, 1 at the bottom right
        fill = mix(rgb(top), rgb(bottom), smoothstep(-1, 1, towards)[..., None])
        over(img, fill, 0.88 * a)
        edge = np.clip(1 - np.abs(d + 2 * c.px) / (1.5 * c.px), 0, 1)
        over(img, rgb(rim), 0.5 * edge * smoothstep(0.2, -0.9, towards))
    return img


def render(name, mode, W, H):
    return DESIGNS[name][1](Canvas(W, H), mode)


def file_name(name, mode):
    return f"patrone-{name}{'-light' if mode == 'light' else ''}.png"


def label_font(size):
    """PatroneOS Sans (fonts-patroneos) for the sheets' labels, or OpenCV's own."""
    path = "/usr/share/fonts/truetype/patroneos/PatroneOSSans-Regular.ttf"
    try:
        from PIL import ImageFont
        return ImageFont.truetype(path, size)
    except (ImportError, OSError):
        return None


def sheet(thumbs, path, cols, tile_w, label_h=44, gap=24, bg="#0e1117", fg="#e6edf7"):
    """Lay out [(label, BGR image)] in a grid with labels underneath."""
    from PIL import Image, ImageDraw
    rows = -(-len(thumbs) // cols)
    th = max(t.shape[0] * tile_w // t.shape[1] for _, t in thumbs)
    W = cols * tile_w + (cols + 1) * gap
    H = rows * (th + label_h) + (rows + 1) * gap
    out = Image.new("RGB", (W, H), bg)
    draw = ImageDraw.Draw(out)
    font = label_font(22)
    for k, (label, img) in enumerate(thumbs):
        r, q = divmod(k, cols)
        x = gap + q * (tile_w + gap)
        y = gap + r * (th + label_h + gap)
        h = img.shape[0] * tile_w // img.shape[1]
        small = cv2.resize(img, (tile_w, h), interpolation=cv2.INTER_AREA)
        out.paste(Image.fromarray(small[..., ::-1]), (x, y + (th - h) // 2))
        draw.text((x, y + th + 10), label, fill=fg, font=font)
    out.save(path, quality=90)
    print("wrote", path)


def zoom_crop(img, aspect):
    """What GNOME's "zoom" shows of img on a screen of this aspect ratio."""
    H, W = img.shape[:2]
    if aspect > W / H:
        h = round(W / aspect)
        y0 = (H - h) // 2
        return img[y0:y0 + h]
    w = round(H * aspect)
    x0 = (W - w) // 2
    return img[:, x0:x0 + w]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("out_dir")
    ap.add_argument("names", nargs="*")
    ap.add_argument("--size", nargs=2, type=int, default=(3840, 2160), metavar=("W", "H"))
    ap.add_argument("--sheet")
    ap.add_argument("--crops")
    args = ap.parse_args()
    W, H = args.size
    names = args.names or list(DESIGNS)
    for n in names:
        if n not in DESIGNS:
            ap.error(f"unknown design {n}; known: {', '.join(DESIGNS)}")

    os.makedirs(args.out_dir, exist_ok=True)
    thumbs, crops = [], []
    for n in names:
        title = DESIGNS[n][0]
        for mode in ("light", "dark"):
            out8 = dithered8(render(n, mode, W, H))[..., ::-1]  # RGB to BGR
            path = os.path.join(args.out_dir, file_name(n, mode))
            cv2.imwrite(path, out8, [cv2.IMWRITE_PNG_COMPRESSION, 9])
            print("wrote", path, f"{os.path.getsize(path) / 1e6:.1f} MB")
            small = cv2.resize(out8, (960, 540), interpolation=cv2.INTER_AREA)
            thumbs.append((f"{title}, {mode}", small))
            if mode == "dark":
                for label, aspect in (("16:10", 16 / 10), ("4:3", 4 / 3), ("21:9", 21 / 9)):
                    crops.append((f"{title}, {label}", zoom_crop(small, aspect)))
    if args.sheet:
        sheet(thumbs, args.sheet, cols=4, tile_w=480)
    if args.crops:
        sheet(crops, args.crops, cols=3, tile_w=420)


if __name__ == "__main__":
    main()
