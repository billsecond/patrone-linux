"""Draws the boot splash's spinner: three interlinked rings, each with a
comet of light running round it, the three taking turns.

    python tools/make_spinner.py system/plymouth/spinner --preview system/plymouth/previews

Writes, for Plymouth's two-step module:
  throbber-0001.png ... throbber-0060.png    the spinner, a 2-second loop
  animation-0001.png ... animation-0030.png  the end animation

Plymouth plays the throbber as a loop that lasts 2 seconds whatever its frame
count, sampled 30 times a second, so 60 frames give each tick a new one. The
end animation plays once, one frame per tick, and holds its last frame.
Patrone's theme turns the end animation off, but two-step needs its frames:
without them it would take the throbber's frames for it and drop the spinner.
It carries on from the loop and lights the three rings in full.

The rings are Borromean, as in the knot: each passes over one of the others
and under the third at every crossing in turn, so no two are linked but the
three can't come apart. They're dim tracks with a gap in the lower ring where
another passes over. A comet runs clockwise round each: its head sprints
ahead and its tail catches up, so it stretches and shrinks, and each ring
does so a third of a loop after the one before, so the sprint goes round the
knot ring by ring. Every frame is drawn at four times its size and scaled
down, which smooths the edges.

--preview DIR also writes, on Patrone's near-black, spinner-sheet.png (every
frame), spinner-preview.gif (the loop, twice the size) and spinner-splash.png
(a mock-up of the whole splash at 1920x1080).
"""

import argparse
import math
from pathlib import Path

import numpy as np
from PIL import Image, ImageFilter

SIZE = 96          # each frame, in pixels
SCALE = 4          # drawn this many times larger, then scaled down
LOOP_FRAMES = 60   # one 2-second loop at 30 frames a second
END_FRAMES = 30

# The knot: three rings of RADIUS whose centers are SPREAD from the middle,
# the first at the top. Distances in pixels of the finished frame.
RADIUS = 25.5
SPREAD = 15.0
STROKE = 4.4       # width of each ring
GAP = 1.6          # the dark gap either side of a ring where it passes over

# The comets. Each ring's head and tail follow the same easing a fraction
# LAG of a loop apart; EASE_LINEAR of their motion is steady and the rest
# eases in and out, so the head sprints, then the tail.
LAG = 0.34
EASE_LINEAR = 0.22
# Where on each ring its head is slowest, measured clockwise from the point
# of the ring furthest from the middle of the knot.
REST_ANGLE = math.radians(-60)

# Patrone's palette (branding/README.md)
BACKGROUND = (0x03, 0x03, 0x03)
HIGHLIGHT = (0x4D, 0xA0, 0xFC)
PATRONE_BLUE = (0x28, 0x8B, 0xFA)
MID = (0x23, 0x72, 0xDD)
DEEP = (0x18, 0x51, 0xA8)
SHADOW = (0x13, 0x43, 0x94)
WHITE = (0xFF, 0xFF, 0xFF)

TRACK_COLOR = SHADOW
TRACK_OPACITY = 0.4
# The comet's color from head (0) to tail (1); it's solid to SOLID of the way
# back, then fades out.
COMET_STOPS = [
    (0.00, tuple(h + (w - h) * 0.35 for h, w in zip(HIGHLIGHT, WHITE))),
    (0.08, HIGHLIGHT),
    (0.35, PATRONE_BLUE),
    (1.00, MID),
]
SOLID = 0.3
GLOW_RADIUS = 2.5  # a soft glow around the comets, in pixels
GLOW_STRENGTH = 0.45

# The splash: where two-step puts the logo and the spinner (patrone.plymouth)
SCREEN = (1920, 1080)
LOGO_ALIGN = (0.5, 0.42)
SPINNER_ALIGN = (0.5, 0.72)


def ring_centers() -> list[tuple[float, float]]:
    """The rings' centers, top first, then clockwise, with the knot centered
    in the frame (it's taller above its middle than below)."""
    angles = [math.radians(a) for a in (-90, 30, 150)]
    centers = [(SPREAD * math.cos(a), SPREAD * math.sin(a)) for a in angles]
    top = min(y for _, y in centers) - RADIUS
    bottom = max(y for _, y in centers) + RADIUS
    shift = -(top + bottom) / 2
    return [(SIZE / 2 + x, SIZE / 2 + y + shift) for x, y in centers]


def crossings(centers):
    """Where the rings cross, as angles round each ring (clockwise from the
    right, as y points down), and which ring is on top at each, alternating
    over and under round every ring."""
    points = []  # (ring a, angle on a, ring b, angle on b)
    for a in range(3):
        for b in range(a + 1, 3):
            (ax, ay), (bx, by) = centers[a], centers[b]
            d = math.hypot(bx - ax, by - ay)
            along = d / 2
            across = math.sqrt(RADIUS**2 - along**2)
            mx, my = ax + (bx - ax) / 2, ay + (by - ay) / 2
            ux, uy = (bx - ax) / d, (by - ay) / d
            for side in (1, -1):
                px, py = mx - side * across * uy, my + side * across * ux
                points.append((a, math.atan2(py - ay, px - ax), b, math.atan2(py - by, px - bx)))

    # Round each ring its crossings alternate, so one choice per ring (is its
    # first crossing over or under?) decides them all; find the choices that
    # put exactly one ring on top at every crossing.
    order = []
    for ring in range(3):
        angles = [(p[1] if p[0] == ring else p[3]) % (2 * math.pi) for p in points if ring in (p[0], p[2])]
        order.append(sorted(angles))

    def over(ring, angle, first_over):
        index = order[ring].index(angle % (2 * math.pi))
        return (index % 2 == 0) == first_over

    for choice in range(8):
        firsts = [bool(choice >> ring & 1) for ring in range(3)]
        if all(over(a, ta, firsts[a]) != over(b, tb, firsts[b]) for a, ta, b, tb in points):
            break
    else:
        raise SystemExit("The rings can't be woven alternately; check RADIUS and SPREAD")
    return [[(angle, 1.0 if over(ring, angle, firsts[ring]) else -1.0) for angle in order[ring]] for ring in range(3)]


def height_round(ring_crossings, theta):
    """How high a ring is at each angle: +1 where it crosses over, -1 where it
    passes under, and a smooth wave between."""
    angles = [a for a, _ in ring_crossings]
    levels = [h for _, h in ring_crossings]
    theta = theta % (2 * math.pi)
    height = np.zeros_like(theta)
    for k in range(len(angles)):
        start, end = angles[k], angles[(k + 1) % len(angles)]
        span = (end - start) % (2 * math.pi)
        offset = (theta - start) % (2 * math.pi)
        inside = offset < span
        height = np.where(inside, levels[k] * np.cos(np.pi * offset / span), height)
    return height


def eased(x):
    """The comets' easing, unwrapped: turns made by time x (in loops), steady
    at EASE_LINEAR of full speed at each whole loop and fastest halfway."""
    whole = np.floor(x)
    u = x - whole
    return whole + EASE_LINEAR * u + (1 - EASE_LINEAR) * (1 - np.cos(np.pi * u)) / 2


def comet_color(position):
    """position: 0 at the head to 1 at the tail; returns RGB, 0 to 1."""
    stops = [s for s, _ in COMET_STOPS]
    channels = [np.interp(position, stops, [c[i] / 255 for _, c in COMET_STOPS]) for i in range(3)]
    return np.stack(channels, axis=-1)


class Spinner:
    def __init__(self):
        n = SIZE * SCALE
        coords = (np.arange(n) + 0.5) / SCALE
        self.x, self.y = np.meshgrid(coords, coords)
        self.centers = ring_centers()
        woven = crossings(self.centers)
        self.theta, self.track, self.halo, self.height, self.outward = [], [], [], [], []
        middle = (SIZE / 2, SIZE / 2)
        for ring, (cx, cy) in enumerate(self.centers):
            dx, dy = self.x - cx, self.y - cy
            off_line = np.abs(np.hypot(dx, dy) - RADIUS)
            theta = np.arctan2(dy, dx)
            self.theta.append(theta)
            self.track.append(self.coverage(STROKE / 2 - off_line))
            self.halo.append(self.coverage(STROKE / 2 + GAP - off_line))
            self.height.append(height_round(woven[ring], theta))
            self.outward.append(math.atan2(cy - middle[1], cx - middle[0]))

    @staticmethod
    def coverage(inside):
        """inside: how far a point is within a shape's edge, in pixels."""
        return np.clip(inside * SCALE + 0.5, 0.0, 1.0)

    def comet(self, ring, head, length):
        """The comet on a ring whose head is at angle head (radians, clockwise)
        and which trails length radians behind it: (RGB, opacity)."""
        behind = (head - self.theta[ring]) % (2 * np.pi)
        on = behind <= length
        position = np.clip(behind / max(length, 1e-6), 0, 1)
        opacity = np.where(on, 1 - smoothstep((position - SOLID) / (1 - SOLID)), 0.0) * self.track[ring]
        # A round cap on the head
        cx, cy = self.centers[ring]
        hx, hy = cx + RADIUS * math.cos(head), cy + RADIUS * math.sin(head)
        cap = self.coverage(STROKE / 2 - np.hypot(self.x - hx, self.y - hy))
        position = np.where(cap > opacity, 0.0, position)
        opacity = np.maximum(opacity, cap)
        return comet_color(position), opacity

    def draw(self, comets, track_opacity=TRACK_OPACITY, fill=(0.0, 0.0, 0.0)):
        """comets: (head, length) per ring. fill: how far each ring is lit in
        full, 0 to 1, for the end animation. Returns an RGBA image."""
        track_rgb = np.array(TRACK_COLOR) / 255
        full_rgb = np.array(PATRONE_BLUE) / 255
        layers, glows = [], []
        for ring, (head, length) in enumerate(comets):
            rgb, alpha = self.comet(ring, head, length)
            lit = fill[ring]
            if lit > 0:
                rgb = rgb * (1 - lit) + full_rgb * lit
                alpha = alpha * (1 - lit) + self.track[ring] * lit
            glows.append(rgb * alpha[..., None])
            under = self.track[ring] * track_opacity * (1 - alpha)
            premultiplied = rgb * alpha[..., None] + track_rgb * under[..., None]
            layers.append((premultiplied, alpha + under))

        # Stack the rings at each point in order of height, lowest first; each
        # one blanks out what's beneath it along its gap.
        order = np.argsort(np.stack(self.height), axis=0)
        color = np.zeros(self.x.shape + (3,))
        opacity = np.zeros(self.x.shape)
        glow = np.zeros(self.x.shape + (3,))
        for level in range(3):
            ring = order[level]
            pick = lambda per_ring: np.choose(ring, per_ring)
            halo = pick(self.halo)
            keep = 1 - halo
            color *= keep[..., None]
            opacity *= keep
            glow *= keep[..., None]
            layer_alpha = pick([a for _, a in layers])
            layer_rgb = np.stack([np.choose(ring, [l[0][..., c] for l in layers]) for c in range(3)], axis=-1)
            glow_rgb = np.stack([np.choose(ring, [g[..., c] for g in glows]) for c in range(3)], axis=-1)
            color = layer_rgb + color * (1 - layer_alpha[..., None])
            opacity = layer_alpha + opacity * (1 - layer_alpha)
            glow = glow_rgb + glow * (1 - layer_alpha[..., None])

        color = shrink(color)
        opacity = shrink(opacity)
        color, opacity = add_glow(color, opacity, shrink(glow))
        straight = color / np.maximum(opacity, 1e-6)[..., None]
        rgba = np.dstack([np.clip(straight, 0, 1), np.clip(opacity, 0, 1)])
        return Image.fromarray((rgba * 255).round().astype(np.uint8), "RGBA")

    def comets_at(self, t):
        """Each ring's comet at time t, in loops: (head, length) per ring."""
        comets = []
        for ring in range(3):
            u = t - ring / 3
            head = eased(u)
            tail = eased(u - LAG)
            start = self.outward[ring] + REST_ANGLE
            comets.append((start + 2 * np.pi * head, 2 * np.pi * (head - tail)))
        return comets


def shrink(image):
    """Averages each SCALE x SCALE block into one pixel."""
    n = image.shape[0] // SCALE
    return image.reshape(n, SCALE, n, SCALE, *image.shape[2:]).mean(axis=(1, 3))


def add_glow(color, opacity, emitted):
    """A blur of the comets, added as light: it shows on a dark background."""
    pixels = (np.clip(emitted, 0, 1) * 255).round().astype(np.uint8)
    blurred = Image.fromarray(pixels, "RGB").filter(ImageFilter.GaussianBlur(GLOW_RADIUS))
    light = np.asarray(blurred, dtype=float) / 255 * GLOW_STRENGTH
    color = color + light * (1 - opacity[..., None])
    opacity = np.maximum(opacity, color.max(axis=-1))
    return color, opacity


def smoothstep(x):
    x = np.clip(x, 0, 1)
    return x * x * (3 - 2 * x)


def loop_frames(spinner):
    return [spinner.draw(spinner.comets_at(n / LOOP_FRAMES)) for n in range(LOOP_FRAMES)]


def end_frames(spinner):
    """Carries on from the loop's last frame while the rings light up in full,
    in the order they sprint."""
    frames = []
    for n in range(END_FRAMES):
        t = 1 + n / LOOP_FRAMES
        progress = (n + 1) / END_FRAMES
        fill = tuple(float(smoothstep((progress - ring * 0.2) / 0.6)) for ring in range(3))
        frames.append(spinner.draw(spinner.comets_at(t), TRACK_OPACITY + (1 - TRACK_OPACITY) * progress, fill))
    return frames


def on_background(frame):
    flat = Image.new("RGB", frame.size, BACKGROUND)
    flat.paste(frame, mask=frame)
    return flat


def write_previews(loop, end, folder: Path, logo: Path):
    folder.mkdir(parents=True, exist_ok=True)

    # Every frame, the loop's six rows then the end animation's three
    columns = 10
    rows = math.ceil((len(loop) + len(end)) / columns)
    sheet = Image.new("RGB", (columns * SIZE, rows * SIZE), BACKGROUND)
    for i, frame in enumerate(loop + end):
        sheet.paste(on_background(frame), ((i % columns) * SIZE, (i // columns) * SIZE))
    sheet.save(folder / "spinner-sheet.png", optimize=True)

    # The loop at twice its size, every frame in one palette, which keeps the
    # file small. GIF frame times are in hundredths of a second: 30, 30, 40
    # milliseconds average a 30th of a second, so it lasts 2 seconds as at boot.
    zoom = 2
    big = [on_background(f).resize((SIZE * zoom, SIZE * zoom), Image.LANCZOS) for f in loop]
    tiles = Image.new("RGB", (columns * SIZE * zoom, math.ceil(len(big) / columns) * SIZE * zoom))
    for i, frame in enumerate(big):
        tiles.paste(frame, ((i % columns) * SIZE * zoom, (i // columns) * SIZE * zoom))
    palette = tiles.quantize(255, method=Image.Quantize.MEDIANCUT)
    gif = [frame.quantize(palette=palette, dither=Image.Dither.NONE) for frame in big]
    durations = [40 if n % 3 == 2 else 30 for n in range(len(gif))]
    gif[0].save(folder / "spinner-preview.gif", save_all=True, append_images=gif[1:],
                duration=durations, loop=0, optimize=True)

    screen = Image.new("RGB", SCREEN, BACKGROUND)
    splash = Image.open(logo).convert("RGBA")
    width, height = SCREEN
    x = round(width * LOGO_ALIGN[0] - splash.width * LOGO_ALIGN[0])
    y = round(height * LOGO_ALIGN[1] - splash.height * LOGO_ALIGN[1])
    screen.paste(splash, (x, y), splash)
    frame = loop[LOOP_FRAMES // 4]
    x = int(width * SPINNER_ALIGN[0] - SIZE / 2)
    y = int(height * SPINNER_ALIGN[1] - SIZE / 2)
    screen.paste(frame, (x, y), frame)
    screen.save(folder / "spinner-splash.png")


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("output", type=Path, help="folder for the frames")
    parser.add_argument("--preview", type=Path, help="folder for a contact sheet, GIF and mock-up")
    args = parser.parse_args()

    spinner = Spinner()
    loop = loop_frames(spinner)
    end = end_frames(spinner)
    args.output.mkdir(parents=True, exist_ok=True)
    for prefix, frames in (("throbber", loop), ("animation", end)):
        for old in args.output.glob(f"{prefix}-*.png"):
            old.unlink()
        for n, frame in enumerate(frames, 1):
            frame.save(args.output / f"{prefix}-{n:04d}.png", optimize=True)
    print(f"{args.output}: {len(loop)} throbber and {len(end)} animation frames, {SIZE}x{SIZE}")

    if args.preview:
        logo = Path(__file__).resolve().parent.parent / "logos" / "patrone-splash.png"
        write_previews(loop, end, args.preview, logo)
        print(f"{args.preview}: spinner-sheet.png, spinner-preview.gif, spinner-splash.png")


if __name__ == "__main__":
    main()
