//! Preferences, saved in `~/.config/patrone-terminal/settings.ini`.

use std::cell::RefCell;
use std::path::{Path, PathBuf};

use gtk::glib;

use crate::theme;

const GROUP: &str = "terminal";

/// The most the background can be see-through, in percent: past this the
/// text gets hard to read over a busy wallpaper.
pub const MAX_TRANSPARENCY: i64 = 60;

#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    /// A Pango font description, such as "JetBrains Mono 11".
    pub font: String,
    pub scrollback_lines: i64,
    pub copy_on_select: bool,
    pub paste_on_right_click: bool,
    /// How see-through the background is, in percent (0 is solid).
    pub transparency: i64,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            font: theme::FONT.to_owned(),
            scrollback_lines: 100_000,
            copy_on_select: true,
            paste_on_right_click: true,
            // The desktop showing faintly through, as in Kali's terminal
            // (the user asked for it, 2026-10-26)
            transparency: 15,
        }
    }
}

thread_local! {
    static CURRENT: RefCell<Settings> = RefCell::new(Settings::load_from(&path()));
}

/// The current settings.
pub fn get() -> Settings {
    CURRENT.with(|current| current.borrow().clone())
}

/// Changes the settings and saves them.
pub fn update(change: impl FnOnce(&mut Settings)) {
    CURRENT.with(|current| {
        let mut settings = current.borrow_mut();
        change(&mut settings);
        if let Err(error) = settings.save_to(&path()) {
            eprintln!("patrone-terminal: could not save settings: {error}");
        }
    });
}

fn path() -> PathBuf {
    glib::user_config_dir()
        .join("patrone-terminal")
        .join("settings.ini")
}

impl Settings {
    /// Reads `path`, using the default for anything missing or unreadable.
    fn load_from(path: &Path) -> Self {
        let defaults = Self::default();
        let file = glib::KeyFile::new();
        if file.load_from_file(path, glib::KeyFileFlags::NONE).is_err() {
            return defaults;
        }
        Self {
            font: file
                .string(GROUP, "font")
                .map(String::from)
                .unwrap_or(defaults.font),
            scrollback_lines: file
                .int64(GROUP, "scrollback-lines")
                .unwrap_or(defaults.scrollback_lines),
            copy_on_select: file
                .boolean(GROUP, "copy-on-select")
                .unwrap_or(defaults.copy_on_select),
            paste_on_right_click: file
                .boolean(GROUP, "paste-on-right-click")
                .unwrap_or(defaults.paste_on_right_click),
            transparency: file
                .int64(GROUP, "background-transparency")
                .map(|percent| percent.clamp(0, MAX_TRANSPARENCY))
                .unwrap_or(defaults.transparency),
        }
    }

    fn save_to(&self, path: &Path) -> Result<(), glib::Error> {
        let file = glib::KeyFile::new();
        file.set_string(GROUP, "font", &self.font);
        file.set_int64(GROUP, "scrollback-lines", self.scrollback_lines);
        file.set_boolean(GROUP, "copy-on-select", self.copy_on_select);
        file.set_boolean(GROUP, "paste-on-right-click", self.paste_on_right_click);
        file.set_int64(GROUP, "background-transparency", self.transparency);
        if let Some(dir) = path.parent() {
            glib::mkdir_with_parents(dir, 0o755);
        }
        file.save_to_file(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_survive_a_save_and_load() {
        let path =
            std::env::temp_dir().join(format!("patrone-terminal-test-{}.ini", std::process::id()));
        let settings = Settings {
            font: "Fira Code 13".to_owned(),
            scrollback_lines: 5_000,
            copy_on_select: false,
            paste_on_right_click: false,
            transparency: 40,
        };
        settings.save_to(&path).unwrap();
        assert_eq!(Settings::load_from(&path), settings);
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn transparency_past_the_most_is_held_to_it() {
        let path = std::env::temp_dir().join(format!(
            "patrone-terminal-test-clear-{}.ini",
            std::process::id()
        ));
        std::fs::write(&path, "[terminal]\nbackground-transparency=95\n").unwrap();
        assert_eq!(Settings::load_from(&path).transparency, MAX_TRANSPARENCY);
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn the_background_lets_the_desktop_show_through() {
        assert!((theme::background(15).alpha() - 0.85).abs() < 1e-6);
        assert_eq!(theme::background(0).alpha(), 1.0);
    }

    #[test]
    fn a_missing_file_gives_the_defaults() {
        assert_eq!(
            Settings::load_from(Path::new("/nonexistent/settings.ini")),
            Settings::default()
        );
    }
}
