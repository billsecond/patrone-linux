//! Desktop entries (the .desktop files that name apps and give their
//! icons), read with GLib's KeyFile.

use std::path::{Path, PathBuf};

use gtk::{gio, glib};

pub const GROUP: &str = "Desktop Entry";

pub struct Entry {
    pub path: PathBuf,
    pub name: String,
    pub icon: Option<gio::Icon>,
    /// Whether the app menus show it: not NoDisplay or Hidden, and for
    /// this desktop
    pub shown: bool,
}

/// Finds an app's desktop entry by its ID (org.patrone.Terminal) in the
/// account's and the system's applications folders.
pub fn find(id: &str) -> Option<Entry> {
    let file_name = format!("{id}.desktop");
    let folders = std::iter::once(glib::user_data_dir()).chain(glib::system_data_dirs());
    for folder in folders {
        let path = folder.join("applications").join(&file_name);
        if let Some(file) = load(&path) {
            return Some(Entry {
                name: file
                    .locale_string(GROUP, "Name", None)
                    .map(|name| name.to_string())
                    .unwrap_or_else(|_| id.to_string()),
                icon: file
                    .string(GROUP, "Icon")
                    .ok()
                    .and_then(|icon| gio::Icon::for_string(&icon).ok()),
                shown: flag(&file, "NoDisplay") != Some(true)
                    && flag(&file, "Hidden") != Some(true)
                    && for_this_desktop(&file),
                path,
            });
        }
    }
    None
}

pub fn load(path: &Path) -> Option<glib::KeyFile> {
    let file = glib::KeyFile::new();
    file.load_from_file(
        path,
        glib::KeyFileFlags::KEEP_COMMENTS | glib::KeyFileFlags::KEEP_TRANSLATIONS,
    )
    .ok()?;
    Some(file)
}

pub fn flag(file: &glib::KeyFile, key: &str) -> Option<bool> {
    file.boolean(GROUP, key).ok()
}

/// Whether an entry is for this desktop, by OnlyShowIn and NotShowIn.
pub fn for_this_desktop(file: &glib::KeyFile) -> bool {
    let current = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();
    let desktops: Vec<&str> = current
        .split(':')
        .filter(|desktop| !desktop.is_empty())
        .collect();
    let list = |key: &str| -> Option<Vec<String>> {
        file.string_list(GROUP, key)
            .ok()
            .map(|list| list.iter().map(|item| item.to_string()).collect())
    };
    if let Some(only) = list("OnlyShowIn")
        && !only
            .iter()
            .any(|desktop| desktops.contains(&desktop.as_str()))
    {
        return false;
    }
    if let Some(not) = list("NotShowIn")
        && not
            .iter()
            .any(|desktop| desktops.contains(&desktop.as_str()))
    {
        return false;
    }
    true
}
