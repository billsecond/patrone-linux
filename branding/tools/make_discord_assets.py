"""Draws PatroneOS's Discord set from the logo's shapes, the wallpapers and
PatroneOS Sans.

    python tools/make_discord_assets.py

Writes, in discord/ (discord/README.md says where each goes in Discord):

  logos/             the logos as large transparent PNGs, and the SVG masters
  server/            the server icons, banner, invite background, Discovery
                     splash and event cover
  channel-banners/   a banner to post at the top of each channel
  emoji/             custom emoji, 128 square, and a loading spinner
  role-icons/        role icons, 256 square
  stickers/          stickers, 320 square
  images/            wallpapers at 1920x1080, to post
  contact-sheet.jpg  all of it at a glance

and checks every file against Discord's limits. discord/fonts/ is copied from
billsecond/patroneos-fonts, not drawn here, but the text here is set in it.
Windows only, as the VM has no OpenCV. Never edit the images by hand: change
this script and run it again.
"""
import argparse
import functools
import math
import shutil
from pathlib import Path

import cv2
import numpy as np
from PIL import Image, ImageDraw, ImageFont

import make_logos
import make_spinner as spin
import patrone_logo as pl
from wallpaper_layers import dithered8

BRANDING = Path(__file__).resolve().parent.parent
OUT = BRANDING / "discord"
SANS = OUT / "fonts" / "PatroneOS-Sans"
MONO = OUT / "fonts" / "PatroneOS-Mono"
ART = BRANDING / "wallpapers"
CLEAN = ART / "patrone-back-02-clean.png"
STARLIT = ART / "patrone-back-03-dark.png"

SS = 8                  # shapes are drawn this many times larger, then shrunk
ICON = 1024
SCREEN = (1920, 1080)
CHANNEL = (1600, 400)
EVENT = (1600, 800)     # Discord crops covers towards 2.5:1, so the logo stays in the middle
EMOJI = 128
ROLE = 256
STICKER = 320

# Tiles are a top and a bottom color. The blues are Patrone's; the rest carry
# the meaning emoji need (done, failed, careful), lit the same way.
BLUE_TILE = (pl.HIGHLIGHT, pl.DEEP)
DARK_TILE = ("#16294D", "#070C18")
GREEN_TILE = ("#3DD68C", "#14804F")
RED_TILE = ("#F4676B", "#B3262B")
AMBER_TILE = ("#FFC24D", "#D0800F")
VIOLET_TILE = ("#9C8CFF", "#4A3BC4")
PINK_TILE = ("#FF7EB0", "#C22D6E")

KB = 1024
LIMITS = [  # folder, size, the most bytes Discord takes
    ("server/server-icon*.png", (ICON, ICON), 10240 * KB),
    ("server/server-banner.png", SCREEN, 10240 * KB),
    ("server/invite-background.png", SCREEN, 10240 * KB),
    ("server/discovery-splash.png", SCREEN, 10240 * KB),
    ("server/event-cover.png", EVENT, 10240 * KB),
    ("channel-banners/*.png", CHANNEL, 10240 * KB),
    ("emoji/*.png", (EMOJI, EMOJI), 256 * KB),
    ("emoji/*.gif", (EMOJI, EMOJI), 256 * KB),
    ("role-icons/*.png", (ROLE, ROLE), 256 * KB),
    ("stickers/*.png", (STICKER, STICKER), 512 * KB),
    ("images/*.jpg", SCREEN, 10240 * KB),
]


def smooth(x):
    x = np.clip(x, 0, 1)
    return x * x * (3 - 2 * x)


def sans(weight, size):
    return ImageFont.truetype(str(SANS / f"PatroneOSSans-{weight}.ttf"), size)


# --- Pictures, as RGB floats ---------------------------------------------------

@functools.lru_cache(maxsize=2)
def wallpaper(path):
    return cv2.imread(str(path), cv2.IMREAD_COLOR)[..., ::-1].astype(np.float32) / 255


def scene(path, size, focus=(0.5, 0.5), zoom=1.0):
    """A wallpaper shrunk to cover size; focus: where the crop sits, 0 to 1."""
    image = wallpaper(path)
    h, w = image.shape[:2]
    k = max(size[0] / w, size[1] / h) * zoom
    cw, ch = round(size[0] / k), round(size[1] / k)
    x0, y0 = round((w - cw) * focus[0]), round((h - ch) * focus[1])
    return cv2.resize(image[y0:y0 + ch, x0:x0 + cw], size, interpolation=cv2.INTER_AREA)


def radial(W, H, center, inner, outer, reach):
    yy, xx = np.mgrid[0:H, 0:W].astype(np.float32) + 0.5
    t = smooth(np.hypot(xx - center[0], yy - center[1]) / reach)[..., None]
    return (pl.rgb(inner) * (1 - t) + pl.rgb(outer) * t).astype(np.float32)


def dimmed(base, center, radii, amount):
    """Darkens an oval, soft at its edge, so the logo stands clear of the scene."""
    H, W = base.shape[:2]
    yy, xx = np.mgrid[0:H, 0:W].astype(np.float32) + 0.5
    d = np.hypot((xx - center[0]) / radii[0], (yy - center[1]) / radii[1])
    return base * (1 - amount * (1 - smooth(d)))[..., None]


def with_logo(base, pieces, width, cx, cy, glow=0.0):
    """The pieces drawn on a picture, width pixels wide round (cx, cy), with a
    soft glow of their own colors, as the wallpapers' logo has."""
    H, W = base.shape[:2]
    x0, y0, x1, y1 = pl.bounds(pieces)
    k = width / (x1 - x0)
    color, alpha = pl.draw(pieces, W, H, k, cx - (x0 + x1) / 2 * k, cy - (y0 + y1) / 2 * k)
    if glow:
        for spread, share in ((0.012, 0.5), (0.05, 0.5)):
            base = base + cv2.GaussianBlur(color, (0, 0), width * spread) * glow * share
    return base * (1 - alpha[..., None]) + color


def with_text(base, text, font, x, cy, color=pl.WHITE):
    """Text starting at x, its capitals centred on cy; returns the picture and
    where the text ends."""
    H, W = base.shape[:2]
    mask = Image.new("L", (W, H), 0)
    left, top, right, bottom = font.getbbox("H")
    ImageDraw.Draw(mask).text((x, cy - (top + bottom) / 2), text, font=font, fill=255)
    cover = (np.asarray(mask, np.float32) / 255)[..., None]
    return base * (1 - cover) + pl.rgb(color).astype(np.float32) * cover, x + font.getlength(text)


def save_picture(path, image):
    path.parent.mkdir(parents=True, exist_ok=True)
    Image.fromarray(dithered8(image), "RGB").save(path, optimize=True)


def save_jpeg(path, image):
    path.parent.mkdir(parents=True, exist_ok=True)
    Image.fromarray(dithered8(image), "RGB").save(path, quality=92, subsampling=0, optimize=True)


# --- Logos ---------------------------------------------------------------------

def mark_rgba(size, share, **mark):
    """The P, share of the way across a transparent size x size square."""
    pieces = pl.mark(**mark)
    x0, y0, x1, y1 = pl.bounds(pieces)
    k = size * share / (x1 - x0)
    color, alpha = pl.draw(pieces, size, size, k, (size - (x0 + x1) * k) / 2, (size - (y0 + y1) * k) / 2, ss=16)
    straight = color / np.maximum(alpha, 1e-6)[..., None]
    return Image.fromarray((np.clip(np.dstack([straight, alpha]), 0, 1) * 255 + 0.5).astype(np.uint8), "RGBA")


def logos(out):
    out.mkdir(parents=True, exist_ok=True)
    mark_rgba(2048, 0.9).save(out / "patroneos-mark.png", optimize=True)
    mark_rgba(2048, 0.9, mono=pl.WHITE).save(out / "patroneos-mark-white.png", optimize=True)
    for name, dark in (("dark-bg", True), ("light-bg", False)):
        make_logos.write_png(out / f"patroneos-logo-{name}.png", pl.side_by_side(dark), width=2400, pad=40)
        make_logos.write_png(out / f"patroneos-logo-stacked-{name}.png", pl.stacked(dark), width=2000, pad=40)
        make_logos.write_png(out / f"patroneos-wordmark-{name}.png", pl.wordmark(dark), width=2400, pad=40)
    (out / "svg").mkdir(exist_ok=True)
    for svg in sorted((BRANDING / "logos").glob("*.svg")):
        shutil.copyfile(svg, out / "svg" / svg.name)


# --- The server ----------------------------------------------------------------

def server_images(out):
    # On dark: the P lit from behind. Discord crops icons to a circle, so all
    # of it stays inside one
    image = radial(ICON, ICON, (ICON / 2, ICON * 0.46), "#123A82", "#04070F", ICON * 0.72)
    save_picture(out / "server-icon.png", with_logo(image, pl.mark(), ICON * 0.68, ICON / 2, ICON / 2, glow=0.5))
    # White on Patrone Blue, the one that reads best small
    image = radial(ICON, ICON, (ICON * 0.35, ICON * 0.2), pl.HIGHLIGHT, pl.DEEP, ICON * 1.15)
    save_picture(out / "server-icon-blue.png",
                 with_logo(image, pl.mark(mono=pl.WHITE), ICON * 0.68, ICON / 2, ICON / 2))

    # Discord shows the banner about 240 wide above the channel list and puts
    # the server's name over its top, so the logo is large. The scene is
    # enlarged from the top, which takes the dunes down clear of the wordmark
    W, H = SCREEN
    image = dimmed(scene(CLEAN, SCREEN, (0.5, 0.0), 1.14), (W / 2, H * 0.52), (W * 0.48, H * 0.4), 0.7)
    save_picture(out / "server-banner.png", with_logo(image, pl.stacked(True), W * 0.58, W / 2, H * 0.51, glow=0.45))
    # The invite's card covers the middle, so the logo sits to its left
    image = dimmed(scene(CLEAN, SCREEN), (W * 0.19, H * 0.47), (W * 0.2, H * 0.3), 0.6)
    save_picture(out / "invite-background.png",
                 with_logo(image, pl.stacked(True), W * 0.26, W * 0.19, H * 0.47, glow=0.45))
    # Starlit already has the logo, drawn at 8K
    save_picture(out / "discovery-splash.png", scene(STARLIT, SCREEN))
    W, H = EVENT
    image = dimmed(scene(CLEAN, EVENT, (0.5, 0.0)), (W / 2, H * 0.47), (W * 0.42, H * 0.44), 0.7)
    save_picture(out / "event-cover.png", with_logo(image, pl.stacked(True), W * 0.5, W / 2, H * 0.47, glow=0.45))


CHANNELS = (  # file, title, wallpaper, where its crop sits
    ("welcome", "Welcome", "generated/patrone-waves.png", (0.5, 0.74)),
    ("rules", "Rules", "generated/patrone-slant.png", (0.5, 0.5)),
    ("announcements", "Announcements", "generated/patrone-bloom.png", (0.5, 0.1)),
    ("releases", "Releases", "generated/patrone-nebula.jpg", (0.5, 0.5)),
    ("general", "General", "generated/patrone-aurora.png", (0.5, 0.3)),
    ("support", "Support", "generated/patrone-horizon.png", (0.5, 0.5)),
    ("showcase", "Showcase", "generated/patrone-orbs.png", (0.5, 0.62)),
    ("development", "Development", "generated/patrone-facets.png", (0.5, 0.5)),
    ("bug-reports", "Bug Reports", "generated/patrone-halftone.png", (0.5, 0.9)),
    ("feedback", "Feedback", "generated/patrone-silk.jpg", (0.5, 0.5)),
)


def channel_banner(title, art, focus):
    W, H = CHANNEL
    image = scene(ART / art, CHANNEL, focus)
    # Dark under the title, whatever the wallpaper is doing there
    fade = 1 - 0.8 * (1 - smooth(np.linspace(0, 1.25, W, dtype=np.float32)))
    image = image * fade[None, :, None]
    image = with_logo(image, pl.mark(), 250, 190, H / 2, glow=0.35)
    image, end = with_text(image, title, sans("Bold", 118), 370, H / 2 - 8)
    # The rule of light under it, as in the Neon Horizon pictures
    xs = np.arange(W, dtype=np.float32)
    light = smooth((xs - 370) / 60) * (1 - smooth((xs - end + 80) / 200))
    rule = np.broadcast_to(pl.rgb(pl.HIGHLIGHT).astype(np.float32), (4, W, 3))
    y = round(H / 2 + 62)
    image[y:y + 4] = image[y:y + 4] * (1 - light[None, :, None]) + rule * light[None, :, None]
    return image


# --- Tiles and glyphs ----------------------------------------------------------
# Glyphs are drawn white on a tile, on a grid 100 by 100, so they fit any size.

class Glyph:
    def __init__(self, size):
        self.size = size * SS
        self.u = self.size / 100
        self.mask = Image.new("L", (self.size, self.size), 0)
        self.d = ImageDraw.Draw(self.mask)

    def line(self, points, width, fill=255):
        px = [(x * self.u, y * self.u) for x, y in points]
        r = width * self.u / 2
        self.d.line(px, fill=fill, width=round(width * self.u), joint="curve")
        for x, y in (px[0], px[-1]):
            self.d.ellipse((x - r, y - r, x + r, y + r), fill=fill)

    def shape(self, points, fill=255):
        self.d.polygon([(x * self.u, y * self.u) for x, y in points], fill=fill)

    def disc(self, x, y, r, fill=255):
        u = self.u
        self.d.ellipse(((x - r) * u, (y - r) * u, (x + r) * u, (y + r) * u), fill=fill)

    def text(self, text, size):
        font = sans("Bold", round(size * self.u))
        left, top, right, bottom = font.getbbox(text)
        self.d.text((self.size / 2 - (left + right) / 2, 50 * self.u - (top + bottom) / 2), text, font=font, fill=255)


def check(g):
    g.line([(26, 52), (43, 69), (75, 33)], 13)


def cross(g):
    g.line([(30, 30), (70, 70)], 13)
    g.line([(70, 30), (30, 70)], 13)


def warning(g):
    g.text("!", 78)


def info(g):
    g.text("i", 78)


def question(g):
    ts = np.linspace(-math.pi, math.pi * 0.45, 40)
    hook = [(50 + 19 * math.cos(t), 37 + 18 * math.sin(t)) for t in ts]
    g.line(hook + [(50, 62)], 12)
    g.disc(50, 79, 7.5)


def new(g):
    g.text("NEW", 38)


def plus_one(g):
    g.text("+1", 56)


def terminal(g):
    g.line([(23, 33), (42, 50), (23, 67)], 10)
    g.line([(50, 69), (77, 69)], 10)


def download(g):
    g.line([(50, 20), (50, 60)], 11)
    g.line([(31, 44), (50, 63), (69, 44)], 11)
    g.line([(25, 79), (75, 79)], 11)


def package(g):
    g.line([(50, 18), (78, 34), (78, 66), (50, 82), (22, 66), (22, 34), (50, 18)], 8)
    g.line([(22, 34), (50, 50), (78, 34)], 8)
    g.line([(50, 50), (50, 82)], 8)


def bug(g):
    g.disc(50, 29, 11)
    g.d.ellipse((33 * g.u, 37 * g.u, 67 * g.u, 83 * g.u), fill=255)
    g.line([(50, 42), (50, 80)], 3.5, fill=0)
    for side in (1, -1):
        g.line([(50 + side * 17, 48), (50 + side * 33, 40)], 6.5)
        g.line([(50 + side * 18, 60), (50 + side * 35, 60)], 6.5)
        g.line([(50 + side * 17, 72), (50 + side * 33, 80)], 6.5)
        g.line([(50 + side * 6, 22), (50 + side * 13, 13)], 5)


def _heart_points(cx, cy, scale):
    ts = np.linspace(0, 2 * math.pi, 120)
    return [(cx + scale * 16 * math.sin(t) ** 3,
             cy - scale * (13 * math.cos(t) - 5 * math.cos(2 * t) - 2 * math.cos(3 * t) - math.cos(4 * t)))
            for t in ts]


def heart(g):
    g.shape(_heart_points(50, 47, 1.95))


def star(g):
    points = [(50 + (31 if i % 2 == 0 else 13) * math.sin(i * math.pi / 5),
               53 - (31 if i % 2 == 0 else 13) * math.cos(i * math.pi / 5)) for i in range(10)]
    g.shape(points)
    g.line(points + points[:1], 8)


def shield(g):
    right = [(50 + 27 * math.cos(t), 52 + 34 * math.sin(t)) for t in np.linspace(0, math.pi / 2, 24)]
    left = [(100 - x, y) for x, y in reversed(right)]
    g.shape([(50, 14), (77, 24), *right, *left, (23, 24)])
    g.line([(38, 49), (47, 58), (63, 39)], 8, fill=0)


def code(g):
    g.line([(33, 33), (16, 50), (33, 67)], 10)
    g.line([(67, 33), (84, 50), (67, 67)], 10)
    g.line([(57, 26), (43, 74)], 10)


def merge(g):
    ts = np.linspace(0, 1, 30)
    curve = [((1 - t) ** 2 * 34 + 2 * t * (1 - t) * 34 + t * t * 69, (1 - t) ** 2 * 34 + 2 * t * (1 - t) * 60 + t * t * 60)
             for t in ts]
    g.line([(34, 26), (34, 74)], 8)
    g.line(curve, 8)
    g.disc(34, 25, 11)
    g.disc(34, 75, 11)
    g.disc(69, 60, 11)


def flask(g):
    g.line([(38, 17), (62, 17)], 8)
    g.line([(44, 18), (44, 41), (23, 77), (27, 83), (73, 83), (77, 77), (56, 41), (56, 18)], 8)
    g.shape([(35, 60), (65, 60), (77, 77), (73, 83), (27, 83), (23, 77)])


def tile(size, colors, radius=0.24):
    """A rounded square, lit from above, with a transparent corner."""
    big = size * SS
    inset = max(2, round(size * 0.02)) * SS
    mask = Image.new("L", (big, big), 0)
    ImageDraw.Draw(mask).rounded_rectangle((inset, inset, big - inset - 1, big - inset - 1),
                                           round(big * radius), fill=255)
    alpha = np.asarray(mask.resize((size, size), Image.BOX), np.float32) / 255
    t = np.linspace(0, 1, size, dtype=np.float32)[:, None, None]
    return (pl.rgb(colors[0]) * (1 - t) + pl.rgb(colors[1]) * t).astype(np.float32) * np.ones((size, size, 1)), alpha


def tiled(size, colors, cover, ink=pl.WHITE):
    """The tile with cover, 0 to 1, painted on it in ink: an RGBA image."""
    color, alpha = tile(size, colors)
    color = color * (1 - cover[..., None]) + pl.rgb(ink) * cover[..., None]
    alpha = np.maximum(alpha, cover)
    return Image.fromarray((np.clip(np.dstack([color, alpha]), 0, 1) * 255 + 0.5).astype(np.uint8), "RGBA")


def glyph_cover(size, draw):
    g = Glyph(size)
    draw(g)
    return np.asarray(g.mask.resize((size, size), Image.LANCZOS), np.float32) / 255


def mark_on_tile(size, colors, share=0.74):
    """The P opened up for small sizes, white on a tile."""
    p = mark_rgba(size, share, mono=pl.WHITE, small=True)
    return tiled(size, colors, np.asarray(p, np.float32)[..., 3] / 255)


def loading_gif(path):
    """The boot spinner's ring on a dark tile, thicker so it reads at emoji
    size. GIF has no soft edges, so the tile's corners are cut hard."""
    spin.SIZE, spin.RADIUS, spin.STROKE, spin.GLOW_RADIUS = EMOJI, 38.0, 14.0, 3.0
    spinner = spin.Spinner()
    color, alpha = tile(EMOJI, DARK_TILE)
    back = Image.fromarray((np.clip(color, 0, 1) * 255 + 0.5).astype(np.uint8), "RGB")
    outside = Image.fromarray(((alpha <= 0.5) * 255).astype(np.uint8), "L")
    frames = []
    for n in range(0, spin.LOOP_FRAMES, 2):
        frame = back.copy()
        ring = spinner.draw(*spinner.comet_at(n / spin.LOOP_FRAMES))
        frame.paste(ring, mask=ring)
        frames.append(frame)
    # One palette for all the frames, so the tile doesn't band or flicker. Its
    # last color is for the cut corners, and no picture has it
    strip = Image.new("RGB", (EMOJI * len(frames), EMOJI))
    for n, frame in enumerate(frames):
        strip.paste(frame, (n * EMOJI, 0))
    palette = strip.quantize(255, Image.Quantize.MEDIANCUT, dither=Image.Dither.NONE)
    colors = palette.getpalette()[:255 * 3]
    palette.putpalette(colors + [0, 0, 0] * (255 - len(colors) // 3) + [255, 0, 255])
    indexed = []
    for frame in frames:
        image = frame.quantize(palette=palette, dither=Image.Dither.NONE)
        image.paste(255, mask=outside)
        indexed.append(image)
    indexed[0].save(path, save_all=True, append_images=indexed[1:], duration=70, loop=0, disposal=2,
                    transparency=255, optimize=False)


def emoji(out):
    out.mkdir(parents=True, exist_ok=True)
    mark_rgba(EMOJI, 0.96, small=True).save(out / "patroneos.png", optimize=True)
    mark_on_tile(EMOJI, BLUE_TILE).save(out / "patroneos_blue.png", optimize=True)
    mark_on_tile(EMOJI, DARK_TILE).save(out / "patroneos_dark.png", optimize=True)
    for name, colors, draw in (("check", GREEN_TILE, check), ("cross", RED_TILE, cross),
                               ("warning", AMBER_TILE, warning), ("info", BLUE_TILE, info),
                               ("question", VIOLET_TILE, question), ("new", BLUE_TILE, new),
                               ("plus_one", BLUE_TILE, plus_one), ("terminal", DARK_TILE, terminal),
                               ("download", BLUE_TILE, download), ("package", AMBER_TILE, package),
                               ("bug", RED_TILE, bug), ("heart", PINK_TILE, heart), ("star", AMBER_TILE, star)):
        tiled(EMOJI, colors, glyph_cover(EMOJI, draw)).save(out / f"patrone_{name}.png", optimize=True)
    loading_gif(out / "patrone_loading.gif")


def role_icons(out):
    out.mkdir(parents=True, exist_ok=True)
    mark_on_tile(ROLE, BLUE_TILE).save(out / "team.png", optimize=True)
    for name, colors, draw in (("moderator", GREEN_TILE, shield), ("developer", DARK_TILE, code),
                               ("contributor", VIOLET_TILE, merge), ("tester", AMBER_TILE, flask),
                               ("supporter", PINK_TILE, heart)):
        tiled(ROLE, colors, glyph_cover(ROLE, draw)).save(out / f"{name}.png", optimize=True)


# --- Stickers ------------------------------------------------------------------

def sticker(word):
    """The P over a navy label that leans as its stem does, cut out with a
    white edge, as a vinyl sticker is, with a shadow so the edge shows on
    Discord's light style too. In blue and touching, the two read as a 2."""
    size = STICKER * 4
    art = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    art.alpha_composite(mark_rgba(size, 0.66), (0, 0), (0, round(size * 0.18)))
    top, bottom = round(size * 0.6), round(size * 0.85)
    lean = pl.SLANT * (bottom - top)
    left, right = size * 0.05, size * 0.95
    points = [(left + lean, top), (right, top), (right - lean, bottom), (left, bottom)]
    label = Image.new("L", (size, size), 0)
    ImageDraw.Draw(label).polygon(points, fill=255)
    t = np.linspace(0, 1, size, dtype=np.float32)[:, None, None]
    paint = (pl.rgb("#21385F") * (1 - t) + pl.rgb(pl.NAVY) * t).astype(np.float32) * np.ones((size, size, 1))
    art.alpha_composite(Image.fromarray(np.dstack([paint * 255, np.asarray(label)]).astype(np.uint8), "RGBA"))
    # The word, as large as fits the label
    px = size * 0.2
    font = sans("Bold", round(px))
    room = (right - left) - lean - size * 0.06
    while font.getlength(word) > room:
        px -= 4
        font = sans("Bold", round(px))
    x0, y0, x1, y1 = font.getbbox(word)
    ImageDraw.Draw(art).text((size / 2 - (x0 + x1) / 2, (top + bottom) / 2 - (y0 + y1) / 2), word,
                             font=font, fill=(255, 255, 255, 255))
    # The white edge, grown out from everything on the sticker
    alpha = np.asarray(art)[..., 3]
    r = 26
    kernel = cv2.getStructuringElement(cv2.MORPH_ELLIPSE, (2 * r + 1, 2 * r + 1))
    edge = cv2.morphologyEx(alpha, cv2.MORPH_CLOSE, cv2.getStructuringElement(cv2.MORPH_ELLIPSE, (161, 161)))
    edge = cv2.dilate(edge, kernel)
    shadow = cv2.GaussianBlur(np.roll(edge, 12, axis=0), (0, 0), 14) * 0.35
    shadow = np.dstack([np.zeros((size, size, 3), np.uint8), shadow.astype(np.uint8)])
    cut = Image.fromarray(shadow, "RGBA")
    cut.alpha_composite(Image.fromarray(np.dstack([np.full((size, size, 3), 255, np.uint8), edge]), "RGBA"))
    cut.alpha_composite(art)
    return cut.resize((STICKER, STICKER), Image.LANCZOS)


def stickers(out):
    out.mkdir(parents=True, exist_ok=True)
    for name, word in (("welcome", "WELCOME!"), ("thanks", "THANKS!"), ("fixed", "FIXED!"),
                       ("shipped", "SHIPPED!"), ("nice", "NICE!"), ("reboot", "REBOOT?")):
        sticker(word).save(out / f"{name}.png", optimize=True)


# --- Wallpapers to post --------------------------------------------------------

IMAGES = (
    ("neon-horizon", "patrone-back-02-dark.png"),
    ("neon-horizon-starlit", "patrone-back-03-dark.png"),
    ("neon-horizon-light", "patrone-back-02-light.png"),
    ("neon-horizon-midnight", "patrone-back-02-midnight.png"),
    ("neon-horizon-blue", "patrone-back-02-blue.png"),
    ("neon-horizon-graphite", "patrone-back-02-graphite.png"),
)


# --- The sheet -----------------------------------------------------------------

def contact_sheet(out):
    """All of it at a glance, on Discord's dark and light colors, with the
    icons, emoji and role icons at the size people see them too."""
    W, DARK, LIGHT, GAP = 1800, (30, 31, 34), (255, 255, 255), 24
    sheet = Image.new("RGB", (W, 6000), DARK)
    d = ImageDraw.Draw(sheet)
    heading, y = sans("SemiBold", 30), GAP

    def title(text):
        nonlocal y
        d.text((GAP, y), text, font=heading, fill=(242, 243, 245))
        y += 50

    def fitted(path, width):
        image = Image.open(path).convert("RGBA")
        return image.resize((width, round(image.height * width / image.width)), Image.LANCZOS)

    def circle(path, size):
        image = fitted(path, size)
        big = Image.new("L", (size * 4, size * 4), 0)
        ImageDraw.Draw(big).ellipse((0, 0, size * 4 - 1, size * 4 - 1), fill=255)
        image.putalpha(big.resize((size, size), Image.LANCZOS))
        return image

    def row(images, x=GAP, gap=GAP):
        nonlocal y
        for image in images:
            sheet.paste(image, (x, y), image)
            x += image.width + gap
        y += max(image.height for image in images) + GAP

    def strip(images, back, gap=12):
        """A row on a band of back, as far as the sheet goes."""
        nonlocal y
        d.rectangle((GAP, y, W - GAP, y + max(image.height for image in images) + 24), fill=back)
        y += 12
        row(images, GAP * 2, gap)

    icon, blue = out / "server/server-icon.png", out / "server/server-icon-blue.png"
    title("Server")
    row([circle(icon, 220), circle(blue, 220), circle(icon, 48), circle(blue, 48), circle(icon, 32), circle(blue, 32),
         fitted(out / "server/server-banner.png", 240), fitted(out / "server/event-cover.png", 440)])
    row([fitted(out / f"server/{name}.png", 568) for name in ("server-banner", "invite-background", "discovery-splash")])

    title("Channel banners")
    names = [c[0] for c in CHANNELS]
    for left, right in zip(names[::2], names[1::2]):
        row([fitted(out / f"channel-banners/{left}.png", 864), fitted(out / f"channel-banners/{right}.png", 864)])

    title("Emoji and role icons")
    small = sorted((out / "emoji").glob("*.png")) + [out / "emoji/patrone_loading.gif"]
    roles = sorted((out / "role-icons").glob("*.png"))
    row([fitted(p, 88) for p in small], gap=10)
    row([fitted(p, 128) for p in roles])
    for back in (DARK, LIGHT):
        strip([fitted(p, 32) for p in small] + [fitted(p, 20) for p in roles], back)

    title("Stickers")
    pictures = [fitted(p, 240) for p in sorted((out / "stickers").glob("*.png"))]
    strip(pictures, (49, 51, 56), 48)
    strip(pictures, LIGHT, 48)

    title("Logos")
    mark = fitted(out / "logos/patroneos-mark.png", 220)
    row([fitted(out / "logos/patroneos-logo-stacked-dark-bg.png", 420),
         fitted(out / "logos/patroneos-logo-dark-bg.png", 700), mark])
    strip([fitted(out / "logos/patroneos-logo-stacked-light-bg.png", 420),
           fitted(out / "logos/patroneos-logo-light-bg.png", 700), mark], LIGHT, GAP)

    title("Fonts")
    for family, weight, sample in (("Sans", "Regular", "PatroneOS Sans: the system's font"),
                                   ("Mono", "Regular", "PatroneOS Mono: sudo apt upgrade"),
                                   ("Document", "Regular", "PatroneOS Document: for documents"),
                                   ("Serif", "Regular", "PatroneOS Serif: for reading"),
                                   ("Classic", "Regular", "PatroneOS Classic: for older layouts")):
        font = ImageFont.truetype(str(OUT / "fonts" / f"PatroneOS-{family}" / f"PatroneOS{family}-{weight}.ttf"), 36)
        d.text((GAP, y), sample, font=font, fill=(242, 243, 245))
        y += 54
    sheet.crop((0, 0, W, y + GAP)).save(out / "contact-sheet.jpg", quality=90, optimize=True)


def checked(out):
    """Every file against Discord's limits."""
    count = 0
    for pattern, size, most in LIMITS:
        for path in sorted(out.glob(pattern)):
            with Image.open(path) as image:
                if image.size != size:
                    raise SystemExit(f"{path}: {image.size}, not {size}")
            if path.stat().st_size > most:
                raise SystemExit(f"{path}: {path.stat().st_size} bytes, more than {most}")
            count += 1
    print(f"{count} files within Discord's limits")


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.parse_args()
    logos(OUT / "logos")
    server_images(OUT / "server")
    for name, title, art, focus in CHANNELS:
        save_picture(OUT / "channel-banners" / f"{name}.png", channel_banner(title, art, focus))
    emoji(OUT / "emoji")
    role_icons(OUT / "role-icons")
    stickers(OUT / "stickers")
    for name, art in IMAGES:
        save_jpeg(OUT / "images" / f"{name}.jpg", scene(ART / art, SCREEN))
    contact_sheet(OUT)
    checked(OUT)


if __name__ == "__main__":
    main()
