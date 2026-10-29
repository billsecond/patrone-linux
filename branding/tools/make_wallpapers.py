"""Make the Neon Horizon wallpapers at 8K: wallpaper 02 and its family (light
and dark, with the full logo, a small one or none, and a few other colour
schemes), and wallpaper 03, light and dark.

usage: make_wallpapers.py OUT_DIR [--size W H] [--preview DIR] [NAME...]

Writes OUT_DIR/patrone-back-NAME.png for each variant (all of them unless
names are given), 7680x4320 unless --size says otherwise, and with
--preview, a 960x540 JPEG of each into DIR.

Every variant starts from the parts of its picture (wallpaper_layers.py):
its background, with the logo painted out, and its other graphics (the rule
of light under the logo, and 02's HUD details), each redrawn at full size.
The logo is drawn again from its vector shapes (patrone_logo.py), sharp at
any size. The colour schemes recolour the background and details; the logo
keeps its own colours except where a scheme says otherwise.
"""
import argparse
import os

import cv2
import numpy as np

import patrone_logo as pl
from wallpaper_layers import Parts, dithered8

HERE = os.path.dirname(os.path.abspath(__file__))
WALLPAPERS = os.path.join(HERE, "..", "wallpapers")

# The pictures, and where things are in them (pixels of the 1672x941
# originals). logo: the stacked logo's place, as pixels per wordmark unit and
# the top-left corner of its box (patrone_logo.stacked), measured from the
# wordmark. Boxes: round the logo; the rule of light under it, its brightest
# rows, and its glow; the HUD details.
PICTURES = {
    "02": dict(file="patrone-back-02.png", logo=(0.4335, 548.0, 306.5),
               logo_box=(530, 285, 1150, 552), rule=(480, 577, 1120, 581), glow=(470, 560, 1130, 598),
               details=[(1150, 128, 1600, 158), (1572, 0, 1672, 330), (0, 612, 96, 941),
                        (100, 740, 362, 757), (100, 757, 300, 768), (100, 768, 362, 832), (355, 818, 612, 829)]),
    "03": dict(file="patrone-back-03.png", logo=(0.436, 542.9, 286.0),
               logo_box=(525, 270, 1148, 535), rule=(620, 556, 1060, 560), glow=(610, 540, 1070, 580),
               details=[]),
}

# The logo's own glow: its colours blurred this many source pixels, at this
# strength, as the pictures' logos have
GLOW = ((2.5, 0.22), (9.0, 0.10))


def bgr(hexcolor):
    """'#rrggbb' as a float BGR triple."""
    v = int(hexcolor.lstrip("#"), 16)
    return np.array([v & 255, (v >> 8) & 255, v >> 16], np.float32) / 255.0


def radial(W, H, inner, outer, cx=0.5, cy=0.45, reach=0.75):
    """A radial gradient from inner at (cx, cy) to outer at reach x the width."""
    y, x = np.mgrid[0:H, 0:W].astype(np.float32)
    r = np.hypot((x - cx * W) / W, (y - cy * H) / W) / reach
    t = np.clip(r, 0, 1)[..., None] ** 1.5
    return bgr(inner) * (1 - t) + bgr(outer) * t


# Colour schemes: each takes the dark scene (background and details) and
# returns it recoloured.

def dark(img):
    return img


def light(img):
    """The same light, inverted: a pale field, the glows as blue ink.
    Lightness flips and the hue stays, so blue stays blue."""
    hls = cv2.cvtColor(np.clip(img, 0, 1), cv2.COLOR_BGR2HLS)
    L = hls[..., 1]
    hls[..., 1] = 0.965 - 0.82 * np.power(L, 0.8)
    # Near black, the hue is mostly noise: keep the paper neutral there.
    hls[..., 2] *= np.clip((L - 0.01) * 10, 0, 1)
    # Pull every hue most of the way to Patrone blue (212 degrees): the
    # glows' cyan-white cores would otherwise turn teal, and the noise pink.
    d = (hls[..., 0] - 212 + 180) % 360 - 180
    hls[..., 0] = (212 + 0.3 * d) % 360
    out = cv2.cvtColor(hls, cv2.COLOR_HLS2BGR)
    return out * bgr("#f3f6fb") / bgr("#f6f6f6")


def midnight(img):
    """Navy in place of black."""
    H, W = img.shape[:2]
    navy = radial(W, H, "#132a57", "#070e1f")
    return 1 - (1 - img) * (1 - navy)


def blue(img):
    """Patrone Blue, with the glows as white light."""
    H, W = img.shape[:2]
    base = radial(W, H, "#2a6fe0", "#0a2459", reach=0.8)
    t = np.clip((img.max(axis=2) - 0.015) * 1.6, 0, 1)[..., None]
    return base * (1 - t) + bgr("#d8ebff") * t


def graphite(img):
    """Grey, the glows as silver."""
    H, W = img.shape[:2]
    luma = (img @ np.array([0.114, 0.587, 0.299], np.float32))[..., None]
    base = radial(W, H, "#17191d", "#0b0c0e")
    return base + luma * 1.2 * bgr("#e8edf4")


# Each scheme's colouring, and its logo: the stacked logo's text white
# ("dark", for dark scenes) or navy ("light"), or all of it white.
SCHEMES = {
    "dark": (dark, "dark"),
    "light": (light, "light"),
    "midnight": (midnight, "dark"),
    "blue": (blue, "white"),
    "graphite": (graphite, "dark"),
}

# Logos: "full" (the stacked logo where the picture has it), "watermark", or
# (what, height as a fraction of the screen's, centre x, centre y as
# fractions). Fractions keep it clear of the edges a 16:10 screen crops.
FULL = "full"
MARK_SMALL = ("mark", 0.075, 0.5, 0.5)
BADGE = ("logo", 0.075, 0.83, 0.82)

# name: (picture, scheme, other graphics, logo). Settings lists them,
# pairing the light ones with their dark ones, in
# system/backgrounds/patrone-wallpapers.xml.
VARIANTS = {
    "02-dark": ("02", "dark", True, FULL),
    "02-light": ("02", "light", True, FULL),
    "02-plain": ("02", "dark", True, None),
    "02-plain-light": ("02", "light", True, None),
    "02-clean": ("02", "dark", False, None),
    "02-mark": ("02", "dark", True, MARK_SMALL),
    "02-mark-light": ("02", "light", True, MARK_SMALL),
    "02-badge": ("02", "dark", False, BADGE),
    "02-midnight": ("02", "midnight", True, MARK_SMALL),
    "02-blue": ("02", "blue", True, MARK_SMALL),
    "02-graphite": ("02", "graphite", True, MARK_SMALL),
    "02-watermark": ("02", "dark", False, "watermark"),
    "03-dark": ("03", "dark", False, FULL),
    "03-light": ("03", "light", False, FULL),
}


def logo_pieces(what, colour):
    """The stacked logo or the mark alone, coloured for a scheme."""
    if what == "mark":
        return pl.mark(mono="#FFFFFF" if colour == "white" else None)
    return pl.stacked(dark=colour != "light", mono="#FFFFFF" if colour == "white" else None)


def drawn(img, pieces, k, dx, dy, pad=2):
    """The pieces drawn at k pixels a unit, shifted by (dx, dy) pixels, in a
    canvas just big enough for them, pad pixels more all round, and within
    img: its corner, and the premultiplied BGR and alpha."""
    H, W = img.shape[:2]
    x0, y0, x1, y1 = pl.bounds(pieces)
    ax, ay = max(0, int(x0 * k + dx) - pad), max(0, int(y0 * k + dy) - pad)
    bx, by = min(W, int(x1 * k + dx) + pad), min(H, int(y1 * k + dy) + pad)
    color, alpha = pl.draw(pieces, bx - ax, by - ay, k, dx - ax, dy - ay)
    return ax, ay, color[..., ::-1].copy(), alpha


def paste(img, d, opacity=1.0):
    ax, ay, color, alpha = d
    dst = img[ay:ay + alpha.shape[0], ax:ax + alpha.shape[1]]
    dst[:] = dst * (1 - alpha[..., None] * opacity) + color * opacity


def render(parts, pic, name, W, H):
    picture, scheme, details, logo = VARIANTS[name]
    recolour, colour = SCHEMES[scheme]
    sx, sy = W / parts.w, H / parts.h
    clear = [parts.logo] + ([] if logo == FULL else [parts.glow])

    img = parts.background(W, H, clear=clear)
    if details:
        FG, A = parts.graphics(W, H, parts.details)
        A = A[..., None]
        img = img * (1 - A) + FG * A
        del FG, A
    d = None
    if logo == FULL:
        row, light = parts.rule_line(W, H)
        img[row:row + light.shape[0]] += light
        k, x, y = pic["logo"]
        d = drawn(img, logo_pieces("logo", colour), k * sx, x * sx, y * sy, pad=int(3 * GLOW[-1][0] * sx))
        # Its glow is light in the scene, so it's recoloured with it; on a
        # light scene light turns to ink, a grey smudge round the text there
        if colour != "light":
            ax, ay, color, alpha = d
            glow = sum(cv2.GaussianBlur(color, (0, 0), sigma * sx) * strength for sigma, strength in GLOW)
            img[ay:ay + alpha.shape[0], ax:ax + alpha.shape[1]] += glow
    img = recolour(img).astype(np.float32)

    if d:
        paste(img, d)
    elif logo == "watermark":
        # The P, taller than the screen and cut off by its bottom edge,
        # faintly lit from inside, with a fine rim of light.
        k = H * 1.05 / pl.MARK_SIZE[1]
        dx, dy = W * 0.62 - pl.MARK_SIZE[0] * k / 2, H * 0.6 - pl.MARK_SIZE[1] * k / 2
        paste(img, drawn(img, pl.mark(), k, dx, dy), opacity=0.09)
        ax, ay, _, a = drawn(img, pl.mark(mono="#FFFFFF"), k, dx, dy)
        rim = np.clip(a - cv2.erode(a, np.ones((5, 5), np.uint8)), 0, 1)
        rim = cv2.GaussianBlur(rim, (0, 0), max(1.0, W / 3840))[..., None] * 0.35
        dst = img[ay:ay + a.shape[0], ax:ax + a.shape[1]]
        dst[:] = dst * (1 - rim) + bgr("#5aa6ff") * rim
    elif logo:
        what, height, cx, cy = logo
        pieces = logo_pieces(what, colour)
        x0, y0, x1, y1 = pl.bounds(pieces)
        k = H * height / (y1 - y0)
        # Keep its place relative to the canvas's centre on any size.
        paste(img, drawn(img, pieces, k, W * cx - (x0 + x1) / 2 * k, H * cy - (y0 + y1) / 2 * k))

    return dithered8(img)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("out_dir")
    ap.add_argument("names", nargs="*")
    ap.add_argument("--size", nargs=2, type=int, default=(7680, 4320), metavar=("W", "H"))
    ap.add_argument("--preview")
    args = ap.parse_args()
    W, H = args.size
    names = args.names or list(VARIANTS)
    for n in names:
        if n not in VARIANTS:
            ap.error(f"unknown variant {n}; known: {', '.join(VARIANTS)}")

    os.makedirs(args.out_dir, exist_ok=True)
    if args.preview:
        os.makedirs(args.preview, exist_ok=True)
    parts = {}
    for n in names:
        picture = VARIANTS[n][0]
        pic = PICTURES[picture]
        if picture not in parts:
            img8 = cv2.imread(os.path.join(WALLPAPERS, pic["file"]), cv2.IMREAD_COLOR)
            parts[picture] = Parts(img8, pic["logo_box"], pic["rule"], pic["glow"], pic["details"])
        out8 = render(parts[picture], pic, n, W, H)
        path = os.path.join(args.out_dir, f"patrone-back-{n}.png")
        cv2.imwrite(path, out8, [cv2.IMWRITE_PNG_COMPRESSION, 9])
        print("wrote", path, f"{os.path.getsize(path) / 1e6:.1f} MB", flush=True)
        if args.preview:
            small = cv2.resize(out8, (960, 540), interpolation=cv2.INTER_AREA)
            cv2.imwrite(os.path.join(args.preview, f"patrone-back-{n}.jpg"), small,
                        [cv2.IMWRITE_JPEG_QUALITY, 88])


if __name__ == "__main__":
    main()
