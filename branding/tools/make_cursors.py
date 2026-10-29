"""Draws Patrone's cursor theme: every cursor in Patrone's blues with a white
edge, and the busy cursors with the boot splash's spinner.

    python3 tools/make_cursors.py build/cursors --preview system/cursors/previews

Writes, in OUTPUT:
  index.theme     names the theme, Patrone, and falls back to Adwaita
  cursor.theme    the system's default cursor theme (x-cursor-theme) points
                  here; it says only "use Patrone"
  cursors/        an Xcursor file for each shape, at 24, 30, 32, 36, 48, 64,
                  72 and 96 pixels, and links for the other names programs
                  ask for (ALIASES)

24 pixels is GNOME's cursor size; the others are what it asks for on HiDPI
screens and at fractional scales. A name the theme lacks falls back to
Adwaita's.

Each shape is drawn from geometry on a 24-unit grid (one unit is a pixel at
24), at four times the size and scaled down, which smooths the edges: a body
shaded from Patrone Blue's mid tone down to its shadow, a white edge, and a
soft shadow under it. Where parts of a shape overlap, the fingers of a hand
for one, a thin white seam shows where one ends. Badges (copy, help, link,
menu, no drop) are discs in Patrone Blue.

The busy cursors, "wait" (the spinner alone) and "progress" (the arrow with
a small one), are the boot splash's spinner on Patrone's near-black: a dim
ring with a comet of light running round it, its head sprinting ahead and
its tail catching up (make_spinner.py). They loop in 1.5 seconds, 60
frames of 25 milliseconds.

Needs NumPy; --preview also needs Pillow. With only --preview, it writes the
previews and no theme: cursors-sheet.png (every cursor at 24 and 48 pixels,
on light and dark, with its hotspot) and busy-preview.gif.
"""
import argparse
import math
import os
import struct
from pathlib import Path

import numpy as np

SIZES = (24, 30, 32, 36, 48, 64, 72, 96)
GRID = 24          # the design grid, in units
SUPERSAMPLE = 4    # drawn this many times larger, then scaled down

# Patrone's palette (branding/README.md)
BACKGROUND = (0x03, 0x03, 0x03)
HIGHLIGHT = (0x4D, 0xA0, 0xFC)
PATRONE_BLUE = (0x28, 0x8B, 0xFA)
MID = (0x23, 0x72, 0xDD)
DEEP = (0x18, 0x51, 0xA8)
SHADOW = (0x13, 0x43, 0x94)
WHITE = (0xFF, 0xFF, 0xFF)

BODY_TOP = MID         # the body's shading, top of the canvas to bottom
BODY_BOTTOM = SHADOW
BADGE = PATRONE_BLUE
EDGE = 1.5             # the white edge, in units
SEAM = 0.7             # the white line where overlapping parts meet
DROP_OFFSET = 0.8      # the shadow, down and to the right, in units
DROP_BLUR = 0.8
DROP_OPACITY = 0.4

# The busy cursors' spinner: the boot splash's comet (make_spinner.py)
LOOP_FRAMES = 60
FRAME_MS = 25
LAG = 0.34
EASE_LINEAR = 0.22
REST_ANGLE = math.radians(-90)
COMET_STOPS = [
    (0.00, tuple(h + (w - h) * 0.35 for h, w in zip(HIGHLIGHT, WHITE))),
    (0.08, HIGHLIGHT),
    (0.35, PATRONE_BLUE),
    (1.00, MID),
]
SOLID = 0.3
TRACK_COLOR = SHADOW
TRACK_OPACITY = 0.4


# Distances, in units: negative inside a shape, positive outside

def circle(x, y, cx, cy, r):
    return np.hypot(x - cx, y - cy) - r


def ring(x, y, cx, cy, r, width):
    return np.abs(np.hypot(x - cx, y - cy) - r) - width / 2


def box(x, y, cx, cy, half_w, half_h, radius=0.0):
    qx = np.abs(x - cx) - half_w + radius
    qy = np.abs(y - cy) - half_h + radius
    outside = np.hypot(np.maximum(qx, 0), np.maximum(qy, 0))
    return outside + np.minimum(np.maximum(qx, qy), 0) - radius


def segment(x, y, ax, ay, bx, by, r):
    """A capsule: the line from a to b, r thick on each side."""
    dx, dy = bx - ax, by - ay
    t = np.clip(((x - ax) * dx + (y - ay) * dy) / max(dx * dx + dy * dy, 1e-9), 0, 1)
    return np.hypot(x - ax - t * dx, y - ay - t * dy) - r


def polyline(x, y, points, r):
    return np.minimum.reduce([segment(x, y, *a, *b, r) for a, b in zip(points, points[1:])])


def arc(cx, cy, radius, start, end, steps=16):
    """Points along an arc, angles in degrees, clockwise from three o'clock."""
    return [(cx + radius * math.cos(math.radians(a)), cy + radius * math.sin(math.radians(a)))
            for a in np.linspace(start, end, steps)]


def polygon(x, y, points):
    points = np.asarray(points, dtype=float)
    distance = (x - points[0, 0]) ** 2 + (y - points[0, 1]) ** 2
    sign = np.ones_like(x)
    for i in range(len(points)):
        px, py = points[i]
        ex, ey = points[i - 1] - points[i]
        wx, wy = x - px, y - py
        t = np.clip((wx * ex + wy * ey) / (ex * ex + ey * ey), 0, 1)
        distance = np.minimum(distance, (wx - ex * t) ** 2 + (wy - ey * t) ** 2)
        above, below_previous = y >= py, y < points[i - 1, 1]
        left = ex * wy > ey * wx
        crosses = (above & below_previous & left) | (~above & ~below_previous & ~left)
        sign = np.where(crosses, -sign, sign)
    return sign * np.sqrt(distance)


def union(*distances):
    return np.minimum.reduce(distances)


def turned(x, y, degrees, cx=GRID / 2, cy=GRID / 2):
    """Coordinates under which a shape drawn upright comes out turned
    clockwise by degrees."""
    a = math.radians(-degrees)
    dx, dy = x - cx, y - cy
    return cx + dx * math.cos(a) - dy * math.sin(a), cy + dx * math.sin(a) + dy * math.cos(a)


def turned_point(point, degrees, cx=GRID / 2, cy=GRID / 2):
    a = math.radians(degrees)
    dx, dy = point[0] - cx, point[1] - cy
    return cx + dx * math.cos(a) - dy * math.sin(a), cy + dx * math.sin(a) + dy * math.cos(a)


class Part:
    """One piece of a cursor, painted in order. fill is "body" (the blue
    shading), an RGB color or an (RGB array, opacity array) pair. edge: it
    takes part in the white edge round the cursor. seam: a white line is
    drawn round it over the parts painted before it."""

    def __init__(self, distance, fill="body", edge=True, seam=False, opacity=1.0):
        self.distance, self.fill, self.edge, self.seam, self.opacity = distance, fill, edge, seam, opacity


# The shapes. Each takes the canvas's coordinates (x, y, in units) and the
# time t in loops (for the busy cursors) and returns its parts.

ARROW = [(4.5, 2.5), (4.5, 19.2), (8.7, 15.4), (11.4, 21.4), (14.0, 20.3), (11.4, 14.5), (16.8, 14.5)]
ARROW_TIP = (4.5, 2.5)


def arrow(x, y, t=None):
    return [Part(polygon(x, y, ARROW))]


def right_arrow_pointer(x, y, t=None):
    return arrow(GRID - x, y)


BADGE_AT = (17.4, 17.6)
BADGE_RADIUS = 4.9


def with_badge(glyph):
    """The arrow with a badge at its lower right, glyph drawn on it in
    white."""
    def shape(x, y, t=None):
        bx, by = BADGE_AT
        return arrow(x, y) + [Part(circle(x, y, bx, by, BADGE_RADIUS), fill=BADGE, seam=True),
                              Part(glyph(x, y, bx, by), fill=WHITE, edge=False)]
    return shape


def plus_glyph(x, y, bx, by):
    return union(box(x, y, bx, by, 2.7, 0.75, 0.35), box(x, y, bx, by, 0.75, 2.7, 0.35))


def question_glyph(x, y, bx, by):
    hook = arc(bx, by - 1.15, 1.65, 190, 400) + [(bx, by + 0.9)]
    return union(polyline(x, y, hook, 0.62), circle(x, y, bx, by + 2.65, 0.72))


def link_glyph(x, y, bx, by):
    # A shortcut's bent arrow: up from the lower left, then right
    stem = [(bx - 2.0, by + 2.8), (bx - 2.0, by + 0.7)] + arc(bx + 0.2, by + 0.7, 2.2, 180, 270, 8)
    head = polygon(x, y, [(bx + 3.1, by - 1.5), (bx + 0.5, by - 3.9), (bx + 0.5, by + 0.9)])
    return union(polyline(x, y, stem + [(bx + 0.9, by - 1.5)], 0.68), head)


def menu_glyph(x, y, bx, by):
    return union(*[box(x, y, bx, by + dy, 2.5, 0.55, 0.3) for dy in (-1.9, 0, 1.9)])


def no_glyph(x, y, bx, by):
    return union(ring(x, y, bx, by, 2.35, 1.05), segment(x, y, bx - 1.6, by - 1.6, bx + 1.6, by + 1.6, 0.52))


def pointing_hand(x, y, t=None):
    return [
        Part(union(box(x, y, 13.7, 16.2, 5.4, 4.7, 2.6), box(x, y, 13.9, 19.5, 4.4, 2.2, 1.2))),
        Part(segment(x, y, 18.9, 13.4, 18.9, 15.6, 1.45), seam=True),
        Part(segment(x, y, 16.2, 12.0, 16.2, 14.6, 1.55), seam=True),
        Part(segment(x, y, 13.2, 11.3, 13.2, 14.2, 1.6), seam=True),
        Part(segment(x, y, 8.6, 17.4, 5.3, 13.8, 1.6), seam=True),
        Part(segment(x, y, 9.8, 3.6, 9.8, 14.2, 1.75), seam=True),
    ]


def open_hand(x, y, t=None):
    return [
        Part(union(box(x, y, 13.5, 16.4, 5.3, 4.5, 2.8), box(x, y, 13.7, 19.6, 4.4, 2.1, 1.2))),
        Part(segment(x, y, 19.0, 8.8, 19.0, 15.0, 1.4), seam=True),
        Part(segment(x, y, 16.1, 5.8, 16.1, 13.6, 1.55), seam=True),
        Part(segment(x, y, 12.9, 5.0, 12.9, 13.6, 1.6), seam=True),
        Part(segment(x, y, 9.7, 6.4, 9.7, 14.0, 1.55), seam=True),
        Part(segment(x, y, 8.5, 17.6, 5.0, 13.2, 1.6), seam=True),
    ]


def closed_hand(x, y, t=None):
    return [
        Part(union(box(x, y, 13.6, 15.9, 5.5, 4.8, 2.8), box(x, y, 13.8, 19.4, 4.4, 2.2, 1.2))),
        Part(segment(x, y, 19.1, 11.6, 19.1, 13.4, 1.45), seam=True),
        Part(segment(x, y, 16.3, 10.4, 16.3, 12.8, 1.6), seam=True),
        Part(segment(x, y, 13.1, 10.0, 13.1, 12.6, 1.6), seam=True),
        Part(segment(x, y, 9.9, 10.6, 9.9, 13.2, 1.6), seam=True),
        Part(segment(x, y, 8.0, 16.6, 11.4, 14.7, 1.6), seam=True),
    ]


def text_beam(x, y, t=None):
    return [Part(union(box(x, y, 12, 12, 0.95, 7.2),
                       box(x, y, 12, 4.9, 3.3, 0.95, 0.9),
                       box(x, y, 12, 19.1, 3.3, 0.95, 0.9)))]


def vertical_text_beam(x, y, t=None):
    return text_beam(*turned(x, y, 90))


def crosshair(x, y, t=None):
    return [Part(union(box(x, y, 12, 5.6, 0.85, 2.6, 0.4), box(x, y, 12, 18.4, 0.85, 2.6, 0.4),
                       box(x, y, 5.6, 12, 2.6, 0.85, 0.4), box(x, y, 18.4, 12, 2.6, 0.85, 0.4))),
            Part(circle(x, y, 12, 12, 1.15))]


def cell(x, y, t=None):
    return [Part(union(box(x, y, 12, 12, 2.1, 7.4, 0.6), box(x, y, 12, 12, 7.4, 2.1, 0.6)))]


def head_up(x, y, tip, half_width, length):
    return polygon(x, y, [(12, tip), (12 + half_width, tip + length), (12 - half_width, tip + length)])


def arrow_up(x, y, tip, tail, half_width=4.2, length=4.6, shaft=1.05):
    """An arrow up the middle of the grid, from tail (y) to tip (y)."""
    return union(head_up(x, y, tip, half_width, length),
                 box(x, y, 12, (tip + length - 0.3 + tail) / 2, shaft, (tail - tip - length + 0.3) / 2))


def move(x, y, t=None):
    heads = [head_up(*turned(x, y, a), 2.3, 3.7, 4.0) for a in (0, 90, 180, 270)]
    return [Part(union(box(x, y, 12, 12, 1.0, 6.4), box(x, y, 12, 12, 6.4, 1.0), *heads))]


def all_scroll(x, y, t=None):
    heads = [Part(head_up(*turned(x, y, a), 2.9, 3.4, 3.6)) for a in (0, 90, 180, 270)]
    return heads + [Part(circle(x, y, 12, 12, 2.1))]


def double_arrow(degrees):
    def shape(x, y, t=None):
        x, y = turned(x, y, degrees)
        return [Part(union(head_up(x, y, 2.6, 4.4, 4.8), head_up(*turned(x, y, 180), 2.6, 4.4, 4.8),
                           box(x, y, 12, 12, 1.05, 5.4)))]
    return shape


def split_arrows(degrees):
    """col-resize (0) and row-resize (90): arrows out from a bar."""
    def shape(x, y, t=None):
        x, y = turned(x, y, degrees)
        return [Part(box(x, y, 12, 12, 1.0, 7.8, 0.5)),
                Part(arrow_up(*turned(x, y, 90), 2.4, 9.6, 3.8, 4.0, 0.95)),
                Part(arrow_up(*turned(x, y, -90), 2.4, 9.6, 3.8, 4.0, 0.95))]
    return shape


EDGE_BAR_Y = 3.4


def edge_arrow(degrees):
    """n-resize (0) and the other sides: an arrow out to an edge."""
    def shape(x, y, t=None):
        x, y = turned(x, y, degrees)
        return [Part(box(x, y, 12, EDGE_BAR_Y, 6.6, 1.0, 0.5)), Part(arrow_up(x, y, 6.4, 19.4))]
    return shape


CORNER = 7.6  # how far the corner is right of and above the middle


def corner_arrow(mirror_x, mirror_y):
    """ne-resize, mirrored for the other corners: an arrow out to a corner."""
    def shape(x, y, t=None):
        if mirror_x:
            x = GRID - x
        if mirror_y:
            y = GRID - y
        cx, cy = 12 + CORNER, 12 - CORNER
        bracket = polyline(x, y, [(cx - 7.0, cy), (cx, cy), (cx, cy + 7.0)], 1.0)
        return [Part(bracket), Part(arrow_up(*turned(x, y, 45), 4.9, 19.2))]
    return shape


def single_arrow(degrees):
    def shape(x, y, t=None):
        return [Part(arrow_up(*turned(x, y, degrees), 2.8, 21.2))]
    return shape


def not_allowed(x, y, t=None):
    return [Part(union(ring(x, y, 12, 12, 6.3, 2.6), segment(x, y, 8.0, 8.0, 16.0, 16.0, 1.3)))]


def x_cursor(x, y, t=None):
    return [Part(union(segment(x, y, 6.2, 6.2, 17.8, 17.8, 1.55), segment(x, y, 17.8, 6.2, 6.2, 17.8, 1.55)))]


def zoom(sign):
    def shape(x, y, t=None):
        lens = Part(union(circle(x, y, 10, 10, 6.9), segment(x, y, 14.6, 14.6, 20.0, 20.0, 1.75)))
        glass = Part(circle(x, y, 10, 10, 4.5), fill=WHITE, edge=False)
        glyph = box(x, y, 10, 10, 2.6, 0.78, 0.3)
        if sign == "+":
            glyph = union(glyph, box(x, y, 10, 10, 0.78, 2.6, 0.3))
        return [lens, glass, Part(glyph, edge=False)]
    return shape


TOOL_TIP = (3.8, 20.2)


def along_tool(x, y):
    """Coordinates along a tool held at 45 degrees with its tip at TOOL_TIP:
    s from the tip up and to the right, w across."""
    dx, dy = x - TOOL_TIP[0], y - TOOL_TIP[1]
    return (dx - dy) / math.sqrt(2), (dx + dy) / math.sqrt(2)


def pencil(x, y, t=None):
    s, w = along_tool(x, y)
    return [Part(box(s, w, 10.4, 0, 4.6, 2.2)),
            Part(box(s, w, 17.0, 0, 1.9, 2.2, 0.8), seam=True),
            Part(polygon(s, w, [(0, 0), (5.6, 2.2), (5.6, -2.2)]), fill=HIGHLIGHT, seam=True),
            Part(polygon(s, w, [(0, 0), (2.0, 0.8), (2.0, -0.8)]), edge=False)]


def color_picker(x, y, t=None):
    s, w = along_tool(x, y)
    return [Part(segment(s, w, 0.6, 0, 9.0, 0, 1.0)),
            Part(box(s, w, 10.0, 0, 0.8, 2.5, 0.3), seam=True),
            Part(segment(s, w, 13.0, 0, 16.0, 0, 2.3), seam=True)]


def dotbox(x, y, t=None):
    frame = np.abs(box(x, y, 12, 12, 6.6, 6.6, 1.0)) - 0.95
    return [Part(frame), Part(circle(x, y, 12, 12, 1.6))]


def eased(x):
    """The comet's easing (make_spinner.py): turns made by time x, in loops."""
    whole = np.floor(x)
    u = x - whole
    return whole + EASE_LINEAR * u + (1 - EASE_LINEAR) * (1 - np.cos(np.pi * u)) / 2


def smoothstep(x):
    x = np.clip(x, 0, 1)
    return x * x * (3 - 2 * x)


def comet_color(position):
    stops = [s for s, _ in COMET_STOPS]
    return np.stack([np.interp(position, stops, [c[i] for _, c in COMET_STOPS]) for i in range(3)], -1)


def spinner(x, y, t, cx, cy, radius, band, stroke):
    """The boot splash's spinner on a ring of Patrone's near-black, band
    wide: a dim track and the comet, its head at the top when t is a whole
    loop."""
    head = REST_ANGLE + 2 * math.pi * float(eased(t))
    length = 2 * math.pi * float(eased(t) - eased(t - LAG))
    track = ring(x, y, cx, cy, radius, stroke)
    behind = (head - np.arctan2(y - cy, x - cx)) % (2 * math.pi)
    position = np.clip(behind / max(length, 1e-6), 0, 1)
    opacity = np.where(behind <= length, 1 - smoothstep((position - SOLID) / (1 - SOLID)), 0.0)
    hx, hy = cx + radius * math.cos(head), cy + radius * math.sin(head)
    cap = circle(x, y, hx, hy, stroke / 2)
    return [Part(ring(x, y, cx, cy, radius, band), fill=BACKGROUND),
            Part(track, fill=TRACK_COLOR, edge=False, opacity=TRACK_OPACITY),
            Part(track, fill=(comet_color(position), opacity), edge=False),
            Part(cap, fill=comet_color(0.0)[None, None], edge=False)]


def wait(x, y, t):
    return spinner(x, y, t, 12, 12, 6.2, 4.2, 2.7)


def progress(x, y, t):
    return arrow(x, y) + spinner(x, y, t, 16.9, 17.0, 3.8, 3.3, 2.1)


# name: (shape, hotspot in units, animated)
SHAPES = {
    "default": (arrow, ARROW_TIP, False),
    "right_ptr": (right_arrow_pointer, (GRID - ARROW_TIP[0], ARROW_TIP[1]), False),
    "pointer": (pointing_hand, (9.8, 2.2), False),
    "grab": (open_hand, (12.9, 12.0), False),
    "grabbing": (closed_hand, (13.4, 13.6), False),
    "text": (text_beam, (12, 12), False),
    "vertical-text": (vertical_text_beam, (12, 12), False),
    "crosshair": (crosshair, (12, 12), False),
    "cell": (cell, (12, 12), False),
    "move": (move, (12, 12), False),
    "all-scroll": (all_scroll, (12, 12), False),
    "ns-resize": (double_arrow(0), (12, 12), False),
    "ew-resize": (double_arrow(90), (12, 12), False),
    "nesw-resize": (double_arrow(45), (12, 12), False),
    "nwse-resize": (double_arrow(-45), (12, 12), False),
    "col-resize": (split_arrows(0), (12, 12), False),
    "row-resize": (split_arrows(90), (12, 12), False),
    "n-resize": (edge_arrow(0), turned_point((12, EDGE_BAR_Y), 0), False),
    "e-resize": (edge_arrow(90), turned_point((12, EDGE_BAR_Y), 90), False),
    "s-resize": (edge_arrow(180), turned_point((12, EDGE_BAR_Y), 180), False),
    "w-resize": (edge_arrow(270), turned_point((12, EDGE_BAR_Y), 270), False),
    "ne-resize": (corner_arrow(False, False), (12 + CORNER, 12 - CORNER), False),
    "nw-resize": (corner_arrow(True, False), (12 - CORNER, 12 - CORNER), False),
    "se-resize": (corner_arrow(False, True), (12 + CORNER, 12 + CORNER), False),
    "sw-resize": (corner_arrow(True, True), (12 - CORNER, 12 + CORNER), False),
    "up-arrow": (single_arrow(0), turned_point((12, 2.8), 0), False),
    "right-arrow": (single_arrow(90), turned_point((12, 2.8), 90), False),
    "down-arrow": (single_arrow(180), turned_point((12, 2.8), 180), False),
    "left-arrow": (single_arrow(270), turned_point((12, 2.8), 270), False),
    "not-allowed": (not_allowed, (12, 12), False),
    "no-drop": (with_badge(no_glyph), ARROW_TIP, False),
    "copy": (with_badge(plus_glyph), ARROW_TIP, False),
    "alias": (with_badge(link_glyph), ARROW_TIP, False),
    "context-menu": (with_badge(menu_glyph), ARROW_TIP, False),
    "help": (with_badge(question_glyph), ARROW_TIP, False),
    "zoom-in": (zoom("+"), (10, 10), False),
    "zoom-out": (zoom("-"), (10, 10), False),
    "progress": (progress, ARROW_TIP, True),
    "wait": (wait, (12, 12), True),
    "X_cursor": (x_cursor, (12, 12), False),
    "pencil": (pencil, TOOL_TIP, False),
    "color-picker": (color_picker, TOOL_TIP, False),
    "dotbox": (dotbox, (12, 12), False),
}

# The other names programs ask for, as links: X11's, GTK's and Qt's, the
# hashed names of Qt, Chromium and Mozilla, and every name Yaru has
ALIASES = {
    "default": "arrow left_ptr top_left_arrow left_arrow dnd-move wayland-cursor wayland_cursor",
    "right_ptr": "draft_large draft_small",
    "pointer": "hand hand2 pointing_hand e29285e634086352946a0e7090d73106 9d800788f1b08800ae810202380a0822",
    "grab": "openhand hand1 5aca4d189052212118709018842178c0",
    "grabbing": "closedhand dnd-none 208530c400c041818281048008011002",
    "text": "xterm ibeam",
    "crosshair": "cross tcross cross_reverse diamond_cross",
    "cell": "plus",
    "move": "fleur size_all pointer-move all-resize 4498f0e0c1937ffe01fd06f973665830 "
            "9081237383d90e509aa00f00170e968f fcf21c00b30f7e3f83fe0dfd12e71cff",
    "ns-resize": "v_double_arrow sb_v_double_arrow double_arrow size_ver size-ver "
                 "00008160000006810000408080010102",
    "ew-resize": "h_double_arrow sb_h_double_arrow size_hor size-hor 028006030e0e7ebffc7f7070c0600140",
    "nesw-resize": "fd_double_arrow size_bdiag size-bdiag fcf1c3c7cd4491d801f1e1c78f100000 "
                   "50585d75b494802d0151028115016902",
    "nwse-resize": "bd_double_arrow size_fdiag size-fdiag c7088f0f3e6c8088236ef8e1e3e70000 "
                   "38c5dff7c7b8962045400281044508d2",
    "col-resize": "split_h 14fef782d02440884392942c11205230 043a9f68147c53184671403ffa811cc5",
    "row-resize": "split_v 2870a09082c103050810ffdffffe0204 c07385c7190e701020ff7ffffd08103c",
    "n-resize": "top_side top_tee",
    "s-resize": "bottom_side bottom_tee",
    "e-resize": "right_side right_tee",
    "w-resize": "left_side left_tee",
    "ne-resize": "top_right_corner ur_angle",
    "nw-resize": "top_left_corner ul_angle",
    "se-resize": "bottom_right_corner lr_angle",
    "sw-resize": "bottom_left_corner ll_angle",
    "up-arrow": "sb_up_arrow up_arrow",
    "down-arrow": "sb_down_arrow down_arrow",
    "left-arrow": "sb_left_arrow",
    "right-arrow": "sb_right_arrow",
    "not-allowed": "crossed_circle forbidden circle 03b6e0fcb3499374a867c041f52298f0",
    "no-drop": "dnd-no-drop",
    "copy": "dnd-copy 1081e37283d90000800003c07f3ef6bf 6407b0e94181790501fd1e167b474872 "
            "b66166c04f8c3109214a4fbd64a50fc8",
    "alias": "link dnd-link 3085a0e285430894940527032f8b26df 640fb0e74195791501fd1ed57b41487f "
             "a2a266d0498c3104214a47bd64ab0fc8",
    "context-menu": "dnd-ask",
    "help": "question_arrow whats_this left_ptr_help 5c6cd98b3f3ebcb1f9c7f1c204630408 "
            "d9ce0ab605698f320427677b458ad60b",
    "progress": "half-busy left_ptr_watch 00000000000000020006000e7e9ffc3f "
                "08e8e1c95fe2fc01f976f1e063a24ccd 3ecb610c1bf2410f44200f48c40d3599",
    "wait": "watch 0426c94ea35c87780ff01dc239897213",
    "X_cursor": "x-cursor pirate",
    "pencil": "draft",
    "dotbox": "dot_box_mask draped_box icon target",
}


class Canvas:
    def __init__(self, size):
        self.size = size
        self.unit = size / GRID  # pixels to a unit
        n = size * SUPERSAMPLE
        coords = (np.arange(n) + 0.5) / (SUPERSAMPLE * self.unit)
        self.x, self.y = np.meshgrid(coords, coords)
        shade = np.clip(self.y / GRID, 0, 1)[..., None]
        self.body = (np.array(BODY_TOP) * (1 - shade) + np.array(BODY_BOTTOM) * shade) / 255

    def coverage(self, distance):
        return np.clip(0.5 - distance * self.unit * SUPERSAMPLE, 0, 1)

    def shrink(self, image):
        n = self.size
        return image.reshape(n, SUPERSAMPLE, n, SUPERSAMPLE, *image.shape[2:]).mean(axis=(1, 3))

    def render(self, parts):
        """Paints the parts; returns premultiplied RGBA, 0 to 1, size x size."""
        color = np.zeros(self.x.shape + (3,))
        alpha = np.zeros(self.x.shape)

        def paint(distance, fill, opacity=1.0):
            nonlocal color, alpha
            if isinstance(fill, str):
                rgb = self.body
            elif isinstance(fill, tuple) and len(fill) == 2:
                rgb, extra = fill
                rgb = np.asarray(rgb) / 255
                opacity = opacity * extra
            else:
                rgb = np.asarray(fill, dtype=float) / 255
            a = self.coverage(distance) * opacity
            color = rgb * a[..., None] + color * (1 - a[..., None])
            alpha = a + alpha * (1 - a)

        outline = union(*[p.distance for p in parts if p.edge]) - EDGE
        paint(outline, WHITE)
        for part in parts:
            if part.seam:
                paint(part.distance - SEAM, WHITE)
            paint(part.distance, part.fill, part.opacity)

        color, alpha = self.shrink(color), self.shrink(alpha)
        drop = self.drop_shadow(self.shrink(self.coverage(outline)))
        alpha = alpha + drop * (1 - alpha)
        return np.dstack([color, alpha])

    def drop_shadow(self, silhouette):
        offset = DROP_OFFSET * self.unit
        sigma = DROP_BLUR * self.unit
        radius = int(math.ceil(3 * sigma + offset))
        taps = np.arange(-radius, radius + 1)
        kernel = np.exp(-0.5 * ((taps - offset) / sigma) ** 2)
        kernel /= kernel.sum()
        padded = np.pad(silhouette, radius)
        rows = np.apply_along_axis(lambda r: np.convolve(r, kernel, mode="same"), 1, padded)
        both = np.apply_along_axis(lambda c: np.convolve(c, kernel, mode="same"), 0, rows)
        return both[radius:-radius, radius:-radius] * DROP_OPACITY


def draw(name, size):
    """Every frame of one cursor at one size: [(rgba, xhot, yhot, delay)]."""
    shape, (hx, hy), animated = SHAPES[name]
    canvas = Canvas(size)
    hot = (min(size - 1, int(hx * canvas.unit)), min(size - 1, int(hy * canvas.unit)))
    if not animated:
        return [(canvas.render(shape(canvas.x, canvas.y)), *hot, 0)]
    return [(canvas.render(shape(canvas.x, canvas.y, n / LOOP_FRAMES)), *hot, FRAME_MS)
            for n in range(LOOP_FRAMES)]


def write_xcursor(path, images):
    """images: [(nominal size, premultiplied RGBA, xhot, yhot, delay)], the
    frames of each size in order."""
    count = len(images)
    position = 16 + 12 * count
    toc, chunks = [], []
    for nominal, rgba, xhot, yhot, delay in images:
        height, width = rgba.shape[:2]
        a = np.clip(np.round(rgba[..., 3] * 255), 0, 255).astype(np.uint32)
        rgb = np.minimum(np.clip(np.round(rgba[..., :3] * 255), 0, 255).astype(np.uint32), a[..., None])
        argb = (a << 24) | (rgb[..., 0] << 16) | (rgb[..., 1] << 8) | rgb[..., 2]
        chunk = struct.pack("<9I", 36, 0xFFFD0002, nominal, 1, width, height, xhot, yhot, delay)
        chunk += argb.astype("<u4").tobytes()
        toc.append(struct.pack("<III", 0xFFFD0002, nominal, position))
        chunks.append(chunk)
        position += len(chunk)
    path.write_bytes(struct.pack("<4sIII", b"Xcur", 16, 0x10000, count) + b"".join(toc) + b"".join(chunks))


def write_theme(folder: Path):
    cursors = folder / "cursors"
    cursors.mkdir(parents=True, exist_ok=True)
    for old in cursors.iterdir():
        old.unlink()
    (folder / "index.theme").write_text(
        "[Icon Theme]\nName=Patrone\nComment=PatroneOS's cursors\nInherits=Adwaita\n")
    (folder / "cursor.theme").write_text("[Icon Theme]\nInherits=Patrone\n")
    for name in SHAPES:
        images = [(size, *frame) for size in SIZES for frame in draw(name, size)]
        write_xcursor(cursors / name, images)
    links = 0
    for target, names in ALIASES.items():
        assert target in SHAPES, target
        for name in names.split():
            os.symlink(target, cursors / name)
            links += 1
    print(f"{folder}: {len(SHAPES)} cursors at {', '.join(map(str, SIZES))} pixels, {links} links")


def write_previews(folder: Path):
    from PIL import Image, ImageDraw, ImageFont

    folder.mkdir(parents=True, exist_ok=True)
    try:
        font = ImageFont.truetype("DejaVuSans.ttf", 12)
    except OSError:
        font = ImageFont.load_default()
    light, dark = (0xF2, 0xF2, 0xF2), (0x24, 0x24, 0x28)

    def flat(rgba, background):
        color = rgba[..., :3] + (1 - rgba[..., 3:]) * np.array(background) / 255
        return Image.fromarray((np.clip(color, 0, 1) * 255).round().astype(np.uint8), "RGB")

    # Every cursor at 24 (shown twice the size) and 48 pixels, on light and
    # dark, its hotspot a red dot
    columns = 4
    tile_w, tile_h, label_h = 4 * 56, 56, 16
    names = list(SHAPES)
    rows = math.ceil(len(names) / columns)
    sheet = Image.new("RGB", (columns * tile_w, rows * (tile_h + label_h)), (0x80, 0x80, 0x80))
    pen = ImageDraw.Draw(sheet)
    for i, name in enumerate(names):
        x0, y0 = (i % columns) * tile_w, (i // columns) * (tile_h + label_h)
        pen.text((x0 + 4, y0 + 2), name, fill=(0, 0, 0), font=font)
        frames = {size: draw(name, size) for size in (24, 48)}
        for j, (size, background) in enumerate([(24, light), (24, dark), (48, light), (48, dark)]):
            rgba, xhot, yhot, _ = frames[size][len(frames[size]) // 4]
            zoom = 2 if size == 24 else 1
            tile = Image.new("RGB", (56, 56), background)
            tile.paste(flat(rgba, background).resize((size * zoom, size * zoom), Image.NEAREST), (4, 4))
            ImageDraw.Draw(tile).rectangle([4 + xhot * zoom, 4 + yhot * zoom,
                                            4 + xhot * zoom + zoom - 1, 4 + yhot * zoom + zoom - 1],
                                           fill=(0xE0, 0x20, 0x20))
            sheet.paste(tile, (x0 + j * 56, y0 + label_h))
    sheet.save(folder / "cursors-sheet.png", optimize=True)

    # The busy cursors at 48 pixels, shown twice the size, on light and dark.
    # GIF frame times are in hundredths of a second: 20 and 30 alternate to
    # average 25 milliseconds.
    size, zoom = 48, 2
    frames = {name: draw(name, size) for name in ("wait", "progress")}
    gif = []
    for n in range(LOOP_FRAMES):
        frame = Image.new("RGB", (4 * size * zoom, size * zoom))
        for j, (name, background) in enumerate([("wait", light), ("wait", dark),
                                                 ("progress", light), ("progress", dark)]):
            image = flat(frames[name][n][0], background).resize((size * zoom, size * zoom), Image.LANCZOS)
            frame.paste(image, (j * size * zoom, 0))
        gif.append(frame)
    palette = gif[LOOP_FRAMES // 4].quantize(255, method=Image.Quantize.MEDIANCUT)
    gif = [f.quantize(palette=palette, dither=Image.Dither.NONE) for f in gif]
    gif[0].save(folder / "busy-preview.gif", save_all=True, append_images=gif[1:],
                duration=[20 if n % 2 else 30 for n in range(LOOP_FRAMES)], loop=0, optimize=True)
    print(f"{folder}: cursors-sheet.png, busy-preview.gif")


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("output", type=Path, nargs="?", help="folder for the theme")
    parser.add_argument("--preview", type=Path, help="folder for a contact sheet and a GIF")
    args = parser.parse_args()
    if not args.output and not args.preview:
        parser.error("give a folder for the theme, --preview, or both")
    if args.output:
        write_theme(args.output)
    if args.preview:
        write_previews(args.preview)


if __name__ == "__main__":
    main()
