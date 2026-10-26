#!/usr/bin/env python3
"""Draw lines of text in a font file, big, to look at its letters.

    proof.py OUT.png FONT.ttf [FONT.ttf...] [--text TEXT] [--size PX]

Each font is drawn at Light, Regular, Bold (300, 400, 700), one line each.
Uses fontconfig with only the given files added, through Pango and cairo,
as GTK does.
"""

import argparse
import os
import sys
import tempfile
from pathlib import Path


def use_fonts(files):
    """Point fontconfig at the system's fonts plus these files"""
    conf = tempfile.NamedTemporaryFile("w", suffix=".conf", delete=False)
    dirs = "".join(f"<dir>{Path(f).resolve().parent}</dir>" for f in files)
    conf.write(
        '<?xml version="1.0"?><!DOCTYPE fontconfig SYSTEM "urn:fontconfig:fonts.dtd">'
        f"<fontconfig><include ignore_missing=\"yes\">/etc/fonts/fonts.conf</include>{dirs}"
        f"<cachedir>{tempfile.gettempdir()}/patrone-sans-fc-cache</cachedir></fontconfig>"
    )
    conf.close()
    os.environ["FONTCONFIG_FILE"] = conf.name


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("out")
    parser.add_argument("families", nargs="+", help="family names")
    parser.add_argument("--font-file", action="append", default=[])
    parser.add_argument("--text", default="Il1| O0 lll illicit Ill ł Ł ŀl ľ ļ 2026 10:00")
    parser.add_argument("--size", type=int, default=72)
    args = parser.parse_args()
    use_fonts(args.font_file)

    import gi

    gi.require_version("Pango", "1.0")
    gi.require_version("PangoCairo", "1.0")
    import cairo
    from gi.repository import Pango, PangoCairo

    lines = [
        (family, weight)
        for family in args.families
        for weight in (Pango.Weight.LIGHT, Pango.Weight.NORMAL, Pango.Weight.BOLD)
    ]
    line_height = int(args.size * 1.5)
    width = int(args.size * len(args.text) * 0.62) + 40
    surface = cairo.ImageSurface(cairo.FORMAT_RGB24, width, line_height * len(lines) + 20)
    cr = cairo.Context(surface)
    cr.set_source_rgb(1, 1, 1)
    cr.paint()
    cr.set_source_rgb(0, 0, 0)
    for n, (family, weight) in enumerate(lines):
        layout = PangoCairo.create_layout(cr)
        description = Pango.FontDescription.from_string(family)
        description.set_weight(weight)
        description.set_absolute_size(args.size * Pango.SCALE)
        layout.set_font_description(description)
        layout.set_text(args.text, -1)
        cr.move_to(20, 10 + n * line_height)
        PangoCairo.show_layout(cr, layout)
    surface.write_to_png(args.out)
    print(args.out, file=sys.stderr)


if __name__ == "__main__":
    main()
