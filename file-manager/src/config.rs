//! Settings, kept in GSettings (`org.patrone.FileManager`), and the lists
//! stored there: folders pinned to Quick access, and Favorites.

use gtk::{gio, glib, prelude::*};

use crate::APP_ID;

thread_local! {
    static SETTINGS: gio::Settings = load();
}

/// The app's settings.
pub fn settings() -> gio::Settings {
    SETTINGS.with(Clone::clone)
}

pub fn flag(key: &str) -> bool {
    settings().boolean(key)
}

pub fn string(key: &str) -> String {
    settings().string(key).into()
}

fn load() -> gio::Settings {
    let default = gio::SettingsSchemaSource::default();
    let installed = default
        .as_ref()
        .and_then(|source| source.lookup(APP_ID, true));
    // A debug build runs from the build tree, where build.rs compiled the
    // schema; an older installed copy could lack new keys.
    let schema = match installed {
        Some(schema) if !cfg!(debug_assertions) => schema,
        installed => gio::SettingsSchemaSource::from_directory(
            concat!(env!("OUT_DIR"), "/schemas"),
            default.as_ref(),
            false,
        )
        .ok()
        .and_then(|source| source.lookup(APP_ID, false))
        .or(installed)
        .expect("the org.patrone.FileManager settings schema isn't installed"),
    };
    gio::Settings::new_full(&schema, None::<&gio::SettingsBackend>, None)
}

/// User folders by the names `pinned-folders` uses for them.
const XDG_NAMES: &[(&str, glib::UserDirectory)] = &[
    ("DESKTOP", glib::UserDirectory::Desktop),
    ("DOCUMENTS", glib::UserDirectory::Documents),
    ("DOWNLOAD", glib::UserDirectory::Downloads),
    ("MUSIC", glib::UserDirectory::Music),
    ("PICTURES", glib::UserDirectory::Pictures),
    ("PUBLICSHARE", glib::UserDirectory::PublicShare),
    ("TEMPLATES", glib::UserDirectory::Templates),
    ("VIDEOS", glib::UserDirectory::Videos),
];

fn resolve(entry: &str) -> Option<gio::File> {
    if let Some(name) = entry.strip_prefix("xdg:") {
        let (_, directory) = XDG_NAMES.iter().find(|(known, _)| *known == name)?;
        let path = glib::user_special_dir(*directory)?;
        // xdg-user-dirs points unset folders at the home folder.
        (path != glib::home_dir()).then(|| gio::File::for_path(path))
    } else {
        Some(gio::File::for_uri(entry))
    }
}

fn entry_for(file: &gio::File) -> String {
    if let Some(path) = file.path() {
        for (name, directory) in XDG_NAMES {
            if glib::user_special_dir(*directory).as_deref() == Some(path.as_path()) {
                return format!("xdg:{name}");
            }
        }
    }
    file.uri().into()
}

fn files(key: &str) -> Vec<gio::File> {
    settings()
        .strv(key)
        .iter()
        .filter_map(|entry| resolve(entry))
        .collect()
}

fn set_files(key: &str, files: &[gio::File]) {
    let entries: Vec<String> = files.iter().map(entry_for).collect();
    let entries: Vec<&str> = entries.iter().map(String::as_str).collect();
    if let Err(error) = settings().set_strv(key, entries.as_slice()) {
        eprintln!("patrone-file-manager: could not save {key}: {error}");
    }
}

/// Folders pinned to Quick access, in order.
pub fn pinned() -> Vec<gio::File> {
    files("pinned-folders")
}

pub fn is_pinned(file: &gio::File) -> bool {
    pinned().iter().any(|pinned| pinned.equal(file))
}

pub fn set_pinned(file: &gio::File, pin: bool) {
    let mut pinned = pinned();
    pinned.retain(|pinned| !pinned.equal(file));
    if pin {
        pinned.push(file.clone());
    }
    set_files("pinned-folders", &pinned);
}

/// Moves a pinned folder to `index` in Quick access.
pub fn move_pinned(file: &gio::File, index: usize) {
    let mut pinned = pinned();
    let Some(from) = pinned.iter().position(|pinned| pinned.equal(file)) else {
        return;
    };
    let file = pinned.remove(from);
    pinned.insert(index.min(pinned.len()), file);
    set_files("pinned-folders", &pinned);
}

/// Files added to Favorites.
pub fn favorites() -> Vec<gio::File> {
    files("favorite-files")
}

pub fn is_favorite(file: &gio::File) -> bool {
    favorites().iter().any(|favorite| favorite.equal(file))
}

pub fn set_favorite(file: &gio::File, favorite: bool) {
    let mut favorites = favorites();
    favorites.retain(|known| !known.equal(file));
    if favorite {
        favorites.insert(0, file.clone());
    }
    set_files("favorite-files", &favorites);
}
