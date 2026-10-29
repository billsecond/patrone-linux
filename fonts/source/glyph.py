"""Glyphs: what a letter's drawing gives, and the masters' numbers.

A glyph function gets a Draw for one master and draws with its numbers:
strokes (engine.Stroke), filled shapes, components and anchors. Draw then
outlines the strokes and spaces the glyph: its sides' sidebearings come
from what they are (a straight stem, a round, a diagonal), as multiples of
the master's own, so spacing follows the weight.
"""

from engine import Nib, Stroke, bounds, ellipse, transform

GLYPHS = {}  # name: (unicodes, upper, function), in drawing order


def glyph(name, *unicodes, upper=False):
    """register a glyph's drawing; upper ones use the capitals' pen and
    sidebearings"""

    def register(f):
        GLYPHS[name] = (list(unicodes), upper, f)
        return f

    return register


# ---------------------------------------------------------------- masters


class Master:
    """One master's numbers. Lowercase: stem (an upright stroke's weight),
    hair (a horizontal stroke's), sb (a straight side's sidebearing);
    capitals the same with u. counter scales counters (narrower in heavy
    weights, where the strokes take the room), join how thin a stroke gets
    where it leaves a stem."""

    # the same in every weight
    X = 530  # x-height
    C = 710  # capital height
    A = 760  # ascender
    D = -230  # descender
    UPM = 1000

    def __init__(self, name, wght, stem, hair, ustem, uhair, sb, usb, counter, join, ov):
        self.name, self.wght = name, wght
        self.stem, self.hair, self.ustem, self.uhair = stem, hair, ustem, uhair
        self.sb, self.usb = sb, usb
        self.counter, self.join, self.ov = counter, join, ov
        self.uov = ov + 2
        self.a, self.b = stem / 2, hair / 2
        self.ua, self.ub = ustem / 2, uhair / 2
        self.nib = Nib(self.a, self.b)
        self.unib = Nib(self.ua, self.ub)
        # how much heavier a round's sides are than a stem
        self.rnd = 1.04
        # the fullness of round curves (0.552 is a circle's)
        self.k = 0.58

    def cw(self, regular):
        """a counter's width: its regular width, for this weight"""
        return regular * self.counter

    def t(self, light, regular, black):
        """a number that depends on the weight, given at the three masters"""
        return {300: light, 400: regular, 900: black}[self.wght]


MASTERS = [
    Master("Light", 300, stem=56, hair=50, ustem=60, uhair=52, sb=70, usb=84,
           counter=1.05, join=0.68, ov=11),
    Master("Regular", 400, stem=84, hair=72, ustem=90, uhair=76, sb=66, usb=80,
           counter=1.0, join=0.62, ov=12),
    Master("Black", 900, stem=180, hair=138, ustem=190, uhair=146, sb=50, usb=62,
           counter=0.80, join=0.52, ov=14),
]


# ---------------------------------------------------------------- drawing


class Draw:
    def __init__(self, master, name, upper=False):
        self.P = master
        self.name = name
        self.upper = upper
        self.strokes = []
        self.contours = []
        self.components = []  # (glyph name, dx, dy)
        self.anchors = {}
        self.sides = (1.0, 1.0)
        self.advance = None  # a fixed advance (figures, spaces)
        self.centre = 0  # where an outline sits in a fixed advance, from its middle
        self.shift = 0  # where the outline lands, when spaced

    @property
    def nib(self):
        return self.P.unib if self.upper else self.P.nib

    def stroke(self, p, s=1.0, nib=None, cap=None, tilt=None):
        st = Stroke(p, nib or self.nib, s, cap, tilt)
        st.key = (self.name, len(self.strokes))
        self.strokes.append(st)
        return st

    def dot(self, cx, cy, r):
        self.contours.append(ellipse(cx, cy, r, r))

    def shape(self, contour):
        self.contours.append(contour)

    def component(self, name, dx=0, dy=0):
        self.components.append((name, dx, dy))

    def anchor(self, name, x, y):
        self.anchors[name] = (x, y)

    def space(self, left, right):
        """sidebearings, as multiples of the master's straight one"""
        self.sides = (left, right)

    def fixed(self, advance, centre=0):
        """a fixed advance (tabular figures), the outline in its middle"""
        self.advance = advance
        self.centre = centre

    def draw(self, name, master=None, dx=0, dy=0, upper=None):
        """draw another glyph's drawing in this one, moved by dx, dy (its
        own spacing ignored: it lands where its function draws it)"""
        unicodes, up, f = GLYPHS[name]
        sub = Draw(master or self.P, f"{self.name}/{name}", up if upper is None else upper)
        f(sub)
        k0 = len(self.strokes)
        for st in sub.strokes:
            st.key = (self.name, len(self.strokes))
            self.strokes.append(st)
        self.contours.extend(sub.contours)
        moved = lambda p: (p[0] + dx, p[1] + dy)  # noqa: E731
        self.transform_all(moved, start=k0, contours_from=len(self.contours) - len(sub.contours))
        return sub

    def transform_all(self, f, start=0, contours_from=0):
        """move or turn what's drawn (from stroke number start on)"""
        for st in self.strokes[start:]:
            st.points = [f(p) for p in st.points]
            st.segs = [(seg[0], *(f(p) for p in seg[1:])) for seg in st.segs]
        self.contours[contours_from:] = [transform(c, f) for c in self.contours[contours_from:]]

    def copy_of(self, name, dx=0, dy=0):
        """a glyph that is another one (a component), with its width"""
        self.component(name, dx, dy)
        self.advance = self.glyphs[name][3]

    # ---- the finished glyph

    def finish(self):
        """outline, space and return (contours, advance, anchors)"""
        contours = list(self.contours)
        for st in self.strokes:
            contours.extend(st.outline())
        box = bounds(contours)
        base = self.P.usb if self.upper else self.P.sb
        if box is None:
            return [], self.advance or 0, dict(self.anchors), 0
        x0, _, x1, _ = box
        if getattr(self, "keep_place", False):
            dx, width = 0, self.advance
        elif self.advance is not None:
            dx = (self.advance - (x1 - x0)) / 2 - x0 + self.centre
            width = self.advance
        else:
            dx = base * self.sides[0] - x0
            width = base * self.sides[0] + (x1 - x0) + base * self.sides[1]
        self.shift = dx
        contours = [transform(c, lambda p: (p[0] + dx, p[1])) for c in contours]
        anchors = {k: (x + dx, y) for k, (x, y) in self.anchors.items()}
        return contours, width, anchors, dx
