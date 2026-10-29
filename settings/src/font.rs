//! The desktop's font: PatroneOS Sans, PatroneOS's own (the default since
//! patrone-branding 0.1.24; the fonts-patroneos package, from
//! billsecond/patroneos-fonts), or Noto Sans.
//! Picking one changes the family of GNOME's interface font and of the title
//! bar font, and keeps their sizes and styles, and the document font with
//! it: PatroneOS Document with PatroneOS Sans, Noto Sans with Noto Sans. The
//! monospace font stays as it is.

use gtk::prelude::*;
use gtk::{gio, pango};

pub const INTERFACE_SCHEMA: &str = "org.gnome.desktop.interface";

/// The fonts to pick from, the default first: family (the value), name,
/// description and the package that installs it. The descriptions are of
/// about the same length, so the cards are of the same height.
pub const FONTS: [(&str, &str, &str, &str); 2] = [
    (
        "PatroneOS Sans",
        "PatroneOS Sans",
        "PatroneOS's own font, with PatroneOS Document for documents",
        "fonts-patroneos",
    ),
    (
        "Noto Sans",
        "Noto Sans",
        "A font made for nearly every language, for documents too",
        "fonts-noto-core",
    ),
];

/// The document font that goes with each choice.
pub fn document_font(family: &str) -> &str {
    if family == "PatroneOS Sans" {
        "PatroneOS Document"
    } else {
        family
    }
}

/// The choice a font setting's family stands for: Patrone Sans, which
/// PatroneOS Sans replaced (and fontconfig still draws as it), is
/// PatroneOS Sans.
pub fn choice_of(family: &str) -> String {
    if family == "Patrone Sans" {
        "PatroneOS Sans".to_owned()
    } else {
        family.to_owned()
    }
}

/// What the fonts' cards show, in each font.
pub const SAMPLE_BIG: &str = "Aa";
pub const SAMPLE: &str = "Il1 O0 10:00";

/// The font keys that follow the choice: the interface's and documents'
/// (`org.gnome.desktop.interface`), and the title bar's (the window
/// manager's, for windows that don't draw their own title bars).
const INTERFACE_KEY: &str = "font-name";
const DOCUMENT_KEY: &str = "document-font-name";
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
    set_family(interface, INTERFACE_KEY, family);
    set_family(interface, DOCUMENT_KEY, document_font(family));
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
    use super::{choice_of, document_font, family_of, with_family};

    #[test]
    fn reads_the_family_of_a_font_setting() {
        assert_eq!(family_of("Noto Sans 11"), "Noto Sans");
        assert_eq!(family_of("PatroneOS Sans SemiBold 11"), "PatroneOS Sans");
        assert_eq!(family_of("Cantarell 10.5"), "Cantarell");
    }

    #[test]
    fn changes_the_family_and_keeps_style_and_size() {
        assert_eq!(with_family("Noto Sans 11", "PatroneOS Sans"), "PatroneOS Sans 11");
        assert_eq!(with_family("Noto Sans Bold 11", "PatroneOS Sans"), "PatroneOS Sans Bold 11");
        assert_eq!(with_family("PatroneOS Document 10.5", "Noto Sans"), "Noto Sans 10.5");
        assert_eq!(with_family("Cantarell Italic 12", "Noto Sans"), "Noto Sans Italic 12");
    }

    #[test]
    fn documents_follow_the_choice() {
        assert_eq!(document_font("PatroneOS Sans"), "PatroneOS Document");
        assert_eq!(document_font("Noto Sans"), "Noto Sans");
    }

    #[test]
    fn patrone_sans_is_patroneos_sans_now() {
        assert_eq!(choice_of("Patrone Sans"), "PatroneOS Sans");
        assert_eq!(choice_of("Noto Sans"), "Noto Sans");
    }

    #[test]
    fn every_font_is_its_own_family() {
        for (family, title, _, _) in super::FONTS {
            assert_eq!(family, title);
            assert_eq!(family_of(&format!("{family} 11")), family);
        }
    }
}
