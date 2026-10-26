//! What the commands do: opening items, the clipboard, deleting, renaming,
//! new items, archives, the Recycle Bin, and the rest of the command bar
//! and right-click menus.

use std::path::PathBuf;

use gtk::{gdk, gio, glib, prelude::*};

use crate::archive;
use crate::clipboard;
use crate::config;
use crate::dialogs::{self, Dialog};
use crate::filecell;
use crate::icons;
use crate::location::Location;
use crate::ops;
use crate::places::{self, Special};
use crate::tab::Page;
use crate::util;
use crate::window::Window;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OpenIn {
    Default,
    NewTab,
    NewWindow,
}

fn parent(window: &Window) -> Option<gtk::Window> {
    Some(window.gtk_window())
}

/// Whether GVfs can browse inside archives (`archive://`).
fn can_browse_archives() -> bool {
    gio::Vfs::default()
        .supported_uri_schemes()
        .iter()
        .any(|scheme| scheme == "archive")
}

/// The folder to browse for an item, if it's one: folders, drives,
/// network places, and ZIP files (as Windows opens them).
fn folder_target(info: &gio::FileInfo) -> Option<gio::File> {
    let file = util::file_of(info)?;
    if let Some(target) = info.attribute_string("standard::target-uri") {
        return Some(gio::File::for_uri(&target));
    }
    if util::is_dir(info) || info.boolean("pfm::drive") {
        return Some(file);
    }
    if util::content_type(info) == "application/zip" && file.is_native() && can_browse_archives() {
        let escaped = glib::Uri::escape_string(&file.uri(), None, false);
        return Some(gio::File::for_uri(&format!("archive://{escaped}/")));
    }
    None
}

/// Opens items: folders in this tab (or a new tab or window), files in
/// their default app.
pub fn open_infos(window: &Window, infos: Vec<gio::FileInfo>, how: OpenIn) {
    let mut folders = Vec::new();
    let mut files = Vec::new();
    for info in infos {
        match folder_target(&info) {
            Some(folder) => folders.push(folder),
            None => files.push(info),
        }
    }
    let own_window = config::flag("open-folders-in-new-window");
    for (index, folder) in folders.into_iter().enumerate() {
        let location = Location::Folder(folder);
        match how {
            OpenIn::NewTab => {
                window.new_tab(location, index == 0);
            }
            OpenIn::NewWindow => new_window(window, location),
            OpenIn::Default if own_window => new_window(window, location),
            OpenIn::Default if index == 0 => window.navigate(location),
            OpenIn::Default => {
                window.new_tab(location, false);
            }
        }
    }
    for info in files {
        launch(window, &info);
    }
}

fn new_window(window: &Window, location: Location) {
    if let Some(app) = window.window.application() {
        Window::new(&app, location).present();
    }
}

fn launch_context(window: &Window) -> gio::AppLaunchContext {
    gtk::prelude::WidgetExt::display(&window.window)
        .app_launch_context()
        .upcast()
}

fn remember_recent(file: &gio::File) {
    gtk::RecentManager::default().add_item(&file.uri());
}

/// Opens a file the way double-clicking does.
pub fn launch(window: &Window, info: &gio::FileInfo) {
    let Some(file) = util::file_of(info) else {
        return;
    };
    let mime = util::content_type(info);
    if mime == "application/x-desktop" {
        // GLib's own launcher reads the entry and starts the app.
        if let Some(path) = file.path()
            && std::process::Command::new("gio")
                .arg("launch")
                .arg(&path)
                .spawn()
                .is_ok()
        {
            return;
        }
    }
    let executable = info.boolean("access::can-execute")
        && matches!(
            mime.as_str(),
            "application/x-executable"
                | "application/x-pie-executable"
                | "application/x-shellscript"
                | "text/x-python"
                | "text/x-python3"
                | "application/x-perl"
        );
    if executable {
        let window = window.clone();
        let info = info.clone();
        glib::spawn_future_local(async move {
            let choice = dialogs::message(
                parent(&window).as_ref(),
                "Open File",
                "question",
                &format!(
                    "Do you want to run \"{}\", or display its contents?",
                    info.display_name()
                ),
                "\"Run\" starts it as a program. Only run programs you trust.",
                &[
                    ("_Run", 0),
                    ("Run in _Terminal", 1),
                    ("_Display", 2),
                    ("Cancel", 3),
                ],
                3,
            )
            .await;
            let Some(path) = file.path() else { return };
            let folder = path.parent().map(PathBuf::from).unwrap_or_default();
            match choice {
                0 => {
                    let _ = std::process::Command::new(&path)
                        .current_dir(&folder)
                        .spawn();
                }
                1 => {
                    let _ = std::process::Command::new("xdg-terminal-exec")
                        .arg(&path)
                        .current_dir(&folder)
                        .spawn();
                }
                2 => open_with_default(&window, &file, "text/plain"),
                _ => {}
            }
        });
        return;
    }
    open_with_default(window, &file, &mime);
}

fn open_with_default(window: &Window, file: &gio::File, mime: &str) {
    match gio::AppInfo::default_for_type(mime, !file.is_native()) {
        Some(app) => open_with(window, &app, std::slice::from_ref(file)),
        None => crate::openwith::show(window, vec![file.clone()], mime.to_owned()),
    }
}

/// Opens files with a chosen app.
pub fn open_with(window: &Window, app: &gio::AppInfo, files: &[gio::File]) {
    match app.launch(files, Some(&launch_context(window))) {
        Ok(()) => files.iter().for_each(remember_recent),
        Err(error) => dialogs::error(
            parent(window).as_ref(),
            "Open",
            &format!("{} couldn't open this file.", app.display_name()),
            &error.to_string(),
        ),
    }
}

pub fn open_with_dialog(window: &Window) {
    let infos = window.tab().selected_infos();
    let Some(first) = infos.first() else { return };
    let mime = util::content_type(first);
    let files: Vec<gio::File> = infos.iter().filter_map(util::file_of).collect();
    crate::openwith::show(window, files, mime);
}

/// Files dropped somewhere: moved, copied or linked there.
pub fn drop_files(
    window: &Window,
    files: Vec<gio::File>,
    folder: gio::File,
    action: gdk::DragAction,
) {
    let window = window.clone();
    glib::spawn_future_local(async move {
        let parent = parent(&window);
        let outcome = if places::is_trash(&folder) {
            ops::trash(parent, files, true).await
        } else if action.contains(gdk::DragAction::LINK) {
            ops::link(parent, files, folder.clone(), true).await
        } else if action.contains(gdk::DragAction::MOVE) {
            ops::move_to(parent, files, folder.clone(), true).await
        } else {
            ops::copy(parent, files, folder.clone(), true).await
        };
        select_in_current(&window, &folder, outcome.created_names(), false);
    });
}

/// Selects new items if they're in the folder the window shows.
fn select_in_current(window: &Window, folder: &gio::File, names: Vec<String>, rename: bool) {
    let tab = window.tab();
    if tab
        .location()
        .folder()
        .is_some_and(|current| current.equal(folder))
    {
        tab.folder.select_when_present(names, rename);
    }
}

pub fn cut(window: &Window) {
    clipboard::set_files(&window.tab().selected_files(), true);
}

pub fn copy(window: &Window) {
    clipboard::set_files(&window.tab().selected_files(), false);
}

pub fn paste(window: &Window, as_shortcut: bool) {
    let Some(folder) = window.tab().location().folder().cloned() else {
        return;
    };
    let window = window.clone();
    glib::spawn_future_local(async move {
        let Some((files, cut)) = clipboard::files().await else {
            return;
        };
        let parent = parent(&window);
        let outcome = if as_shortcut {
            ops::link(parent, files, folder.clone(), true).await
        } else if cut {
            let outcome = ops::move_to(parent, files, folder.clone(), true).await;
            if !outcome.cancelled {
                clipboard::finish_cut();
            }
            outcome
        } else {
            ops::copy(parent, files, folder.clone(), true).await
        };
        select_in_current(&window, &folder, outcome.created_names(), false);
    });
}

pub fn rename(window: &Window) {
    let tab = window.tab();
    if tab.page() == Page::Folder {
        tab.folder.start_rename();
    }
}

/// Finishes a rename typed in place, warning first when the extension
/// changes, as Windows does.
pub fn rename_to(window: &Window, file: gio::File, name: String) {
    let window = window.clone();
    glib::spawn_future_local(async move {
        let parent = parent(&window);
        let old = file
            .basename()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let is_dir = file
            .query_info_future(
                "standard::type",
                gio::FileQueryInfoFlags::NOFOLLOW_SYMLINKS,
                glib::Priority::DEFAULT,
            )
            .await
            .is_ok_and(|info| info.file_type() == gio::FileType::Directory);
        let (_, old_extension) = util::split_extension(&old);
        let (_, new_extension) = util::split_extension(&name);
        if !is_dir && !old_extension.eq_ignore_ascii_case(new_extension) {
            let choice = dialogs::message(
                parent.as_ref(),
                "Rename",
                "warning",
                "If you change a file name extension, the file might become unusable.",
                "Are you sure you want to change it?",
                &[("_Yes", 0), ("_No", 1)],
                1,
            )
            .await;
            if choice != 0 {
                return;
            }
        }
        if let Some(renamed) = ops::rename(parent, file.clone(), name, true).await
            && let (Some(folder), Some(new_name)) = (renamed.parent(), renamed.basename())
        {
            select_in_current(
                &window,
                &folder,
                vec![new_name.to_string_lossy().into_owned()],
                false,
            );
        }
    });
}

/// The confirmation Windows shows before deleting for good.
async fn confirm_permanent_delete(window: &Window, infos: &[gio::FileInfo], recycle: bool) -> bool {
    let (title, question) = match infos {
        [info] if util::is_dir(info) => (
            "Delete Folder",
            if recycle {
                "Are you sure you want to move this folder to the Recycle Bin?"
            } else {
                "Are you sure you want to permanently delete this folder?"
            }
            .to_owned(),
        ),
        [_] => (
            "Delete File",
            if recycle {
                "Are you sure you want to move this file to the Recycle Bin?"
            } else {
                "Are you sure you want to permanently delete this file?"
            }
            .to_owned(),
        ),
        many => (
            "Delete Multiple Items",
            if recycle {
                format!(
                    "Are you sure you want to move these {} items to the Recycle Bin?",
                    util::thousands(many.len() as u64)
                )
            } else {
                format!(
                    "Are you sure you want to permanently delete these {} items?",
                    util::thousands(many.len() as u64)
                )
            },
        ),
    };
    let dialog = Dialog::new(parent(window).as_ref(), title);
    dialog.window.set_default_size(440, -1);
    let heading = gtk::Label::builder()
        .label(&question)
        .xalign(0.0)
        .wrap(true)
        .width_chars(36)
        .max_width_chars(52)
        .build();
    heading.add_css_class("dialog-heading");
    dialog.body.append(&heading);
    if let [info] = infos {
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 16);
        row.append(&icons::image(icons::for_info(info), 48));
        let text = gtk::Box::new(gtk::Orientation::Vertical, 2);
        let mut lines = vec![
            info.display_name().to_string(),
            format!("Type: {}", filecell::type_text(info)),
        ];
        if !util::is_dir(info) {
            lines.push(format!("Size: {}", util::size(info.size().max(0) as u64)));
        }
        if let Some(time) = util::modified(info) {
            lines.push(format!("Date modified: {}", util::date_time(&time)));
        }
        for line in lines {
            let label = gtk::Label::builder()
                .label(&line)
                .xalign(0.0)
                .ellipsize(gtk::pango::EllipsizeMode::Middle)
                .build();
            label.add_css_class("dialog-detail");
            text.append(&label);
        }
        row.append(&text);
        dialog.body.append(&row);
    }
    dialog.add_button("_Yes", 0, true);
    dialog.add_button("_No", 1, false);
    dialog.run().await == 0
}

/// Delete: to the Recycle Bin, or for good with Shift+Delete (and always
/// for items already in the Recycle Bin).
pub fn delete(window: &Window, permanent: bool) {
    let tab = window.tab();
    let infos: Vec<gio::FileInfo> = tab
        .selected_infos()
        .into_iter()
        .filter(|info| !info.boolean("pfm::drive"))
        .collect();
    if infos.is_empty() {
        return;
    }
    let files: Vec<gio::File> = infos.iter().filter_map(util::file_of).collect();
    let in_trash = files.iter().all(places::is_trash);
    let window = window.clone();
    glib::spawn_future_local(async move {
        let parent = parent(&window);
        if permanent || in_trash {
            if confirm_permanent_delete(&window, &infos, false).await {
                ops::delete(parent, files, "Deleting").await;
            }
        } else if !config::flag("confirm-trash")
            || confirm_permanent_delete(&window, &infos, true).await
        {
            ops::trash(parent, files, true).await;
        }
    });
}

/// "Copy as path": each path in double quotes, one per line.
pub fn copy_path(window: &Window) {
    let tab = window.tab();
    let mut files = tab.selected_files();
    if files.is_empty()
        && let Some(folder) = tab.location().folder()
    {
        files.push(folder.clone());
    }
    let text: Vec<String> = files
        .iter()
        .map(|file| format!("\"{}\"", util::display_path(file)))
        .collect();
    if !text.is_empty() {
        clipboard::set_text(&text.join("\n"));
    }
}

/// Share: by email, Bluetooth, or onto a removable drive.
pub fn share(window: &Window) {
    let files = window.tab().selected_files();
    let paths: Vec<PathBuf> = files.iter().filter_map(|file| file.path()).collect();
    if paths.is_empty() {
        return;
    }
    let menu = crate::menu::Menu::new();
    menu.heading("Share");
    {
        let paths = paths.clone();
        menu.item(
            crate::menu::Icon::Glyph("share"),
            "Send by email",
            None,
            glib::find_program_in_path("xdg-email").is_some(),
            move || {
                let mut command = std::process::Command::new("xdg-email");
                for path in &paths {
                    command.arg("--attach").arg(path);
                }
                let _ = command.spawn();
            },
        );
    }
    if glib::find_program_in_path("bluetooth-sendto").is_some() {
        let paths = paths.clone();
        menu.item(
            crate::menu::Icon::Glyph("send-to"),
            "Send with Bluetooth",
            None,
            true,
            move || {
                let _ = std::process::Command::new("bluetooth-sendto")
                    .args(&paths)
                    .spawn();
            },
        );
    }
    for mount in places::mounts()
        .into_iter()
        .filter(|mount| mount.can_eject() || mount.can_unmount())
    {
        let files = files.clone();
        let target = mount.root();
        let window = window.clone();
        menu.item(
            crate::menu::Icon::Color(icons::small_drive_icon(&mount)),
            &format!("Copy to {}", mount.name()),
            None,
            true,
            move || {
                let parent = parent(&window);
                let (files, target) = (files.clone(), target.clone());
                glib::spawn_future_local(async move {
                    ops::copy(parent, files, target, true).await;
                });
            },
        );
    }
    let content: gtk::Widget = window.tab().content.clone().upcast();
    let x = content.width() as f64 - 300.0;
    menu.popup_at(&content, x.max(24.0), 8.0);
}

pub fn properties(window: &Window) {
    let tab = window.tab();
    let mut infos = tab.selected_infos();
    let window = window.clone();
    glib::spawn_future_local(async move {
        if infos.is_empty() {
            match tab.location() {
                Location::Folder(folder) => {
                    if let Some(info) = crate::home::info_for(&folder).await {
                        infos.push(info);
                    }
                }
                Location::ThisPc | Location::Home => {
                    if let Some(info) = crate::home::info_for(&places::root()).await {
                        info.set_attribute_boolean("pfm::drive", true);
                        info.set_display_name(&places::root_name());
                        infos.push(info);
                    }
                }
                _ => {}
            }
        }
        if !infos.is_empty() {
            crate::properties::show(&window, infos);
        }
    });
}

/// Opens the terminal in the selected folder, or in this one.
pub fn open_terminal(window: &Window) {
    let tab = window.tab();
    let selected = tab.selected_infos();
    let folder = match selected.as_slice() {
        [info] if util::is_dir(info) => util::file_of(info),
        _ => tab.location().folder().cloned(),
    };
    let Some(path) = folder.and_then(|folder| folder.path()) else {
        return;
    };
    if !crate::run::terminal(&path, None) {
        dialogs::error(
            parent(window).as_ref(),
            "Open in Terminal",
            "No terminal app was found.",
            "",
        );
    }
}

/// The folders a pin command applies to: the selected ones, or this folder.
fn folders_for_pin(window: &Window) -> Vec<gio::File> {
    let tab = window.tab();
    let selected: Vec<gio::File> = tab
        .selected_infos()
        .iter()
        .filter(|info| util::is_dir(info))
        .filter_map(util::file_of)
        .collect();
    if !selected.is_empty() {
        return selected;
    }
    tab.location().folder().cloned().into_iter().collect()
}

pub fn toggle_pin(window: &Window) {
    let folders = folders_for_pin(window);
    let pin = !folders.iter().all(config::is_pinned);
    for folder in folders {
        config::set_pinned(&folder, pin);
    }
}

pub fn is_pinned_selection(window: &Window) -> bool {
    let folders = folders_for_pin(window);
    !folders.is_empty() && folders.iter().all(config::is_pinned)
}

pub fn toggle_favorite(window: &Window) {
    let files = window.tab().selected_files();
    let favorite = !files.iter().all(config::is_favorite);
    for file in files {
        config::set_favorite(&file, favorite);
    }
}

pub fn compress(window: &Window, format: archive::Format) {
    let files = window.tab().selected_files();
    if files.is_empty() {
        return;
    }
    let window = window.clone();
    glib::spawn_future_local(async move {
        if let Some(target) = ops::compress(parent(&window), files, format).await
            && let (Some(folder), Some(name)) = (target.parent(), target.basename())
        {
            // As on Windows, the new archive waits to be named.
            select_in_current(
                &window,
                &folder,
                vec![name.to_string_lossy().into_owned()],
                true,
            );
        }
    });
}

/// "Extract All": asks where, then extracts and shows the files.
pub fn extract(window: &Window) {
    let infos = window.tab().selected_infos();
    let [info] = infos.as_slice() else { return };
    let Some(archive_file) = util::file_of(info) else {
        return;
    };
    let Some(folder) = archive_file.parent() else {
        return;
    };
    let name = info.name().to_string_lossy().into_owned();
    let suggested = folder.child(archive::folder_name_for(&name));
    let window = window.clone();
    glib::spawn_future_local(async move {
        let dialog = Dialog::new(
            parent(&window).as_ref(),
            "Extract Compressed (Zipped) Folders",
        );
        dialog.window.set_default_size(520, -1);
        let heading = gtk::Label::builder()
            .label("Select a Destination and Extract Files")
            .xalign(0.0)
            .build();
        heading.add_css_class("dialog-heading");
        dialog.body.append(&heading);
        let label = gtk::Label::builder()
            .label("Files will be extracted to this folder:")
            .xalign(0.0)
            .build();
        dialog.body.append(&label);
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let entry = gtk::Entry::builder()
            .text(util::display_path(&suggested))
            .hexpand(true)
            .build();
        let browse = gtk::Button::with_mnemonic("B_rowse…");
        browse.add_css_class("dialog-button");
        row.append(&entry);
        row.append(&browse);
        dialog.body.append(&row);
        let show = gtk::CheckButton::with_label("Show extracted files when complete");
        show.set_active(true);
        dialog.body.append(&show);
        {
            let entry = entry.clone();
            let parent = dialog.window.clone();
            browse.connect_clicked(move |_| {
                let entry = entry.clone();
                let chooser = gtk::FileDialog::builder()
                    .title("Select a destination")
                    .modal(true)
                    .build();
                chooser.select_folder(Some(&parent), gio::Cancellable::NONE, move |result| {
                    if let Ok(folder) = result {
                        entry.set_text(&util::display_path(&folder));
                    }
                });
            });
        }
        dialog.add_button("_Extract", 0, true);
        dialog.add_button("Cancel", 1, false);
        if dialog.run().await != 0 {
            return;
        }
        let target = gio::File::for_commandline_arg(entry.text().as_str());
        if ops::extract(parent(&window), archive_file, target.clone()).await {
            crate::undo::record(crate::undo::Action::Create {
                file: target.clone(),
                folder: true,
                template: None,
            });
            if show.is_active() {
                window.navigate(Location::Folder(target));
            }
        }
    });
}

pub fn create_shortcut(window: &Window) {
    let tab = window.tab();
    let files = tab.selected_files();
    let Some(folder) = tab.location().folder().cloned() else {
        return;
    };
    let window = window.clone();
    glib::spawn_future_local(async move {
        let outcome = ops::link(parent(&window), files, folder.clone(), true).await;
        select_in_current(&window, &folder, outcome.created_names(), false);
    });
}

pub fn new_folder(window: &Window) {
    let Some(folder) = window.tab().location().folder().cloned() else {
        return;
    };
    let window = window.clone();
    glib::spawn_future_local(async move {
        if let Some(created) = ops::new_folder(parent(&window), folder.clone(), true).await {
            let name = created
                .basename()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            select_in_current(&window, &folder, vec![name], true);
        }
    });
}

/// The New menu's file kinds: "text", or "template:URI".
pub fn new_file(window: &Window, kind: &str) {
    let Some(folder) = window.tab().location().folder().cloned() else {
        return;
    };
    let (base, extension, template) = if let Some(uri) = kind.strip_prefix("template:") {
        let template = gio::File::for_uri(uri);
        let name = template
            .basename()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let (stem, extension) = util::split_extension(&name);
        (stem.to_owned(), extension.to_owned(), Some(template))
    } else {
        ("New Text Document".to_owned(), ".txt".to_owned(), None)
    };
    let window = window.clone();
    glib::spawn_future_local(async move {
        if let Some(created) = ops::new_file(
            parent(&window),
            folder.clone(),
            &base,
            &extension,
            template,
            true,
        )
        .await
        {
            let name = created
                .basename()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            select_in_current(&window, &folder, vec![name], true);
        }
    });
}

/// The files in the Templates folder, for the New menu.
pub fn templates() -> Vec<gio::File> {
    let Some(folder) = places::user_folder(Special::Templates) else {
        return Vec::new();
    };
    let Ok(entries) = std::fs::read_dir(folder.path().unwrap_or_default()) else {
        return Vec::new();
    };
    let mut files: Vec<PathBuf> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_file()
                && !path
                    .file_name()
                    .is_some_and(|n| n.to_string_lossy().starts_with('.'))
        })
        .collect();
    files.sort();
    files.into_iter().map(gio::File::for_path).collect()
}

pub fn restore(window: &Window, all: bool) {
    let selected = window.tab().selected_files();
    let window = window.clone();
    glib::spawn_future_local(async move {
        let items = if all {
            ops::trash_contents().await
        } else {
            selected
        };
        if !items.is_empty() {
            ops::restore(parent(&window), items).await;
        }
    });
}

/// "Set as desktop background" for a picture.
pub fn set_background(window: &Window) {
    let files = window.tab().selected_files();
    let Some(file) = files.first() else { return };
    let schema = gio::SettingsSchemaSource::default()
        .and_then(|source| source.lookup("org.gnome.desktop.background", true));
    if schema.is_none() {
        dialogs::error(
            parent(window).as_ref(),
            "Set as background",
            "The desktop background can't be changed here.",
            "",
        );
        return;
    }
    let settings = gio::Settings::new("org.gnome.desktop.background");
    let uri = file.uri();
    let _ = settings.set_string("picture-uri", &uri);
    let _ = settings.set_string("picture-uri-dark", &uri);
}

/// "Map network drive" becomes "Connect to server": type an address.
pub fn connect_to_server(window: &Window) {
    let window = window.clone();
    glib::spawn_future_local(async move {
        let dialog = Dialog::new(parent(&window).as_ref(), "Connect to Server");
        dialog.window.set_default_size(480, -1);
        let heading = gtk::Label::builder()
            .label("What network folder would you like to open?")
            .xalign(0.0)
            .build();
        heading.add_css_class("dialog-heading");
        dialog.body.append(&heading);
        let entry = gtk::Entry::builder()
            .placeholder_text("smb://server/share")
            .activates_default(true)
            .build();
        dialog.body.append(&entry);
        let examples = gtk::Label::builder()
            .label("Examples: smb://server/share, sftp://user@host/folder, ftp://ftp.example.com, dav://server/path")
            .xalign(0.0)
            .wrap(true)
            .width_chars(40)
            .build();
        examples.add_css_class("dialog-detail");
        dialog.body.append(&examples);
        dialog.add_button("_Connect", 0, true);
        dialog.add_button("Cancel", 1, false);
        if dialog.run().await == 0 {
            let address = entry.text().trim().to_owned();
            if !address.is_empty() {
                window.navigate(Location::Folder(gio::File::for_uri(&address)));
            }
        }
    });
}

/// Ejects or unmounts the selected drive (or the one this folder is on).
pub fn eject(window: &Window) {
    let tab = window.tab();
    let mut mounts: Vec<gio::Mount> = Vec::new();
    if tab.page() == Page::ThisPc {
        for drive in tab.this_pc().selected_drives() {
            if let crate::thispc::Drive::Mount(mount) = drive {
                mounts.push(mount);
            }
        }
    } else if let Some(folder) = tab.location().folder() {
        mounts.extend(places::mount_containing(folder));
    }
    for mount in mounts {
        let operation = gtk::MountOperation::new(Some(&window.gtk_window()));
        let window = window.clone();
        let name = mount.name().to_string();
        let done = move |result: Result<(), glib::Error>| {
            if let Err(error) = result {
                dialogs::error(
                    Some(&window.gtk_window()),
                    "Eject",
                    &format!("Can't eject {name}."),
                    &dialogs::describe(&error),
                );
            }
        };
        if mount.can_eject() {
            mount.eject_with_operation(
                gio::MountUnmountFlags::NONE,
                Some(&operation),
                gio::Cancellable::NONE,
                done,
            );
        } else {
            mount.unmount_with_operation(
                gio::MountUnmountFlags::NONE,
                Some(&operation),
                gio::Cancellable::NONE,
                done,
            );
        }
    }
}
