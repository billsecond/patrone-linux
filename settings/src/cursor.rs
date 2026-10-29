//! The mouse pointer: the cursor themes installed, with each one's own
//! pictures of its pointers (read from its Xcursor files), and GNOME's
//! cursor-theme and cursor-size settings, which GNOME Shell and apps follow.
//!
//! Only themes with the cursor names GNOME asks for are offered. GNOME
//! Shell (mutter's Xcursor loader) looks a cursor up by its name in the
//! theme and the themes it inherits from, then in the system's default
//! theme, and only then by its old X11 name: a theme with only the old
//! names (DMZ, X.Org's whiteglass and redglass) shows the default theme's
//! pointers (Patrone's), so choosing it would seem to do nothing.

use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;
use std::time::Duration;

use gtk::glib;
use gtk::prelude::*;

pub const THEME_KEY: &str = "cursor-theme";
pub const SIZE_KEY: &str = "cursor-size";

/// The sizes to pick from, as GNOME's Accessibility page names them.
pub const SIZES: [(i32, &str, &str); 5] = [
    (24, "Default", "24 pixels, the usual size"),
    (32, "Medium", "32 pixels"),
    (48, "Large", "48 pixels"),
    (64, "Larger", "64 pixels"),
    (96, "Largest", "96 pixels"),
];

/// The pointers a theme's card shows, each by its names, newest first:
/// the arrow, the hand over links, the text cursor and the busy one.
pub const SHOWN: [&[&str]; 4] = [
    &["default", "left_ptr"],
    &["pointer", "hand2", "hand1"],
    &["text", "xterm"],
    &["wait", "watch"],
];

/// The names GNOME asks for that a theme must have (or inherit) to be
/// offered.
const NEEDED: [&str; 3] = ["default", "pointer", "text"];

/// How deep Inherits chains are followed, as libXcursor does.
const MAX_INHERITS: usize = 4;

/// Patrone's words for the themes it ships or knows; others say where
/// they're installed.
const KNOWN: [(&str, &str); 5] = [
    ("Patrone", "Patrone's own: blue, with a white edge"),
    ("Adwaita", "GNOME's own: black, with a white edge"),
    ("Yaru", "Yaru's: dark grey, with a white edge"),
    ("breeze_cursors", "KDE's Breeze: black, with a white edge"),
    ("Breeze_Light", "KDE's Breeze: white, with a dark edge"),
];

/// One picture of a cursor: its first frame at one size, in cairo's
/// ARGB32 (premultiplied, native byte order, as Xcursor files have it).
pub struct Image {
    pub nominal: u32,
    pub width: i32,
    pub height: i32,
    pub xhot: f64,
    pub yhot: f64,
    pub pixels: Vec<u8>,
}

/// A cursor at each size its file has.
pub struct Cursor {
    images: Vec<Image>,
}

impl Cursor {
    /// The picture to draw at `size` pixels on a surface `scale` times
    /// denser: the smallest one at least that big, or the biggest.
    pub fn image_for(&self, size: f64, scale: f64) -> Option<&Image> {
        let wanted = size * scale;
        self.images
            .iter()
            .filter(|image| f64::from(image.nominal) >= wanted)
            .min_by_key(|image| image.nominal)
            .or_else(|| self.images.iter().max_by_key(|image| image.nominal))
    }
}

pub struct Theme {
    /// Its folder's name, the setting's value
    pub name: String,
    pub title: String,
    pub detail: String,
    /// The pointers in SHOWN's order, where it has them
    pub cursors: Vec<Option<Cursor>>,
}

/// Where cursor themes are, in the order they're looked up (libXcursor's
/// and GTK's): the account's, then the system's.
pub fn search_dirs() -> Vec<PathBuf> {
    let mut dirs = vec![glib::user_data_dir().join("icons"), glib::home_dir().join(".icons")];
    dirs.extend(glib::system_data_dirs().into_iter().map(|dir| dir.join("icons")));
    let mut seen = HashSet::new();
    dirs.retain(|dir| seen.insert(dir.clone()));
    dirs
}

/// The [Icon Theme] section's value for `key`, untranslated.
fn index_value(text: &str, key: &str) -> Option<String> {
    let mut in_section = false;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_section = line == "[Icon Theme]";
        } else if in_section {
            if let Some((name, value)) = line.split_once('=') {
                if name.trim() == key {
                    return Some(value.trim().to_string());
                }
            }
        }
    }
    None
}

fn index_of(dirs: &[PathBuf], theme: &str) -> Option<String> {
    dirs.iter().find_map(|dir| fs::read_to_string(dir.join(theme).join("index.theme")).ok())
}

/// The themes `theme` inherits from.
fn parents(dirs: &[PathBuf], theme: &str) -> Vec<String> {
    index_of(dirs, theme)
        .and_then(|text| index_value(&text, "Inherits"))
        .map(|list| {
            list.split([',', ';'])
                .map(str::trim)
                .filter(|name| !name.is_empty())
                .map(String::from)
                .collect()
        })
        .unwrap_or_default()
}

/// The file for cursor `name` in `theme` or a theme it inherits from (not
/// the system's default theme, which is what a missing one falls back to).
pub fn find(dirs: &[PathBuf], theme: &str, name: &str) -> Option<PathBuf> {
    fn search(dirs: &[PathBuf], theme: &str, name: &str, depth: usize, visited: &mut Vec<String>) -> Option<PathBuf> {
        if depth > MAX_INHERITS || visited.iter().any(|seen| seen == theme) {
            return None;
        }
        visited.push(theme.to_string());
        let found = dirs.iter().map(|dir| dir.join(theme).join("cursors").join(name)).find(|path| path.is_file());
        found.or_else(|| {
            parents(dirs, theme)
                .iter()
                .find_map(|parent| search(dirs, parent, name, depth + 1, visited))
        })
    }
    search(dirs, theme, name, 0, &mut Vec::new())
}

/// A cursor by the first of `names` that `theme` has.
pub fn load(dirs: &[PathBuf], theme: &str, names: &[&str]) -> Option<Cursor> {
    names
        .iter()
        .find_map(|name| find(dirs, theme, name))
        .and_then(|path| fs::read(path).ok())
        .and_then(|data| parse(&data))
}

/// The first frame at each size in an Xcursor file.
pub fn parse(data: &[u8]) -> Option<Cursor> {
    const IMAGE: u32 = 0xfffd_0002;
    let word = |at: usize| -> Option<u32> {
        let bytes = data.get(at..at.checked_add(4)?)?;
        Some(u32::from_le_bytes(bytes.try_into().ok()?))
    };
    if data.get(..4)? != b"Xcur" {
        return None;
    }
    let header = word(4)? as usize;
    let count = word(12)? as usize;
    let mut images: Vec<Image> = Vec::new();
    for entry in 0..count.min(4096) {
        let at = header + entry * 12;
        let (kind, nominal, position) = (word(at)?, word(at + 4)?, word(at + 8)? as usize);
        if kind != IMAGE || images.iter().any(|image| image.nominal == nominal) {
            continue;
        }
        let (width, height) = (word(position + 16)?, word(position + 20)?);
        if width == 0 || height == 0 || width > 0x7fff || height > 0x7fff {
            continue;
        }
        let start = position + word(position)? as usize;
        let pixels = data.get(start..start + (width * height * 4) as usize)?;
        images.push(Image {
            nominal,
            width: width as i32,
            height: height as i32,
            xhot: f64::from(word(position + 24)?),
            yhot: f64::from(word(position + 28)?),
            // Little-endian ARGB words, as cairo wants them in this
            // machine's byte order
            pixels: pixels
                .chunks_exact(4)
                .flat_map(|pixel| u32::from_le_bytes([pixel[0], pixel[1], pixel[2], pixel[3]]).to_ne_bytes())
                .collect(),
        });
    }
    (!images.is_empty()).then_some(Cursor { images })
}

/// The cursor themes installed that GNOME can show, Patrone's first.
pub fn installed() -> Vec<Theme> {
    let dirs = search_dirs();
    let user_dirs = &dirs[..2];
    let mut names: Vec<(String, bool)> = Vec::new();
    for (index, dir) in dirs.iter().enumerate() {
        let Ok(entries) = fs::read_dir(dir) else { continue };
        let mut found: Vec<String> = entries
            .flatten()
            .filter(|entry| entry.path().join("cursors").is_dir())
            .filter_map(|entry| entry.file_name().into_string().ok())
            .collect();
        found.sort();
        for name in found {
            if name != "default" && !names.iter().any(|(seen, _)| *seen == name) {
                names.push((name, index < user_dirs.len()));
            }
        }
    }

    let mut themes: Vec<Theme> = names
        .into_iter()
        .filter(|(name, _)| NEEDED.iter().all(|cursor| find(&dirs, name, cursor).is_some()))
        .map(|(name, for_you)| {
            let title = index_of(&dirs, &name)
                .and_then(|text| index_value(&text, "Name"))
                .unwrap_or_else(|| name.clone());
            let detail = KNOWN
                .iter()
                .find(|(known, _)| *known == name)
                .map(|(_, detail)| detail.to_string())
                .unwrap_or_else(|| if for_you { "Installed for you".into() } else { "Installed for everyone".into() });
            let cursors = SHOWN.iter().map(|names| load(&dirs, &name, names)).collect();
            Theme { name, title, detail, cursors }
        })
        .collect();
    themes.sort_by_key(|theme| (theme.name != "Patrone", theme.title.to_lowercase()));
    themes
}

/// Shows the pointer that's over `widget`'s window in the new theme at once.
/// GNOME Shell drops its cursors when the theme changes, but the one on
/// screen stays until an app asks for another; so for a moment this
/// window asks for the busy one, then for none of its own again.
pub fn refresh_pointer(widget: &impl IsA<gtk::Widget>) {
    let Some(root) = widget.root() else { return };
    let window: gtk::Widget = root.upcast();
    window.set_cursor_from_name(Some("progress"));
    glib::timeout_add_local_once(Duration::from_millis(300), move || window.set_cursor(None::<&gtk::gdk::Cursor>));
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    /// An Xcursor file with one image per (size, width, height, hotspot).
    fn xcursor(images: &[(u32, u32, u32, (u32, u32))]) -> Vec<u8> {
        let mut data = b"Xcur".to_vec();
        for word in [16u32, 0x1_0000, images.len() as u32] {
            data.extend(word.to_le_bytes());
        }
        let mut position = 16 + 12 * images.len() as u32;
        let mut chunks = Vec::new();
        for &(size, width, height, (xhot, yhot)) in images {
            data.extend(0xfffd_0002u32.to_le_bytes());
            data.extend(size.to_le_bytes());
            data.extend(position.to_le_bytes());
            let mut chunk = Vec::new();
            for word in [36, 0xfffd_0002, size, 1, width, height, xhot, yhot, 50] {
                chunk.extend(u32::to_le_bytes(word));
            }
            for _ in 0..width * height {
                chunk.extend(0xff10_2030u32.to_le_bytes());
            }
            position += chunk.len() as u32;
            chunks.push(chunk);
        }
        data.extend(chunks.concat());
        data
    }

    #[test]
    fn reads_each_size_once() {
        let data = xcursor(&[(24, 24, 24, (3, 4)), (24, 24, 24, (0, 0)), (48, 48, 48, (6, 8))]);
        let cursor = parse(&data).unwrap();
        assert_eq!(cursor.images.len(), 2);
        let small = cursor.image_for(24.0, 1.0).unwrap();
        assert_eq!((small.nominal, small.xhot, small.yhot), (24, 3.0, 4.0));
        assert_eq!(cursor.image_for(24.0, 2.0).unwrap().nominal, 48);
        assert_eq!(cursor.image_for(30.0, 1.0).unwrap().nominal, 48);
        // Bigger than any: the biggest
        assert_eq!(cursor.image_for(96.0, 1.0).unwrap().nominal, 48);
        assert_eq!(&small.pixels[..4], &0xff10_2030u32.to_ne_bytes());
    }

    #[test]
    fn rejects_what_isnt_a_cursor() {
        assert!(parse(b"").is_none());
        assert!(parse(b"PNG not a cursor").is_none());
        let mut cut = xcursor(&[(24, 24, 24, (0, 0))]);
        cut.truncate(cut.len() - 10);
        assert!(parse(&cut).is_none());
    }

    fn theme(root: &Path, name: &str, cursors: &[&str], inherits: Option<&str>) {
        let dir = root.join(name).join("cursors");
        fs::create_dir_all(&dir).unwrap();
        for cursor in cursors {
            fs::write(dir.join(cursor), xcursor(&[(24, 2, 2, (0, 0))])).unwrap();
        }
        if let Some(parent) = inherits {
            let index = format!("[Icon Theme]\nName={name} theme\nInherits={parent}\n");
            fs::write(root.join(name).join("index.theme"), index).unwrap();
        }
    }

    #[test]
    fn finds_cursors_in_the_themes_a_theme_inherits_from() {
        let root = std::env::temp_dir().join(format!("patrone-cursor-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        theme(&root, "Base", &["default", "pointer", "text"], None);
        theme(&root, "Child", &["default"], Some("Base"));
        theme(&root, "Old", &["left_ptr", "hand2", "xterm"], None);
        theme(&root, "Loop", &[], Some("Loop"));
        let dirs = vec![root.clone()];
        assert!(find(&dirs, "Child", "pointer").unwrap().starts_with(root.join("Base")));
        assert!(find(&dirs, "Child", "default").unwrap().starts_with(root.join("Child")));
        // Only old X11 names: GNOME wouldn't show it, so it isn't offered
        assert!(find(&dirs, "Old", "default").is_none());
        assert!(load(&dirs, "Old", SHOWN[0]).is_some());
        assert!(find(&dirs, "Loop", "default").is_none());
        let index = fs::read_to_string(root.join("Child/index.theme")).unwrap();
        assert_eq!(index_value(&index, "Name").as_deref(), Some("Child theme"));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn reads_index_files() {
        let text = "[Icon Theme]\nName = DMZ (White)\nName[de]=DMZ (Weiß)\nInherits=Adwaita, hicolor\n[Other]\nName=No\n";
        assert_eq!(index_value(text, "Name").as_deref(), Some("DMZ (White)"));
        assert_eq!(index_value(text, "Inherits").as_deref(), Some("Adwaita, hicolor"));
        assert_eq!(index_value("[Other]\nName=No\n", "Name"), None);
    }

    #[test]
    fn the_sizes_are_gnomes() {
        assert_eq!(SIZES.map(|(size, _, _)| size), [24, 32, 48, 64, 96]);
    }
}
