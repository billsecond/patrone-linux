//! The desktop bubble's shape (the user's design, 2026-10-26): half an
//! ellipse coming out of the screen's left edge, as small as its files allow
//! and at most half the screen wide, and where the files and the Recycle
//! Bin go in it. The taskbar drew it until File Manager took the desktop
//! over (2026-10-27); this is its `bubbleShape.js`, with the same sizes.

/// A file's place: its icon and two lines of its name.
pub const CELL_WIDTH: f64 = 92.0;
pub const CELL_HEIGHT: f64 = 100.0;
/// From the screen's edge to the first column, and from the files to the
/// bubble's curved edge.
const INSET: f64 = 20.0;
const EDGE: f64 = 12.0;
/// The smallest bubble, for one file or none.
const MIN_WIDTH: f64 = 250.0;
const MIN_HEIGHT: f64 = 210.0;
/// How much wider than tall it grows, until it's as tall as the screen.
const ASPECT: f64 = 1.25;
const STEP: f64 = 8.0;
/// Where on the edge the Recycle Bin goes: this far below the middle.
const BIN_ANGLE: f64 = std::f64::consts::FRAC_PI_4;
/// Room kept clear around the Recycle Bin.
const BIN_GAP: f64 = 4.0;
/// The soft shadow around the curved edge, inside the bubble's window.
pub const SHADOW: f64 = 12.0;

#[derive(Clone, Debug, PartialEq)]
pub struct Shape {
    /// How far the bubble reaches into the screen.
    pub a: f64,
    /// Half its height.
    pub b: f64,
    /// The top left corners of the files' places, in the order they fill,
    /// from the bubble's top left corner (where the screen's edge meets
    /// its top).
    pub cells: Vec<(f64, f64)>,
    /// The Recycle Bin's top left corner, from the same corner.
    pub bin: Option<(f64, f64)>,
}

/// The bubble for `count` files in a free area `width` by `height`, with
/// the Recycle Bin's size when it rides on the edge.
pub fn shape(width: f64, height: f64, count: usize, bin: Option<(f64, f64)>) -> Shape {
    let max_a = (width / 2.0).floor();
    let max_b = (height / 2.0 - 6.0).floor();
    let wanted = count.max(1);
    let mut a = MIN_WIDTH.min(max_a);
    loop {
        let b = (a / ASPECT).max(MIN_HEIGHT).min(max_b).round();
        let spot = bin.map(|size| bin_spot(a, b, size));
        let avoid = spot.zip(bin).map(|((x, y), (w, h))| {
            (
                x - BIN_GAP,
                y - BIN_GAP,
                w + 2.0 * BIN_GAP,
                h + 2.0 * BIN_GAP,
            )
        });
        let cells = cells_in(a, b, avoid);
        if cells.len() >= wanted || a >= max_a {
            return Shape {
                a: a.round(),
                b,
                cells,
                bin: spot,
            };
        }
        a = (a + STEP).min(max_a);
    }
}

/// The Recycle Bin's top left corner: centred on the curved edge, down and
/// to the right of the middle, half in the bubble and half out (the user,
/// 2026-10-27).
fn bin_spot(a: f64, b: f64, (width, height): (f64, f64)) -> (f64, f64) {
    let x = a * BIN_ANGLE.cos();
    let y = b * BIN_ANGLE.sin();
    ((x - width / 2.0).round(), (b + y - height / 2.0).round())
}

/// The files' places: rows across the middle, each as long as the curve
/// allows, leaving the Recycle Bin's spot clear. They fill down the left
/// side first, then the next column, as on Windows' desktop.
fn cells_in(a: f64, b: f64, avoid: Option<(f64, f64, f64, f64)>) -> Vec<(f64, f64)> {
    let (ra, rb) = (a - EDGE, b - EDGE);
    let rows = (2.0 * rb / CELL_HEIGHT).floor().max(0.0) as usize;
    let top = b - rows as f64 * CELL_HEIGHT / 2.0;
    let mut cells = Vec::new();
    for row in 0..rows {
        let y = top + row as f64 * CELL_HEIGHT;
        // The row's edge farthest from the middle sets how far it reaches.
        let far = (y - b).abs().max((y + CELL_HEIGHT - b).abs());
        let reach = ra * (1.0 - (far / rb).powi(2)).max(0.0).sqrt();
        let mut x = INSET;
        while x + CELL_WIDTH <= reach {
            let blocked = avoid.is_some_and(|(ax, ay, aw, ah)| {
                x < ax + aw && x + CELL_WIDTH > ax && y < ay + ah && y + CELL_HEIGHT > ay
            });
            if !blocked {
                cells.push((x.round(), y.round()));
            }
            x += CELL_WIDTH;
        }
    }
    cells.sort_by(|p, q| p.0.total_cmp(&q.0).then(p.1.total_cmp(&q.1)));
    cells
}

/// Whether a point, from the bubble's top left corner, is inside it.
pub fn contains(a: f64, b: f64, x: f64, y: f64) -> bool {
    let dy = y - b;
    x >= 0.0 && (x / a).powi(2) + (dy / b).powi(2) <= 1.0
}

#[cfg(test)]
mod tests {
    use super::*;

    const BIN: Option<(f64, f64)> = Some((92.0, 85.0));

    #[test]
    fn one_file_gets_the_smallest_bubble() {
        let shape = shape(1600.0, 858.0, 1, BIN);
        assert_eq!(shape.a, 250.0);
        assert_eq!(shape.b, 210.0);
        assert!(!shape.cells.is_empty());
    }

    #[test]
    fn it_grows_with_the_files_up_to_half_the_screen() {
        let small = shape(1600.0, 858.0, 7, BIN);
        let large = shape(1600.0, 858.0, 20, BIN);
        assert!(small.cells.len() >= 7);
        assert!(large.cells.len() >= 20);
        assert!(large.a > small.a);
        let full = shape(1600.0, 858.0, 500, BIN);
        assert_eq!(full.a, 800.0);
        assert!(full.cells.len() < 500);
    }

    #[test]
    fn files_fill_down_the_left_side_first() {
        let shape = shape(1600.0, 858.0, 20, None);
        let (x0, y0) = shape.cells[0];
        let (x1, y1) = shape.cells[1];
        assert_eq!(x0, x1);
        assert!(y1 > y0);
    }

    #[test]
    fn no_file_sits_under_the_recycle_bin() {
        let shape = shape(1600.0, 858.0, 60, BIN);
        let (bx, by) = shape.bin.expect("the bin's spot");
        for (x, y) in shape.cells {
            let overlaps =
                x < bx + 92.0 && x + CELL_WIDTH > bx && y < by + 85.0 && y + CELL_HEIGHT > by;
            assert!(!overlaps, "a file at {x},{y} is under the bin at {bx},{by}");
        }
    }

    #[test]
    fn the_bin_sits_half_over_the_edge() {
        let shape = shape(1600.0, 858.0, 10, BIN);
        let (bx, by) = shape.bin.expect("the bin's spot");
        let (cx, cy) = (bx + 46.0, by + 42.5);
        let edge = (cx / shape.a).powi(2) + ((cy - shape.b) / shape.b).powi(2);
        assert!(
            (edge - 1.0).abs() < 0.02,
            "the bin's middle is at {edge} of the edge"
        );
    }

    #[test]
    fn the_middle_is_inside_and_the_corners_are_not() {
        assert!(contains(250.0, 210.0, 10.0, 210.0));
        assert!(!contains(250.0, 210.0, 240.0, 10.0));
        assert!(!contains(250.0, 210.0, -1.0, 210.0));
    }
}
