"""Upscale the Patrone wallpaper to 8K, keeping flat graphics (logo, text, lines) crisp.

usage: upscale_wallpaper.py SRC OUT_BASENAME [width height]

Writes OUT_BASENAME.png. (Not JPEG: it leaves blotches in the dark gradients.)

How it works (see wallpaper_layers.py):
  1. Take the source apart into a clean background and its crisp graphics.
  2. Upscale the background with Lanczos (glows and gradients survive this well).
  3. For the crisp elements, upscale a coverage (alpha) mask and re-sharpen its
     edges, then composite them back using a smooth foreground-colour field, so
     edges come out sharp at the new resolution with their original colours.
  4. Add fine luminance grain at the new resolution to avoid banding.
"""
import sys

import cv2

from wallpaper_layers import Parts, grain, to8

src_path, out_base = sys.argv[1], sys.argv[2]
W, H = (int(sys.argv[3]), int(sys.argv[4])) if len(sys.argv) > 4 else (7680, 4320)

img8 = cv2.imread(src_path, cv2.IMREAD_COLOR)
h, w = img8.shape[:2]
print(f"source {w}x{h} -> {W}x{H}")

parts = Parts(img8)
BG = parts.background(W, H)
FG, A = parts.graphics(W, H, [parts.logo] + parts.details)
A = A[..., None]
out = BG * (1 - A) + FG * A
del BG, FG, A

out8 = to8(grain(out))
del out
cv2.imwrite(out_base + ".png", out8, [cv2.IMWRITE_PNG_COMPRESSION, 6])
print("wrote", out_base + ".png")
