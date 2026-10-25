"""Traces the Patrone mark from the logo concept into terminal text.

Each character cell is split into four quadrant "pixels" (the Unicode block
characters such as ▟ and ▛), and each cell is colored with the nearest of
the Patrone blues, so the mark keeps its shape and its gradient. The output
is a fastfetch logo: $1 to $5 select the blues, $6 the wordmark's white and
$7 the rule blue, as set in branding/fastfetch/config.jsonc.

    python tools/make_terminal_logo.py concepts/logo-concept-01.webp fastfetch/logo.txt --width 24
"""

import argparse

import numpy as np
from PIL import Image

# The blues measured from the concept (branding/README.md), lightest first.
# Their order is the $1..$5 order in the fastfetch config.
BLUES = [
    (0x4D, 0xA0, 0xFC),  # Highlight
    (0x28, 0x8B, 0xFA),  # Patrone Blue
    (0x23, 0x72, 0xDD),  # Mid
    (0x18, 0x51, 0xA8),  # Deep
    (0x13, 0x43, 0x94),  # Shadow
]
WORDMARK = 6
RULE = 7

# Quadrants, as (top left, top right, bottom left, bottom right) bits.
QUADRANTS = {
    (0, 0, 0, 0): " ", (1, 0, 0, 0): "▘", (0, 1, 0, 0): "▝", (0, 0, 1, 0): "▖",
    (0, 0, 0, 1): "▗", (1, 1, 0, 0): "▀", (0, 0, 1, 1): "▄", (1, 0, 1, 0): "▌",
    (0, 1, 0, 1): "▐", (1, 0, 0, 1): "▚", (0, 1, 1, 0): "▞", (1, 1, 1, 0): "▛",
    (1, 1, 0, 1): "▜", (1, 0, 1, 1): "▙", (0, 1, 1, 1): "▟", (1, 1, 1, 1): "█",
}

# Terminal cells are about twice as tall as they are wide.
CELL_ASPECT = 2.0


def mark_mask(image: np.ndarray) -> tuple[np.ndarray, tuple[int, int, int, int]]:
    """The blue mark in the top part of the concept, and its bounding box."""
    rgb = image.astype(int)
    r, g, b = rgb[..., 0], rgb[..., 1], rgb[..., 2]
    blue = (b > 90) & (b > r + 40)
    # The primary logo's mark sits above the wordmark, in the top third.
    blue[int(image.shape[0] * 0.36):, :] = False
    ys, xs = np.nonzero(blue)
    return blue, (xs.min(), ys.min(), xs.max() + 1, ys.max() + 1)


def nearest_blue(color: np.ndarray) -> int:
    distances = [sum((int(c) - v) ** 2 for c, v in zip(color, blue)) for blue in BLUES]
    return 1 + distances.index(min(distances))


def gradient_blue(t: float) -> int:
    """The blue at `t` along the gradient, 0 at the top right, 1 at the
    bottom left. Weighted toward the lighter end, as in the concept."""
    return 1 + min(len(BLUES) - 1, int(t**1.4 * len(BLUES)))


def trace(image: np.ndarray, width: int, colors: str) -> list[str]:
    mask, (left, top, right, bottom) = mark_mask(image)
    # A quadrant is half a cell wide and half a cell tall.
    quad_w = (right - left) / (2 * width)
    quad_h = quad_w * CELL_ASPECT
    rows = round((bottom - top) / (2 * quad_h))
    lines = []
    for row in range(rows):
        line = ""
        current = None
        for col in range(width):
            bits = []
            cell_pixels = []
            for dy in (0, 1):
                for dx in (0, 1):
                    x0 = int(left + (2 * col + dx) * quad_w)
                    y0 = int(top + (2 * row + dy) * quad_h)
                    x1 = max(x0 + 1, int(left + (2 * col + dx + 1) * quad_w))
                    y1 = max(y0 + 1, int(top + (2 * row + dy + 1) * quad_h))
                    area = mask[y0:y1, x0:x1]
                    bits.append(1 if area.mean() > 0.5 else 0)
                    cell_pixels.append(image[y0:y1, x0:x1][area])
            char = QUADRANTS[tuple(bits)]
            if char != " ":
                if colors == "gradient":
                    t = ((width - 0.5 - col) / width + (row + 0.5) / rows) / 2
                    color = gradient_blue(t)
                else:
                    pixels = np.concatenate(cell_pixels)
                    color = nearest_blue(pixels.mean(axis=0)) if len(pixels) else 2
                if color != current:
                    line += f"${color}"
                    current = color
            line += char
        lines.append(line.rstrip())
    return lines


def centered(text: str, width: int) -> str:
    return " " * max(0, (width - len(text)) // 2) + text


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("concept")
    parser.add_argument("output")
    parser.add_argument("--width", type=int, default=30, help="width of the mark in characters")
    parser.add_argument(
        "--colors",
        choices=["gradient", "sampled"],
        default="gradient",
        help="a smooth diagonal gradient, or each cell's nearest blue from the concept",
    )
    args = parser.parse_args()

    image = np.asarray(Image.open(args.concept).convert("RGB"))
    lines = trace(image, args.width, args.colors)
    width = max(args.width, 17)
    lines += [
        "",
        f"${WORDMARK}" + centered("P A T R O N E", width),
        f"${RULE}" + centered("━━━ L I N U X ━━━", width),
    ]
    with open(args.output, "w", encoding="utf-8", newline="\n") as out:
        out.write("\n".join(lines) + "\n")
    print("\n".join(lines))


if __name__ == "__main__":
    main()
