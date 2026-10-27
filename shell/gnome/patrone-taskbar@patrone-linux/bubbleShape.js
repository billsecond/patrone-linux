// The desktop bubble's shape (desktopBubble.js): half an ellipse coming out
// of the screen's left edge, as small as its files allow and at most half
// the screen wide, and where the files and the Recycle Bin go in it. Kept
// apart, with no imports, so it can be tried outside GNOME Shell.

// A file's place: its icon and two lines of its name
export const CELL_WIDTH = 92;
export const CELL_HEIGHT = 100;
// From the screen's edge to the first column, and from the files to the
// bubble's curved edge
const INSET = 20;
const EDGE = 12;
// The smallest bubble, for one file or none
const MIN_WIDTH = 250;
const MIN_HEIGHT = 210;
// How much wider than tall it grows, until it's as tall as the screen
const ASPECT = 1.25;
const STEP = 8;
// Where on the edge the Recycle Bin goes: this far below the middle
const BIN_ANGLE = Math.PI / 4;
// Room kept clear around the Recycle Bin
const BIN_GAP = 4;

/**
 * The bubble for `count` files.
 *
 * @param {object} params
 * @param {number} params.width - the screen's free area, across
 * @param {number} params.height - and down
 * @param {number} params.count - how many files
 * @param {number[]|null} params.bin - the Recycle Bin's width and height, or
 *   null when it isn't shown
 * @param {number} params.scale - the desktop's scale factor
 * @returns {{a: number, b: number, cells: number[][], bin: number[]|null}}
 *   how far the bubble reaches into the screen (a) and half its height (b);
 *   the top left corners of the files' places, in the order they fill; and
 *   the Recycle Bin's top left corner. All from the bubble's top left
 *   corner, where the screen's edge meets its top.
 */
export function bubbleShape({width, height, count, bin = null, scale = 1}) {
    const maxA = Math.floor(width / 2);
    const maxB = Math.floor(height / 2 - 6 * scale);
    const wanted = Math.max(count, 1);
    for (let a = Math.min(MIN_WIDTH * scale, maxA); ; a = Math.min(a + STEP * scale, maxA)) {
        const b = Math.round(Math.min(Math.max(a / ASPECT, MIN_HEIGHT * scale), maxB));
        const spot = bin ? binSpot(a, b, bin, scale) : null;
        const gap = BIN_GAP * scale;
        const avoid = spot && [spot[0] - gap, spot[1] - gap, bin[0] + 2 * gap, bin[1] + 2 * gap];
        const cells = cellsIn(a, b, avoid, scale);
        if (cells.length >= wanted || a >= maxA)
            return {a: Math.round(a), b, cells, bin: spot};
    }
}

// Whether a point is in the ellipse: x from the left edge, y from the middle
function inside(x, y, a, b) {
    return (x / a) ** 2 + (y / b) ** 2 <= 1;
}

// The Recycle Bin's top left corner: on a line out from the middle of the
// left edge, down and to the right, as far out as it fits inside the edge
function binSpot(a, b, [width, height], scale) {
    const edge = EDGE * scale;
    const ra = a - edge;
    const rb = b - edge;
    let spot = null;
    for (let t = 0.96; t > 0.2; t -= 0.02) {
        const x = t * ra * Math.cos(BIN_ANGLE);
        const y = t * rb * Math.sin(BIN_ANGLE);
        const [left, right] = [x - width / 2, x + width / 2];
        const [top, bottom] = [y - height / 2, y + height / 2];
        spot = [Math.max(left, edge), b + top];
        if (left >= edge && [[left, top], [left, bottom], [right, top], [right, bottom]]
            .every(([px, py]) => inside(px, py, ra, rb)))
            break;
    }
    return spot.map(Math.round);
}

// The files' places: rows across the middle, each as long as the curve
// allows, leaving the Recycle Bin's spot clear. They fill down the left
// side first, then the next column, as on Windows' desktop.
function cellsIn(a, b, avoid, scale) {
    const [width, height] = [CELL_WIDTH * scale, CELL_HEIGHT * scale];
    const edge = EDGE * scale;
    const [ra, rb] = [a - edge, b - edge];
    const rows = Math.floor(2 * rb / height);
    const top = b - rows * height / 2;
    const cells = [];
    for (let row = 0; row < rows; row++) {
        const y = top + row * height;
        // The row's edge farthest from the middle sets how far it reaches
        const far = Math.max(Math.abs(y - b), Math.abs(y + height - b));
        const reach = ra * Math.sqrt(Math.max(0, 1 - (far / rb) ** 2));
        for (let x = INSET * scale; x + width <= reach; x += width) {
            if (avoid && x < avoid[0] + avoid[2] && x + width > avoid[0] &&
                y < avoid[1] + avoid[3] && y + height > avoid[1])
                continue;
            cells.push([Math.round(x), Math.round(y)]);
        }
    }
    return cells.sort((p, q) => p[0] - q[0] || p[1] - q[1]);
}
