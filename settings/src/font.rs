//! The desktop's font: Patrone Sans, Patrone's own (the default since
//! patrone-branding 0.1.6), Noto Sans with a tailed l and a slashed zero
//! (`fonts/` in Patrone's repository, the fonts-patrone-sans package), or
//! Noto Sans itself, the font it's based on.
//! Picking one changes the family of GNOME's interface and document fonts
//! and of the title bar font, and keeps their sizes and styles; the
//! monospace font stays as it is.

use gtk::prelude::*;
use gtk::{gio, pango};

pub const INTERFACE_SCHEMA: &str = "org.gnome.desktop.interface";

/// The fonts to pick from, the default first: family (the value), name,
/// description and the package that installs it. The descriptions are of
/// about the same length, so the cards are of the same height.
pub const FONTS: [(&str, &str, &str, &str); 2] = [
    (
        "Patrone Sans",
        "Patrone Sans",
        "Patrone's font: Noto Sans with a tailed l and a slashed zero",
        "fonts-patrone-sans",
    ),
    (
        "Noto Sans",
        "Noto Sans",
        "The font Patrone Sans is based on, without those changes",
        "fonts-noto-core",
    ),
];

/// What the fonts' cards show, in each font.
pub const SAMPLE_BIG: &str = "Aa";
pub const SAMPLE: &str = "Il1 O0 10:00";

/// The font keys that follow the choice: the interface's and documents'
/// (`org.gnome.desktop.interface`), and the title bar's (the window
/// manager's, for windows that don't draw their own title bars).
const INTERFACE_KEYS: [&str; 2] = ["font-name", "document-font-name"];
const TITLE_BAR_KEY: &str = "titlebar-font";

/// The family of a font setting such as "Noto Sans Bold 11".
pub fn family_of(description: &str) -> String {
    pango::FontDescription::from_string(description)
        .family()
        .map(|family| family.to_string())
        .unwrap_or_default()
}

/// A font setting with another family, its style and size kept.
pub fn with_family(description: &str, family: &str) -> String {
    let mut font = pango::FontDescription::from_string(description);
    font.set_family(family);
    font.to_string()
}

/// Sets the desktop's font to `family`.
pub fn apply(family: &str, interface: &gio::Settings, window_manager: Option<&gio::Settings>) {
    for key in INTERFACE_KEYS {
        set_family(interface, key, family);
    }
    if let Some(window_manager) = window_manager {
        set_family(window_manager, TITLE_BAR_KEY, family);
    }
}

fn set_family(settings: &gio::Settings, key: &str, family: &str) {
    let current = settings.string(key);
    let new = with_family(&current, family);
    if current != new {
        let _ = settings.set_string(key, &new);
    }
}

#[cfg(test)]
mod tests {
    use super::{family_of, with_family};

    #[test]
    fn reads_the_family_of_a_font_setting() {
        assert_eq!(family_of("Noto Sans 11"), "Noto Sans");
        assert_eq!(family_of("Patrone Sans Bold 11"), "Patrone Sans");
        assert_eq!(family_of("Cantarell 10.5"), "Cantarell");
    }

    #[test]
    fn changes_the_family_and_keeps_style_and_size() {
        assert_eq!(with_family("Noto Sans 11", "Patrone Sans"), "Patrone Sans 11");
        assert_eq!(with_family("Noto Sans Bold 11", "Patrone Sans"), "Patrone Sans Bold 11");
        assert_eq!(with_family("Patrone Sans 10.5", "Noto Sans"), "Noto Sans 10.5");
        assert_eq!(with_family("Cantarell Italic 12", "Noto Sans"), "Noto Sans Italic 12");
    }

    #[test]
    fn every_font_is_its_own_family() {
        for (family, title, _, _) in super::FONTS {
            assert_eq!(family, title);
            assert_eq!(family_of(&format!("{family} 11")), family);
        }
    }
}
