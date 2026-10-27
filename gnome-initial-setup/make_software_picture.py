#!/usr/bin/env python3
# Draws the first-login setup's "Get started with more applications" picture
# (patrone-software.svg, in place of Ubuntu's app-center.svg): Patrone
# Software's icon in the middle and ten of the Craft apps round it, joined by
# dotted lines, as Ubuntu's draws App Center among its apps. The icons are
# PNGs inside the SVG.
#
#   python3 gnome-initial-setup/make_software_picture.py gnome-initial-setup/patrone-software.svg
#
# Run on a Patrone system with Patrone's app index (its catalog has the Craft
# apps' icons) and GNOME Software.
import base64
import io
import math
import sys

import cairo
import gi

gi.require_version('Rsvg', '2.0')
from gi.repository import Rsvg  # noqa: E402

SIZE = 423  # Ubuntu's picture's size
ICONS = '/var/lib/flatpak/appstream/patrone-app-index/x86_64/active/icons/128x128'
APPS = ['photocraft', 'pdfcraft', 'filmcraft', 'wordcraft', 'lightcraft',
        'vectorcraft', 'gridcraft', 'deckcraft', 'soundcraft', 'mailcraft']
CENTER_ICON = 112
APP_ICON = 52
RING = 150
LINE = '#9aa0a6'


def svg_png(path, size):
    """An SVG icon drawn as a PNG, size pixels square."""
    surface = cairo.ImageSurface(cairo.FORMAT_ARGB32, size, size)
    viewport = Rsvg.Rectangle()
    viewport.x = viewport.y = 0
    viewport.width = viewport.height = size
    Rsvg.Handle.new_from_file(path).render_document(cairo.Context(surface), viewport)
    data = io.BytesIO()
    surface.write_to_png(data)
    return data.getvalue()


def image(png, x, y, size):
    return (f'<image x="{x:.1f}" y="{y:.1f}" width="{size}" height="{size}" '
            f'xlink:href="data:image/png;base64,{base64.b64encode(png).decode()}"/>')


def app_icon(name):
    prefix = 'io.github.billsecond' if name == 'mailcraft' else 'ai.storyteller'
    with open(f'{ICONS}/{prefix}.{name}.png', 'rb') as f:
        return f.read()


middle = SIZE / 2
parts = []
for i, name in enumerate(APPS):
    angle = -math.pi / 2 + 2 * math.pi * i / len(APPS)
    x, y = middle + RING * math.cos(angle), middle + RING * math.sin(angle)
    # A dotted line from Patrone Software to the app, stopping short of both
    start = CENTER_ICON / 2 + 10
    end = RING - APP_ICON / 2 - 8
    x1, y1 = middle + start * math.cos(angle), middle + start * math.sin(angle)
    x2, y2 = middle + end * math.cos(angle), middle + end * math.sin(angle)
    parts.append(f'<line x1="{x1:.1f}" y1="{y1:.1f}" x2="{x2:.1f}" y2="{y2:.1f}" stroke="{LINE}" '
                 f'stroke-width="2" stroke-linecap="round" stroke-dasharray="0.1 7"/>')
    parts.append(f'<circle cx="{x2:.1f}" cy="{y2:.1f}" r="3" fill="{LINE}"/>')
    parts.append(image(app_icon(name), x - APP_ICON / 2, y - APP_ICON / 2, APP_ICON))
software = svg_png('/usr/share/icons/hicolor/scalable/apps/org.gnome.Software.svg', CENTER_ICON * 2)
parts.append(image(software, middle - CENTER_ICON / 2, middle - CENTER_ICON / 2, CENTER_ICON))

with open(sys.argv[1], 'w') as out:
    out.write(f'<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" '
              f'width="{SIZE}" height="{SIZE}" viewBox="0 0 {SIZE} {SIZE}">\n')
    out.write('\n'.join(parts) + '\n</svg>\n')
print(sys.argv[1])
