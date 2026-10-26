"""Wallpaper 01 taken apart, so it can be redrawn at any size and in other colours.

The original is one flat picture: a dark background with blue light streaks,
and crisp graphics over it (the logo, and the terminal details in two
corners). Parts() separates them at the source's size:

  - the background, with the graphics painted out;
  - the graphics, as a colour field and a coverage (alpha) mask.

How:
  1. Denoise the source to remove JPEG blocking.
  2. Find the crisp elements inside a few regions of interest and inpaint them
     out to get a clean background.
  3. Measure the elements' coverage, and spread their interior colours
     outward so edge pixels get the element's colour, not a mix.

Upscaling the two separately keeps the glows smooth and the graphics sharp:
the background with Lanczos, the coverage with cubic and its edges
re-sharpened. Used by upscale_wallpaper.py and make_wallpapers.py.
"""
import cv2
import numpy as np

# Regions holding crisp graphics, in pixels of the 1672x941 original
# (x0, y0, x1, y1). They're scaled if the source is re-exported at another size.
REF_W, REF_H = 1672, 941
LOGO = (550, 280, 1120, 630)
MARK = (700, 285, 975, 500)  # the P alone, above the wordmark
DETAILS = [(1140, 0, 1672, 380), (0, 600, 700, 941)]

# Edge sharpening, tuned at 8K (source edges, ~1.5 px, end up ~2 px wide):
# the coverage is steepened by K8K when the source is enlarged to 7680 wide.
K8K = 3.5


class Parts:
    def __init__(self, img8):
        h, w = img8.shape[:2]
        self.w, self.h = w, h
        self.logo = self._box(LOGO)
        self.mark = self._box(MARK)
        self.details = [self._box(b) for b in DETAILS]

        # 1. Denoise. All colour work happens in float: smoothing in 8-bit
        # flattens the dark gradients into visible bands. The 8-bit denoised
        # copy is only used to find features.
        den8 = cv2.fastNlMeansDenoisingColored(img8, None, 3, 3, 7, 21)
        src = img8.astype(np.float32) / 255.0
        # Edge-preserving smoothing removes JPEG blocks and grain but keeps strong edges.
        den = cv2.bilateralFilter(src, 9, 0.05, 2.5)
        maxch = den.max(axis=2)

        roi = np.zeros((h, w), np.float32)
        logo_roi = np.zeros((h, w), bool)
        for (x0, y0, x1, y1) in [self.logo] + self.details:
            roi[y0:y1, x0:x1] = 1.0
        x0, y0, x1, y1 = self.logo
        logo_roi[y0:y1, x0:x1] = True

        # 2. Candidate foreground: thin bright features above the local median,
        # plus the large logo shapes (bright against a near-black background).
        med = cv2.medianBlur(den8, 31).astype(np.float32).max(axis=2) / 255.0
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
        # more than the graphics to clear the remaining JPEG blocks in the glows.
        self.bg = cv2.bilateralFilter(bg, 9, 0.08, 3.0)

        # 3. Coverage: brightness above background, relative to the local peak.
        d = np.clip(maxch - bg.max(axis=2), 0, None) * roi * mask0
        peak = cv2.dilate(d, cv2.getStructuringElement(cv2.MORPH_ELLIPSE, (7, 7)))
        alpha = np.where(peak > 0.04, np.clip(d / np.maximum(peak, 1e-4), 0, 1), 0).astype(np.float32)
        # Pixels whose whole 3x3 neighbourhood is bright are interior, even
        # across the P's internal facet edges where brightness changes sharply.
        interior = cv2.erode(d, np.ones((3, 3), np.uint8)) > 0.5 * peak
        alpha[interior & (peak > 0.04)] = 1.0
        self.alpha = alpha

        # For drawings without the logo: inpainting leaves a faint outline of
        # it, and misses the dimmest of its rules, which never matter when the
        # logo is drawn back over them. Here everything in its area that
        # stands out at all is refilled with a wide, smooth average of the
        # surroundings (a normalized blur), so nothing of it remains.
        faint = ((maxch - med) > 0.015) | (alpha > 0.01)
        hole = cv2.dilate((faint & logo_roi).astype(np.uint8),
                          cv2.getStructuringElement(cv2.MORPH_ELLIPSE, (25, 25))).astype(np.float32)
        keep = 1 - hole
        fill = cv2.GaussianBlur(self.bg * keep[..., None], (0, 0), 40) / \
            np.maximum(cv2.GaussianBlur(keep, (0, 0), 40), 1e-6)[..., None]
        soft = cv2.GaussianBlur(hole, (0, 0), 3.0)[..., None]
        self.bg_clear = self.bg * (1 - soft) + fill * soft

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

    def _box(self, b):
        x0, y0, x1, y1 = b
        w, h = self.w, self.h
        return (int(x0 * w / REF_W), int(y0 * h / REF_H), int(x1 * w / REF_W), int(y1 * h / REF_H))

    def background(self, W, H, clear=False):
        """The background alone, W x H; clear: cleared for drawing without the logo."""
        BG = cv2.resize(self.bg_clear if clear else self.bg, (W, H), interpolation=cv2.INTER_LANCZOS4)
        # A little extra definition on the glows' edges.
        blur = cv2.GaussianBlur(BG, (0, 0), 3.0 * W / 7680)
        return BG + 0.35 * (BG - blur)

    def graphics(self, W, H, regions, exclude=()):
        """Colour and coverage, W x H, of the graphics in regions (boxes from
        self), less any in the boxes in exclude."""
        a = np.zeros_like(self.alpha)
        for (x0, y0, x1, y1) in regions:
            a[y0:y1, x0:x1] = self.alpha[y0:y1, x0:x1]
        for (x0, y0, x1, y1) in exclude:
            a[y0:y1, x0:x1] = 0
        FG = cv2.resize(self.fg, (W, H), interpolation=cv2.INTER_CUBIC)
        A = cv2.resize(a, (W, H), interpolation=cv2.INTER_CUBIC)
        # Edges 2 px wide at 8K, 1 px at 4K, and as sharp as the source allows below.
        return FG, _sharpen(A, K8K * min(1.0, 2 * W / 7680))

    def graphic(self, region, scale):
        """Colour and coverage of the graphics in region, at scale times their size."""
        x0, y0, x1, y1 = region
        fg, a = self.fg[y0:y1, x0:x1], self.alpha[y0:y1, x0:x1]
        # Draw at 8K's scale at least, where the edges are tuned, and shrink
        # from there: smaller copies come out sharp and smooth.
        s8 = 7680 / self.w
        big = max(scale, s8)
        size = (round((x1 - x0) * big), round((y1 - y0) * big))
        FG = cv2.resize(fg, size, interpolation=cv2.INTER_CUBIC)
        A = _sharpen(cv2.resize(a, size, interpolation=cv2.INTER_CUBIC), K8K * big / s8)
        if big > scale:
            size = (round((x1 - x0) * scale), round((y1 - y0) * scale))
            FGA = cv2.resize(FG * A[..., None], size, interpolation=cv2.INTER_AREA)
            A = cv2.resize(A, size, interpolation=cv2.INTER_AREA)
            FG = FGA / np.maximum(A, 1e-4)[..., None]
        return FG, A


def _sharpen(A, k):
    """Steepen upscaled coverage edges by k."""
    return np.clip((A - 0.5) * k + 0.5, 0, 1)


def grain(img, seed=7):
    """Fine luminance grain at full resolution; it also dithers the dark
    gradients so they don't band."""
    H, W = img.shape[:2]
    rng = np.random.default_rng(seed)
    g = rng.standard_normal((H, W), dtype=np.float32) * (1.5 / 255.0)
    img += g[..., None]
    return img


def to8(img):
    return (np.clip(img, 0, 1) * 255.0 + 0.5).astype(np.uint8)


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
