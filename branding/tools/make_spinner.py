"""Draws the boot splash's spinner: a single ring with a comet of light
running round it.

    python tools/make_spinner.py system/plymouth/spinner --preview system/plymouth/previews

Writes, for Plymouth's two-step module:
  throbber-0001.png ... throbber-0060.png    the spinner, a 2-second loop
  animation-0001.png ... animation-0030.png  the end animation

Plymouth plays the throbber as a loop that lasts 2 seconds whatever its frame
count, sampled 30 times a second, so 60 frames give each tick a new one. The
end animation plays once, one frame per tick, and holds its last frame.
Patrone's theme turns the end animation off, but two-step needs its frames:
without them it would take the throbber's frames for it and drop the spinner.
It carries on from the loop and lights the ring in full.

The ring is a dim track. The comet runs clockwise round it: its head sprints
ahead and its tail catches up, so it stretches and shrinks once a loop.
(Until 2026-10-27 there were three interlinked rings; the user asked for a
single ring.) Every frame is drawn at four times its size and scaled down,
which smooths the edges.

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

RADIUS = 30.0      # the ring, in pixels of the finished frame
STROKE = 5.0       # its width

# The comet's head and tail follow the same easing a fraction LAG of a loop
# apart; EASE_LINEAR of their motion is steady and the rest eases in and
# out, so the head sprints, then the tail.
LAG = 0.34
EASE_LINEAR = 0.22
REST_ANGLE = math.radians(-90)  # where the head is slowest: the top

# Patrone's palette (branding/README.md)
BACKGROUND = (0x03, 0x03, 0x03)
HIGHLIGHT = (0x4D, 0xA0, 0xFC)
PATRONE_BLUE = (0x28, 0x8B, 0xFA)
MID = (0x23, 0x72, 0xDD)
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
GLOW_RADIUS = 2.5  # a soft glow around the comet, in pixels
GLOW_STRENGTH = 0.45

# The splash: where two-step puts the logo and the spinner (patrone.plymouth)
SCREEN = (1920, 1080)
LOGO_ALIGN = (0.5, 0.42)
SPINNER_ALIGN = (0.5, 0.72)


def eased(x):
    """The comet's easing, unwrapped: turns made by time x (in loops), steady
    at EASE_LINEAR of full speed at each whole loop and fastest halfway."""
    whole = np.floor(x)
    u = x - whole
    return whole + EASE_LINEAR * u + (1 - EASE_LINEAR) * (1 - np.cos(np.pi * u)) / 2


def comet_color(position):
    """position: 0 at the head to 1 at the tail; returns RGB, 0 to 1."""
    stops = [s for s, _ in COMET_STOPS]
    channels = [np.interp(position, stops, [c[i] / 255 for _, c in COMET_STOPS]) for i in range(3)]
    return np.stack(channels, axis=-1)


def smoothstep(x):
    x = np.clip(x, 0, 1)
    return x * x * (3 - 2 * x)


class Spinner:
    def __init__(self):
        n = SIZE * SCALE
        coords = (np.arange(n) + 0.5) / SCALE
        self.x, self.y = np.meshgrid(coords, coords)
        self.cx = self.cy = SIZE / 2
        dx, dy = self.x - self.cx, self.y - self.cy
        self.theta = np.arctan2(dy, dx)
        self.track = self.coverage(STROKE / 2 - np.abs(np.hypot(dx, dy) - RADIUS))

    @staticmethod
    def coverage(inside):
        """inside: how far a point is within a shape's edge, in pixels."""
        return np.clip(inside * SCALE + 0.5, 0.0, 1.0)

    def comet(self, head, length):
        """The comet whose head is at angle head (radians, clockwise) and which
        trails length radians behind it: (RGB, opacity)."""
        behind = (head - self.theta) % (2 * np.pi)
        on = behind <= length
        position = np.clip(behind / max(length, 1e-6), 0, 1)
        opacity = np.where(on, 1 - smoothstep((position - SOLID) / (1 - SOLID)), 0.0) * self.track
        # A round cap on the head
        hx, hy = self.cx + RADIUS * math.cos(head), self.cy + RADIUS * math.sin(head)
        cap = self.coverage(STROKE / 2 - np.hypot(self.x - hx, self.y - hy))
        position = np.where(cap > opacity, 0.0, position)
        opacity = np.maximum(opacity, cap)
        return comet_color(position), opacity

    def draw(self, head, length, track_opacity=TRACK_OPACITY, fill=0.0):
        """fill: how far the ring is lit in full, 0 to 1, for the end
        animation. Returns an RGBA image."""
        rgb, alpha = self.comet(head, length)
        if fill > 0:
            rgb = rgb * (1 - fill) + np.array(PATRONE_BLUE) / 255 * fill
            alpha = alpha * (1 - fill) + self.track * fill
        under = self.track * track_opacity * (1 - alpha)
        color = rgb * alpha[..., None] + np.array(TRACK_COLOR) / 255 * under[..., None]
        opacity = alpha + under
        glow = rgb * alpha[..., None]

        color = shrink(color)
        opacity = shrink(opacity)
        color, opacity = add_glow(color, opacity, shrink(glow))
        straight = color / np.maximum(opacity, 1e-6)[..., None]
        rgba = np.dstack([np.clip(straight, 0, 1), np.clip(opacity, 0, 1)])
        return Image.fromarray((rgba * 255).round().astype(np.uint8), "RGBA")

    @staticmethod
    def comet_at(t):
        """The comet at time t, in loops: (head, length)."""
        head = eased(t)
        tail = eased(t - LAG)
        return REST_ANGLE + 2 * np.pi * head, 2 * np.pi * (head - tail)


def shrink(image):
    """Averages each SCALE x SCALE block into one pixel."""
    n = image.shape[0] // SCALE
    return image.reshape(n, SCALE, n, SCALE, *image.shape[2:]).mean(axis=(1, 3))


def add_glow(color, opacity, emitted):
    """A blur of the comet, added as light: it shows on a dark background."""
    pixels = (np.clip(emitted, 0, 1) * 255).round().astype(np.uint8)
    blurred = Image.fromarray(pixels, "RGB").filter(ImageFilter.GaussianBlur(GLOW_RADIUS))
    light = np.asarray(blurred, dtype=float) / 255 * GLOW_STRENGTH
    color = color + light * (1 - opacity[..., None])
    opacity = np.maximum(opacity, color.max(axis=-1))
    return color, opacity


def loop_frames(spinner):
    return [spinner.draw(*spinner.comet_at(n / LOOP_FRAMES)) for n in range(LOOP_FRAMES)]


def end_frames(spinner):
    """Carries on from the loop's last frame while the ring lights up in full."""
    frames = []
    for n in range(END_FRAMES):
        progress = (n + 1) / END_FRAMES
        head, length = spinner.comet_at(1 + n / LOOP_FRAMES)
        frames.append(spinner.draw(head, length, TRACK_OPACITY + (1 - TRACK_OPACITY) * progress,
                                   float(smoothstep(progress / 0.8))))
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
