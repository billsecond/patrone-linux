//! Where a tab is: Home, Gallery, This PC, a folder (any GIO location,
//! including the Recycle Bin and network shares) or search results.

use gtk::{gio, glib, prelude::*};

use crate::icons;
use crate::places::{self, Special};

#[derive(Clone, Debug)]
pub enum Location {
    Home,
    Gallery,
    ThisPc,
    Folder(gio::File),
    Search { root: gio::File, query: String },
}

impl Location {
    /// A string that identifies the place, for comparing and remembering it.
    pub fn key(&self) -> String {
        match self {
            Location::Home => "home".into(),
            Location::Gallery => "gallery".into(),
            Location::ThisPc => "this-pc".into(),
            Location::Folder(file) => file.uri().into(),
            Location::Search { root, query } => format!("search:{}?{query}", root.uri()),
        }
    }

    pub fn same(&self, other: &Location) -> bool {
        self.key() == other.key()
    }

    pub fn title(&self) -> String {
        match self {
            Location::Home => "Home".into(),
            Location::Gallery => "Gallery".into(),
            Location::ThisPc => "This PC".into(),
            Location::Folder(file) => places::display_name(file),
            Location::Search { root, .. } => {
                format!("Search Results in {}", places::display_name(root))
            }
        }
    }

    /// The 16-pixel color icon for tabs and menus.
    pub fn small_icon(&self) -> &'static str {
        match self {
            Location::Home => "pfm-nav-home",
            Location::Gallery => "pfm-nav-gallery",
            Location::ThisPc => "pfm-nav-computer",
            Location::Folder(file) => icons::small_folder_icon(file),
            Location::Search { root, .. } => icons::small_folder_icon(root),
        }
    }

    /// The folder new items and pasted items go into, if this is one.
    pub fn folder(&self) -> Option<&gio::File> {
        match self {
            Location::Folder(file) => Some(file),
            _ => None,
        }
    }

    /// Where the search box searches from here.
    pub fn search_root(&self) -> gio::File {
        match self {
            Location::Folder(file) => file.clone(),
            Location::Search { root, .. } => root.clone(),
            _ => places::home(),
        }
    }

    pub fn is_trash(&self) -> bool {
        matches!(self, Location::Folder(file) if places::is_trash(file))
    }

    /// Where the Up button goes.
    pub fn parent(&self) -> Option<Location> {
        match self {
            Location::Home | Location::Gallery | Location::ThisPc => None,
            Location::Search { root, .. } => Some(Location::Folder(root.clone())),
            Location::Folder(file) => {
                match places::special_of(file) {
                    Some(Special::Trash | Special::Network) => return Some(Location::Home),
                    Some(Special::Root) => return Some(Location::ThisPc),
                    _ => {}
                }
                if places::mount_root_of(file).is_some() {
                    return Some(if file.is_native() {
                        Location::ThisPc
                    } else {
                        Location::Folder(places::network())
                    });
                }
                match file.parent() {
                    Some(parent) => Some(Location::Folder(parent)),
                    None if !file.is_native() => Some(Location::Folder(places::network())),
                    None => Some(Location::ThisPc),
                }
            }
        }
    }

    /// The address bar's breadcrumbs, outermost first.
    pub fn crumbs(&self) -> Vec<(String, Location)> {
        let file = match self {
            Location::Folder(file) => file,
            _ => return vec![(self.title(), self.clone())],
        };
        let chain = |anchor: &gio::File| -> Vec<(String, Location)> {
            let mut chain = Vec::new();
            let mut current = Some(file.clone());
            while let Some(folder) = current {
                let at_anchor = folder.equal(anchor);
                chain.push((
                    places::display_name(&folder),
                    Location::Folder(folder.clone()),
                ));
                if at_anchor {
                    break;
                }
                current = folder.parent();
            }
            chain.reverse();
            chain
        };

        if places::is_trash(file) {
            return chain(&places::trash());
        }
        if places::special_of(file) == Some(Special::Network) {
            return vec![("Network".into(), self.clone())];
        }
        if !file.is_native() {
            // A network share: Network > server > share > folder.
            let mut root = file.clone();
            while let Some(parent) = root.parent() {
                root = parent;
            }
            let mut crumbs = vec![("Network".to_owned(), Location::Folder(places::network()))];
            crumbs.extend(chain(&root));
            return crumbs;
        }
        // The whole path from This PC and the drive, as Windows shows
        // This PC > Local Disk (C:) > Users > name; the address bar folds
        // what doesn't fit into its « button.
        let anchor = places::mount_containing(file)
            .map(|mount| mount.root())
            .unwrap_or_else(places::root);
        let mut crumbs = vec![("This PC".to_owned(), Location::ThisPc)];
        crumbs.extend(chain(&anchor));
        crumbs
    }

    /// The text the address bar shows when editing.
    pub fn edit_text(&self) -> String {
        match self {
            Location::Home => "Home".into(),
            Location::Gallery => "Gallery".into(),
            Location::ThisPc => "This PC".into(),
            Location::Folder(file) if places::is_trash_root(file) => "Recycle Bin".into(),
            Location::Folder(file) => match file.path() {
                Some(path) => path.to_string_lossy().into_owned(),
                None => file.parse_name().into(),
            },
            Location::Search { root, .. } => Location::Folder(root.clone()).edit_text(),
        }
    }
}

/// What text typed into the address bar names: a place, or a file or
/// folder (relative paths are relative to `current`).
pub enum Typed {
    Place(Location),
    File(gio::File),
}

pub fn parse_typed(text: &str, current: Option<&gio::File>) -> Option<Typed> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    let lower = text.to_lowercase();
    match lower.as_str() {
        "home" | "quick access" => return Some(Typed::Place(Location::Home)),
        "gallery" => return Some(Typed::Place(Location::Gallery)),
        "this pc" | "computer" | "my computer" => return Some(Typed::Place(Location::ThisPc)),
        "recycle bin" | "trash" => return Some(Typed::Place(Location::Folder(places::trash()))),
        "network" => return Some(Typed::Place(Location::Folder(places::network()))),
        "desktop" | "documents" | "downloads" | "music" | "pictures" | "videos" => {
            let special = match lower.as_str() {
                "desktop" => Special::Desktop,
                "documents" => Special::Documents,
                "downloads" => Special::Downloads,
                "music" => Special::Music,
                "pictures" => Special::Pictures,
                _ => Special::Videos,
            };
            if let Some(folder) = places::user_folder(special) {
                return Some(Typed::Place(Location::Folder(folder)));
            }
        }
        _ => {}
    }
    let file = if text.contains("://") || text.starts_with("trash:") || text.starts_with("network:")
    {
        gio::File::for_uri(text)
    } else if text == "~" {
        places::home()
    } else if let Some(rest) = text.strip_prefix("~/") {
        gio::File::for_path(glib::home_dir().join(rest))
    } else if text.starts_with('/') {
        gio::File::for_path(text)
    } else {
        match current {
            Some(folder) => folder.resolve_relative_path(text),
            None => gio::File::for_path(glib::home_dir().join(text)),
        }
    };
    Some(Typed::File(file))
}
