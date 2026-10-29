"""A wallpaper picture taken apart, so it can be redrawn at any size and in
other colours.

The Neon Horizon pictures (wallpapers/patrone-back-02.png and -03.png) are
each one flat 1672x941 image: a dark scene of glows, dunes and water, with
crisp graphics over it (the logo, the rule of light under it, and in 02 the
HUD details in two corners). Parts() separates them at the source's size:

  - the background, with the graphics painted out;
  - the graphics, as a colour field and a coverage (alpha) mask.

How:
  1. Denoise the source to remove compression blocking.
  2. Find the crisp elements inside the boxes given and inpaint them out to
     get a clean background.
  3. Measure the elements' coverage, and spread their interior colours
     outward so edge pixels get the element's colour, not a mix.

Upscaling the two separately keeps the glows smooth and the graphics sharp:
the background with Lanczos, the coverage with cubic and its edges
re-sharpened. The logo itself isn't kept: make_wallpapers.py paints it out
and draws it again from its vector shapes (patrone_logo.py).
"""
import cv2
import numpy as np

# Edge sharpening, tuned at 8K (source edges, ~1.5 px, end up ~2 px wide):
# the coverage is steepened by K8K when the source is enlarged to 7680 wide.
K8K = 3.5


class Parts:
    def __init__(self, img8, logo, rule, glow, details):
        """img8: the picture (BGR, 8-bit). Boxes (x0, y0, x1, y1) in its
        pixels: logo, round the logo; rule, round the rule of light under it,
        just its brightest rows, and glow, round its glow; details, a list
        round the other graphics to keep."""
        h, w = img8.shape[:2]
        self.w, self.h = w, h
        self.logo, self.rule, self.glow, self.details = logo, rule, glow, details

        # 1. Denoise. All colour work happens in float: smoothing in 8-bit
        # flattens the dark gradients into visible bands. The 8-bit denoised
        # copy is only used to find features.
        den8 = cv2.fastNlMeansDenoisingColored(img8, None, 3, 3, 7, 21)
        src = img8.astype(np.float32) / 255.0
        # Edge-preserving smoothing removes blocks and grain but keeps strong edges.
        den = cv2.bilateralFilter(src, 9, 0.05, 2.5)

        # The rule: a line two pixels thick in its glow. Its rows become the
        # glow alone, a blend of the rows above and below, and the light it
        # added is kept, a column at a time, to draw it again sharp.
        x0, y0, x1, y1 = rule
        above, below = den[y0 - 1, x0:x1], den[y1, x0:x1]
        light = np.zeros((w, 3), np.float32)
        for y in range(y0, y1):
            t = (y - y0 + 1) / (y1 - y0 + 1)
            glow_only = above * (1 - t) + below * t
            light[x0:x1] += np.clip(den[y, x0:x1] - glow_only, 0, None)
            den[y, x0:x1] = glow_only
        self.rule_light = cv2.GaussianBlur(light[None], (0, 0), 1.0)[0]
        maxch = den.max(axis=2)

        roi = self._mask([logo] + details).astype(np.float32)
        logo_roi = self._mask([logo])

        # 2. Candidate foreground: thin bright features above the local median,
        # plus the large logo shapes (bright against a near-black background).
        med = cv2.medianBlur(den8, 31).astype(np.float32).max(axis=2) / 255.0
        self._med = med
        thin = (maxch - med) > 0.05
        big = logo_roi & (maxch > 0.18)
        cand = ((thin | big) & (roi > 0)).astype(np.uint8)
        mask0 = cv2.dilate(cand, cv2.getStructuringElement(cv2.MORPH_ELLIPSE, (7, 7)))

        # Inpaint works in 8-bit, so only take its values under the masked
        # graphics, then smooth them so the filled area has no steps.
        painted = cv2.inpaint(den8, mask0 * 255, 7, cv2.INPAINT_TELEA).astype(np.float32) / 255.0
        under = mask0.astype(bool)
        bg = den.copy()
        bg[under] = painted[under]
        soft = cv2.GaussianBlur(mask0.astype(np.float32), (0, 0), 4.0)[..., None]
        bg = bg * (1 - soft) + cv2.GaussianBlur(bg, (0, 0), 4.0) * soft
        # The background is only glows and gradients, so smooth it a little
        # more than the graphics to clear the remaining blocks in the glows.
        self.bg = cv2.bilateralFilter(bg, 9, 0.08, 3.0)
        self._maxch = maxch

        # 3. Coverage: brightness above background, relative to the local peak.
        d = np.clip(maxch - self.bg.max(axis=2), 0, None) * roi * mask0
        peak = cv2.dilate(d, cv2.getStructuringElement(cv2.MORPH_ELLIPSE, (7, 7)))
        alpha = np.where(peak > 0.04, np.clip(d / np.maximum(peak, 1e-4), 0, 1), 0).astype(np.float32)
        # Pixels whose whole 3x3 neighbourhood is bright are interior, even
        # across the P's internal facet edges where brightness changes sharply.
        interior = cv2.erode(d, np.ones((3, 3), np.uint8)) > 0.5 * peak
        alpha[interior & (peak > 0.04)] = 1.0
        self.alpha = alpha

        # Foreground colour field: spread confident interior colours outward
        # so edge pixels get the element's colour rather than a mix with the
        # background.
        wgt = (alpha > 0.85).astype(np.float32)
        fg = den.copy()
        filled = np.zeros((h, w), bool)
        for sigma in (1.2, 3.0, 8.0):
            num = cv2.GaussianBlur(den * wgt[..., None], (0, 0), sigma)
            dw = cv2.GaussianBlur(wgt, (0, 0), sigma)
            ok = (dw > 1e-3) & ~filled
            fg[ok] = num[ok] / dw[ok][:, None]
            filled |= ok
        self.fg = fg
        self._cleared = {}

    def _mask(self, boxes):
        m = np.zeros((self.h, self.w), bool)
        for (x0, y0, x1, y1) in boxes:
            m[y0:y1, x0:x1] = True
        return m

    def cleared(self, boxes):
        """The source's background with everything in boxes that stands out
        at all refilled smoothly from the edge of what's left (_push_pull):
        inpainting alone leaves a faint outline of the logo, and misses the
        dimmest of its glow."""
        key = tuple(boxes)
        if key not in self._cleared:
            faint = ((self._maxch - self._med) > 0.015) | (self.alpha > 0.01)
            hole = cv2.dilate((faint & self._mask(boxes)).astype(np.uint8),
                              cv2.getStructuringElement(cv2.MORPH_ELLIPSE, (25, 25))).astype(np.float32)
            fill = cv2.GaussianBlur(_push_pull(self.bg, 1 - hole), (0, 0), 6.0)
            soft = cv2.GaussianBlur(hole, (0, 0), 3.0)[..., None]
            self._cleared[key] = self.bg * (1 - soft) + fill * soft
        return self._cleared[key]

    def background(self, W, H, clear=()):
        """The background alone, W x H, with the graphics in the boxes in
        clear wiped away entirely (see cleared())."""
        bg = self.cleared(list(clear)) if clear else self.bg
        BG = cv2.resize(bg, (W, H), interpolation=cv2.INTER_LANCZOS4)
        # A little extra definition on the glows' edges.
        blur = cv2.GaussianBlur(BG, (0, 0), 3.0 * W / 7680)
        return BG + 0.35 * (BG - blur)

    def rule_line(self, W, H, thickness=1.6):
        """The rule's light, W x H, as a sharp line thickness source pixels
        thick: the first row it touches, and the light to add from there."""
        x0, y0, x1, y1 = self.rule
        sx, sy = W / self.w, H / self.h
        top, bottom = ((y0 + y1) / 2 - thickness / 2) * sy, ((y0 + y1) / 2 + thickness / 2) * sy
        rows = np.arange(int(top), int(np.ceil(bottom)))
        cover = np.clip(np.minimum(rows + 1, bottom) - np.maximum(rows, top), 0, 1).astype(np.float32)
        xs = (np.arange(W) + 0.5) / sx - 0.5
        per_px = np.stack([np.interp(xs, np.arange(self.w), self.rule_light[:, c]) for c in range(3)], -1)
        return rows[0], cover[:, None, None] * (per_px / thickness)[None].astype(np.float32)

    def graphics(self, W, H, regions):
        """Colour and coverage, W x H, of the graphics in the boxes in regions."""
        a = np.zeros_like(self.alpha)
        for (x0, y0, x1, y1) in regions:
            a[y0:y1, x0:x1] = self.alpha[y0:y1, x0:x1]
        FG = cv2.resize(self.fg, (W, H), interpolation=cv2.INTER_CUBIC)
        A = cv2.resize(a, (W, H), interpolation=cv2.INTER_CUBIC)
        # Edges 2 px wide at 8K, 1 px at 4K, and as sharp as the source allows below.
        return FG, _sharpen(A, K8K * min(1.0, 2 * W / 7680))


def _push_pull(img, w):
    """img with its pixels of weight w below 1 filled in from the rest, each
    from the nearest: averaged down a pyramid of halvings until every hole is
    covered, then carried back up, so the fill meets its edges smoothly."""
    h, wd = w.shape
    if min(h, wd) <= 2:
        total = max(float(w.sum()), 1e-6)
        mean = (img * w[..., None]).sum(axis=(0, 1)) / total
        return np.broadcast_to(mean, img.shape).astype(np.float32)
    half = ((wd + 1) // 2, (h + 1) // 2)
    small_w = cv2.resize(w, half, interpolation=cv2.INTER_AREA)
    small = cv2.resize(img * w[..., None], half, interpolation=cv2.INTER_AREA) / \
        np.maximum(small_w, 1e-6)[..., None]
    coarse = _push_pull(small, np.clip(small_w * 4, 0, 1))
    up = cv2.resize(coarse, (wd, h), interpolation=cv2.INTER_LINEAR)
    return img * w[..., None] + up * (1 - w[..., None])


def _sharpen(A, k):
    """Steepen upscaled coverage edges by k."""
    return np.clip((A - 0.5) * k + 0.5, 0, 1)


def dithered8(img):
    """To 8 bits with an 8x8 ordered dither: no banding, and unlike grain it
    adds no texture of its own and compresses about three times better in
    PNG, while keeping the gradients' averages closer to the originals."""
    b = np.zeros((1, 1))
    for _ in range(3):
        b = np.block([[4 * b, 4 * b + 2], [4 * b + 3, 4 * b + 1]])
    H, W = img.shape[:2]
    t = np.tile((b + 0.5) / b.size, (H // 8 + 1, W // 8 + 1))[:H, :W, None].astype(np.float32)
    return np.floor(np.clip(img, 0, 1) * 255.0 + t).clip(0, 255).astype(np.uint8)
