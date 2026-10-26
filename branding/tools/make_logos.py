"""Cuts the Patrone logo out of the concept into transparent PNGs.

    python tools/make_logos.py concepts/logo-concept-01.webp logos/

Writes:
  patrone-logo.png            the mark alone, square
  patrone-logo-text-dark.png  the mark beside the wordmark, light text, for
                              dark backgrounds (login screen, dark mode)
  patrone-logo-text.png       the same with dark text, for light backgrounds
  patrone-logo-stacked.png    the whole primary logo as in the concept (mark,
                              wordmark, ruled LINUX), light text
  patrone-splash.png          the stacked logo at boot-splash size

The concept is drawn on near-black, so a pixel's brightest channel tells how
much of it is logo: that becomes its opacity, and the color is recovered by
undoing the blend with black. Dark blues stay solid because they are still far
brighter than the background.
"""

import argparse
from pathlib import Path

import numpy as np
from PIL import Image

# Where each part sits in logo-concept-01 (left, top, right, bottom).
MARK = (571, 71, 943, 357)
WORDMARK_AND_RULE = (372, 392, 1164, 530)
WHOLE_LOGO = (372, 71, 1164, 530)

# Brightest-channel values: at or below LOW is background, at or above HIGH
# fully logo.
LOW, HIGH = 12, 72
DARK_TEXT = (0x1A, 0x1A, 0x1A)
PADDING = 24
SPLASH_WIDTH = 380


def cut_out(image: np.ndarray, box: tuple[int, int, int, int]) -> Image.Image:
    left, top, right, bottom = box
    rgb = image[top:bottom, left:right].astype(float)
    brightest = rgb.max(axis=2)
    alpha = np.clip((brightest - LOW) / (HIGH - LOW), 0.0, 1.0)
    color = np.clip(rgb / np.maximum(alpha, 1e-3)[..., None], 0, 255)
    rgba = np.dstack([color, alpha * 255]).round().astype(np.uint8)
    return Image.fromarray(rgba, "RGBA")


def padded(image: Image.Image, padding: int = PADDING, square: bool = False) -> Image.Image:
    width, height = image.size
    if square:
        width = height = max(width, height)
    canvas = Image.new("RGBA", (width + 2 * padding, height + 2 * padding))
    canvas.paste(image, (padding + (width - image.width) // 2, padding + (height - image.height) // 2))
    return canvas


def with_dark_text(image: Image.Image) -> Image.Image:
    """Turns the white wordmark dark, leaving the blues alone."""
    pixels = np.asarray(image).copy()
    rgb = pixels[..., :3].astype(int)
    grey = (rgb.max(axis=2) - rgb.min(axis=2)) < 40
    pixels[grey, :3] = DARK_TEXT
    return Image.fromarray(pixels, "RGBA")


def side_by_side(mark: Image.Image, text: Image.Image) -> Image.Image:
    """The mark to the left of the wordmark, the mark a bit taller."""
    mark_height = round(text.height * 1.45)
    mark = mark.resize((round(mark.width * mark_height / mark.height), mark_height), Image.LANCZOS)
    gap = text.height // 3
    canvas = Image.new("RGBA", (mark.width + gap + text.width, mark_height))
    canvas.paste(mark, (0, 0))
    canvas.paste(text, (mark.width + gap, (mark_height - text.height) // 2))
    return canvas


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("concept")
    parser.add_argument("output_dir")
    args = parser.parse_args()

    image = np.asarray(Image.open(args.concept).convert("RGB"))
    out = Path(args.output_dir)
    out.mkdir(parents=True, exist_ok=True)

    mark = cut_out(image, MARK)
    text = cut_out(image, WORDMARK_AND_RULE)
    lockup = side_by_side(mark, text)
    stacked = cut_out(image, WHOLE_LOGO)
    splash_height = round(stacked.height * SPLASH_WIDTH / stacked.width)
    outputs = {
        "patrone-logo.png": padded(mark, square=True),
        "patrone-logo-text-dark.png": padded(lockup),
        "patrone-logo-text.png": padded(with_dark_text(lockup)),
        "patrone-logo-stacked.png": padded(stacked),
        "patrone-splash.png": stacked.resize((SPLASH_WIDTH, splash_height), Image.LANCZOS),
    }
    for name, logo in outputs.items():
        logo.save(out / name, optimize=True)
        print(f"{name}: {logo.width}x{logo.height}")


if __name__ == "__main__":
    main()
