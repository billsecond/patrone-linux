"""Traces the Patrone mark into terminal text, for fastfetch.

Each character cell takes whichever block or shape character best matches the
mark under it: full and partial blocks, sextants (2x3 cells), the diagonal
"smooth mosaic" wedges and eighth blocks from Unicode's block elements and
Symbols for Legacy Computing. VTE, Patrone Terminal's engine, draws all of
them itself as exact shapes, so slopes come out as straight diagonals rather
than stair steps. The shapes are matched as VTE draws them: terminal-glyphs.png
is VTE's drawing of terminal-glyphs.txt (render_vte.py, below), so a new VTE
can be measured again. The mark is traced from the logo's transparency, with
the grid shifted to where it fits best. Each cell is colored along a diagonal
gradient through the Patrone blues, light at the top right.

The output is a fastfetch logo: $1 up select the blues, lightest first, then
the wordmark's white and the rule blue. The script prints their numbers for
branding/fastfetch/config.jsonc.

    python tools/make_terminal_logo.py logos/patrone-logo.png fastfetch/logo.txt --preview logo.png

To measure the shapes again (on a machine with VTE's GTK 4 bindings and cage):

    python tools/make_terminal_logo.py --sheet sheet.ansi
    WLR_BACKENDS=headless WLR_RENDERER=pixman GSK_RENDERER=cairo \\
        cage -- python3 tools/render_vte.py sheet.ansi tools/terminal-glyphs.png 34 18 "JetBrains Mono 26"
"""

import argparse
import itertools
from pathlib import Path

import numpy as np
from PIL import Image

TOOLS = Path(__file__).resolve().parent

# The blues measured from the concept (branding/README.md), lightest first
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

# The sheet's layout: a row of full blocks to find the grid by, then the
# characters of terminal-glyphs.txt, 32 to a row
SHEET_COLUMNS = 32
# Each shape is compared at this many pixels a cell
MASK = (16, 36)

# Which characters may be used. "blocks" are the oldest and most widely
# drawn; "smooth" adds sextants, wedges and eighth blocks (Unicode 13, drawn
# by VTE, Kitty, WezTerm and Windows Terminal); "octants" adds 2x4 cells
# (Unicode 16, newer still).
GLYPH_SETS = {
    "blocks": [(0x20, 0x20), (0x2588, 0x2588), (0x2580, 0x2580), (0x2584, 0x2584),
               (0x258C, 0x258C), (0x2590, 0x2590), (0x2596, 0x259F)],
    # (Without the one-eighth bars, ▁ ▔ ▏ ▕, which show as hairlines)
    "smooth": [(0x1FB00, 0x1FB6F), (0x2582, 0x2587), (0x2589, 0x258E), (0x1FB82, 0x1FB8B)],
    "octants": [(0x1CD00, 0x1CDE5)],
}


def sheet_characters() -> str:
    return (TOOLS / "terminal-glyphs.txt").read_text(encoding="utf-8")


def write_sheet(path: str) -> None:
    """The text whose drawing by VTE is terminal-glyphs.png."""
    chars = sheet_characters()
    rows = ["█" * SHEET_COLUMNS]
    rows += [chars[i:i + SHEET_COLUMNS].ljust(SHEET_COLUMNS) for i in range(0, len(chars), SHEET_COLUMNS)]
    Path(path).write_text("\n".join("\x1b[97m" + row for row in rows) + "\n", encoding="utf-8")


def load_glyphs(sets: list[str]) -> tuple[list[str], np.ndarray, float]:
    """The allowed characters, their shapes as VTE draws them (coverage, 0 to
    1, MASK pixels a cell), and a cell's height over its width."""
    pixels = np.asarray(Image.open(TOOLS / "terminal-glyphs.png").convert("L")).astype(float)
    background = np.median(pixels[-8:, -8:])
    coverage = np.clip((pixels - background) / (255 - background), 0, 1)
    # The first row is full blocks: it gives the grid's corner and pitch,
    # which are fractions of a pixel. The row below starts with a space.
    ys, xs = np.nonzero(coverage > 0.5)
    first_x, first_y = xs.min(), ys.min()
    across, down = first_y + 5, first_x + 5
    cell_w = coverage[across].sum() / SHEET_COLUMNS
    cell_h = coverage[: first_y + int(3 * cell_w), down].sum()
    left = first_x - (coverage[across, first_x - 1] if first_x else 0)
    top = first_y - (coverage[first_y - 1, down] if first_y else 0)
    sheet = Image.fromarray(coverage.astype(np.float32), mode="F")

    allowed = set()
    for name in sets:
        for lo, hi in GLYPH_SETS[name]:
            allowed.update(chr(c) for c in range(lo, hi + 1))
    chars, masks = [], []
    for i, char in enumerate(sheet_characters()):
        if char not in allowed:
            continue
        row, col = 1 + i // SHEET_COLUMNS, i % SHEET_COLUMNS
        box = (left + col * cell_w, top + row * cell_h, left + (col + 1) * cell_w, top + (row + 1) * cell_h)
        masks.append(np.asarray(sheet.resize(MASK, Image.BOX, box=box)).astype(float))
        chars.append(char)
    return chars, np.stack(masks), cell_h / cell_w


def load_mark(path: str, scale: int = 6) -> np.ndarray:
    """The mark's coverage (0 to 1), enlarged and cropped to the mark."""
    image = Image.open(path).convert("RGBA")
    image = image.resize((image.width * scale, image.height * scale), Image.LANCZOS)
    alpha = np.asarray(image)[..., 3].astype(float) / 255
    ys, xs = np.nonzero(alpha > 0.5)
    return alpha[ys.min():ys.max() + 1, xs.min():xs.max() + 1]


def cell_coverage(alpha: np.ndarray, width: int, aspect: float, ox: float, oy: float) -> np.ndarray:
    """The mark under each cell, MASK pixels a cell, for a grid `width` cells
    wide shifted by (ox, oy) cells. Shape: (rows, width, MASK[1], MASK[0])."""
    cell_w = alpha.shape[1] / (width - 1)
    cell_h = cell_w * aspect
    rows = int(np.ceil(alpha.shape[0] / cell_h + oy))
    # Pad the mark so every cell lies inside the image, then sample it
    pad_x, pad_y = int(np.ceil(cell_w)), int(np.ceil(cell_h))
    image = Image.fromarray(np.pad(alpha, ((pad_y, pad_y), (pad_x, pad_x))).astype(np.float32), mode="F")
    out = np.zeros((rows, width, MASK[1], MASK[0]))
    for row, col in itertools.product(range(rows), range(width)):
        x0 = pad_x + (col - ox) * cell_w
        y0 = pad_y + (row - oy) * cell_h
        cell = image.resize(MASK, Image.BOX, box=(x0, y0, x0 + cell_w, y0 + cell_h))
        out[row, col] = np.asarray(cell)
    return out


def best_glyphs(coverage: np.ndarray, masks: np.ndarray) -> tuple[np.ndarray, float]:
    """Each cell's closest shape, and the total squared difference."""
    flat = coverage.reshape(-1, MASK[0] * MASK[1])
    shapes = masks.reshape(len(masks), -1)
    # |a - b|^2 = |a|^2 - 2ab + |b|^2, for every cell and shape at once
    cost = (flat ** 2).sum(1)[:, None] - 2 * flat @ shapes.T + (shapes ** 2).sum(1)[None, :]
    choice = cost.argmin(1)
    return choice.reshape(coverage.shape[:2]), float(cost.min(1).sum())


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


def trace(path: str, width: int, sets: list[str], steps: int) -> list[str]:
    chars, masks, aspect = load_glyphs(sets)
    alpha = load_mark(path)
    best = None
    for ox, oy in itertools.product(np.linspace(0, 1, 6, endpoint=False), repeat=2):
        coverage = cell_coverage(alpha, width, aspect, ox, oy)
        choice, cost = best_glyphs(coverage, masks)
        if best is None or cost < best[1]:
            best = (choice, cost)
    choice = best[0]
    rows = choice.shape[0]
    lines = []
    for row in range(rows):
        line, current = "", None
        for col in range(width):
            char = chars[choice[row, col]]
            if char != " ":
                # Along the diagonal from the top right to the bottom left
                t = ((width - 0.5 - col) / width + (row + 0.5) / rows) / 2
                blue = 1 + min(steps - 1, int(t * steps))
                if blue != current:
                    line += f"${blue}"
                    current = blue
            line += char
        lines.append(line.rstrip())
    while lines and not lines[0].strip():
        lines.pop(0)
    while lines and not lines[-1].strip():
        lines.pop()
    return lines


def strip_colors(line: str) -> str:
    out, i = "", 0
    while i < len(line):
        if line[i] == "$" and line[i + 1:i + 2].isdigit():
            i += 2
        else:
            out += line[i]
            i += 1
    return out


def centered(text: str, width: int) -> str:
    return " " * max(0, (width - len(text)) // 2) + text


def preview(lines: list[str], path: str, colors: list[tuple[int, int, int]]) -> None:
    """Draws the logo's shapes as VTE does (from terminal-glyphs.png), on
    Patrone Terminal's background and on a light one. Letters are left as
    boxes: render_vte.py draws the real thing."""
    chars, masks, aspect = load_glyphs(list(GLYPH_SETS))
    index = {char: i for i, char in enumerate(chars)}
    cell_w, cell_h = 18, int(round(18 * aspect))
    big = {char: np.asarray(Image.fromarray((masks[i] * 255).astype(np.uint8)).resize(
        (cell_w, cell_h), Image.BILINEAR)).astype(float) / 255 for char, i in index.items()}
    columns = max(len(strip_colors(line)) for line in lines) + 2
    panels = []
    for background in [(0x07, 0x0A, 0x10), (0xF6, 0xF7, 0xF9)]:
        canvas = np.ones(((len(lines) + 2) * cell_h, columns * cell_w, 3)) * background
        for row, line in enumerate(lines, start=1):
            color, col, i = WHITE, 1, 0
            while i < len(line):
                if line[i] == "$" and line[i + 1:i + 2].isdigit():
                    color = colors[int(line[i + 1]) - 1]
                    i += 2
                    continue
                y, x = row * cell_h, col * cell_w
                if line[i] in big:
                    shape = big[line[i]][..., None]
                elif line[i] != " ":
                    shape = np.zeros((cell_h, cell_w, 1))
                    shape[cell_h // 4:-cell_h // 4, 2:-2] = 0.6
                else:
                    shape = None
                if shape is not None:
                    region = canvas[y:y + cell_h, x:x + cell_w]
                    region[:] = region * (1 - shape) + np.array(color) * shape
                col += 1
                i += 1
        panels.append(canvas)
    Image.fromarray(np.concatenate(panels, axis=1).astype(np.uint8)).save(path)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("logo", nargs="?", help="the mark, on a transparent background")
    parser.add_argument("output", nargs="?")
    parser.add_argument("--width", type=int, default=26, help="width of the mark in characters")
    parser.add_argument("--glyphs", default="blocks,smooth",
                        help="the character sets to use, of: " + ", ".join(GLYPH_SETS))
    parser.add_argument("--steps", type=int, default=7, help="the gradient's number of blues (fastfetch has $1 to $9)")
    parser.add_argument("--preview", help="also draw the result to this PNG")
    parser.add_argument("--sheet", help="write the text to draw with render_vte.py for terminal-glyphs.png, and stop")
    args = parser.parse_args()

    if args.sheet:
        write_sheet(args.sheet)
        return
    blues = gradient_colors(args.steps)
    lines = trace(args.logo, args.width, args.glyphs.split(","), len(blues))
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
        preview(lines, args.preview, colors)


if __name__ == "__main__":
    main()
