"""Traces the Patrone mark into terminal text, for fastfetch.

Each character cell is split into four quadrant "pixels" (the Unicode block
characters such as ▟ and ▛, which terminals like VTE draw as exact
rectangles). The mark is traced from the logo's own transparency, with the
grid shifted to where the mark's straight edges land most cleanly, and stray
quadrants and pinholes cleared. Each cell is colored along a diagonal
gradient through the Patrone blues, light at the top right, or with the
nearest blue sampled from the logo (--colors sampled). The output is a
fastfetch logo: $1 up select the blues, lightest first, then the wordmark's
white and the rule blue. The script prints their numbers for
branding/fastfetch/config.jsonc.

    python tools/make_terminal_logo.py logos/patrone-logo.png fastfetch/logo.txt --preview logo.png
"""

import argparse
import itertools

import numpy as np
from PIL import Image, ImageDraw, ImageFont

# The blues measured from the concept (branding/README.md), lightest first.
# Their order is the $1..$5 order in the fastfetch config.
BLUES = [
    (0x4D, 0xA0, 0xFC),  # Highlight
    (0x28, 0x8B, 0xFA),  # Patrone Blue
    (0x23, 0x72, 0xDD),  # Mid
    (0x18, 0x51, 0xA8),  # Deep
    (0x13, 0x43, 0x94),  # Shadow
]
# The wordmark is white and its rule blue; their numbers follow the blues.
WHITE = (0xFF, 0xFF, 0xFF)
RULE_BLUE = (0x3E, 0x91, 0xDE)

# Quadrants, as (top left, top right, bottom left, bottom right) bits.
QUADRANTS = {
    (0, 0, 0, 0): " ", (1, 0, 0, 0): "▘", (0, 1, 0, 0): "▝", (0, 0, 1, 0): "▖",
    (0, 0, 0, 1): "▗", (1, 1, 0, 0): "▀", (0, 0, 1, 1): "▄", (1, 0, 1, 0): "▌",
    (0, 1, 0, 1): "▐", (1, 0, 0, 1): "▚", (0, 1, 1, 0): "▞", (1, 1, 1, 0): "▛",
    (1, 1, 0, 1): "▜", (1, 0, 1, 1): "▙", (0, 1, 1, 1): "▟", (1, 1, 1, 1): "█",
}
BITS = {char: bits for bits, char in QUADRANTS.items()}

# Terminal cells are about twice as tall as they are wide.
CELL_ASPECT = 2.0
# The logo is enlarged this many times before tracing, so each quadrant's
# coverage is measured from many pixels.
SCALE = 6


def load_mark(path: str) -> tuple[np.ndarray, np.ndarray]:
    """The mark's coverage (0 to 1) and colors, cropped to the mark."""
    image = Image.open(path).convert("RGBA")
    image = image.resize((image.width * SCALE, image.height * SCALE), Image.LANCZOS)
    rgba = np.asarray(image).astype(float)
    alpha = rgba[..., 3] / 255
    ys, xs = np.nonzero(alpha > 0.5)
    box = (slice(ys.min(), ys.max() + 1), slice(xs.min(), xs.max() + 1))
    return alpha[box], rgba[box][..., :3]


def quadrant_coverage(alpha: np.ndarray, width: int, ox: float, oy: float) -> np.ndarray:
    """How much of each quadrant the mark covers, for a grid `width` cells
    wide shifted by (ox, oy) quadrants. Shape: (rows * 2, width * 2)."""
    quad_w = alpha.shape[1] / (2 * width - 1)
    quad_h = quad_w * CELL_ASPECT
    rows = int(np.ceil((alpha.shape[0] / quad_h + oy) / 2))
    # An integral image gives each rectangle's sum in four lookups.
    integral = np.pad(alpha.cumsum(0).cumsum(1), ((1, 0), (1, 0)))
    xs = np.clip(np.round((np.arange(2 * width + 1) - ox) * quad_w).astype(int), 0, alpha.shape[1])
    ys = np.clip(np.round((np.arange(2 * rows + 1) - oy) * quad_h).astype(int), 0, alpha.shape[0])
    sums = integral[np.ix_(ys[1:], xs[1:])] - integral[np.ix_(ys[:-1], xs[1:])] \
        - integral[np.ix_(ys[1:], xs[:-1])] + integral[np.ix_(ys[:-1], xs[:-1])]
    return sums / (quad_w * quad_h)


def best_grid(alpha: np.ndarray, width: int) -> tuple[np.ndarray, tuple[float, float]]:
    """The grid offset where fewest quadrants are only partly covered: the
    one where the mark's edges fall along quadrant edges."""
    best = None
    for ox, oy in itertools.product(np.linspace(0, 0.75, 4), repeat=2):
        coverage = quadrant_coverage(alpha, width, ox, oy)
        error = np.minimum(coverage, 1 - coverage).clip(0).sum()
        if best is None or error < best[0]:
            best = (error, coverage, (ox, oy))
    return best[1], best[2]


def tidy(bits: np.ndarray) -> np.ndarray:
    """Clears quadrants with no filled neighbor, and fills pinholes."""
    padded = np.pad(bits, 1)
    neighbors = padded[:-2, 1:-1] + padded[2:, 1:-1] + padded[1:-1, :-2] + padded[1:-1, 2:]
    bits = bits.copy()
    bits[(bits == 1) & (neighbors == 0)] = 0
    bits[(bits == 0) & (neighbors == 4)] = 1
    return bits


def nearest_blue(color: np.ndarray) -> int:
    distances = [sum((float(c) - v) ** 2 for c, v in zip(color, blue)) for blue in BLUES]
    return 1 + distances.index(min(distances))


def gradient_blue(t: float, steps: int) -> int:
    """The blue at `t` along the gradient, 0 at the top right, 1 at the
    bottom left, as one of `steps` colors."""
    return 1 + min(steps - 1, int(t * steps))


def gradient_colors(steps: int) -> list[tuple[int, int, int]]:
    """`steps` colors evenly along the Patrone blues, lightest first."""
    stops = np.array(BLUES, dtype=float)
    colors = []
    for i in range(steps):
        position = i / (steps - 1) * (len(BLUES) - 1)
        low = min(int(position), len(BLUES) - 2)
        mix = position - low
        colors.append(tuple(int(round(c)) for c in stops[low] * (1 - mix) + stops[low + 1] * mix))
    return colors


def trace(path: str, width: int, colors: str, steps: int) -> list[str]:
    alpha, rgb = load_mark(path)
    coverage, (ox, oy) = best_grid(alpha, width)
    bits = tidy((coverage > 0.5).astype(int))
    quad_w = alpha.shape[1] / (2 * width - 1)
    quad_h = quad_w * CELL_ASPECT
    rows = bits.shape[0] // 2
    lines = []
    for row in range(rows):
        line = ""
        current = None
        for col in range(width):
            cell = bits[2 * row:2 * row + 2, 2 * col:2 * col + 2]
            char = QUADRANTS[tuple(cell.flatten())]
            if char != " ":
                x0, x1 = (int(round((2 * col + d - ox) * quad_w)) for d in (0, 2))
                y0, y1 = (int(round((2 * row + d - oy) * quad_h)) for d in (0, 2))
                x0, y0 = max(x0, 0), max(y0, 0)
                if colors == "gradient":
                    # Along the diagonal from the top right to the bottom left
                    cx, cy = (x0 + x1) / 2 / alpha.shape[1], (y0 + y1) / 2 / alpha.shape[0]
                    blue = gradient_blue(min(max(((1 - cx) + cy) / 2, 0), 1), steps)
                else:
                    # The logo's color under the cell, from inside the mark
                    # (its edges are darker)
                    inside = alpha[y0:y1, x0:x1] > 0.99
                    pixels = rgb[y0:y1, x0:x1][inside]
                    blue = nearest_blue(np.median(pixels, axis=0)) if len(pixels) else 2
                if blue != current:
                    line += f"${blue}"
                    current = blue
            line += char
        lines.append(line.rstrip())
    while lines and not lines[-1].strip():
        lines.pop()
    return lines


def centered(text: str, width: int) -> str:
    return " " * max(0, (width - len(text)) // 2) + text


def preview(lines: list[str], path: str, font_path: str, colors: list[tuple[int, int, int]]) -> None:
    """Draws the logo the way VTE does, block characters as exact rectangles,
    on Patrone Terminal's dark background and on a light one, where the
    wordmark is dark."""
    font = ImageFont.truetype(font_path, 20)
    cell_w = round(font.getlength("M"))
    cell_h = 2 * cell_w
    columns = max(len(strip_colors(line)) for line in lines) + 2
    sheet = Image.new("RGB", (2 * columns * cell_w, (len(lines) + 2) * cell_h))
    draw = ImageDraw.Draw(sheet)
    palette = {i + 1: color for i, color in enumerate(colors)}
    for panel, background in enumerate([(0x1C, 0x1F, 0x26), (0xF6, 0xF7, 0xF9)]):
        left = panel * columns * cell_w
        draw.rectangle([left, 0, left + columns * cell_w, sheet.height], fill=background)
        for row, line in enumerate(lines, start=1):
            color, col, i = (0, 0, 0), 1, 0
            while i < len(line):
                if line[i] == "$" and line[i + 1:i + 2].isdigit():
                    color = palette[int(line[i + 1])]
                    if panel == 1 and color == WHITE:
                        color = (0x20, 0x24, 0x2C)
                    i += 2
                    continue
                x, y = left + col * cell_w, row * cell_h
                if line[i] in BITS:
                    for k, bit in enumerate(BITS[line[i]]):
                        if bit:
                            qx, qy = k % 2, k // 2
                            draw.rectangle([x + qx * cell_w / 2, y + qy * cell_h / 2,
                                            x + (qx + 1) * cell_w / 2 - 1, y + (qy + 1) * cell_h / 2 - 1],
                                           fill=color)
                elif line[i] != " ":
                    draw.text((x, y + cell_h / 2), line[i], font=font, fill=color, anchor="lm")
                col += 1
                i += 1
    sheet.save(path)


def strip_colors(line: str) -> str:
    out, i = "", 0
    while i < len(line):
        if line[i] == "$" and line[i + 1:i + 2].isdigit():
            i += 2
        else:
            out += line[i]
            i += 1
    return out


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("logo", help="the mark, on a transparent background")
    parser.add_argument("output")
    parser.add_argument("--width", type=int, default=24, help="width of the mark in characters")
    parser.add_argument(
        "--colors",
        choices=["gradient", "sampled"],
        default="gradient",
        help="a smooth diagonal gradient, or each cell's nearest blue from the logo",
    )
    parser.add_argument("--steps", type=int, default=7, help="the gradient's number of blues (fastfetch has $1 to $9)")
    parser.add_argument("--preview", help="also draw the result to this PNG")
    parser.add_argument("--font", default="/usr/share/fonts/truetype/jetbrains-mono/JetBrainsMono-Bold.ttf",
                        help="the font for the preview's wordmark")
    args = parser.parse_args()

    blues = gradient_colors(args.steps) if args.colors == "gradient" else BLUES
    lines = trace(args.logo, args.width, args.colors, len(blues))
    wordmark, rule = len(blues) + 1, len(blues) + 2
    width = max(len(strip_colors(line)) for line in lines)
    lines += [
        "",
        f"${wordmark}" + centered("P A T R O N E", width),
        f"${rule}" + centered("── L I N U X ──", width),
    ]
    with open(args.output, "w", encoding="utf-8", newline="\n") as out:
        out.write("\n".join(lines) + "\n")
    print("\n".join(lines))
    # The colors for branding/fastfetch/config.jsonc
    colors = blues + [WHITE, RULE_BLUE]
    for number, (r, g, b) in enumerate(colors, start=1):
        code = "1;97" if (r, g, b) == WHITE else f"38;2;{r};{g};{b}"
        print(f'"{number}": "{code}",')
    if args.preview:
        preview(lines, args.preview, args.font, colors)


if __name__ == "__main__":
    main()
