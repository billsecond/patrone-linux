//! The little pictures on Patrone Settings' choices: the taskbar at each
//! size, in each layout, in each color and with each kind of pin icon,
//! where each arrangement puts the title bar buttons, each theme's
//! taskbar and title bar, and each mouse pointer, from its own pictures.
//! Drawn with cairo, at whatever size they're given.

use std::f64::consts::PI;

use gtk::cairo::{Context, Filter, FontSlant, FontWeight, Format, ImageSurface, LineCap, LineJoin, LinearGradient};
use gtk::gdk::prelude::GdkCairoContextExt;
use gtk::gdk_pixbuf::Pixbuf;

use crate::cursor::Cursor;

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
    // The title, as a short bar in the middle: a word ran into the buttons
    let title = ww * 0.16;
    fill_rounded(cr, x + (ww - title) / 2.0, y + bar / 2.0 - 2.0, title, 4.0, 2.0, 0x9aa0aa, 1.0);

    let (order, left) = buttons_for(layout);
    let cell = bar * 0.75;
    let glyph = bar * 0.3;
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

/// One close button (or minimize, maximize) in a title bar button style,
/// centered on (cx, cy) in a title bar `bar` tall whose text is `ink`.
fn styled_button(cr: &Context, style: &str, button: Button, cx: f64, cy: f64, bar: f64, ink: u32) {
    match style {
        "wide" => {
            if button == Button::Close {
                // As under the pointer: red
                let cell = bar * 1.3;
                color(cr, 0xc42b1c, 1.0);
                cr.rectangle(cx - cell / 2.0, cy - bar / 2.0, cell, bar);
                let _ = cr.fill();
                symbol(cr, button, cx, cy, bar * 0.3, 0xffffff);
            } else {
                symbol(cr, button, cx, cy, bar * 0.3, ink);
            }
        }
        "round" => {
            let unit = bar * 0.62;
            fill_rounded(cr, cx - unit / 2.0, cy - unit / 2.0, unit, unit, unit / 2.0, ink, 0.14);
            symbol(cr, button, cx, cy, unit * 0.42, ink);
        }
        "dots" => {
            let unit = bar * 0.42;
            let rgb = match button {
                Button::Minimize => 0xfebc2e,
                Button::Maximize => 0x28c840,
                Button::Close => 0xff5f57,
            };
            fill_rounded(cr, cx - unit / 2.0, cy - unit / 2.0, unit, unit, unit / 2.0, rgb, 1.0);
        }
        _ => {
            // Glossy: a framed square, blue, or red to close
            let unit = bar * 0.62;
            let (top, bottom) = if button == Button::Close { (0xf2967a, 0xc8381b) } else { (0x5b9dff, 0x1f63d8) };
            rounded(cr, cx - unit / 2.0, cy - unit / 2.0, unit, unit, 2.0);
            vertical_gradient(cr, cy - unit / 2.0, unit, top, bottom);
            let _ = cr.fill_preserve();
            color(cr, 0xffffff, 1.0);
            cr.set_line_width(1.0);
            let _ = cr.stroke();
            symbol(cr, button, cx, cy, unit * 0.45, 0xffffff);
        }
    }
}

/// A title bar button style: a window's title bar with its buttons. "theme"
/// (each theme's own) shows one close button in each style instead.
pub fn button_style(cr: &Context, w: f64, h: f64, style: &str) {
    desktop(cr, w, h);
    let (x, y, ww, wh) = (w * 0.08, h * 0.14, w * 0.84, h * 0.72);
    fill_rounded(cr, x, y, ww, wh, 6.0, 0xffffff, 1.0);
    let bar = (wh * 0.30).max(18.0);
    let _ = cr.save();
    rounded(cr, x, y, ww, wh, 6.0);
    cr.clip();
    color(cr, 0xeceef2, 1.0);
    cr.rectangle(x, y, ww, bar);
    let _ = cr.fill();
    fill_rounded(cr, x + 10.0, y + bar / 2.0 - 2.0, ww * 0.16, 4.0, 2.0, 0x9aa0aa, 1.0);

    let cy = y + bar / 2.0;
    let right = x + ww;
    if style == "theme" {
        for (i, each) in ["wide", "round", "dots", "glossy"].iter().enumerate() {
            let cx = right - bar * 0.75 - (3 - i) as f64 * bar * 1.15;
            styled_button(cr, each, Button::Close, cx, cy, bar, 0x3a3d45);
        }
    } else {
        let cell = if style == "wide" { bar * 1.3 } else { bar * 0.8 };
        for (i, button) in [Button::Minimize, Button::Maximize, Button::Close].iter().enumerate() {
            let cx = right - cell * (3 - i) as f64 + cell / 2.0 - if style == "wide" { 0.0 } else { 4.0 };
            styled_button(cr, style, *button, cx, cy, bar, 0x3a3d45);
        }
    }
    let _ = cr.restore();

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

/// The Patrone "P", simplified, in Patrone blue or, on a colored Start
/// button, white.
fn start_mark(cr: &Context, x: f64, y: f64, size: f64, rgb: u32) {
    color(cr, rgb, 1.0);
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
    start_mark(cr, 6.0, y, icon, 0x3d8eff);
    app_icons(cr, 8.0 + icon * 1.6, y, icon, 4, None);
    clock(cr, w, y, icon);
}

/// The clock at the right of a taskbar: the time over the date.
fn clock(cr: &Context, w: f64, iy: f64, icon: f64) {
    let width = icon * 2.2;
    fill_rounded(cr, w - width - 6.0, iy + icon * 0.1, width, icon * 0.32, 1.5, 0x5a5f6b, 1.0);
    fill_rounded(cr, w - width - 6.0, iy + icon * 0.58, width, icon * 0.32, 1.5, 0x8a8f99, 1.0);
}

/// A taskbar layout: the bar along the bottom or the top, and where Start
/// and the apps go on it.
pub fn taskbar_layout(cr: &Context, w: f64, h: f64, layout: &str) {
    desktop(cr, w, h);
    let bar = (h * 0.17).max(14.0);
    let top = layout == "top";
    let ty = if top { 0.0 } else { h - bar };
    let window_y = if top { bar + h * 0.1 } else { h * 0.1 };
    fill_rounded(cr, w * 0.16, window_y, w * 0.6, h * 0.5, 4.0, 0xffffff, 0.92);
    color(cr, 0xf3f4f7, 0.96);
    cr.rectangle(0.0, ty, w, bar);
    let _ = cr.fill();

    let icon = bar * 0.56;
    let iy = ty + (bar - icon) / 2.0;
    let gap = icon * 0.55;
    let apps = 4;
    let apps_width = icon * apps as f64 + gap * (apps - 1) as f64;
    match layout {
        "center" => {
            let x = (w - icon - gap - apps_width) / 2.0;
            start_mark(cr, x, iy, icon, 0x3d8eff);
            app_icons(cr, x + icon + gap, iy, icon, apps, None);
        }
        "split" => {
            start_mark(cr, 5.0, iy, icon, 0x3d8eff);
            app_icons(cr, (w - apps_width) / 2.0, iy, icon, apps, None);
        }
        _ => {
            start_mark(cr, 5.0, iy, icon, 0x3d8eff);
            app_icons(cr, 5.0 + icon + gap, iy, icon, apps, None);
        }
    }
    clock(cr, w, iy, icon);
}

/// Where the taskbar's color comes from: the navy of Patrone's wallpaper,
/// or the theme's grey, which matches nothing in it.
pub fn taskbar_color(cr: &Context, w: f64, h: f64, source: &str) {
    desktop(cr, w, h);
    fill_rounded(cr, w * 0.16, h * 0.1, w * 0.6, h * 0.5, 4.0, 0xffffff, 0.92);
    let bar = (h * 0.2).max(16.0);
    let ty = h - bar;
    let (rgb, line, line_alpha) = if source == "wallpaper" { (0x0f1c2e, 0x618fd1, 0.6) } else { (0x26262b, 0xffffff, 0.1) };
    color(cr, rgb, 0.97);
    cr.rectangle(0.0, ty, w, bar);
    let _ = cr.fill();
    color(cr, line, line_alpha);
    cr.rectangle(0.0, ty, w, 1.0);
    let _ = cr.fill();

    let icon = bar * 0.56;
    let iy = ty + (bar - icon) / 2.0;
    start_mark(cr, 5.0, iy, icon, 0x3d8eff);
    app_icons(cr, 5.0 + icon * 1.55, iy, icon, 4, None);
    clock(cr, w, iy, icon);
}

/// The taskbar's web, terminal, files and mail pins, close up: Patrone's
/// icons, or (for "app") the apps' own, each in its own style.
pub fn pin_icons(cr: &Context, w: f64, h: f64, icons: &str) {
    desktop(cr, w, h);
    let bar = h * 0.42;
    let ty = h - bar;
    color(cr, 0xf3f4f7, 0.96);
    cr.rectangle(0.0, ty, w, bar);
    let _ = cr.fill();

    let size = bar * 0.58;
    let gap = size * 0.5;
    let mut x = (w - size * 4.0 - gap * 3.0) / 2.0;
    let y = ty + (bar - size) / 2.0;
    cr.set_line_cap(LineCap::Round);
    cr.set_line_join(LineJoin::Round);
    for job in ["web", "terminal", "files", "mail"] {
        if icons == "app" {
            app_pin(cr, job, x, y, size);
        } else {
            patrone_pin(cr, job, x, y, size);
        }
        x += size + gap;
    }
}

/// Patrone's icon for a pin's job, as the taskbar's icons folder draws it.
fn patrone_pin(cr: &Context, job: &str, x: f64, y: f64, s: f64) {
    let (cx, cy) = (x + s / 2.0, y + s / 2.0);
    match job {
        "web" => {
            cr.arc(cx, cy, s * 0.46, 0.0, 2.0 * PI);
            color(cr, 0x3d8eff, 1.0);
            let _ = cr.fill();
            color(cr, 0xffffff, 1.0);
            cr.set_line_width(s * 0.07);
            let _ = cr.save();
            cr.translate(cx, cy);
            cr.scale(0.45, 1.0);
            cr.arc(0.0, 0.0, s * 0.42, 0.0, 2.0 * PI);
            let _ = cr.restore();
            cr.move_to(cx, cy - s * 0.42);
            cr.line_to(cx, cy + s * 0.42);
            cr.move_to(cx - s * 0.42, cy);
            cr.line_to(cx + s * 0.42, cy);
            let _ = cr.stroke();
        }
        "terminal" => {
            fill_rounded(cr, x, y + s * 0.06, s, s * 0.88, s * 0.16, 0x56647e, 1.0);
            fill_rounded(cr, x + s * 0.08, y + s * 0.2, s * 0.84, s * 0.66, s * 0.08, 0x141b27, 1.0);
            color(cr, 0xffffff, 1.0);
            cr.set_line_width(s * 0.08);
            cr.move_to(x + s * 0.22, y + s * 0.36);
            cr.line_to(x + s * 0.38, y + s * 0.5);
            cr.line_to(x + s * 0.22, y + s * 0.64);
            let _ = cr.stroke();
            fill_rounded(cr, x + s * 0.46, y + s * 0.6, s * 0.28, s * 0.08, s * 0.04, 0x4da0fc, 1.0);
        }
        "files" => {
            fill_rounded(cr, x, y + s * 0.12, s * 0.45, s * 0.3, s * 0.06, 0xdd961a, 1.0);
            fill_rounded(cr, x, y + s * 0.2, s, s * 0.7, s * 0.07, 0xdd961a, 1.0);
            fill_rounded(cr, x + s * 0.08, y + s * 0.26, s * 0.84, s * 0.2, s * 0.02, 0xffffff, 0.95);
            let _ = cr.save();
            rounded(cr, x, y + s * 0.34, s, s * 0.56, s * 0.07);
            cr.clip();
            color(cr, 0xfec94a, 1.0);
            let _ = cr.paint();
            color(cr, 0x2a74e0, 1.0);
            cr.rectangle(x, y + s * 0.72, s, s * 0.2);
            let _ = cr.fill();
            let _ = cr.restore();
        }
        _ => {
            fill_rounded(cr, x, y + s * 0.16, s, s * 0.68, s * 0.12, 0x3d8eff, 1.0);
            color(cr, 0xffffff, 1.0);
            cr.set_line_width(s * 0.08);
            cr.move_to(x + s * 0.16, y + s * 0.3);
            cr.line_to(cx, y + s * 0.54);
            cr.line_to(x + s * 0.84, y + s * 0.3);
            let _ = cr.stroke();
        }
    }
}

/// Stand-ins for other apps' icons, each in a style of its own.
fn app_pin(cr: &Context, job: &str, x: f64, y: f64, s: f64) {
    let (cx, cy) = (x + s / 2.0, y + s / 2.0);
    match job {
        "web" => {
            cr.arc(cx, cy, s * 0.46, 0.0, 2.0 * PI);
            color(cr, 0xff7139, 1.0);
            let _ = cr.fill();
            cr.arc(cx + s * 0.06, cy - s * 0.04, s * 0.26, 0.0, 2.0 * PI);
            color(cr, 0x8f4cf0, 1.0);
            let _ = cr.fill();
        }
        "terminal" => {
            fill_rounded(cr, x + s * 0.04, y + s * 0.04, s * 0.92, s * 0.92, s * 0.2, 0x1e1e1e, 1.0);
            color(cr, 0xd0d0d0, 1.0);
            cr.set_line_width(s * 0.07);
            cr.move_to(x + s * 0.24, y + s * 0.34);
            cr.line_to(x + s * 0.4, y + s * 0.48);
            cr.line_to(x + s * 0.24, y + s * 0.62);
            cr.move_to(x + s * 0.48, y + s * 0.66);
            cr.line_to(x + s * 0.72, y + s * 0.66);
            let _ = cr.stroke();
        }
        "files" => {
            fill_rounded(cr, x, y + s * 0.14, s * 0.42, s * 0.26, s * 0.06, 0x3f6fb5, 1.0);
            fill_rounded(cr, x, y + s * 0.22, s, s * 0.66, s * 0.08, 0x3f6fb5, 1.0);
            fill_rounded(cr, x, y + s * 0.34, s, s * 0.54, s * 0.08, 0x6d9be0, 1.0);
        }
        _ => {
            fill_rounded(cr, x, y + s * 0.18, s, s * 0.64, s * 0.06, 0xffffff, 1.0);
            rounded(cr, x + 0.5, y + s * 0.18 + 0.5, s - 1.0, s * 0.64 - 1.0, s * 0.06);
            color(cr, 0xa9afba, 1.0);
            cr.set_line_width(1.0);
            let _ = cr.stroke();
            color(cr, 0xd9443a, 1.0);
            cr.set_line_width(s * 0.07);
            cr.move_to(x + s * 0.12, y + s * 0.28);
            cr.line_to(cx, y + s * 0.52);
            cr.line_to(x + s * 0.88, y + s * 0.28);
            let _ = cr.stroke();
        }
    }
}

/// A theme: a window's title bar and the taskbar, as the theme draws them.
pub fn theme(cr: &Context, w: f64, h: f64, theme: &str) {
    desktop(cr, w, h);

    // The window, its title bar as the theme draws it (Patrone's in the
    // dark style, where it differs most), and the theme's own buttons
    let (x, y, ww, wh) = (w * 0.10, h * 0.09, w * 0.66, h * 0.56);
    let bar = (wh * 0.24).max(14.0);
    let radius = match theme {
        "classic" => 0.0,
        "royal" => 3.0,
        _ => 6.0,
    };
    fill_rounded(cr, x, y, ww, wh, radius, 0xffffff, 1.0);
    let _ = cr.save();
    rounded(cr, x, y, ww, wh, radius);
    cr.clip();
    let ink = match theme {
        "royal" => {
            vertical_gradient(cr, y, bar, 0x3a8cff, 0x0846b8);
            cr.rectangle(x, y, ww, bar);
            let _ = cr.fill();
            0xffffff
        }
        "classic" => {
            color(cr, 0x3d4b63, 1.0);
            cr.rectangle(x, y, ww, bar);
            let _ = cr.fill();
            0xffffff
        }
        "patrone" => {
            color(cr, 0x16213a, 1.0);
            cr.rectangle(x, y, ww, bar);
            let _ = cr.fill();
            color(cr, 0x288bfa, 1.0);
            cr.rectangle(x, y + bar - 1.5, ww, 1.5);
            let _ = cr.fill();
            0xe8eef7
        }
        "studio" => {
            vertical_gradient(cr, y, bar, 0xf7f7f9, 0xdcdce2);
            cr.rectangle(x, y, ww, bar);
            let _ = cr.fill();
            color(cr, 0x000000, 0.12);
            cr.rectangle(x, y + bar - 1.0, ww, 1.0);
            let _ = cr.fill();
            0x2a2a2e
        }
        // Modern: the window's own color, with no line
        _ => 0x1b1b1b,
    };
    let title_size = bar * 0.5;
    if theme == "studio" {
        let width = title_size * 3.2;
        text(cr, x + (ww - width) / 2.0, y + bar * 0.68, title_size, ink, FontWeight::Bold, FontSlant::Normal, "Window");
    } else {
        let weight = if theme == "modern" { FontWeight::Normal } else { FontWeight::Bold };
        text(cr, x + 6.0, y + bar * 0.68, title_size, ink, weight, FontSlant::Normal, "Window");
    }

    let style = crate::theme::theme_buttons(theme);
    let cy = y + bar / 2.0;
    let cell = if style == "wide" { bar * 1.3 } else { bar * 0.8 };
    for (i, button) in [Button::Minimize, Button::Maximize, Button::Close].iter().enumerate() {
        let cx = x + ww - cell * (3 - i) as f64 + cell / 2.0 - if style == "wide" { 0.0 } else { 4.0 };
        styled_button(cr, style, *button, cx, cy, bar, ink);
    }
    let _ = cr.restore();

    // The taskbar
    let tb = (h * 0.17).max(14.0);
    let ty = h - tb;
    let icon = tb * 0.56;
    let iy = ty + (tb - icon) / 2.0;
    match theme {
        "royal" => {
            vertical_gradient(cr, ty, tb, 0x3b7ff0, 0x1d4fc4);
            cr.rectangle(0.0, ty, w, tb);
            let _ = cr.fill();
            // The green Start button
            let start = icon * 2.0;
            rounded(cr, -6.0, ty, start + 6.0, tb, tb / 2.0);
            vertical_gradient(cr, ty, tb, 0x45a845, 0x2c7f2c);
            let _ = cr.fill();
            // White on green, as the taskbar draws it
            start_mark(cr, icon * 0.45, iy, icon, 0xffffff);
            app_icons(cr, start + 6.0, iy, icon, 4, None);
            // The tray
            vertical_gradient(cr, ty, tb, 0x1594f0, 0x0d6fd6);
            cr.rectangle(w * 0.78, ty, w * 0.22, tb);
            let _ = cr.fill();
        }
        "classic" => {
            color(cr, 0xe1e4e9, 1.0);
            cr.rectangle(0.0, ty, w, tb);
            let _ = cr.fill();
            color(cr, 0xc2c7cf, 1.0);
            cr.rectangle(0.0, ty, w, 1.0);
            let _ = cr.fill();
            start_mark(cr, 5.0, iy, icon, 0x3d8eff);
            // The app in front: a square highlight with a line under it
            let ax = 5.0 + icon * 1.55;
            fill_rounded(cr, ax - icon * 0.2, ty + 1.0, icon * 1.4, tb - 1.0, 0.0, 0xccd2da, 1.0);
            fill_rounded(cr, ax - icon * 0.2, ty + tb - 2.0, icon * 1.4, 2.0, 0.0, 0x2f62b1, 1.0);
            app_icons(cr, ax, iy, icon, 4, None);
            clock(cr, w, iy, icon);
        }
        "studio" => {
            color(cr, 0xf6f6f8, 0.7);
            cr.rectangle(0.0, ty, w, tb);
            let _ = cr.fill();
            start_mark(cr, 5.0, iy, icon, 0x3d8eff);
            let count = 4;
            let shelf = icon * count as f64 + icon * 0.55 * (count - 1) as f64 + icon * 0.8;
            let sx = 5.0 + icon * 1.45;
            fill_rounded(cr, sx, ty + 2.0, shelf, tb - 4.0, (tb - 4.0) * 0.3, 0xffffff, 0.75);
            app_icons(cr, sx + icon * 0.4, iy - 1.0, icon, count, Some(0x3a3a3c));
        }
        "modern" => {
            // No line along the bar, and a blue line under each running app
            color(cr, 0xf3f4f7, 0.96);
            cr.rectangle(0.0, ty, w, tb);
            let _ = cr.fill();
            start_mark(cr, 5.0, iy, icon, 0x3d8eff);
            let ax = 7.0 + icon * 1.55;
            app_icons(cr, ax, iy, icon, 4, None);
            for i in 0..2 {
                let lx = ax + f64::from(i) * icon * 1.55 + icon * 0.3;
                fill_rounded(cr, lx, ty + tb - 2.5, icon * 0.4, 2.0, 1.0, 0x288bfa, 1.0);
            }
            clock(cr, w, iy, icon);
        }
        _ => {
            color(cr, 0xf3f4f7, 0.96);
            cr.rectangle(0.0, ty, w, tb);
            let _ = cr.fill();
            color(cr, 0xdfe1e6, 1.0);
            cr.rectangle(0.0, ty, w, 1.0);
            let _ = cr.fill();
            start_mark(cr, 5.0, iy, icon, 0x3d8eff);
            app_icons(cr, 7.0 + icon * 1.55, iy, icon, 4, None);
            clock(cr, w, iy, icon);
        }
    }
}

/// A plain taskbar along the bottom, for the Desktop page's pictures; where
/// its top edge is.
fn plain_taskbar(cr: &Context, w: f64, h: f64) -> f64 {
    let bar = (h * 0.15).max(14.0);
    let ty = h - bar;
    color(cr, 0xf3f4f7, 0.96);
    cr.rectangle(0.0, ty, w, bar);
    let _ = cr.fill();
    let icon = bar * 0.56;
    let iy = ty + (bar - icon) / 2.0;
    start_mark(cr, 5.0, iy, icon, 0x3d8eff);
    app_icons(cr, 5.0 + icon * 1.55, iy, icon, 3, None);
    clock(cr, w, iy, icon);
    ty
}

/// A file on the desktop: a page in its color over a line for its name.
fn desktop_file(cr: &Context, x: f64, y: f64, s: f64, rgb: u32, ink: u32) {
    fill_rounded(cr, x + s * 0.2, y, s * 0.6, s * 0.68, s * 0.1, rgb, 1.0);
    fill_rounded(cr, x + s * 0.08, y + s * 0.8, s * 0.84, s * 0.12, s * 0.06, ink, 0.75);
}

/// Patrone's Recycle Bin: a blue bin under its lid, a white loop on it.
fn recycle_bin_icon(cr: &Context, x: f64, y: f64, s: f64) {
    fill_rounded(cr, x + s * 0.1, y, s * 0.8, s * 0.13, s * 0.06, 0x288bfa, 1.0);
    cr.move_to(x + s * 0.16, y + s * 0.2);
    cr.line_to(x + s * 0.84, y + s * 0.2);
    cr.line_to(x + s * 0.76, y + s);
    cr.line_to(x + s * 0.24, y + s);
    cr.close_path();
    color(cr, 0x288bfa, 1.0);
    let _ = cr.fill();
    cr.set_line_width(s * 0.08);
    cr.set_line_cap(LineCap::Round);
    cr.arc(x + s * 0.5, y + s * 0.6, s * 0.17, -0.3 * PI, 1.4 * PI);
    color(cr, 0xffffff, 1.0);
    let _ = cr.stroke();
}

const FILE_COLORS: [u32; 6] = [0x3d8eff, 0xf2b636, 0x4c9a55, 0xd9473f, 0x5aa0ff, 0xc061cb];

/// Where the desktop's files show: on the desktop, in the panel, drawn
/// sliding out over a window, with the wallpaper under the files, the pill
/// on its edge and the Recycle Bin in its corner, or nowhere: a clean
/// desktop, with nothing on it.
pub fn desktop_files(cr: &Context, w: f64, h: f64, place: &str) {
    desktop(cr, w, h);
    let top = plain_taskbar(cr, w, h);
    let s = top * 0.17;
    if place == "hidden" {
        return;
    }
    let panel = place == "panel";
    let reach = if panel { w * 0.8 } else { w };
    if panel {
        rounded(cr, w * 0.46, top * 0.12, w * 0.46, top * 0.74, 3.0);
        color(cr, 0xf3f4f7, 0.97);
        let _ = cr.fill();
        fill_rounded(cr, w * 0.46, top * 0.12, w * 0.46, top * 0.13, 3.0, 0xd5d9e0, 1.0);
        let _ = cr.save();
        cr.rectangle(0.0, 0.0, reach, top);
        cr.clip();
        desktop(cr, w, h);
        let _ = cr.restore();
        cr.rectangle(reach, 0.0, 4.0, top);
        color(cr, 0x000000, 0.22);
        let _ = cr.fill();
    }
    for (i, &rgb) in FILE_COLORS.iter().enumerate() {
        let (column, row) = ((i / 3) as f64, (i % 3) as f64);
        desktop_file(cr, 7.0 + column * s * 1.3, 8.0 + row * s * 1.25, s, rgb, 0xffffff);
    }
    recycle_bin_icon(cr, reach - s - 10.0, top - s * 1.25, s * 0.85);
    if panel {
        fill_rounded(cr, reach - 5.0, top / 2.0 - top * 0.12, 5.0, top * 0.24, 2.5, 0x288bfa, 1.0);
    }
}

/// A clean desktop, with the corner that shows its files marked, or none.
pub fn desktop_corner(cr: &Context, w: f64, h: f64, corner: &str) {
    desktop(cr, w, h);
    plain_taskbar(cr, w, h);
    if corner == "none" {
        return;
    }
    let x = if corner.ends_with("left") { 0.0 } else { w };
    let y = if corner.starts_with("top") { 0.0 } else { h };
    cr.arc(x, y, h * 0.34, 0.0, 2.0 * PI);
    color(cr, 0x288bfa, 0.35);
    let _ = cr.fill();
    cr.arc(x, y, h * 0.2, 0.0, 2.0 * PI);
    color(cr, 0x288bfa, 0.95);
    let _ = cr.fill();
}

/// A Patrone Terminal window, see-through: its title bar, then a prompt.
fn terminal_window(cr: &Context, x: f64, y: f64, w: f64, h: f64) {
    let _ = cr.save();
    rounded(cr, x, y, w, h, 4.0);
    cr.clip();
    // More see-through than the terminal's own, so the logo shows at this size
    color(cr, 0x0d0f14, 0.6);
    let _ = cr.paint();
    let bar = (h * 0.13).max(7.0);
    color(cr, 0x26282e, 0.97);
    cr.rectangle(x, y, w, bar);
    let _ = cr.fill();
    let line = (h * 0.035).max(1.5);
    fill_rounded(cr, x + w * 0.04, y + bar + line * 2.0, w * 0.3, line, line / 2.0, 0x5aa0ff, 0.9);
    fill_rounded(cr, x + w * 0.04, y + bar + line * 4.0, w * 0.06, line, line / 2.0, 0x5aa0ff, 0.9);
    let _ = cr.restore();
}

/// Where the terminal opened at sign-in goes: large, over the wallpaper's
/// logo, the ">_" prompt at the bottom left and the dashed line at the top
/// right showing beside it, or at its own size in the middle.
pub fn sign_in_terminal(cr: &Context, w: f64, h: f64, place: &str) {
    desktop(cr, w, h);
    let top = plain_taskbar(cr, w, h);
    // The wallpaper's logo, its P over the wordmark and the rule
    let (cx, cy, s) = (w / 2.0, top * 0.5, top * 0.3);
    start_mark(cr, cx - s / 2.0, cy - s * 0.75, s, 0x3d8eff);
    fill_rounded(cr, cx - w * 0.18, cy + s * 0.4, w * 0.36, s * 0.2, s * 0.05, 0xffffff, 0.95);
    fill_rounded(cr, cx - w * 0.18, cy + s * 0.75, w * 0.36, s * 0.07, s * 0.03, 0x3e91de, 0.95);
    text(cr, w * 0.077, h * 0.77, top * 0.09, 0x9ec5ff, FontWeight::Bold, FontSlant::Normal, ">_");
    for i in 0..5 {
        fill_rounded(cr, w * (0.852 + f64::from(i) * 0.016), h * 0.175, w * 0.011, 1.4, 0.7, 0x3e91de, 0.9);
    }
    // In the picture's fractions, as the taskbar places it (startupWindows.js);
    // a terminal's own size is about half a 1920 by 1080 screen
    if place == "logo" {
        terminal_window(cr, w * 0.114, h * 0.086, w * 0.719, h * 0.74);
    } else if place == "center" {
        let (tw, th) = (w * 0.48, top * 0.64);
        terminal_window(cr, (w - tw) / 2.0, (top - th) / 2.0, tw, th);
    }
}

/// The Recycle Bin docked to the desktop's corner, or a desktop without it.
pub fn recycle_bin(cr: &Context, w: f64, h: f64, shown: &str) {
    desktop(cr, w, h);
    let top = plain_taskbar(cr, w, h);
    fill_rounded(cr, w * 0.12, top * 0.12, w * 0.5, top * 0.55, 4.0, 0xffffff, 0.92);
    if shown == "shown" {
        let s = top * 0.26;
        recycle_bin_icon(cr, w - s - 12.0, top - s * 1.45, s);
        fill_rounded(cr, w - s * 1.15 - 12.0, top - s * 0.32, s * 1.3, s * 0.13, s * 0.06, 0xffffff, 0.8);
    }
}

/// A cursor's picture at `size` pixels, its hotspot at (x, y). The
/// picture comes from the theme's next size up, scaled down, so it stays
/// sharp on a dense screen (GTK replays cards drawn with cairo at the
/// screen's scale).
fn pointer(cr: &Context, cursor: &Cursor, x: f64, y: f64, size: f64) {
    let Some(image) = cursor.image_for(size, 2.0) else { return };
    let Ok(surface) =
        ImageSurface::create_for_data(image.pixels.clone(), Format::ARgb32, image.width, image.height, image.width * 4)
    else {
        return;
    };
    let scale = size / f64::from(image.nominal);
    let _ = cr.save();
    cr.translate(x - image.xhot * scale, y - image.yhot * scale);
    cr.scale(scale, scale);
    let _ = cr.set_source_surface(&surface, 0.0, 0.0);
    cr.source().set_filter(Filter::Good);
    let _ = cr.paint();
    let _ = cr.restore();
}

/// A mouse pointer theme's pointers where they'd be (cursor::SHOWN's
/// order): the arrow and the busy one on the desktop, the hand over a
/// window's button and the text cursor in its text.
pub fn pointer_theme(cr: &Context, w: f64, h: f64, cursors: &[Option<Cursor>]) {
    desktop(cr, w, h);
    let (x, y, ww, wh) = (w * 0.42, h * 0.14, w * 0.5, h * 0.72);
    fill_rounded(cr, x, y, ww, wh, 6.0, 0xffffff, 1.0);
    let bar = wh * 0.22;
    fill_rounded(cr, x, y, ww, bar, 6.0, 0xeceef2, 1.0);
    color(cr, 0xeceef2, 1.0);
    cr.rectangle(x, y + bar / 2.0, ww, bar / 2.0);
    let _ = cr.fill();
    fill_rounded(cr, x + ww * 0.12, y + wh * 0.36, ww * 0.72, 4.0, 2.0, 0xc4c9d2, 1.0);
    fill_rounded(cr, x + ww * 0.12, y + wh * 0.5, ww * 0.48, 4.0, 2.0, 0xc4c9d2, 1.0);
    fill_rounded(cr, x + ww * 0.46, y + wh * 0.68, ww * 0.42, wh * 0.18, 4.0, 0x288bfa, 1.0);

    let spots = [
        (w * 0.13, h * 0.2),
        (x + ww * 0.7, y + wh * 0.78),
        (x + ww * 0.36, y + wh * 0.38),
        (w * 0.2, h * 0.68),
    ];
    for (cursor, (cx, cy)) in cursors.iter().zip(spots) {
        if let Some(cursor) = cursor {
            pointer(cr, cursor, cx, cy, 26.0);
        }
    }
}

/// The arrow at a pointer size, as big as it'd be on the screen, in the
/// middle of the picture.
pub fn pointer_size(cr: &Context, w: f64, h: f64, arrow: Option<&Cursor>, size: f64) {
    desktop(cr, w, h);
    let Some(arrow) = arrow else { return };
    let Some(image) = arrow.image_for(size, 2.0) else { return };
    // Where its hotspot goes for the picture's box to be centered
    let scale = size / f64::from(image.nominal);
    let x = (w - f64::from(image.width) * scale) / 2.0 + image.xhot * scale;
    let y = (h - f64::from(image.height) * scale) / 2.0 + image.yhot * scale;
    pointer(cr, arrow, x, y, size);
}

/// A plain color, for the None card and the colors to pick from, with a thin
/// line so that one like the window's own still shows.
pub fn plain_color(cr: &Context, w: f64, h: f64, rgb: u32) {
    fill_rounded(cr, 0.0, 0.0, w, h, 6.0, rgb, 1.0);
    rounded(cr, 0.5, 0.5, w - 1.0, h - 1.0, 6.0);
    color(cr, 0x808080, 0.35);
    cr.set_line_width(1.0);
    let _ = cr.stroke();
}

/// A wallpaper's picture, filling the card as GNOME's zoom fills a screen, or
/// an empty card until it has loaded.
#[allow(deprecated)] // GDK's other ways to draw a pixbuf take far more code
pub fn wallpaper(cr: &Context, w: f64, h: f64, picture: Option<&Pixbuf>) {
    let Some(picture) = picture else {
        fill_rounded(cr, 0.0, 0.0, w, h, 8.0, 0x808080, 0.18);
        return;
    };
    let _ = cr.save();
    rounded(cr, 0.0, 0.0, w, h, 8.0);
    cr.clip();
    let (pw, ph) = (f64::from(picture.width()), f64::from(picture.height()));
    let scale = (w / pw).max(h / ph);
    cr.translate((w - pw * scale) / 2.0, (h - ph * scale) / 2.0);
    cr.scale(scale, scale);
    cr.set_source_pixbuf(picture, 0.0, 0.0);
    cr.source().set_filter(Filter::Good);
    let _ = cr.paint();
    let _ = cr.restore();
}
