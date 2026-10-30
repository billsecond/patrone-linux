"""Draws the abstract wallpapers: rich, lit forms rather than flat shapes.

usage: make_abstract_wallpapers.py OUT_DIR [--size W H] [NAME...]

Writes OUT_DIR/patrone-NAME.jpg, 3840x2160 unless --size says otherwise (all
of them unless names are given). Each is a height field or a 3D scene shaded
with light, drawn from Patrone's colors by code alone: no photos, downloaded
art or image models. They're saved as make_gallery_wallpapers.py saves its
pictures: fine grain in the flat darks, then a JPEG.

Every "random" choice is a hash, as in make_generated_wallpapers.py, so the
same design comes out the same.
"""
import argparse
import math
import os

import cv2
import numpy as np

from make_gallery_wallpapers import grain, grained, save
from make_generated_wallpapers import Canvas, fbm, mix, ramp, rgb, smoothstep, unit

DESIGNS = {}


def design(fn):
    DESIGNS[fn.__name__] = fn
    return fn


def normals(h, strength, px):
    """Unit normals of a height field, h in units per pixel size px."""
    dy, dx = np.gradient(h)
    nx, ny = -dx / px * strength, -dy / px * strength
    nz = 1 / np.sqrt(1 + nx * nx + ny * ny)
    return nx * nz, ny * nz, nz


def unit3(x, y, z):
    n = math.sqrt(x * x + y * y + z * z)
    return x / n, y / n, z / n


def lit(n, light, power):
    """Diffuse and specular light on normals n, seen from straight ahead."""
    lx, ly, lz = light
    hx, hy, hz = unit3(lx, ly, lz + 1)
    diffuse = np.clip(n[0] * lx + n[1] * ly + n[2] * lz, 0, 1)
    specular = np.clip(n[0] * hx + n[1] * hy + n[2] * hz, 0, 1) ** power
    return diffuse, specular


def warped(x, y, seed, octaves=4, amount=3.0):
    """Noise pulled through two layers of noise, which makes it flow."""
    qx, qy = fbm(x, y, seed, octaves), fbm(x + 5.2, y + 1.3, seed + 1, octaves)
    rx = fbm(x + amount * qx + 1.7, y + amount * qy + 9.2, seed + 2, octaves)
    ry = fbm(x + amount * qx + 8.3, y + amount * qy + 2.8, seed + 3, octaves)
    return fbm(x + amount * rx, y + amount * ry, seed + 4, octaves), qx, rx


def vignette(u, v, a, b):
    return smoothstep(a, b, np.hypot(u / 1.3, v / 0.85))


@design
def silk(c):
    """Blue satin in long folds, lit from the top left, a cool light behind."""
    step = 2 if c.W > 2000 else 1
    u, v = c.uv(step)
    a = math.radians(-24)
    x, y = u * math.cos(a) - v * math.sin(a), u * math.sin(a) + v * math.cos(a)
    bend = fbm(u * 0.6 + 3.1, v * 0.6 - 2.2, 301, 2)
    # noise stretched along the cloth hangs in long folds, and steepening it
    # makes them crease
    folds = fbm(x * 0.55 + bend * 0.35, y * 2.4 + bend * 1.2, 304, 3, 2.0, 0.42)
    h = 0.13 * np.tanh(folds * 3.2)
    n = normals(cv2.GaussianBlur(h, (0, 0), 1.2 * c.W / 3840 * 2 / step), 1.0, c.px * step)
    key, shine = lit(n, unit3(-0.55, -0.6, 0.58), 28)
    back, glint = lit(n, unit3(0.7, 0.35, 0.6), 12)
    shade = np.clip(0.84 * key ** 1.9 + 0.2 * back ** 2.5, 0, 1)
    img = ramp(shade, [(0, "#030813"), (0.22, "#0b2a63"), (0.45, "#1851a8"), (0.68, "#288bfa"),
                       (0.86, "#6fb4ff"), (1, "#d8ecff")])
    img = img + shine[..., None] * rgb("#cfe6ff") * 0.42 + glint[..., None] * rgb("#6a4dff") * 0.22
    return c.full(img * (1 - 0.5 * vignette(u, v, 0.6, 1.25))[..., None])


@design
def liquid(c):
    """Liquid metal in slow, heavy swells, reflecting a studio's soft lights."""
    step = 2 if c.W > 2000 else 1
    u, v = c.uv(step)
    h, _, _ = warped(u * 0.62 + 2.3, v * 0.62 - 1.1, 311, 2, 1.9)
    h = cv2.GaussianBlur(0.26 * h, (0, 0), 3 * c.W / 1920 / step)
    nx, ny, nz = normals(h, 1.0, c.px * step)
    rx_, ry_ = 2 * nz * nx, 2 * nz * ny
    t = np.clip(0.5 - 0.5 * ry_, 0, 1)
    env = ramp(t, [(0, "#01040c"), (0.28, "#07183e"), (0.45, "#134394"), (0.58, "#2372dd"), (0.7, "#288bfa"),
                   (0.8, "#8cc8ff"), (0.86, "#f4faff"), (0.9, "#7db8ff"), (1, "#0f3478")])
    box = np.exp(-(((rx_ + 0.3) / 0.3) ** 2 + ((ry_ + 0.55) / 0.12) ** 2))
    violet = np.exp(-(((rx_ - 0.55) / 0.45) ** 2 + ((ry_ - 0.3) / 0.35) ** 2))
    img = env + box[..., None] * rgb("#ffffff") * 0.7 + violet[..., None] * rgb("#5a3df0") * 0.5
    return c.full(img * (1 - 0.5 * vignette(u, v, 0.6, 1.25))[..., None])


def stars(c, count, seed):
    """Points of light of many sizes, as a picture of c's size."""
    W, H = c.W, c.H
    i = np.arange(count)
    x = (unit(i, 0, seed) * W).astype(np.int32)
    y = (unit(i, 1, seed) * H).astype(np.int32)
    b = unit(i, 2, seed).astype(np.float32) ** 6
    field = np.zeros((H, W), np.float32)
    np.add.at(field, (y, x), b)
    s = W / 3840
    return cv2.GaussianBlur(field, (0, 0), 0.9 * s) * 9 * s * s + cv2.GaussianBlur(field, (0, 0), 3.0 * s) * 16 * s * s


@design
def nebula(c):
    """Soft clouds of lit gas, blue and violet, with stars behind."""
    step = 2 if c.W > 2000 else 1
    u, v = c.uv(step)
    x, y = u * 0.95 + 4.1, v * 0.95 + 2.7
    gas, qx, rx = warped(x, y, 321, 5, 1.3)
    near, _, _ = warped(x + 0.03, y + 0.025, 321, 5, 1.3)
    wisp, _, _ = warped(x * 1.7 + 2.0, y * 1.7 - 1.0, 331, 5, 1.0)
    core = np.exp(-(((u - 0.25) / 0.85) ** 2 + ((v + 0.05) / 0.5) ** 2))
    cloud = np.clip(0.5 + 0.75 * gas, 0, 1) ** 2.2 * (0.35 + 0.9 * core)
    relief = np.clip((near - gas) * 7, -1, 1)
    glow = cloud * (1 + 0.5 * relief) + 0.5 * np.clip(0.5 + 0.8 * wisp, 0, 1) ** 3 * (0.2 + 0.8 * core)
    img = ramp(np.clip(glow * 1.25, 0, 1), [(0, "#02040c"), (0.2, "#07174a"), (0.45, "#1851a8"), (0.68, "#288bfa"),
                                            (0.85, "#93bcff"), (1, "#f5f8ff")])
    violet = smoothstep(-0.1, 0.45, qx)[..., None] * 0.42
    img = mix(img, img * rgb("#c466ff") * 1.5, violet)
    dust = smoothstep(0.1, 0.55, fbm(x * 1.3 + rx, y * 1.3 - qx, 341, 4))[..., None]
    img = c.full(img * (1 - 0.6 * dust))
    sky = (1 - np.clip(c.full(np.clip(glow, 0, 1) * 1.6), 0, 1)) ** 2
    return img + (stars(c, 5200, 351) * sky)[..., None] * rgb("#dfe9ff")


@design
def shallows(c):
    """Sunlight through rippled water, playing on a seabed that falls away."""
    W, H = c.W, c.H
    # the light on the seabed, seen from straight above: each ray of an even
    # grid is bent by the water's slope, and they gather where they cross
    u, v = c.uv(1)
    x = np.broadcast_to(u, (H, W)).astype(np.float32)
    y = np.broadcast_to(v, (H, W)).astype(np.float32)
    water = fbm(x * 8.0 + 1.3, y * 8.0 - 4.2, 361, 2, 2.0, 0.4) + 0.5 * fbm(x * 3.0, y * 3.0, 362, 2)
    gy, gx = np.gradient(cv2.GaussianBlur(water, (0, 0), 1.5 * W / 3840))
    rms = float(np.sqrt((gx * gx + gy * gy).mean()))
    reach = 0.2 * H / 8.0 / rms
    light = []
    for k in (0.96, 1.0, 1.04):     # each color bends a little differently
        px = np.clip(np.arange(W, dtype=np.float32)[None, :] + gx * reach * k, 0, W - 1).astype(np.int32)
        py = np.clip(np.arange(H, dtype=np.float32)[:, None] + gy * reach * k, 0, H - 1).astype(np.int32)
        count = np.bincount((py * W + px).ravel(), minlength=W * H).reshape(H, W).astype(np.float32)
        light.append(cv2.GaussianBlur(count, (0, 0), 1.8 * W / 1920))
    light = np.stack(light, axis=-1)
    # the seabed in perspective: nearer at the bottom, misty far away
    depth = 1.0 / np.clip(v + 0.95, 0.12, None)
    gx_ = (np.arange(W, dtype=np.float32)[None, :] - W / 2) * (depth * 0.3) + W / 2
    gy_ = np.broadcast_to((1 - (depth - depth.min()) / (depth.max() - depth.min())) * (H - 1), (H, W))
    map_x = np.float32(np.mod(gx_, W - 1))
    map_y = np.float32(gy_)
    seen = cv2.remap(light, map_x, map_y, cv2.INTER_LINEAR)
    far = smoothstep(0.0, 1.0, (0.5 - v) / 1.0)[..., None]
    soft = cv2.GaussianBlur(seen, (0, 0), 7 * W / 1920)
    seabed = mix(rgb("#06315f"), rgb("#1a78c4"), far) * (0.5 + 0.5 * np.clip(soft, 0, 2))
    bright = np.clip(seen - 1.0, 0, None) ** 0.9
    tint = bright * mix(np.array([0.3, 0.7, 1.0], np.float32), np.array([0.6, 0.9, 1.0], np.float32), far)
    img = seabed + 0.34 * tint * (1 - 0.6 * far)
    return mix(img, rgb("#3aa0ea"), far ** 3 * 0.75)


@design
def ink(c):
    """Ink poured into still water, drawn out by slow currents.

    Three colors of ink are carried along a swirling flow, step by step, in
    a fixed 3840x2160 tank, so the design is the same at any size. Each
    step interpolates linearly: cubic steps leave ripples along the edges."""
    W, H = 3840, 2160
    t = Canvas(W, H)
    u, v = t.uv(1)
    x = np.broadcast_to(u, (H, W)).astype(np.float32)
    y = np.broadcast_to(v, (H, W)).astype(np.float32)

    def flow(seed):
        psi = fbm(x * 1.5 + 2.1, y * 1.5 - 3.3, seed, 4, 2.0, 0.55)
        gy, gx = np.gradient(psi)
        return gy, -gx

    a, b = flow(391), flow(392)
    gx, gy = np.meshgrid(np.arange(W, dtype=np.float32), np.arange(H, dtype=np.float32))
    # one long band of ink across the tank, its color changing along it
    band = np.exp(-(((y + 0.32 * x - 0.1 * np.sin(x * 3.1)) / 0.15) ** 2))
    band = band + 0.7 * np.exp(-(((y + 0.2 * x + 0.34 + 0.08 * np.sin(x * 4.3 + 1)) / 0.09) ** 2))
    ink_ = [band * (0.5 + 0.5 * np.cos(2 * math.pi * (x / 1.5 + k / 3))) ** 2 for k in range(3)]
    steps = 170
    for s in range(steps):
        f = s / steps
        vx, vy = a[0] * (1 - f) + b[0] * f, a[1] * (1 - f) + b[1] * f
        map_x = gx - vx * 3040
        map_y = gy - vy * 3040
        ink_ = [cv2.remap(d, map_x, map_y, cv2.INTER_LINEAR, borderMode=cv2.BORDER_REPLICATE) for d in ink_]
    total = np.clip(sum(ink_), 0, None)
    dy_, dx_ = np.gradient(cv2.GaussianBlur(total, (0, 0), 2.4))
    relief = np.clip((-dx_ * 0.55 - dy_ * 0.7) * 52, -1, 1)
    colors = [rgb("#288bfa"), rgb("#19c3e6"), rgb("#6a43f0")]
    light = sum(d[..., None] * col for d, col in zip(ink_, colors))
    img = rgb("#040a18") + (1 - np.exp(-1.5 * light)) * (1 + 0.45 * relief[..., None])
    edges = np.exp(-3 * total)[..., None]
    deep = ramp(np.clip(0.55 + 0.9 * v, 0, 1), [(0, "#071633"), (1, "#02050e")])
    img = img * (1 - edges) + deep * edges
    return cv2.resize(np.clip(img, 0, 1), (c.W, c.H), interpolation=cv2.INTER_CUBIC)


@design
def marble(c):
    """Polished dark stone with veins of gold running through it."""
    step = 1
    u, v = c.uv(step)
    x, y = u * 1.1 - 1.3, v * 1.1 + 2.1
    d, qx, rx = warped(x, y, 381, 5, 2.4)
    clouds = 0.5 + 0.5 * fbm(x * 1.4 + 2 * qx, y * 1.4 + 2 * rx, 386, 5)
    img = ramp(np.clip(clouds, 0, 1), [(0, "#030712"), (0.45, "#081a3d"), (0.7, "#10306f"), (1, "#1c4fa3")])
    strength = smoothstep(-0.25, 0.35, fbm(x * 1.2, y * 1.2, 389, 3))
    # veins thin out rather than fade, so they stay gold
    wide = 1 - smoothstep(0.004 + 0.03 * strength, 0.008 + 0.05 * strength, np.abs(d))
    second = fbm(x * 2.1 + 3 * rx, y * 2.1 + 3 * qx, 388, 5)
    thin = 1 - smoothstep(0.002 + 0.006 * strength, 0.005 + 0.01 * strength, np.abs(second))
    glow = cv2.GaussianBlur(wide, (0, 0), 9 * c.W / 3840 * 2 / step)
    gold = ramp(np.clip(clouds * 1.5, 0, 1), [(0, "#a8701c"), (0.5, "#e0ab3e"), (1, "#ffe9a8")])
    vein = np.clip(wide + 0.75 * thin, 0, 1)[..., None]
    img = img * (1 - vein) + gold * vein + glow[..., None] * rgb("#b98327") * 0.16 * strength[..., None]
    sheen = np.exp(-(((u + 0.45 - v * 0.6) / 0.42) ** 2))
    img = img + sheen[..., None] * rgb("#7fa8e8") * 0.07
    return c.full(img * (1 - 0.5 * vignette(u, v, 0.6, 1.25))[..., None])


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("out_dir")
    ap.add_argument("names", nargs="*")
    ap.add_argument("--size", nargs=2, type=int, default=(3840, 2160), metavar=("W", "H"))
    args = ap.parse_args()
    W, H = args.size
    for name in args.names:
        if name not in DESIGNS:
            ap.error(f"unknown design {name}; known: {', '.join(DESIGNS)}")
    c = Canvas(W, H)
    noise = grain(W, H)
    for name in args.names or DESIGNS:
        img = DESIGNS[name](c).astype(np.float32)
        path = os.path.join(args.out_dir, f"patrone-{name}.jpg")
        save(path, grained(np.clip(img, 0, 1)[..., ::-1], noise))
        print(path, f"{os.path.getsize(path) / 1e6:.1f} MB", flush=True)


if __name__ == "__main__":
    main()
