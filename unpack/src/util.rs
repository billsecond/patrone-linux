//! Sizes, dates and small helpers shared by the windows.

use std::path::{Path, PathBuf};
use std::process::Command;

use gtk::glib;

/// Three significant digits, as Windows shows sizes: "512 bytes",
/// "1.20 KB", "12.3 MB", "123 GB".
pub fn size(bytes: u64) -> String {
    if bytes < 1024 {
        return if bytes == 1 {
            "1 byte".into()
        } else {
            format!("{bytes} bytes")
        };
    }
    let units = ["KB", "MB", "GB", "TB", "PB"];
    let mut value = bytes as f64 / 1024.0;
    let mut unit = 0;
    while value >= 1000.0 && unit < units.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    let text = if value < 10.0 {
        format!("{:.2}", (value * 100.0).floor() / 100.0)
    } else if value < 100.0 {
        format!("{:.1}", (value * 10.0).floor() / 10.0)
    } else {
        format!("{:.0}", value.floor())
    };
    format!("{text} {}", units[unit])
}

/// The locale's short date and the time without seconds: "10/26/2026 3:45 PM".
pub fn date_time(seconds: i64) -> String {
    let Ok(time) = glib::DateTime::from_unix_local(seconds) else {
        return String::new();
    };
    let date = time.format("%x").map(String::from).unwrap_or_default();
    let twelve_hour = time
        .format("%p")
        .map(|ampm| !ampm.is_empty())
        .unwrap_or(false);
    let clock = time
        .format(if twelve_hour { "%l:%M %p" } else { "%H:%M" })
        .map(|text| text.trim().to_owned())
        .unwrap_or_default();
    format!("{date} {clock}")
}

/// "~/Downloads" for a path in the home folder, else the path.
pub fn display_path(path: &Path) -> String {
    let home = glib::home_dir();
    match path.strip_prefix(&home) {
        Ok(rest) if rest.as_os_str().is_empty() => "~".into(),
        Ok(rest) => format!("~/{}", rest.display()),
        Err(_) => path.display().to_string(),
    }
}

/// A path typed by someone: "~/" expands to the home folder.
pub fn typed_path(text: &str) -> PathBuf {
    let text = text.trim();
    if text == "~" {
        glib::home_dir()
    } else if let Some(rest) = text.strip_prefix("~/") {
        glib::home_dir().join(rest)
    } else {
        PathBuf::from(text)
    }
}

/// Runs a program and returns what it printed, if it succeeded.
pub fn output(program: &str, arguments: &[&str]) -> Option<String> {
    let output = Command::new(program)
        .args(arguments)
        .env("LC_ALL", "C.UTF-8")
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).into_owned())
}

/// A new, empty folder of the account's own in the temporary folder, for
/// files opened from an archive.
pub fn temporary_folder() -> Option<PathBuf> {
    use std::os::unix::fs::DirBuilderExt;
    let base = glib::tmp_dir();
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|time| time.as_nanos())
        .unwrap_or_default();
    (0..100).find_map(|attempt| {
        let folder = base.join(format!(
            "patrone-unpack-{}-{stamp:x}-{attempt}",
            std::process::id()
        ));
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&folder)
            .ok()
            .map(|()| folder)
    })
}

/// Whether a program can be found on PATH.
pub fn have(program: &str) -> bool {
    glib::find_program_in_path(program).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes() {
        assert_eq!(size(0), "0 bytes");
        assert_eq!(size(1), "1 byte");
        assert_eq!(size(1023), "1023 bytes");
        assert_eq!(size(1024), "1.00 KB");
        assert_eq!(size(15_000), "14.6 KB");
        assert_eq!(size(300_000), "292 KB");
        assert_eq!(size(5 * 1024 * 1024 * 1024), "5.00 GB");
    }

    #[test]
    fn paths() {
        let home = glib::home_dir();
        assert_eq!(typed_path("~/Downloads"), home.join("Downloads"));
        assert_eq!(typed_path(" /tmp/x "), PathBuf::from("/tmp/x"));
        assert_eq!(display_path(&home.join("a b")), "~/a b");
        assert_eq!(display_path(Path::new("/srv")), "/srv");
    }
}
