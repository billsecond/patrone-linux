//! Patrone's themes and title bar button styles, and what choosing one
//! changes: the theme setting the taskbar follows, and the style sheets
//! imported into the account's GTK 3 and GTK 4 style sheets: the theme's
//! title bars, where it has its own, then the title bar buttons. Which side
//! the title bar buttons go on, and the taskbar's layout, are the account's
//! own choices, which no theme changes.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use gtk::gio;
use gtk::glib;
use gtk::prelude::*;

/// Each theme: its setting value, name and description. "royal" is shown
/// as Sapphire (its name since 0.3.7); the value stays, so nobody's choice
/// changes.
pub const THEMES: [(&str, &str, &str); 5] = [
    ("patrone", "Patrone", "Patrone's own look, light or dark"),
    ("modern", "Modern", "Clean and flat, with rounder buttons and no lines"),
    ("classic", "Classic", "Plain and square, with solid title bars"),
    ("royal", "Sapphire", "Glossy sapphire blue, with a green Start button"),
    ("studio", "Studio", "Soft and translucent, with round colored window buttons"),
];

/// Each title bar button style (org.patrone.desktop title-buttons): its
/// setting value, name and description. "theme" is the theme's own.
pub const BUTTON_STYLES: [(&str, &str, &str); 5] = [
    ("theme", "Theme's own", "Whichever style the theme comes with"),
    ("wide", "Wide", "Flat and wide; close turns red under the pointer"),
    ("round", "Round", "Small symbols in soft circles"),
    ("dots", "Dots", "Red, yellow and green dots, symbols on hover"),
    ("glossy", "Glossy", "Framed blue squares and a red close"),
];

/// The button style each theme comes with.
pub fn theme_buttons(theme: &str) -> &'static str {
    match theme {
        "modern" => "round",
        "royal" => "glossy",
        "studio" => "dots",
        _ => "wide",
    }
}

/// The title bar buttons at the top right or top left (GNOME's
/// button-layout): minimize, maximize and close, close nearest the corner.
pub const LAYOUT_RIGHT: &str = "appmenu:minimize,maximize,close";
pub const LAYOUT_LEFT: &str = "close,minimize,maximize:appmenu";

const BEGIN: &str = "/* Patrone Settings: theme (Patrone Settings changes these lines; your own go below) */";
const END: &str = "/* Patrone Settings: end of theme */";

/// Where the style sheets are installed (with themes/ and titlebuttons/ in
/// it); PATRONE_SETTINGS_DATA points elsewhere, to try them from the source
/// tree's data folder.
fn data_dir() -> PathBuf {
    std::env::var_os("PATRONE_SETTINGS_DATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/usr/share/patrone"))
}

/// The style sheets for one GTK version (gtk-4.0 or gtk-3.0): the theme's
/// title bars, if it has its own, then the title bar buttons.
fn sheets(data: &Path, version: &str, theme: &str, buttons: &str) -> Vec<PathBuf> {
    let mut sheets = Vec::new();
    let title_bars = data.join("themes").join(theme).join(format!("{version}.css"));
    if title_bars.is_file() {
        sheets.push(title_bars);
    }
    let style = if buttons == "theme" { theme_buttons(theme) } else { buttons };
    sheets.push(data.join("titlebuttons").join(style).join(format!("{version}.css")));
    sheets
}

/// `text` without Patrone's lines, if it has them.
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

/// `text` with `sheets` imported at the top, between Patrone's marked lines.
/// The account's own rules come after. In the apps that load the sheets
/// themselves (live_style.rs) those rules always win; in others a rule has to
/// be as specific as the sheet's too, which say `window:not(.patrone-live)`:
/// `window.csd headerbar`, not `headerbar`.
fn with_sheets(text: &str, sheets: &[PathBuf]) -> String {
    let rest = without_theme(text);
    if sheets.is_empty() {
        return rest;
    }
    let imports: String = sheets
        .iter()
        .map(|sheet| format!("@import url(\"file://{}\");\n", sheet.display()))
        .collect();
    format!("{BEGIN}\n{imports}{END}\n{rest}")
}

/// Puts the style sheets into one GTK version's gtk.css.
fn write_style(config_dir: &Path, version: &str, sheets: &[PathBuf]) -> io::Result<()> {
    let path = config_dir.join(version).join("gtk.css");
    let existing = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == io::ErrorKind::NotFound => String::new(),
        Err(error) => return Err(error),
    };
    let updated = with_sheets(&existing, sheets);
    if updated == existing {
        return Ok(());
    }
    fs::create_dir_all(path.parent().unwrap_or(config_dir))?;
    fs::write(&path, updated)
}

/// Writes the account's GTK 3 and GTK 4 style sheets for the theme and
/// title bar button style that are set. Patrone's apps and GNOME Settings
/// follow at once (live_style.rs); other apps read them when they're next
/// opened.
pub fn write_styles(desktop: &gio::Settings) {
    let theme = desktop.string("theme");
    let buttons = desktop.string("title-buttons");
    let config_dir = glib::user_config_dir();
    for version in ["gtk-4.0", "gtk-3.0"] {
        let sheets = sheets(&data_dir(), version, &theme, &buttons);
        if let Err(error) = write_style(&config_dir, version, &sheets) {
            eprintln!("patrone-settings: can't update {version}/gtk.css: {error}");
        }
    }
}

fn set(desktop: &gio::Settings, key: &str, value: &str) {
    if desktop.string(key) != value {
        if let Err(error) = desktop.set_string(key, value) {
            eprintln!("patrone-settings: can't set {key}: {error}");
        }
    }
}

/// Switches to `theme`: the setting the taskbar follows, and its style for
/// apps.
pub fn apply(theme: &str, desktop: &gio::Settings) {
    set(desktop, "theme", theme);
    write_styles(desktop);
}

/// Switches the title bar buttons to `style` ("theme": the theme's own).
pub fn apply_buttons(style: &str, desktop: &gio::Settings) {
    set(desktop, "title-buttons", style);
    write_styles(desktop);
}

#[cfg(test)]
mod tests {
    use super::*;

    const DATA: &str = "/usr/share/patrone";

    fn data() -> PathBuf {
        PathBuf::from(DATA)
    }

    #[test]
    fn the_sheets_go_on_top_of_the_accounts_own_rules() {
        let own = "window { opacity: 1; }\n";
        let sheets = [data().join("titlebuttons/wide/gtk-4.0.css")];
        let styled = with_sheets(own, &sheets);
        assert!(styled.starts_with(BEGIN));
        assert!(styled.contains(&format!("@import url(\"file://{DATA}/titlebuttons/wide/gtk-4.0.css\");")));
        assert!(styled.ends_with(own));
    }

    #[test]
    fn changing_them_replaces_the_old_lines_only() {
        let own = "label { color: red; }\n";
        let first = with_sheets(own, &[data().join("themes/royal/gtk-4.0.css"), data().join("titlebuttons/glossy/gtk-4.0.css")]);
        let second = with_sheets(&first, &[data().join("titlebuttons/dots/gtk-4.0.css")]);
        assert_eq!(second.matches(BEGIN).count(), 1);
        assert!(second.contains("titlebuttons/dots"));
        assert!(!second.contains("royal") && !second.contains("glossy"));
        assert!(second.ends_with(own));
        // No sheets at all takes the lines out
        assert_eq!(with_sheets(&second, &[]), own);
    }

    #[test]
    fn each_theme_brings_its_own_buttons_unless_one_is_chosen() {
        let names = |sheets: Vec<PathBuf>| {
            sheets.iter().map(|s| s.strip_prefix(DATA).unwrap().display().to_string()).collect::<Vec<_>>()
        };
        // (The installed themes/ folder may not exist here; only the buttons
        // don't depend on it.)
        let royal = names(sheets(&data(), "gtk-4.0", "royal", "theme"));
        assert_eq!(royal.last().unwrap(), "titlebuttons/glossy/gtk-4.0.css");
        let patrone = names(sheets(&data(), "gtk-3.0", "patrone", "theme"));
        assert_eq!(patrone.last().unwrap(), "titlebuttons/wide/gtk-3.0.css");
        let chosen = names(sheets(&data(), "gtk-4.0", "studio", "round"));
        assert_eq!(chosen.last().unwrap(), "titlebuttons/round/gtk-4.0.css");
        for (theme, _, _) in THEMES {
            assert!(BUTTON_STYLES.iter().any(|&(style, _, _)| style == theme_buttons(theme)));
        }
    }

    #[test]
    fn every_sheet_named_is_in_the_source_tree() {
        let tree = Path::new(env!("CARGO_MANIFEST_DIR")).join("data");
        for (theme, _, _) in THEMES {
            for (buttons, _, _) in BUTTON_STYLES {
                for version in ["gtk-4.0", "gtk-3.0"] {
                    for sheet in sheets(&tree, version, theme, buttons) {
                        assert!(sheet.is_file(), "{}", sheet.display());
                    }
                }
            }
        }
    }

    /// GTK keeps what it read from gtk.css for as long as an app runs, so a
    /// rule in a GTK 4 sheet that reached the windows of apps that load the
    /// sheets themselves (live_style.rs) would stay after a change.
    #[test]
    fn gtk4_sheets_keep_off_windows_that_follow_changes() {
        let tree = Path::new(env!("CARGO_MANIFEST_DIR")).join("data");
        let mut sheets_read = 0;
        for folder in ["themes", "titlebuttons"] {
            for entry in fs::read_dir(tree.join(folder)).unwrap() {
                let sheet = entry.unwrap().path().join("gtk-4.0.css");
                if !sheet.is_file() {
                    continue;
                }
                sheets_read += 1;
                let mut text = fs::read_to_string(&sheet).unwrap();
                while let (Some(begin), Some(end)) = (text.find("/*"), text.find("*/")) {
                    text.replace_range(begin..end + 2, "");
                }
                for rule in text.split('{') {
                    let selectors = rule.rsplit(['}', ';']).next().unwrap().trim();
                    if selectors.is_empty() || selectors.starts_with('@') {
                        continue;
                    }
                    for selector in selectors.split(',') {
                        assert!(selector.contains(":not(.patrone-live)"), "{}: {selector}", sheet.display());
                    }
                }
            }
        }
        assert_eq!(sheets_read, 9);
    }

    #[test]
    fn writes_and_keeps_the_file() {
        let dir = std::env::temp_dir().join(format!("patrone-settings-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("gtk-4.0")).unwrap();
        fs::write(dir.join("gtk-4.0/gtk.css"), "button { margin: 1px; }\n").unwrap();

        write_style(&dir, "gtk-4.0", &[data().join("titlebuttons/dots/gtk-4.0.css")]).unwrap();
        let written = fs::read_to_string(dir.join("gtk-4.0/gtk.css")).unwrap();
        assert!(written.contains("titlebuttons/dots/gtk-4.0.css"));
        assert!(written.ends_with("button { margin: 1px; }\n"));

        write_style(&dir, "gtk-4.0", &[]).unwrap();
        assert_eq!(fs::read_to_string(dir.join("gtk-4.0/gtk.css")).unwrap(), "button { margin: 1px; }\n");

        // No file, and nothing to import: none is made
        write_style(&dir, "gtk-3.0", &[]).unwrap();
        assert!(!dir.join("gtk-3.0/gtk.css").exists());
        let _ = fs::remove_dir_all(&dir);
    }
}
