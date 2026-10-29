"""Draws every PatroneOS logo file from the vector shapes in patrone_logo.py.

    python tools/make_logos.py

Writes, in logos/ (the vector masters, then pictures drawn from them):

  patrone-mark.svg                 the P
  patrone-mark-mono.svg            the P in one color (currentColor: black
                                   unless the page sets another)
  patrone-wordmark.svg             PATRONEOS, navy and blue, for light backgrounds
  patrone-wordmark-dark.svg        PATRONEOS, white and blue, for dark backgrounds
  patrone-logo-text.svg            the P beside the wordmark, for light backgrounds;
  patrone-logo-text-dark.svg       for dark ones. 72 pixels tall: Settings ->
                                   About shows its logo at its own size
  patrone-logo-stacked.svg         the P over the wordmark, as in the Neon
  patrone-logo-stacked-dark.svg    Horizon pictures
  patrone-logo.png                 the P, 420 square (os-release's LOGO, About)
  patrone-logo-text.png            the side-by-side logo, 248 tall
  patrone-logo-text-dark.png
  patrone-logo-stacked.png         the stacked logo, 840 wide
  patrone-logo-stacked-dark.png
  patrone-splash.png               the stacked logo for the boot splash
  patrone-logo-login.png           the side-by-side logo for the login screen,
                                   which GNOME shows at the file's own size

and in icons/, laid out as Yaru's folders, the P in place of Ubuntu's logo:
start-here.png at each of Yaru's sizes (Yaru's distributor-logo icons link
to it), and start-here-symbolic.svg and distributor-logo-symbolic.svg, the P
in one color, 16 by 16, which GTK paints in the text's color.
"""
import argparse
from pathlib import Path

import numpy as np
from PIL import Image

import patrone_logo as pl

BRANDING = Path(__file__).resolve().parent.parent
PADDING = 24
SPLASH_WIDTH = 380
LOGIN_HEIGHT = 42
ABOUT_HEIGHT = 72
ICON_SIZES = (16, 22, 24, 32, 48, 256)


def write_svg(path, pieces, height=None, title="PatroneOS"):
    if height:
        x0, y0, x1, y1 = pl.bounds(pieces)
        size = (round((x1 - x0) * height / (y1 - y0), 2), height)
    else:
        size = None
    path.write_text(pl.svg(pieces, size=size, title=title), encoding="utf-8", newline="\n")
    print(path.relative_to(BRANDING).as_posix())


def write_png(path, pieces, **fit):
    image = Image.fromarray(pl.png_rgba(pieces, **fit), "RGBA")
    image.save(path, optimize=True)
    print(f"{path.relative_to(BRANDING).as_posix()}: {image.width}x{image.height}")


def square_mark(size, pad):
    """The P centred in a size x size square, pad pixels clear of its edges."""
    pieces = pl.mark()
    x0, y0, x1, y1 = pl.bounds(pieces)
    k = (size - 2 * pad) / max(x1 - x0, y1 - y0)
    color, alpha = pl.draw(pieces, size, size, k, (size - (x1 - x0) * k) / 2 - x0 * k,
                           (size - (y1 - y0) * k) / 2 - y0 * k, ss=16)
    straight = color / np.maximum(alpha, 1e-6)[..., None]
    rgba = (np.clip(np.dstack([straight, alpha]), 0, 1) * 255 + 0.5).astype(np.uint8)
    return Image.fromarray(rgba, "RGBA")


def symbolic(path):
    """The P in one color in a 16 x 16 icon, as GTK's symbolic icons are drawn."""
    pieces = pl.mark(mono="#bebebe")
    x0, y0, x1, y1 = pl.bounds(pieces)
    k = 15.0 / (x1 - x0)
    pieces = pl.transformed(pieces, k, 0.5 - x0 * k, 8 - (y0 + y1) / 2 * k)
    body = "".join(f'<path fill="#bebebe" d="{"".join(c.svg() for c in p.contours)}"/>' for p in pieces)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text('<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16">'
                    + body + "</svg>\n", encoding="utf-8", newline="\n")
    print(path.relative_to(BRANDING).as_posix())


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.parse_args()
    logos = BRANDING / "logos"
    logos.mkdir(exist_ok=True)

    write_svg(logos / "patrone-mark.svg", pl.mark())
    mono = pl.mark(mono="#000000")
    text = pl.svg(mono, title="PatroneOS").replace('fill="#000000"', 'fill="currentColor"')
    (logos / "patrone-mark-mono.svg").write_text(text, encoding="utf-8", newline="\n")
    print("logos/patrone-mark-mono.svg")
    write_svg(logos / "patrone-wordmark.svg", pl.wordmark(dark=False))
    write_svg(logos / "patrone-wordmark-dark.svg", pl.wordmark(dark=True))
    write_svg(logos / "patrone-logo-text.svg", pl.side_by_side(dark=False), height=ABOUT_HEIGHT)
    write_svg(logos / "patrone-logo-text-dark.svg", pl.side_by_side(dark=True), height=ABOUT_HEIGHT)
    write_svg(logos / "patrone-logo-stacked.svg", pl.stacked(dark=False))
    write_svg(logos / "patrone-logo-stacked-dark.svg", pl.stacked(dark=True))

    square_mark(420, PADDING).save(logos / "patrone-logo.png", optimize=True)
    print("logos/patrone-logo.png: 420x420")
    write_png(logos / "patrone-logo-text.png", pl.side_by_side(dark=False), height=248, pad=PADDING)
    write_png(logos / "patrone-logo-text-dark.png", pl.side_by_side(dark=True), height=248, pad=PADDING)
    write_png(logos / "patrone-logo-stacked.png", pl.stacked(dark=False), width=840, pad=PADDING)
    write_png(logos / "patrone-logo-stacked-dark.png", pl.stacked(dark=True), width=840, pad=PADDING)
    write_png(logos / "patrone-splash.png", pl.stacked(dark=True), width=SPLASH_WIDTH)
    write_png(logos / "patrone-logo-login.png", pl.side_by_side(dark=True), height=LOGIN_HEIGHT)

    icons = BRANDING / "icons"
    for size in ICON_SIZES:
        for scale, folder in ((1, f"{size}x{size}"), (2, f"{size}x{size}@2x")):
            px = size * scale
            path = icons / folder / "places" / "start-here.png"
            path.parent.mkdir(parents=True, exist_ok=True)
            square_mark(px, px * PADDING / 420).save(path, optimize=True)
    print(f"icons/*/places/start-here.png: {', '.join(map(str, ICON_SIZES))} and @2x")
    for name in ("start-here-symbolic.svg", "distributor-logo-symbolic.svg"):
        symbolic(icons / "scalable" / "places" / name)


if __name__ == "__main__":
    main()
