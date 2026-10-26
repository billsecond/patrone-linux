"""Make the wallpaper 01 family: light and dark, with the full logo, a small
one or none, and a few other colour schemes.

usage: make_wallpapers.py SRC OUT_DIR [--size W H] [--preview DIR] [NAME...]

Writes OUT_DIR/patrone-back-01-NAME.png for each variant (all of them unless
names are given), 3840x2160 unless --size says otherwise, and with
--preview, a 960x540 JPEG of each into DIR. 4K holds all the detail the
1672-pixel original has, at about 3 MB a file with the ordered dither
(wallpaper_layers.dithered8).

Every variant starts from the same parts of the original (wallpaper_layers.py):
its background, its corner details and its logo, each redrawn at full size.
The colour schemes recolour the background and details; the logo keeps its
own colours except where a scheme says otherwise.
"""
import argparse
import os

import cv2
import numpy as np

from wallpaper_layers import Parts, dithered8


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
    """The same light, inverted: a pale field, the streaks as blue ink.
    Lightness flips and the hue stays, so blue stays blue."""
    hls = cv2.cvtColor(np.clip(img, 0, 1), cv2.COLOR_BGR2HLS)
    L = hls[..., 1]
    hls[..., 1] = 0.965 - 0.82 * np.power(L, 0.8)
    # Near black, the hue is mostly noise: keep the paper neutral there.
    hls[..., 2] *= np.clip((L - 0.01) * 10, 0, 1)
    # Pull every hue most of the way to Patrone blue (212 degrees): the
    # streaks' cyan-white cores would otherwise turn teal, and the noise pink.
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
    """Patrone Blue, with the streaks as white light."""
    H, W = img.shape[:2]
    base = radial(W, H, "#2a6fe0", "#0a2459", reach=0.8)
    t = np.clip((img.max(axis=2) - 0.015) * 1.6, 0, 1)[..., None]
    return base * (1 - t) + bgr("#d8ebff") * t


def graphite(img):
    """Grey, the streaks as silver."""
    H, W = img.shape[:2]
    luma = (img @ np.array([0.114, 0.587, 0.299], np.float32))[..., None]
    base = radial(W, H, "#17191d", "#0b0c0e")
    return base + luma * 1.2 * bgr("#e8edf4")


# How each scheme colours the logo: whites (the wordmark) and blues (the P,
# "LINUX" and its rules).

def whiteness(fg):
    return np.clip((fg.min(axis=2) - 0.55) / 0.3, 0, 1)[..., None]


def logo_dark(fg):
    return fg


def logo_light(fg):
    w = whiteness(fg)
    return fg * (1 - w) + bgr("#14223a") * w


def logo_white(fg):
    return np.broadcast_to(bgr("#ffffff"), fg.shape).copy()


SCHEMES = {
    "dark": (dark, logo_dark),
    "light": (light, logo_light),
    "midnight": (midnight, logo_dark),
    "blue": (blue, logo_white),
    "graphite": (graphite, logo_dark),
}

# Logos: None, "full" (where the original has it), or (region, size relative
# to the original logo, centre x, centre y as fractions of the canvas).
# Fractions keep the logo clear of the edges a 16:10 screen crops.
MARK_SMALL = ("mark", 0.42, 0.5, 0.5)
BADGE = ("logo", 0.3, 0.83, 0.82)

# name: (scheme, corner details, logo). Settings lists them, pairing the
# light ones with their dark ones, in system/backgrounds/patrone-wallpapers.xml.
VARIANTS = {
    "light": ("light", True, "full"),
    "plain": ("dark", True, None),
    "plain-light": ("light", True, None),
    "clean": ("dark", False, None),
    "mark": ("dark", True, MARK_SMALL),
    "mark-light": ("light", True, MARK_SMALL),
    "badge": ("dark", False, BADGE),
    "midnight": ("midnight", True, MARK_SMALL),
    "blue": ("blue", True, MARK_SMALL),
    "graphite": ("graphite", True, MARK_SMALL),
    "watermark": ("dark", False, "watermark"),
}


def paste(img, fg, a, cx, cy, opacity=1.0):
    """Composite fg with coverage a, centred at (cx, cy) in pixels, clipped to img."""
    H, W = img.shape[:2]
    h, w = a.shape
    x0, y0 = round(cx - w / 2), round(cy - h / 2)
    sx0, sy0 = max(0, -x0), max(0, -y0)
    dx0, dy0 = max(0, x0), max(0, y0)
    dx1, dy1 = min(W, x0 + w), min(H, y0 + h)
    if dx1 <= dx0 or dy1 <= dy0:
        return
    sub_a = a[sy0:sy0 + dy1 - dy0, sx0:sx0 + dx1 - dx0][..., None] * opacity
    sub_fg = fg[sy0:sy0 + dy1 - dy0, sx0:sx0 + dx1 - dx0]
    dst = img[dy0:dy1, dx0:dx1]
    dst[:] = dst * (1 - sub_a) + sub_fg * sub_a


def render(parts, name, W, H):
    scheme, details, logo = VARIANTS[name]
    recolour, logo_colour = SCHEMES[scheme]
    s = W / parts.w  # the original's scale at this size

    img = parts.background(W, H, clear=logo != "full")
    if details:
        # The bottom-left details' box overlaps the logo's: leave the logo's
        # rule to the logo.
        FG, A = parts.graphics(W, H, parts.details, exclude=[parts.logo])
        A = A[..., None]
        img = img * (1 - A) + FG * A
        del FG, A
    img = recolour(img).astype(np.float32)

    if logo == "full":
        FG, A = parts.graphics(W, H, [parts.logo])
        A = A[..., None]
        img = img * (1 - A) + logo_colour(FG) * A
    elif logo == "watermark":
        # The P, far larger than the screen's height and cut off by its
        # bottom edge, faintly lit from inside, with a fine rim of light.
        fg, a = parts.graphic(parts.mark, s * 4.6)
        rim = np.clip(a - cv2.erode(a, np.ones((5, 5), np.uint8)), 0, 1)
        rim = cv2.GaussianBlur(rim, (0, 0), max(1.0, W / 3840))
        paste(img, fg, a, W * 0.62, H * 0.6, opacity=0.09)
        paste(img, np.broadcast_to(bgr("#5aa6ff"), fg.shape), rim, W * 0.62, H * 0.6, opacity=0.35)
    elif logo:
        region, size, cx, cy = logo
        box = parts.mark if region == "mark" else parts.logo
        fg, a = parts.graphic(box, s * size)
        # Keep its place relative to the canvas's centre on any size.
        paste(img, logo_colour(fg), a, W * cx, H * cy)

    return dithered8(img)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("src")
    ap.add_argument("out_dir")
    ap.add_argument("names", nargs="*")
    ap.add_argument("--size", nargs=2, type=int, default=(3840, 2160), metavar=("W", "H"))
    ap.add_argument("--preview")
    args = ap.parse_args()
    W, H = args.size
    names = args.names or list(VARIANTS)
    for n in names:
        if n not in VARIANTS:
            ap.error(f"unknown variant {n}; known: {', '.join(VARIANTS)}")

    parts = Parts(cv2.imread(args.src, cv2.IMREAD_COLOR))
    os.makedirs(args.out_dir, exist_ok=True)
    if args.preview:
        os.makedirs(args.preview, exist_ok=True)
    for n in names:
        out8 = render(parts, n, W, H)
        path = os.path.join(args.out_dir, f"patrone-back-01-{n}.png")
        cv2.imwrite(path, out8, [cv2.IMWRITE_PNG_COMPRESSION, 9])
        print("wrote", path, f"{os.path.getsize(path) / 1e6:.1f} MB")
        if args.preview:
            small = cv2.resize(out8, (960, 540), interpolation=cv2.INTER_AREA)
            cv2.imwrite(os.path.join(args.preview, f"patrone-back-01-{n}.jpg"), small,
                        [cv2.IMWRITE_JPEG_QUALITY, 88])


if __name__ == "__main__":
    main()
