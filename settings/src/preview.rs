//! The little pictures on Patrone Settings' choices: the taskbar at each
//! size, where each arrangement puts the title bar buttons, and each theme's
//! taskbar and title bar. Drawn with cairo, at whatever size they're given.

use std::f64::consts::PI;

use gtk::cairo::{Context, FontSlant, FontWeight, LinearGradient};

/// Which corner the title bar buttons go in.
#[derive(Clone, Copy)]
pub enum Layout {
    Right,
    Left,
}

#[derive(Clone, Copy, PartialEq)]
enum Button {
    Minimize,
    Maximize,
    Close,
}

fn color(cr: &Context, rgb: u32, alpha: f64) {
    let channel = |shift: u32| f64::from((rgb >> shift) & 0xff) / 255.0;
    cr.set_source_rgba(channel(16), channel(8), channel(0), alpha);
}

fn rounded(cr: &Context, x: f64, y: f64, w: f64, h: f64, r: f64) {
    let r = r.min(w / 2.0).min(h / 2.0);
    cr.new_sub_path();
    cr.arc(x + w - r, y + r, r, -PI / 2.0, 0.0);
    cr.arc(x + w - r, y + h - r, r, 0.0, PI / 2.0);
    cr.arc(x + r, y + h - r, r, PI / 2.0, PI);
    cr.arc(x + r, y + r, r, PI, 1.5 * PI);
    cr.close_path();
}

fn fill_rounded(cr: &Context, x: f64, y: f64, w: f64, h: f64, r: f64, rgb: u32, alpha: f64) {
    rounded(cr, x, y, w, h, r);
    color(cr, rgb, alpha);
    let _ = cr.fill();
}

fn vertical_gradient(cr: &Context, y: f64, h: f64, top: u32, bottom: u32) {
    let gradient = LinearGradient::new(0.0, y, 0.0, y + h);
    for (offset, rgb) in [(0.0, top), (1.0, bottom)] {
        let channel = |shift: u32| f64::from((rgb >> shift) & 0xff) / 255.0;
        gradient.add_color_stop_rgb(offset, channel(16), channel(8), channel(0));
    }
    let _ = cr.set_source(&gradient);
}

/// Patrone's desktop: navy to black, with a streak of blue light.
fn desktop(cr: &Context, w: f64, h: f64) {
    rounded(cr, 0.0, 0.0, w, h, 8.0);
    cr.clip();
    let gradient = LinearGradient::new(0.0, 0.0, w, h);
    gradient.add_color_stop_rgb(0.0, 0.06, 0.12, 0.22);
    gradient.add_color_stop_rgb(1.0, 0.01, 0.01, 0.03);
    let _ = cr.set_source(&gradient);
    let _ = cr.paint();
    cr.move_to(w * 0.58, 0.0);
    cr.line_to(w * 0.70, 0.0);
    cr.line_to(w * 0.32, h);
    cr.line_to(w * 0.20, h);
    cr.close_path();
    color(cr, 0x288bfa, 0.22);
    let _ = cr.fill();
}

fn text(cr: &Context, x: f64, y: f64, size: f64, rgb: u32, weight: FontWeight, slant: FontSlant, label: &str) {
    cr.select_font_face("Sans", slant, weight);
    cr.set_font_size(size);
    color(cr, rgb, 1.0);
    cr.move_to(x, y);
    let _ = cr.show_text(label);
}

fn text_width(cr: &Context, size: f64, weight: FontWeight, label: &str) -> f64 {
    cr.select_font_face("Sans", FontSlant::Normal, weight);
    cr.set_font_size(size);
    cr.text_extents(label).map(|e| e.x_advance()).unwrap_or(0.0)
}

/// A minimize, maximize or close symbol, centered on (cx, cy).
fn symbol(cr: &Context, button: Button, cx: f64, cy: f64, size: f64, rgb: u32) {
    color(cr, rgb, 1.0);
    cr.set_line_width((size / 7.0).max(1.0));
    let half = size / 2.0;
    match button {
        Button::Minimize => {
            cr.move_to(cx - half, cy + half * 0.2);
            cr.line_to(cx + half, cy + half * 0.2);
        }
        Button::Maximize => cr.rectangle(cx - half * 0.85, cy - half * 0.85, size * 0.85, size * 0.85),
        Button::Close => {
            cr.move_to(cx - half * 0.85, cy - half * 0.85);
            cr.line_to(cx + half * 0.85, cy + half * 0.85);
            cr.move_to(cx + half * 0.85, cy - half * 0.85);
            cr.line_to(cx - half * 0.85, cy + half * 0.85);
        }
    }
    let _ = cr.stroke();
}

fn buttons_for(layout: Layout) -> (&'static [Button], bool) {
    match layout {
        Layout::Right => (&[Button::Minimize, Button::Maximize, Button::Close], false),
        Layout::Left => (&[Button::Close, Button::Minimize, Button::Maximize], true),
    }
}

/// The title bar arrangements: a window, its title, and its buttons, with
/// close in red so it's easy to find.
pub fn buttons(cr: &Context, w: f64, h: f64, layout: Layout) {
    desktop(cr, w, h);
    let (x, y, ww, wh) = (w * 0.08, h * 0.14, w * 0.84, h * 0.72);
    fill_rounded(cr, x, y, ww, wh, 6.0, 0xffffff, 1.0);
    let bar = (wh * 0.30).max(18.0);
    fill_rounded(cr, x, y, ww, bar, 6.0, 0xeceef2, 1.0);
    color(cr, 0xeceef2, 1.0);
    cr.rectangle(x, y + bar / 2.0, ww, bar / 2.0);
    let _ = cr.fill();
    let title = "Window";
    let size = bar * 0.42;
    let width = text_width(cr, size, FontWeight::Bold, title);
    text(cr, x + (ww - width) / 2.0, y + bar * 0.66, size, 0x1b1d22, FontWeight::Bold, FontSlant::Normal, title);

    let (order, left) = buttons_for(layout);
    let cell = bar * 0.95;
    let glyph = bar * 0.34;
    for (i, button) in order.iter().enumerate() {
        let cx = if left {
            x + 6.0 + cell * (i as f64 + 0.5)
        } else {
            x + ww - 6.0 - cell * ((order.len() - i) as f64 - 0.5)
        };
        let cy = y + bar / 2.0;
        if *button == Button::Close {
            fill_rounded(cr, cx - cell * 0.42, cy - cell * 0.42, cell * 0.84, cell * 0.84, 4.0, 0xd83b2a, 1.0);
            symbol(cr, *button, cx, cy, glyph, 0xffffff);
        } else {
            symbol(cr, *button, cx, cy, glyph, 0x3a3d45);
        }
    }

    // Lines of text in the window
    for line in 0..3 {
        let ly = y + bar + 10.0 + f64::from(line) * 9.0;
        if ly + 4.0 > y + wh - 4.0 {
            break;
        }
        fill_rounded(cr, x + 10.0, ly, ww * [0.7, 0.55, 0.62][line as usize], 4.0, 2.0, 0xdfe2e8, 1.0);
    }
}

/// App icons on a taskbar: colored squares.
fn app_icons(cr: &Context, x: f64, y: f64, size: f64, count: usize, dots: Option<u32>) -> f64 {
    const COLORS: [u32; 5] = [0x3d8eff, 0x2d3038, 0x7c8494, 0x5aa0ff, 0x4c9a55];
    let gap = size * 0.55;
    let mut left = x;
    for i in 0..count {
        fill_rounded(cr, left, y, size, size, size * 0.25, COLORS[i % COLORS.len()], 1.0);
        if let Some(rgb) = dots {
            if i < 2 {
                fill_rounded(cr, left + size / 2.0 - 1.5, y + size + 2.0, 3.0, 3.0, 1.5, rgb, 1.0);
            }
        }
        left += size + gap;
    }
    left - gap
}

/// The Patrone "P", simplified.
fn start_mark(cr: &Context, x: f64, y: f64, size: f64) {
    color(cr, 0x3d8eff, 1.0);
    cr.move_to(x + size * 0.15, y);
    cr.line_to(x + size * 0.85, y);
    cr.line_to(x + size, y + size * 0.4);
    cr.line_to(x + size * 0.45, y + size * 0.4);
    cr.close_path();
    let _ = cr.fill();
    cr.move_to(x + size * 0.3, y + size * 0.5);
    cr.line_to(x + size * 0.6, y + size * 0.5);
    cr.line_to(x + size * 0.3, y + size);
    cr.line_to(x, y + size);
    cr.close_path();
    let _ = cr.fill();
}

/// The taskbar at each size: bar height and icon size in the picture.
pub fn taskbar_size(cr: &Context, w: f64, h: f64, bar: f64, icon: f64) {
    desktop(cr, w, h);
    fill_rounded(cr, w * 0.16, h * 0.12, w * 0.6, h * 0.5, 4.0, 0xffffff, 0.92);
    color(cr, 0xf3f4f7, 0.96);
    cr.rectangle(0.0, h - bar, w, bar);
    let _ = cr.fill();
    let y = h - bar + (bar - icon) / 2.0;
    start_mark(cr, 6.0, y, icon);
    app_icons(cr, 8.0 + icon * 1.6, y, icon, 4, None);
    // The clock
    fill_rounded(cr, w - icon * 2.2 - 6.0, y + icon * 0.1, icon * 2.2, icon * 0.32, 1.5, 0x5a5f6b, 1.0);
    fill_rounded(cr, w - icon * 2.2 - 6.0, y + icon * 0.58, icon * 2.2, icon * 0.32, 1.5, 0x8a8f99, 1.0);
}

/// A theme: a window's title bar and the taskbar, as the theme draws them.
pub fn theme(cr: &Context, w: f64, h: f64, theme: &str) {
    desktop(cr, w, h);

    // The window
    let (x, y, ww, wh) = (w * 0.10, h * 0.09, w * 0.66, h * 0.56);
    let bar = (wh * 0.24).max(14.0);
    let radius = if theme == "classic" { 3.0 } else { 6.0 };
    fill_rounded(cr, x, y, ww, wh, radius, 0xffffff, 1.0);
    match theme {
        "classic" => {
            rounded(cr, x, y, ww, bar, radius);
            vertical_gradient(cr, y, bar, 0x3a8cff, 0x0846b8);
            let _ = cr.fill();
            text(cr, x + 6.0, y + bar * 0.68, bar * 0.5, 0xffffff, FontWeight::Bold, FontSlant::Normal, "Window");
        }
        _ => {
            fill_rounded(cr, x, y, ww, bar, radius, if theme == "studio" { 0xe9e9ec } else { 0xf0f1f4 }, 1.0);
            color(cr, 0xd9dce2, 1.0);
            cr.rectangle(x, y + bar - 1.0, ww, 1.0);
            let _ = cr.fill();
        }
    }
    let cy = y + bar / 2.0;
    let unit = bar * 0.62;
    match theme {
        "studio" => {
            for (i, rgb) in [0xff5f57, 0xfebc2e, 0x28c840].iter().enumerate() {
                fill_rounded(cr, x + 6.0 + i as f64 * unit * 1.25, cy - unit / 2.0, unit, unit, unit / 2.0, *rgb, 1.0);
            }
        }
        "classic" => {
            for i in 0..3 {
                let bx = x + ww - 4.0 - (3 - i) as f64 * (unit + 2.0);
                rounded(cr, bx, cy - unit / 2.0, unit, unit, 2.0);
                vertical_gradient(cr, cy - unit / 2.0, unit, if i == 2 { 0xf2967a } else { 0x5b9dff },
                    if i == 2 { 0xc8381b } else { 0x1f63d8 });
                let _ = cr.fill_preserve();
                color(cr, 0xffffff, 1.0);
                cr.set_line_width(1.0);
                let _ = cr.stroke();
                let button = [Button::Minimize, Button::Maximize, Button::Close][i];
                symbol(cr, button, bx + unit / 2.0, cy, unit * 0.45, 0xffffff);
            }
        }
        "modern" => {
            for (i, button) in [Button::Minimize, Button::Maximize, Button::Close].iter().enumerate() {
                let cx = x + ww - (3 - i) as f64 * unit * 1.6 + unit * 0.5;
                if *button == Button::Close {
                    color(cr, 0xc42b1c, 1.0);
                    cr.rectangle(cx - unit * 0.8, y, unit * 1.6, bar - 1.0);
                    let _ = cr.fill();
                    symbol(cr, *button, cx, cy, unit * 0.5, 0xffffff);
                } else {
                    symbol(cr, *button, cx, cy, unit * 0.5, 0x3a3d45);
                }
            }
        }
        _ => {
            for (i, button) in [Button::Minimize, Button::Maximize, Button::Close].iter().enumerate() {
                let cx = x + ww - 6.0 - (3 - i) as f64 * unit * 1.3 + unit * 0.65;
                fill_rounded(cr, cx - unit / 2.0, cy - unit / 2.0, unit, unit, unit / 2.0, 0xdfe1e6, 1.0);
                symbol(cr, *button, cx, cy, unit * 0.42, 0x3a3d45);
            }
        }
    }

    // The taskbar
    let tb = (h * 0.17).max(14.0);
    let ty = h - tb;
    let icon = tb * 0.56;
    let iy = ty + (tb - icon) / 2.0;
    match theme {
        "classic" => {
            vertical_gradient(cr, ty, tb, 0x3b7ff0, 0x1d4fc4);
            cr.rectangle(0.0, ty, w, tb);
            let _ = cr.fill();
            // The green Start button
            let start = w * 0.24;
            rounded(cr, -6.0, ty, start + 6.0, tb, tb / 2.0);
            vertical_gradient(cr, ty, tb, 0x45a845, 0x2c7f2c);
            let _ = cr.fill();
            start_mark(cr, 4.0, iy, icon);
            text(cr, 6.0 + icon * 1.2, ty + tb * 0.68, tb * 0.5, 0xffffff, FontWeight::Bold, FontSlant::Italic, "start");
            app_icons(cr, start + 6.0, iy, icon, 3, None);
            // The tray
            vertical_gradient(cr, ty, tb, 0x1594f0, 0x0d6fd6);
            cr.rectangle(w * 0.78, ty, w * 0.22, tb);
            let _ = cr.fill();
        }
        "studio" => {
            color(cr, 0xf6f6f8, 0.7);
            cr.rectangle(0.0, ty, w, tb);
            let _ = cr.fill();
            let count = 4;
            let shelf = icon * count as f64 + icon * 0.55 * (count - 1) as f64 + icon * 1.4;
            let sx = (w - shelf) / 2.0;
            fill_rounded(cr, sx, ty + 2.0, shelf, tb - 4.0, (tb - 4.0) * 0.3, 0xffffff, 0.75);
            start_mark(cr, sx + icon * 0.35, iy - 1.0, icon);
            app_icons(cr, sx + icon * 1.65, iy - 1.0, icon, count - 1, Some(0x3a3a3c));
        }
        "modern" => {
            color(cr, 0xf3f4f7, 0.96);
            cr.rectangle(0.0, ty, w, tb);
            let _ = cr.fill();
            let count = 5;
            let width = icon * count as f64 + icon * 0.55 * (count - 1) as f64;
            let sx = (w - width) / 2.0;
            start_mark(cr, sx, iy, icon);
            app_icons(cr, sx + icon * 1.55, iy, icon, count - 1, None);
        }
        _ => {
            color(cr, 0xf3f4f7, 0.96);
            cr.rectangle(0.0, ty, w, tb);
            let _ = cr.fill();
            start_mark(cr, 5.0, iy, icon);
            app_icons(cr, 7.0 + icon * 1.55, iy, icon, 4, None);
        }
    }
}
