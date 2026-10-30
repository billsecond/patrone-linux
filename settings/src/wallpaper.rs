//! The wallpaper: one of the pictures installed, from the same lists GNOME
//! Settings' Appearance page reads (gnome-background-properties), or None, a
//! plain color. Both are GNOME's own settings, for the desktop and the lock
//! screen, which GNOME Shell and the taskbar follow at once. The color is
//! kept in org.patrone.desktop too, so that it's still there after a picture.

use std::fs;
use std::path::{Path, PathBuf};

use gtk::gdk_pixbuf::{Colorspace, Pixbuf};
use gtk::gio;
use gtk::glib;
use gtk::prelude::*;

pub const BACKGROUND_SCHEMA: &str = "org.gnome.desktop.background";
const LOCK_SCHEMA: &str = "org.gnome.desktop.screensaver";
/// The color None shows, in org.patrone.desktop.
pub const COLOR_KEY: &str = "background-color";
/// The value of the None card; every other card's is its picture's path.
pub const NONE: &str = "none";

/// The colors to pick from: name and value. The first is the setting's
/// default.
pub const COLORS: [(&str, &str); 10] = [
    ("Patrone Blue", "#1851a8"),
    ("Navy", "#16213a"),
    ("Slate", "#3d4b63"),
    ("Teal", "#0f5f66"),
    ("Green", "#1f6b45"),
    ("Plum", "#5b2a6e"),
    ("Rust", "#8a3b1e"),
    ("Grey", "#6f7580"),
    ("Mist", "#dde3ec"),
    ("Black", "#030303"),
];

/// How wide a card's picture is kept, for screens of twice the density.
const THUMBNAIL_WIDTH: i32 = 376;

/// A wallpaper in the lists: dark is the picture for the dark style, where
/// it has its own, and options how GNOME fits it to the screen.
#[derive(Debug, PartialEq)]
pub struct Wallpaper {
    pub name: String,
    pub picture: PathBuf,
    pub dark: Option<PathBuf>,
    pub options: String,
}

fn element<'a>(text: &'a str, name: &str) -> Option<&'a str> {
    let (open, close) = (format!("<{name}>"), format!("</{name}>"));
    let start = text.find(&open)? + open.len();
    let end = start + text[start..].find(&close)?;
    Some(text[start..end].trim())
}

fn unescaped(text: &str) -> String {
    text.replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&apos;", "'").replace("&amp;", "&")
}

/// The wallpapers in one list.
fn parse(xml: &str) -> Vec<Wallpaper> {
    let mut text = xml.to_string();
    while let (Some(begin), Some(end)) = (text.find("<!--"), text.find("-->")) {
        if end < begin {
            break;
        }
        text.replace_range(begin..end + 3, "");
    }
    text.split("<wallpaper")
        .skip(1)
        // Not <wallpapers>, the list itself
        .filter(|entry| entry.starts_with([' ', '\n', '\t', '>']))
        .filter(|entry| !entry.split('>').next().unwrap_or("").contains("deleted=\"true\""))
        .filter_map(|entry| {
            Some(Wallpaper {
                name: unescaped(element(entry, "name")?),
                picture: PathBuf::from(unescaped(element(entry, "filename")?)),
                dark: element(entry, "filename-dark").map(|path| PathBuf::from(unescaped(path))),
                options: element(entry, "options").unwrap_or("zoom").to_string(),
            })
        })
        .collect()
}

/// The wallpapers installed: the account's lists, then the system's.
pub fn installed() -> Vec<Wallpaper> {
    let mut found: Vec<Wallpaper> = Vec::new();
    for dir in std::iter::once(glib::user_data_dir()).chain(glib::system_data_dirs()) {
        let Ok(entries) = fs::read_dir(dir.join("gnome-background-properties")) else { continue };
        let mut lists: Vec<PathBuf> = entries
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|extension| extension == "xml"))
            .collect();
        lists.sort();
        for list in lists {
            let Ok(text) = fs::read_to_string(&list) else { continue };
            for wallpaper in parse(&text) {
                if wallpaper.picture.is_file() && !found.iter().any(|seen| seen.picture == wallpaper.picture) {
                    found.push(wallpaper);
                }
            }
        }
    }
    found
}

/// The card's value for the wallpaper that's set: NONE, or the path of the
/// one it matches, in its light or dark version (following links: wallpaper
/// 01's names are links to 02's), or of the picture if it isn't one of them.
pub fn picked(background: &gio::Settings, wallpapers: &[Wallpaper]) -> String {
    if background.string("picture-options") == "none" {
        return NONE.to_string();
    }
    let Some(path) = gio::File::for_uri(&background.string("picture-uri")).path() else {
        return String::new();
    };
    let same = |other: &Path| match (fs::canonicalize(&path), fs::canonicalize(other)) {
        (Ok(first), Ok(second)) => first == second,
        _ => path == other,
    };
    wallpapers
        .iter()
        .find(|wallpaper| same(&wallpaper.picture) || wallpaper.dark.as_deref().is_some_and(same))
        .map_or_else(|| path.display().to_string(), |wallpaper| wallpaper.picture.display().to_string())
}

/// Changes a schema's settings together, so nothing reads them half set.
fn write(schema: &str, change: impl Fn(&gio::Settings)) {
    let settings = gio::Settings::new(schema);
    settings.delay();
    change(&settings);
    settings.apply();
}

fn schema_has(settings: &gio::Settings, key: &str) -> bool {
    settings.settings_schema().is_some_and(|schema| schema.has_key(key))
}

/// Shows a picture on the desktop and the lock screen, as GNOME Settings'
/// Appearance page does.
pub fn show_picture(wallpaper: &Wallpaper) {
    let uri = gio::File::for_path(&wallpaper.picture).uri();
    let dark = wallpaper.dark.as_ref().map_or_else(|| uri.clone(), |path| gio::File::for_path(path).uri());
    for schema in [BACKGROUND_SCHEMA, LOCK_SCHEMA] {
        write(schema, |settings| {
            let _ = settings.set_string("picture-uri", &uri);
            if schema_has(settings, "picture-uri-dark") {
                let _ = settings.set_string("picture-uri-dark", &dark);
            }
            let _ = settings.set_string("picture-options", &wallpaper.options);
        });
    }
}

/// Shows a plain color on the desktop and the lock screen. picture-options
/// is what hides the picture; picture-uri stays as it was.
pub fn show_color(color: &str) {
    for schema in [BACKGROUND_SCHEMA, LOCK_SCHEMA] {
        write(schema, |settings| {
            let _ = settings.set_string("primary-color", color);
            let _ = settings.set_string("secondary-color", color);
            let _ = settings.set_string("color-shading-type", "solid");
            let _ = settings.set_string("picture-options", "none");
        });
    }
}

/// "#rrggbb" as a number, for drawing.
pub fn color_number(color: &str) -> u32 {
    color.strip_prefix('#').and_then(|hex| u32::from_str_radix(hex, 16).ok()).unwrap_or(0)
}

/// A card's picture, as its pixels: a Pixbuf can't move between threads.
pub struct Thumbnail {
    pixels: glib::Bytes,
    has_alpha: bool,
    width: i32,
    height: i32,
    rowstride: i32,
}

impl Thumbnail {
    fn of(pixbuf: &Pixbuf) -> Self {
        Thumbnail {
            pixels: pixbuf.read_pixel_bytes(),
            has_alpha: pixbuf.has_alpha(),
            width: pixbuf.width(),
            height: pixbuf.height(),
            rowstride: pixbuf.rowstride(),
        }
    }

    pub fn pixbuf(&self) -> Pixbuf {
        Pixbuf::from_bytes(&self.pixels, Colorspace::Rgb, self.has_alpha, 8, self.width, self.height, self.rowstride)
    }
}

/// A card's picture. The wallpapers are up to 8K, so this runs off the main
/// thread and keeps what it makes: the first time takes a second or two for
/// each, and after that they come straight from the account's cache.
pub fn thumbnail(picture: &Path) -> Option<Thumbnail> {
    let modified = fs::metadata(picture).and_then(|data| data.modified()).ok()?;
    let stamp = modified.duration_since(std::time::UNIX_EPOCH).ok()?.as_secs();
    let name = format!("{}:{stamp}:{THUMBNAIL_WIDTH}", picture.display());
    let key = glib::compute_checksum_for_string(glib::ChecksumType::Sha256, &name)?;
    let folder = glib::user_cache_dir().join("patrone-settings").join("wallpapers");
    let cached = folder.join(format!("{key}.png"));
    if let Ok(pixbuf) = Pixbuf::from_file(&cached) {
        return Some(Thumbnail::of(&pixbuf));
    }
    let pixbuf = Pixbuf::from_file_at_scale(picture, THUMBNAIL_WIDTH, -1, true).ok()?;
    // Written whole, then moved: another window may be reading the folder
    let partial = folder.join(format!("{key}.{}.part", std::process::id()));
    if fs::create_dir_all(&folder).is_ok() && pixbuf.savev(&partial, "png", &[]).is_ok() {
        let _ = fs::rename(&partial, &cached);
    }
    Some(Thumbnail::of(&pixbuf))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_lists() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE wallpapers SYSTEM "gnome-wp-list.dtd">
<!-- A <wallpaper> in a comment doesn't count -->
<wallpapers>
  <wallpaper deleted="false">
    <name>Neon &amp; Blue</name>
    <filename>/usr/share/backgrounds/a-light.png</filename>
    <filename-dark>/usr/share/backgrounds/a-dark.png</filename-dark>
    <options>zoom</options>
  </wallpaper>
  <wallpaper deleted="true">
    <name>Gone</name>
    <filename>/usr/share/backgrounds/gone.png</filename>
  </wallpaper>
  <wallpaper>
    <name>Plain</name>
    <filename>/usr/share/backgrounds/b.png</filename>
    <options>centered</options>
  </wallpaper>
</wallpapers>
"#;
        assert_eq!(
            parse(xml),
            [
                Wallpaper {
                    name: "Neon & Blue".into(),
                    picture: "/usr/share/backgrounds/a-light.png".into(),
                    dark: Some("/usr/share/backgrounds/a-dark.png".into()),
                    options: "zoom".into(),
                },
                Wallpaper {
                    name: "Plain".into(),
                    picture: "/usr/share/backgrounds/b.png".into(),
                    dark: None,
                    options: "centered".into(),
                },
            ]
        );
    }

    #[test]
    fn reads_patrones_own_list() {
        let list = Path::new(env!("CARGO_MANIFEST_DIR")).join("../branding/system/backgrounds/patrone-wallpapers.xml");
        // Not there when the package is built, from this folder alone
        if let Ok(text) = fs::read_to_string(list) {
            let wallpapers = parse(&text);
            assert!(wallpapers.len() >= 20);
            assert_eq!(wallpapers[0].name, "Neon Horizon");
            assert!(wallpapers[0].dark.is_some());
        }
    }

    #[test]
    fn colors_are_ones_gnome_reads() {
        assert_eq!(COLORS[0].1, "#1851a8");
        for (_, color) in COLORS {
            assert_eq!(color.len(), 7);
            assert!(gtk::gdk::RGBA::parse(color).is_ok(), "{color}");
        }
        assert_eq!(color_number("#1851a8"), 0x1851a8);
    }
}
