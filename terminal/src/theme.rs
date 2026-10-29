//! Patrone Dark: the terminal's colors and default font.
//!
//! A near-black background with the Patrone blues from `branding/README.md`
//! for the cursor, selection and the blue ANSI colors.

use gtk::gdk;
use vte::prelude::*;

/// The default font: PatroneOS Mono, PatroneOS's own (the fonts-patroneos
/// package). A single family, so the font chooser in Preferences can show it.
pub const FONT: &str = "PatroneOS Mono 11";

/// The default before PatroneOS Mono, which settings saved by an older
/// version may hold without anyone having picked it.
pub const OLD_FONT: &str = "JetBrains Mono 11";

const FOREGROUND: &str = "#D6DEEB";
const BACKGROUND: &str = "#070A10";
const CURSOR: &str = "#288BFA"; // Patrone Blue
const SELECTION: &str = "#1851A8"; // Deep

/// ANSI colors 0-15: black, red, green, yellow, blue, magenta, cyan, white,
/// then the bright versions in the same order.
const PALETTE: [&str; 16] = [
    "#1C2230", "#E5484D", "#3FC56B", "#F2C14E", "#288BFA", "#B072F5", "#2BC3D6", "#C5CDD9",
    "#5C6678", "#FF6B6F", "#6EE592", "#FFD97A", "#4DA0FC", "#C99BFF", "#5FE0EE", "#FFFFFF",
];

fn rgba(hex: &str) -> gdk::RGBA {
    gdk::RGBA::parse(hex).expect("theme colors are valid")
}

/// The background at a transparency, in percent: the desktop shows through
/// it, as in Kali's terminal, while the text stays solid.
pub fn background(transparency: i64) -> gdk::RGBA {
    let color = rgba(BACKGROUND);
    let alpha = 1.0 - transparency.clamp(0, 100) as f32 / 100.0;
    gdk::RGBA::new(color.red(), color.green(), color.blue(), alpha)
}

pub fn apply(terminal: &vte::Terminal) {
    let palette: Vec<gdk::RGBA> = PALETTE.iter().map(|hex| rgba(hex)).collect();
    let palette: Vec<&gdk::RGBA> = palette.iter().collect();
    terminal.set_colors(Some(&rgba(FOREGROUND)), Some(&rgba(BACKGROUND)), &palette);
    terminal.set_color_cursor(Some(&rgba(CURSOR)));
    terminal.set_color_cursor_foreground(Some(&rgba(BACKGROUND)));
    terminal.set_color_highlight(Some(&rgba(SELECTION)));
    terminal.set_color_highlight_foreground(Some(&rgba("#FFFFFF")));
    terminal.set_bold_is_bright(false);
}

/// Styles the window chrome around the terminals to match. The window
/// itself is clear, so a terminal's see-through background shows the
/// desktop and not the window behind it.
pub const CSS: &str = "
window.patrone-terminal,
window.patrone-terminal notebook,
window.patrone-terminal notebook > stack,
window.patrone-terminal scrolledwindow,
window.patrone-terminal vte-terminal {
    background-color: transparent;
}
window.patrone-terminal headerbar {
    background-color: #0D121B;
    color: #D6DEEB;
    box-shadow: none;
}
window.patrone-terminal notebook > header {
    background-color: #0D121B;
    border-bottom: 1px solid #1C2230;
}
window.patrone-terminal notebook > header tab:checked {
    box-shadow: inset 0 -2px #288BFA;
}
vte-terminal {
    padding: 6px 10px;
}
";
