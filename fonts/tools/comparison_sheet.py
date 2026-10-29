#!/usr/bin/env python3
"""Draw a sheet comparing the installed Patrone Sans with a new build of
it, at the desktop's sizes.

    comparison_sheet.py OUT.png NEW.ttf

The new font is copied under another name (Patrone Sans Next) and added
to the system's fonts (fontconfig); each sample is drawn through Pango and
cairo, as GTK draws text, at 96 dpi (100% scale) with GNOME's defaults:
greyscale smoothing, slight hinting, no hinted metrics. Needs the system's
python3, with PyGObject and fontTools.
"""

import os
import sys
import tempfile
from pathlib import Path

LEFT, RIGHT = "Patrone Sans", "Patrone Sans Next"
SIZES = (9, 10, 11)  # points; the desktop's text is 11, File Manager's lists 9
SAMPLES = [
    "<b>Settings</b>   Wi-Fi   Bluetooth   Displays   Sound   Power   Keyboard",
    "Illustration_01.png   Invoice 2026-10-01.pdf   1,024 items, 10.4 GB free",
    "Password: Il1|O0oI   Key: 0O1l-I0O8-B8S5   Volume 100%   10:45 AM",
    "Install all 10 updates? The files will be moved to the Recycle Bin.",
    "Zażółć gęślą jaźń · Příliš žluťoučký kůň · Árvíztűrő tükörfúrógép · Şçğ",
    "Ελληνικά and Кириллица come from Noto Sans   “Quotes” — dashes – … ©",
]
GLYPHS = "Il1| O0 10:00 aegs Rk"
FIGURES = ["1,111.10", "9,876.09", "  210.00", "10:01:11"]
WEIGHTS = [("Light", 300), ("Regular", 400), ("Medium", 500), ("SemiBold", 600), ("Bold", 700),
           ("Black", 900)]
THEMES = [
    ("Light", (0.98, 0.98, 0.98), (0.12, 0.12, 0.13), (0.45, 0.45, 0.47)),
    ("Dark", (0.13, 0.13, 0.15), (0.96, 0.96, 0.97), (0.62, 0.62, 0.65)),
]
COLUMN = 700
MARGIN = 24


def renamed_copy(path, family):
    """the font under another family name, in a folder of its own"""
    from fontTools.ttLib import TTFont

    font = TTFont(path)
    old = font["name"].getDebugName(1)
    for record in font["name"].names:
        if record.nameID in (1, 3, 4, 6, 16, 25):
            text = record.toUnicode()
            text = text.replace(old, family).replace(old.replace(" ", ""), family.replace(" ", ""))
            record.string = text
    folder = Path(tempfile.mkdtemp(prefix="patrone-sans-next-"))
    out = folder / "PatroneSansNext.ttf"
    font.save(out)
    return out


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
    if len(sys.argv) != 3:
        sys.exit(__doc__)
    out, new = sys.argv[1], sys.argv[2]
    use_fonts([renamed_copy(new, RIGHT)])

    import gi

    gi.require_version("Pango", "1.0")
    gi.require_version("PangoCairo", "1.0")
    import cairo
    from gi.repository import Pango, PangoCairo

    options = cairo.FontOptions()
    options.set_antialias(cairo.ANTIALIAS_GRAY)
    options.set_hint_style(cairo.HINT_STYLE_SLIGHT)
    options.set_hint_metrics(cairo.HINT_METRICS_OFF)

    def layout(cr, family, points, text, weight=400, markup=True):
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
        return lay

    def resolved(family):
        """the file fontconfig picks for a family, to label the columns"""
        surface = cairo.ImageSurface(cairo.FORMAT_RGB24, 10, 10)
        lay = layout(cairo.Context(surface), family, 11, "a", markup=False)
        font = lay.get_context().load_font(lay.get_font_description())
        return font.describe().to_string()

    def rows():
        out = [("glyphs", None)]
        for points in SIZES:
            out.append(("label", f"{points} pt"))
            out.extend(("text", (points, s)) for s in SAMPLES)
        out.append(("label", "The six weights at 11 pt"))
        out.append(("weights", None))
        out.append(("label", "Figures at 11 pt: both tabular (every digit one width)"))
        out.extend(("figure", f) for f in FIGURES)
        out.append(("label", "Pixels at 10 pt, four times as big"))
        out.append(("zoom", "Il1| O0 Wi-Fi 2026-10-01 ąę"))
        return out

    def row_height(kind, value):
        return {"title": 34, "glyphs": 72, "label": 30, "weights": 26, "figure": 22,
                "zoom": 4 * 22 + 8}.get(kind) or {9: 18, 10: 20, 11: 22}[value[0]]

    body = rows()
    panel_h = sum(row_height(k, v) for k, v in body) + 34 + 2 * MARGIN
    width = 2 * COLUMN + 3 * MARGIN
    surface = cairo.ImageSurface(cairo.FORMAT_RGB24, width, panel_h * len(THEMES))
    cr = cairo.Context(surface)
    top = 0
    for name, background, ink, faint in THEMES:
        cr.set_source_rgb(*background)
        cr.rectangle(0, top, width, panel_h)
        cr.fill()
        y = top + MARGIN
        cr.set_source_rgb(*ink)
        lay = layout(cr, "Noto Sans", 12, f"<b>{name} style: Patrone Sans 1 (left) and Patrone Sans 2 "
                     "(right), at 100% scale</b>")
        cr.move_to(MARGIN, y)
        PangoCairo.show_layout(cr, lay)
        y += 34
        for kind, value in body:
            h = row_height(kind, value)
            if kind == "label":
                cr.set_source_rgb(*faint)
                lay = layout(cr, "Noto Sans", 9, value)
                cr.move_to(MARGIN, y + 6)
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
                        cx += lay.get_pixel_size()[0] + 16
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
        top += panel_h
    surface.write_to_png(out)
    print(f"{out}: left is {resolved(LEFT)}, right is {resolved(RIGHT)}")


if __name__ == "__main__":
    main()
