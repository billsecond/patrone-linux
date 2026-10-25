"""Upscale the Patrone wallpaper to 8K, keeping flat graphics (logo, text, lines) crisp.

usage: upscale_wallpaper.py SRC OUT_BASENAME [width height]

Writes OUT_BASENAME.png. (Not JPEG: it leaves blotches in the dark gradients.)

How it works:
  1. Denoise the source to remove JPEG blocking.
  2. Find the crisp elements inside a few regions of interest and inpaint them
     out to get a clean background.
  3. Upscale the background with Lanczos (glows and gradients survive this well).
  4. For the crisp elements, upscale a coverage (alpha) mask and re-sharpen its
     edges, then composite them back using a smooth foreground-colour field, so
     edges come out sharp at the new resolution with their original colours.
  5. Add fine luminance grain at the new resolution to avoid banding.
"""
import sys

import cv2
import numpy as np

src_path, out_base = sys.argv[1], sys.argv[2]
W, H = (int(sys.argv[3]), int(sys.argv[4])) if len(sys.argv) > 4 else (7680, 4320)

img8 = cv2.imread(src_path, cv2.IMREAD_COLOR)
h, w = img8.shape[:2]
print(f"source {w}x{h} -> {W}x{H}")

# Regions holding crisp graphics, in source pixels (x0, y0, x1, y1). The values
# are proportional so they also work if the source is re-exported at another size.
def box(x0, y0, x1, y1):
    return (int(x0 * w / 1672), int(y0 * h / 941), int(x1 * w / 1672), int(y1 * h / 941))

LOGO = box(550, 280, 1120, 630)
DETAIL_BOXES = [box(1140, 0, 1672, 380), box(0, 600, 700, 941)]

# 1. Denoise. All colour work happens in float: smoothing in 8-bit flattens the
# dark gradients into visible bands. The 8-bit denoised copy is only used to
# find features.
den8 = cv2.fastNlMeansDenoisingColored(img8, None, 3, 3, 7, 21)
src = img8.astype(np.float32) / 255.0
# Edge-preserving smoothing removes JPEG blocks and grain but keeps strong edges.
den = cv2.bilateralFilter(src, 9, 0.05, 2.5)
maxch = den.max(axis=2)

roi = np.zeros((h, w), np.float32)
logo_roi = np.zeros((h, w), bool)
for (x0, y0, x1, y1) in [LOGO] + DETAIL_BOXES:
    roi[y0:y1, x0:x1] = 1.0
x0, y0, x1, y1 = LOGO
logo_roi[y0:y1, x0:x1] = True

# 2. Candidate foreground: thin bright features above the local median, plus
# the large logo shapes (bright against a near-black background).
med = cv2.medianBlur(den8, 31).astype(np.float32).max(axis=2) / 255.0
thin = (maxch - med) > 0.05
big = logo_roi & (maxch > 0.18)
cand = ((thin | big) & (roi > 0)).astype(np.uint8)
mask0 = cv2.dilate(cand, cv2.getStructuringElement(cv2.MORPH_ELLIPSE, (7, 7)))

# Inpaint works in 8-bit, so only take its values under the masked graphics,
# then smooth them so the filled area has no steps.
painted = cv2.inpaint(den8, mask0 * 255, 7, cv2.INPAINT_TELEA).astype(np.float32) / 255.0
under = mask0.astype(bool)
bg = den.copy()
bg[under] = painted[under]
soft = cv2.GaussianBlur(mask0.astype(np.float32), (0, 0), 4.0)[..., None]
bg = bg * (1 - soft) + cv2.GaussianBlur(bg, (0, 0), 4.0) * soft
# The background is only glows and gradients, so smooth it a little more than
# the graphics to clear the remaining JPEG blocks in the glows.
bg_up_src = cv2.bilateralFilter(bg, 9, 0.08, 3.0)

# 3. Coverage: brightness above background, relative to the local peak.
d = np.clip(maxch - bg.max(axis=2), 0, None) * roi * mask0
peak = cv2.dilate(d, cv2.getStructuringElement(cv2.MORPH_ELLIPSE, (7, 7)))
alpha = np.where(peak > 0.04, np.clip(d / np.maximum(peak, 1e-4), 0, 1), 0).astype(np.float32)
# Pixels whose whole 3x3 neighbourhood is bright are interior, even across the
# P's internal facet edges where brightness changes sharply.
interior = cv2.erode(d, np.ones((3, 3), np.uint8)) > 0.5 * peak
alpha[interior & (peak > 0.04)] = 1.0

# 4. Foreground colour field: spread confident interior colours outward so edge
# pixels get the element's colour rather than a mix with the background.
wgt = (alpha > 0.85).astype(np.float32)
fg = den.copy()
filled = np.zeros((h, w), bool)
for sigma in (1.2, 3.0, 8.0):
    num = cv2.GaussianBlur(den * wgt[..., None], (0, 0), sigma)
    dw = cv2.GaussianBlur(wgt, (0, 0), sigma)
    ok = (dw > 1e-3) & ~filled
    fg[ok] = num[ok] / dw[ok][:, None]
    filled |= ok

# 5. Upscale
BG = cv2.resize(bg_up_src, (W, H), interpolation=cv2.INTER_LANCZOS4)
FG = cv2.resize(fg, (W, H), interpolation=cv2.INTER_CUBIC)
A = cv2.resize(alpha, (W, H), interpolation=cv2.INTER_CUBIC)
k = 3.5  # edge steepness: source edges (~1.5 px) end up ~2 px wide at 8K
A = np.clip((A - 0.5) * k + 0.5, 0, 1)[..., None]
del bg, bg_up_src, fg, alpha

# A little extra definition on the background's glow edges.
blur = cv2.GaussianBlur(BG, (0, 0), 3.0)
BG = BG + 0.35 * (BG - blur)
del blur

out = BG * (1 - A) + FG * A
del BG, FG, A

# 6. Grain (luminance only), also dithers the dark gradients.
rng = np.random.default_rng(7)
grain = rng.standard_normal((H, W), dtype=np.float32) * (1.5 / 255.0)
out += grain[..., None]
del grain

out8 = (np.clip(out, 0, 1) * 255.0 + 0.5).astype(np.uint8)
del out
cv2.imwrite(out_base + ".png", out8, [cv2.IMWRITE_PNG_COMPRESSION, 6])
print("wrote", out_base + ".png")
