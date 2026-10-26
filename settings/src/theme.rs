//! Patrone's themes, and what choosing one changes: the theme setting the
//! taskbar follows, and the theme's style for apps, imported into the
//! account's GTK 3 and GTK 4 style sheets. Which side the title bar buttons
//! go on is the account's own choice, which no theme changes.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use gtk::gio;
use gtk::glib;
use gtk::prelude::*;

/// Each theme: its setting value, name and description.
pub const THEMES: [(&str, &str, &str); 4] = [
    ("patrone", "Patrone", "Patrone's own look: a light or dark taskbar, its buttons on the left"),
    ("modern", "Modern", "Clean and flat, with the taskbar's buttons centered"),
    ("classic", "Classic", "A blue taskbar and title bars, with a green Start button"),
    ("studio", "Studio", "Soft and translucent, with round colored window buttons"),
];

/// The title bar buttons at the top right or top left (GNOME's
/// button-layout): minimize, maximize and close, close nearest the corner.
pub const LAYOUT_RIGHT: &str = "appmenu:minimize,maximize,close";
pub const LAYOUT_LEFT: &str = "close,minimize,maximize:appmenu";

const BEGIN: &str = "/* Patrone Settings: theme (Patrone Settings changes these lines; your own go below) */";
const END: &str = "/* Patrone Settings: end of theme */";

/// Where the themes' style sheets are installed; PATRONE_SETTINGS_THEMES
/// points elsewhere, to try them from the source tree.
fn themes_dir() -> PathBuf {
    std::env::var_os("PATRONE_SETTINGS_THEMES")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/usr/share/patrone/themes"))
}

/// `text` without the theme's lines, if it has them.
fn without_theme(text: &str) -> String {
    if let (Some(begin), Some(end)) = (text.find(BEGIN), text.find(END)) {
        if begin < end {
            let after = &text[end + END.len()..];
            let after = after.strip_prefix('\n').unwrap_or(after);
            return format!("{}{}", &text[..begin], after);
        }
    }
    text.to_string()
}

/// `text` with `theme`'s style sheet imported at the top (the account's own
/// rules come after, so they still win), or with none for Patrone's own.
fn with_theme(text: &str, theme: &str, sheet: &Path) -> String {
    let rest = without_theme(text);
    if theme == "patrone" {
        return rest;
    }
    format!("{BEGIN}\n@import url(\"file://{}\");\n{END}\n{rest}", sheet.display())
}

/// Puts the theme's style sheet into one GTK version's gtk.css.
fn write_style(config_dir: &Path, version: &str, theme: &str) -> io::Result<()> {
    let path = config_dir.join(version).join("gtk.css");
    let existing = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == io::ErrorKind::NotFound => String::new(),
        Err(error) => return Err(error),
    };
    let sheet = themes_dir().join(theme).join(format!("{version}.css"));
    let updated = with_theme(&existing, theme, &sheet);
    if updated == existing {
        return Ok(());
    }
    fs::create_dir_all(path.parent().unwrap_or(config_dir))?;
    fs::write(&path, updated)
}

/// Switches to `theme`: the setting the taskbar follows, and the theme's
/// style for GTK 3 and GTK 4 apps, which apps pick up when they're next
/// opened.
pub fn apply(theme: &str, desktop: &gio::Settings) {
    if desktop.string("theme") != theme {
        if let Err(error) = desktop.set_string("theme", theme) {
            eprintln!("patrone-settings: can't set the theme: {error}");
        }
    }
    let config_dir = glib::user_config_dir();
    for version in ["gtk-4.0", "gtk-3.0"] {
        if let Err(error) = write_style(&config_dir, version, theme) {
            eprintln!("patrone-settings: can't update {version}/gtk.css: {error}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHEET: &str = "/usr/share/patrone/themes/classic/gtk-4.0.css";

    #[test]
    fn a_theme_goes_on_top_of_the_accounts_own_rules() {
        let own = "window { opacity: 1; }\n";
        let styled = with_theme(own, "classic", Path::new(SHEET));
        assert!(styled.starts_with(BEGIN));
        assert!(styled.contains(&format!("@import url(\"file://{SHEET}\");")));
        assert!(styled.ends_with(own));
    }

    #[test]
    fn changing_theme_replaces_the_old_one_only() {
        let own = "label { color: red; }\n";
        let classic = with_theme(own, "classic", Path::new(SHEET));
        let studio = with_theme(&classic, "studio", Path::new("/x/studio/gtk-4.0.css"));
        assert_eq!(studio.matches(BEGIN).count(), 1);
        assert!(studio.contains("/x/studio/gtk-4.0.css"));
        assert!(!studio.contains(SHEET));
        assert!(studio.ends_with(own));
    }

    #[test]
    fn patrones_own_theme_takes_its_lines_out() {
        let own = "label { color: red; }\n";
        let classic = with_theme(own, "classic", Path::new(SHEET));
        assert_eq!(with_theme(&classic, "patrone", Path::new(SHEET)), own);
        assert_eq!(with_theme("", "patrone", Path::new(SHEET)), "");
    }

    #[test]
    fn writes_and_keeps_the_file() {
        let dir = std::env::temp_dir().join(format!("patrone-settings-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("gtk-4.0")).unwrap();
        fs::write(dir.join("gtk-4.0/gtk.css"), "button { margin: 1px; }\n").unwrap();

        write_style(&dir, "gtk-4.0", "studio").unwrap();
        let written = fs::read_to_string(dir.join("gtk-4.0/gtk.css")).unwrap();
        assert!(written.contains("studio/gtk-4.0.css"));
        assert!(written.ends_with("button { margin: 1px; }\n"));

        write_style(&dir, "gtk-4.0", "patrone").unwrap();
        assert_eq!(fs::read_to_string(dir.join("gtk-4.0/gtk.css")).unwrap(), "button { margin: 1px; }\n");

        // No file, and Patrone's own theme: none is made
        write_style(&dir, "gtk-3.0", "patrone").unwrap();
        assert!(!dir.join("gtk-3.0/gtk.css").exists());
        let _ = fs::remove_dir_all(&dir);
    }
}
