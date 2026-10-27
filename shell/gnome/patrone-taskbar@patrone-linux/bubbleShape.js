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
        const spot = bin ? binSpot(a, b, bin) : null;
        const gap = BIN_GAP * scale;
        const avoid = spot && [spot[0] - gap, spot[1] - gap, bin[0] + 2 * gap, bin[1] + 2 * gap];
        const cells = cellsIn(a, b, avoid, scale);
        if (cells.length >= wanted || a >= maxA)
            return {a: Math.round(a), b, cells, bin: spot};
    }
}

// The Recycle Bin's top left corner: centred on the curved edge, down and
// to the right of the middle, half in the bubble and half out (the user,
// 2026-10-27; it used to sit just inside the edge)
function binSpot(a, b, [width, height]) {
    const x = a * Math.cos(BIN_ANGLE);
    const y = b * Math.sin(BIN_ANGLE);
    return [Math.round(x - width / 2), Math.round(b + y - height / 2)];
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
