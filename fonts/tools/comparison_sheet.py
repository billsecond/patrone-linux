#!/usr/bin/env python3
"""Draw a sheet comparing Noto Sans with Patrone Sans at desktop sizes.

    comparison_sheet.py OUT.png FONT.ttf [FONT.ttf...]

The fonts given are added to the system's (fontconfig), and each sample is
drawn through Pango and cairo, as GTK draws text, at 96 dpi (100% scale)
with GNOME's defaults: greyscale smoothing, slight hinting, no hinted
metrics. Needs the system's python3 (for PyGObject), not the build's venv.
"""

import os
import sys
import tempfile
from pathlib import Path

LEFT, RIGHT = "Noto Sans", "Patrone Sans"
SIZES = (9, 10, 11)  # points; the desktop's text is 11, File Manager's lists 9
SAMPLES = [
    "<b>Settings</b>   Wi-Fi   Bluetooth   Displays   Sound   Power",
    "Illustration_01.png   Invoice 2026-10-01.pdf   1,024 items, 10.4 GB",
    "Password: Il1|O0oI   Key: 0O1l-I0O8-B8S5   Volume 100%",
    "Install all 10 updates? The files will be moved to the Recycle Bin.",
    "<i>Italic: all 10 files in Illinois, 0.01 ml</i>   <b>Bold: Il1 O0 all</b>",
]
GLYPHS = "Il1| O0 ll ill ł 10:00"
FIGURES = ["1,111.10", "9,876.09", "  210.00", "10:01:11"]
WEIGHTS = [("Regular", 400), ("Medium", 500), ("SemiBold", 600), ("Bold", 700)]
THEMES = [
    ("Light", (0.98, 0.98, 0.98), (0.12, 0.12, 0.13), (0.45, 0.45, 0.47)),
    ("Dark", (0.13, 0.13, 0.15), (0.96, 0.96, 0.97), (0.62, 0.62, 0.65)),
]
COLUMN = 640
MARGIN = 24


def use_fonts(files):
    """fontconfig with the system's fonts and the given files"""
    dirs = "".join(f"<dir>{Path(f).resolve().parent}</dir>" for f in files)
    conf = tempfile.NamedTemporaryFile("w", suffix=".conf", delete=False)
    conf.write(
        '<?xml version="1.0"?><!DOCTYPE fontconfig SYSTEM "urn:fontconfig:fonts.dtd">'
        f'<fontconfig><include ignore_missing="yes">/etc/fonts/fonts.conf</include>{dirs}'
        f"<cachedir>{tempfile.gettempdir()}/patrone-sans-fc-cache</cachedir></fontconfig>"
    )
    conf.close()
    os.environ["FONTCONFIG_FILE"] = conf.name


def main():
    if len(sys.argv) < 3:
        sys.exit(__doc__)
    out, files = sys.argv[1], sys.argv[2:]
    use_fonts(files)

    import gi

    gi.require_version("Pango", "1.0")
    gi.require_version("PangoCairo", "1.0")
    import cairo
    from gi.repository import Pango, PangoCairo

    options = cairo.FontOptions()
    options.set_antialias(cairo.ANTIALIAS_GRAY)
    options.set_hint_style(cairo.HINT_STYLE_SLIGHT)
    options.set_hint_metrics(cairo.HINT_METRICS_OFF)

    def layout(cr, family, points, text, weight=400, markup=True, tnum=False):
        lay = PangoCairo.create_layout(cr)
        PangoCairo.context_set_font_options(lay.get_context(), options)
        PangoCairo.context_set_resolution(lay.get_context(), 96)
        desc = Pango.FontDescription.from_string(f"{family} {points}")
        desc.set_weight(weight)
        lay.set_font_description(desc)
        if markup:
            lay.set_markup(text, -1)
        else:
            lay.set_text(text, -1)
        if tnum:
            attrs = Pango.AttrList()
            attrs.insert(Pango.attr_font_features_new("tnum 1"))
            lay.set_attributes(attrs)
        return lay

    def resolved(family):
        """the file fontconfig picks for a family, to label the columns"""
        surface = cairo.ImageSurface(cairo.FORMAT_RGB24, 10, 10)
        lay = layout(cairo.Context(surface), family, 11, "a", markup=False)
        font = lay.get_context().load_font(lay.get_font_description())
        return font.describe().to_string()

    def panel(theme, height_only=False):
        name, background, ink, faint = theme
        height = 0
        rows = []
        rows.append(("title", f"{name} style: {LEFT} (left) and {RIGHT} (right), at 100% scale"))
        rows.append(("glyphs", None))
        for points in SIZES:
            rows.append(("label", f"{points} pt"))
            for sample in SAMPLES:
                rows.append(("text", (points, sample)))
        rows.append(("label", "Weights at 11 pt (Noto Sans as installed has Regular and Bold only)"))
        rows.append(("weights", None))
        rows.append(("label", "Figures at 11 pt: both have tabular figures (all digits one width)"))
        for figure in FIGURES:
            rows.append(("figure", figure))
        rows.append(("label", "Pixels at 10 pt, four times as big"))
        rows.append(("zoom", "Il1| O0 Wi-Fi 2026-10-01"))
        return name, background, ink, faint, rows

    def row_height(kind, value):
        return {
            "title": 34,
            "glyphs": 72,
            "label": 30,
            "text": {9: 18, 10: 20, 11: 22}[value[0]] if kind == "text" else 0,
            "weights": 26,
            "figure": 22,
            "zoom": 4 * 22 + 8,
        }[kind]

    panels = [panel(t) for t in THEMES]
    total = sum(sum(row_height(k, v) for k, v in rows) + 2 * MARGIN for *_, rows in panels)
    width = 2 * COLUMN + 3 * MARGIN
    surface = cairo.ImageSurface(cairo.FORMAT_RGB24, width, total)
    cr = cairo.Context(surface)
    top = 0
    for name, background, ink, faint, rows in panels:
        height = sum(row_height(k, v) for k, v in rows) + 2 * MARGIN
        cr.set_source_rgb(*background)
        cr.rectangle(0, top, width, height)
        cr.fill()
        y = top + MARGIN
        for kind, value in rows:
            h = row_height(kind, value)
            if kind in ("title", "label"):
                cr.set_source_rgb(*(ink if kind == "title" else faint))
                lay = layout(cr, "Noto Sans", 12 if kind == "title" else 9,
                             f"<b>{value}</b>" if kind == "title" else value)
                cr.move_to(MARGIN, y + (6 if kind == "label" else 0))
                PangoCairo.show_layout(cr, lay)
                y += h
                continue
            for column, family in enumerate((LEFT, RIGHT)):
                x = MARGIN + column * (COLUMN + MARGIN)
                cr.set_source_rgb(*ink)
                if kind == "glyphs":
                    lay = layout(cr, family, 36, GLYPHS, markup=False)
                    cr.move_to(x, y)
                    PangoCairo.show_layout(cr, lay)
                elif kind == "text":
                    points, sample = value
                    lay = layout(cr, family, points, sample)
                    cr.move_to(x, y)
                    PangoCairo.show_layout(cr, lay)
                elif kind == "weights":
                    cx = x
                    for label, weight in WEIGHTS:
                        lay = layout(cr, family, 11, label, weight=weight, markup=False)
                        cr.move_to(cx, y)
                        PangoCairo.show_layout(cr, lay)
                        cx += lay.get_pixel_size()[0] + 18
                elif kind == "figure":
                    lay = layout(cr, family, 11, value, markup=False)
                    w = lay.get_pixel_size()[0]
                    cr.move_to(x + 160 - w, y)
                    PangoCairo.show_layout(cr, lay)
                elif kind == "zoom":
                    small = cairo.ImageSurface(cairo.FORMAT_RGB24, COLUMN // 4, 22)
                    sc = cairo.Context(small)
                    sc.set_source_rgb(*background)
                    sc.paint()
                    sc.set_source_rgb(*ink)
                    lay = layout(sc, family, 10, value, markup=False)
                    sc.move_to(2, 2)
                    PangoCairo.show_layout(sc, lay)
                    cr.save()
                    cr.translate(x, y)
                    cr.scale(4, 4)
                    cr.set_source_surface(small, 0, 0)
                    cr.get_source().set_filter(cairo.FILTER_NEAREST)
                    cr.paint()
                    cr.restore()
            y += h
        top += height
    surface.write_to_png(out)
    print(f"{out}: {LEFT} is {resolved(LEFT)}, {RIGHT} is {resolved(RIGHT)}")


if __name__ == "__main__":
    main()
