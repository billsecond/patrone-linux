//! Views, columns, sorting and grouping, and remembering them per folder.

use std::cell::RefCell;
use std::path::PathBuf;

use gtk::{gio, glib, prelude::*};

use crate::config;
use crate::location::Location;
use crate::places::{self, Special};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum View {
    ExtraLarge,
    Large,
    Medium,
    Small,
    List,
    Details,
    Tiles,
    Content,
}

pub const VIEWS: [View; 8] = [
    View::ExtraLarge,
    View::Large,
    View::Medium,
    View::Small,
    View::List,
    View::Details,
    View::Tiles,
    View::Content,
];

impl View {
    pub fn id(self) -> &'static str {
        match self {
            View::ExtraLarge => "extra-large",
            View::Large => "large",
            View::Medium => "medium",
            View::Small => "small",
            View::List => "list",
            View::Details => "details",
            View::Tiles => "tiles",
            View::Content => "content",
        }
    }

    pub fn from_id(id: &str) -> Option<View> {
        VIEWS.into_iter().find(|view| view.id() == id)
    }

    pub fn label(self) -> &'static str {
        match self {
            View::ExtraLarge => "Extra large icons",
            View::Large => "Large icons",
            View::Medium => "Medium icons",
            View::Small => "Small icons",
            View::List => "List",
            View::Details => "Details",
            View::Tiles => "Tiles",
            View::Content => "Content",
        }
    }

    pub fn glyph(self) -> &'static str {
        match self {
            View::ExtraLarge => "view-extra-large",
            View::Large => "view-large",
            View::Medium => "view-medium",
            View::Small => "view-small",
            View::List => "view-list",
            View::Details => "view-details",
            View::Tiles => "view-tiles",
            View::Content => "view-content",
        }
    }

    /// Windows' keyboard shortcut for the view: Ctrl+Shift+1 to 8.
    pub fn accel(self) -> String {
        let number = VIEWS.iter().position(|view| *view == self).unwrap() + 1;
        format!("Ctrl+Shift+{number}")
    }

    /// The next bigger or smaller view, for Ctrl+mouse wheel.
    pub fn step(self, bigger: bool) -> View {
        let order = [
            View::Details,
            View::List,
            View::Small,
            View::Medium,
            View::Large,
            View::ExtraLarge,
        ];
        let index = order.iter().position(|view| *view == self).unwrap_or(0);
        let next = if bigger {
            (index + 1).min(order.len() - 1)
        } else {
            index.saturating_sub(1)
        };
        order[next]
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Column {
    Name,
    Modified,
    Created,
    Accessed,
    Type,
    Size,
    Owner,
    Permissions,
    /// The Recycle Bin's "Date deleted".
    Deleted,
    /// The Recycle Bin's "Original location".
    Origin,
    /// Search results' "Folder".
    Folder,
    /// What a system folder is for, with System folder hints on.
    Purpose,
}

pub const COLUMNS: [Column; 12] = [
    Column::Name,
    Column::Modified,
    Column::Created,
    Column::Accessed,
    Column::Type,
    Column::Size,
    Column::Owner,
    Column::Permissions,
    Column::Deleted,
    Column::Origin,
    Column::Folder,
    Column::Purpose,
];

impl Column {
    pub fn id(self) -> &'static str {
        match self {
            Column::Name => "name",
            Column::Modified => "modified",
            Column::Created => "created",
            Column::Accessed => "accessed",
            Column::Type => "type",
            Column::Size => "size",
            Column::Owner => "owner",
            Column::Permissions => "permissions",
            Column::Deleted => "deleted",
            Column::Origin => "origin",
            Column::Folder => "folder",
            Column::Purpose => "purpose",
        }
    }

    pub fn from_id(id: &str) -> Option<Column> {
        COLUMNS.into_iter().find(|column| column.id() == id)
    }

    pub fn label(self) -> &'static str {
        match self {
            Column::Name => "Name",
            Column::Modified => "Date modified",
            Column::Created => "Date created",
            Column::Accessed => "Date accessed",
            Column::Type => "Type",
            Column::Size => "Size",
            Column::Owner => "Owner",
            Column::Permissions => "Permissions",
            Column::Deleted => "Date deleted",
            Column::Origin => "Original location",
            Column::Folder => "Folder",
            Column::Purpose => "Purpose",
        }
    }

    pub fn width(self) -> i32 {
        match self {
            Column::Name => 280,
            Column::Modified | Column::Created | Column::Accessed | Column::Deleted => 176,
            Column::Type => 150,
            Column::Size => 100,
            Column::Owner => 110,
            Column::Permissions => 110,
            Column::Origin | Column::Folder => 260,
            Column::Purpose => 190,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ViewState {
    pub view: View,
    pub sort: Column,
    pub descending: bool,
    pub group: Option<Column>,
}

impl ViewState {
    /// What a folder shows when it has no view of its own: Windows'
    /// templates, where pictures and videos get large icons.
    pub fn default_for(location: &Location) -> ViewState {
        let settings = config::settings();
        let mut view = View::from_id(&settings.string("default-view")).unwrap_or(View::Details);
        if let Location::Folder(file) = location
            && matches!(
                places::special_of(file),
                Some(Special::Pictures | Special::Videos)
            )
        {
            view = View::from_id(&settings.string("pictures-view")).unwrap_or(View::Large);
        }
        let mut state = ViewState {
            view,
            sort: Column::from_id(&settings.string("default-sort")).unwrap_or(Column::Name),
            descending: settings.boolean("default-sort-descending"),
            group: Column::from_id(&settings.string("default-group-by")),
        };
        match location {
            Location::Folder(file) if places::is_trash_root(file) => {
                state.view = View::Details;
            }
            Location::Folder(file) if places::special_of(file) == Some(Special::Downloads) => {
                // Windows groups Downloads by date.
                state.sort = Column::Modified;
                state.descending = true;
                state.group = Some(Column::Modified);
            }
            Location::Search { .. } => {
                state.view = View::Details;
            }
            _ => {}
        }
        state
    }

    /// The folder's own view if it has one, else the default.
    pub fn for_location(location: &Location) -> ViewState {
        if config::flag("remember-folder-views")
            && let Some(state) = load(&location.key())
        {
            return state;
        }
        ViewState::default_for(location)
    }
}

fn data_dir() -> PathBuf {
    glib::user_data_dir().join("patrone-file-manager")
}

thread_local! {
    static VIEWS_FILE: RefCell<Option<glib::KeyFile>> = const { RefCell::new(None) };
    static HISTORY_FILE: RefCell<Option<glib::KeyFile>> = const { RefCell::new(None) };
}

fn with_file<R>(
    cell: &'static std::thread::LocalKey<RefCell<Option<glib::KeyFile>>>,
    name: &str,
    action: impl FnOnce(&glib::KeyFile) -> R,
) -> R {
    cell.with(|slot| {
        let mut slot = slot.borrow_mut();
        let file = slot.get_or_insert_with(|| {
            let file = glib::KeyFile::new();
            let _ = file.load_from_file(data_dir().join(name), glib::KeyFileFlags::NONE);
            file
        });
        action(file)
    })
}

fn save_file(file: &glib::KeyFile, name: &str) {
    glib::mkdir_with_parents(data_dir(), 0o700);
    if let Err(error) = file.save_to_file(data_dir().join(name)) {
        eprintln!("patrone-file-manager: could not save {name}: {error}");
    }
}

fn load(key: &str) -> Option<ViewState> {
    with_file(&VIEWS_FILE, "folder-views.ini", |file| {
        let view = View::from_id(&file.string(key, "view").ok()?)?;
        Some(ViewState {
            view,
            sort: file
                .string(key, "sort")
                .ok()
                .and_then(|id| Column::from_id(&id))
                .unwrap_or(Column::Name),
            descending: file.boolean(key, "descending").unwrap_or(false),
            group: file
                .string(key, "group")
                .ok()
                .and_then(|id| Column::from_id(&id)),
        })
    })
}

/// Remembers a folder's view, as Windows does.
pub fn remember(location: &Location, state: &ViewState) {
    if !config::flag("remember-folder-views") {
        return;
    }
    let key = location.key();
    with_file(&VIEWS_FILE, "folder-views.ini", |file| {
        file.set_string(&key, "view", state.view.id());
        file.set_string(&key, "sort", state.sort.id());
        file.set_boolean(&key, "descending", state.descending);
        file.set_string(&key, "group", state.group.map(Column::id).unwrap_or("none"));
        save_file(file, "folder-views.ini");
    });
}

/// Folder Options' "Reset Folders": every folder goes back to the default.
pub fn forget_all() {
    with_file(&VIEWS_FILE, "folder-views.ini", |file| {
        for group in file.groups() {
            let _ = file.remove_group(&group);
        }
        save_file(file, "folder-views.ini");
    });
}

/// Folder Options' "Apply to Folders": this view becomes every folder's.
pub fn apply_to_all(state: &ViewState) {
    let settings = config::settings();
    let _ = settings.set_string("default-view", state.view.id());
    let _ = settings.set_string("pictures-view", state.view.id());
    let _ = settings.set_string("default-sort", state.sort.id());
    let _ = settings.set_boolean("default-sort-descending", state.descending);
    let _ = settings.set_string(
        "default-group-by",
        state.group.map(Column::id).unwrap_or("none"),
    );
    forget_all();
}

/// Counts a visit to a folder, for Quick access's frequent folders.
pub fn record_visit(folder: &gio::File) {
    if !config::flag("show-frequent-folders") || places::is_trash(folder) {
        return;
    }
    let key = folder.uri();
    let now = glib::real_time() / 1_000_000;
    with_file(&HISTORY_FILE, "history.ini", |file| {
        let visits = file.integer(&key, "visits").unwrap_or(0);
        file.set_integer(&key, "visits", visits + 1);
        file.set_int64(&key, "last", now);
        save_file(file, "history.ini");
    });
}

/// The most used folders lately, most used first.
pub fn frequent_folders(limit: usize) -> Vec<gio::File> {
    let now = glib::real_time() / 1_000_000;
    let mut scored: Vec<(f64, String)> = with_file(&HISTORY_FILE, "history.ini", |file| {
        file.groups()
            .iter()
            .map(|uri| {
                let visits = file.integer(uri, "visits").unwrap_or(0) as f64;
                let days = (now - file.int64(uri, "last").unwrap_or(0)) as f64 / 86_400.0;
                (visits / (1.0 + days / 7.0), uri.to_string())
            })
            .collect()
    });
    scored.sort_by(|a, b| b.0.total_cmp(&a.0));
    scored
        .into_iter()
        .map(|(_, uri)| gio::File::for_uri(&uri))
        .filter(|file| !file.is_native() || file.query_exists(gio::Cancellable::NONE))
        .take(limit)
        .collect()
}

/// Folder Options' "Clear File Explorer history".
pub fn clear_history() {
    with_file(&HISTORY_FILE, "history.ini", |file| {
        for group in file.groups() {
            let _ = file.remove_group(&group);
        }
        save_file(file, "history.ini");
    });
}
