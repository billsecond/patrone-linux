//! Every menu: Windows 11's right-click menus for items and for the
//! background, "Show more options" (the full, classic menu), the
//! navigation pane's and the tabs' menus, and the command bar's New, Sort,
//! View, Filter and "See more" dropdowns.

use std::rc::Rc;

use gtk::{gio, glib, prelude::*};

use crate::archive;
use crate::commandbar::MenuKind;
use crate::commands;
use crate::config;
use crate::icons::{self, Kind};
use crate::location::Location;
use crate::menu::{Icon, Menu};
use crate::navpane::NodeKind;
use crate::places::{self, Special};
use crate::tab::Page;
use crate::undo;
use crate::util;
use crate::viewstate::{Column, VIEWS};
use crate::window::Window;

/// Runs a window action, as a menu item does.
fn act(window: &Window, name: &str) -> impl Fn() + 'static {
    let window = window.downgrade();
    let name = format!("win.{name}");
    move || {
        if let Some(window) = window.upgrade() {
            let _ = gtk::prelude::WidgetExt::activate_action(&window.window, &name, None);
        }
    }
}

fn act_with(window: &Window, name: &str, value: glib::Variant) -> impl Fn() + 'static {
    let window = window.downgrade();
    let name = format!("win.{name}");
    move || {
        if let Some(window) = window.upgrade() {
            let _ = gtk::prelude::WidgetExt::activate_action(&window.window, &name, Some(&value));
        }
    }
}

fn enabled(window: &Window, name: &str) -> bool {
    window
        .window
        .lookup_action(name)
        .is_some_and(|action| action.is_enabled())
}

fn state_string(window: &Window, name: &str) -> String {
    window
        .window
        .lookup_action(name)
        .and_then(|action| action.state())
        .and_then(|state| state.get::<String>())
        .unwrap_or_default()
}

fn state_bool(window: &Window, name: &str) -> bool {
    window
        .window
        .lookup_action(name)
        .and_then(|action| action.state())
        .and_then(|state| state.get::<bool>())
        .unwrap_or(false)
}

/// A drive's own commands: Mount (or connect a mapped network drive),
/// Eject or Unmount, and Disconnect for network drives.
fn drive_items(window: &Window, menu: &Rc<Menu>) {
    if enabled(window, "mount") {
        menu.item(
            Icon::Glyph("drive"),
            "Mount",
            None,
            true,
            act(window, "mount"),
        );
    }
    if enabled(window, "eject") {
        let mounts = commands::unmountable(window);
        menu.item(
            Icon::Glyph("eject"),
            crate::thispc::unmount_label(&mounts),
            None,
            true,
            act(window, "eject"),
        );
    }
    if enabled(window, "disconnect") {
        menu.item(
            Icon::Glyph("close"),
            "Disconnect",
            None,
            true,
            act(window, "disconnect"),
        );
    }
}

/// A network computer's or shared folder's menu, as Windows': open it,
/// mount or map a shared folder (the user, 2026-10-28: "Allow me to right
/// click on a network folder and mount it"), copy its address.
fn network_item_menu(
    window: &Window,
    item: &crate::drives::NetworkItem,
    widget: &gtk::Widget,
    x: f64,
    y: f64,
) {
    let menu = Menu::new();
    menu.item(
        Icon::Glyph("open"),
        "Open",
        Some("Enter"),
        true,
        act(window, "open"),
    );
    if !window.desktop {
        menu.item(
            Icon::Glyph("tab-new"),
            "Open in new tab",
            None,
            true,
            act(window, "open-new-tab"),
        );
    }
    menu.item(
        Icon::Glyph("window-new"),
        "Open in new window",
        None,
        true,
        act(window, "open-new-window"),
    );
    menu.separator();
    if enabled(window, "mount") {
        menu.item(
            Icon::Glyph("drive"),
            "Mount",
            None,
            true,
            act(window, "mount"),
        );
    }
    menu.item(
        Icon::Glyph("network"),
        "Map network drive…",
        None,
        true,
        act(window, "map-share"),
    );
    let uri = match item {
        crate::drives::NetworkItem::Computer(uri) | crate::drives::NetworkItem::Share(uri) => uri,
    };
    let address = crate::drives::windows_address(uri);
    menu.item(
        Icon::Glyph("copy-path"),
        "Copy address",
        None,
        true,
        move || crate::clipboard::set_text(&address),
    );
    menu.popup_at(widget, x, y);
}

/// The item's right-click menu, in Windows 11's form.
pub fn item_menu(window: &Window, widget: &gtk::Widget, x: f64, y: f64) {
    let tab = window.tab();
    let selected = tab.selected_infos();
    if selected.is_empty() {
        // Drives on This PC that aren't mounted have no files yet.
        if tab.page() == Page::ThisPc && !tab.this_pc().selected_drives().is_empty() {
            let menu = Menu::new();
            drive_items(window, &menu);
            menu.separator();
            menu.item(
                Icon::Glyph("network"),
                "Map network drive…",
                None,
                true,
                act(window, "map-drive"),
            );
            menu.popup_at(widget, x, y);
        }
        return;
    }
    // A computer or shared folder on the Network page: no file commands
    if let Some(item) = commands::selected_network_item(window) {
        network_item_menu(window, &item, widget, x, y);
        return;
    }
    let menu = Menu::new();
    let in_trash = tab.location().is_trash()
        || selected
            .iter()
            .all(|info| info.has_attribute("trash::orig-path"));
    let all_folders = selected
        .iter()
        .all(|info| util::is_dir(info) || info.boolean("pfm::drive"));
    let any_folder = selected.iter().any(util::is_dir);
    let drives = selected.iter().all(|info| info.boolean("pfm::drive"));
    let single = selected.len() == 1;

    if in_trash {
        menu.item(
            Icon::Glyph("restore"),
            "Restore",
            None,
            true,
            act(window, "restore-selected"),
        );
        menu.separator();
        menu.item(
            Icon::Glyph("cut"),
            "Cut",
            Some("Ctrl+X"),
            true,
            act(window, "cut"),
        );
        menu.item(
            Icon::Glyph("delete"),
            "Delete",
            Some("Delete"),
            true,
            act(window, "delete"),
        );
        menu.separator();
        menu.item(
            Icon::Glyph("properties"),
            "Properties",
            Some("Alt+Enter"),
            true,
            act(window, "properties"),
        );
        menu.popup_at(widget, x, y);
        return;
    }

    if !drives {
        menu.icon_row(vec![
            (
                "cut",
                "Cut (Ctrl+X)",
                enabled(window, "cut"),
                Rc::new(act(window, "cut")),
            ),
            (
                "copy",
                "Copy (Ctrl+C)",
                enabled(window, "copy"),
                Rc::new(act(window, "copy")),
            ),
            (
                "rename",
                "Rename (F2)",
                enabled(window, "rename"),
                Rc::new(act(window, "rename")),
            ),
            (
                "share",
                "Share",
                enabled(window, "share"),
                Rc::new(act(window, "share")),
            ),
            (
                "delete",
                "Delete (Delete)",
                enabled(window, "delete"),
                Rc::new(act(window, "delete")),
            ),
        ]);
    }

    menu.item(
        Icon::Glyph("open"),
        "Open",
        Some("Enter"),
        true,
        act(window, "open"),
    );
    // A disk image, as Windows offers for .iso files, or a shared folder on
    // the Network page
    if !drives && enabled(window, "mount") {
        menu.item(
            Icon::Glyph("drive"),
            "Mount",
            None,
            true,
            act(window, "mount"),
        );
    }
    if enabled(window, "map-share") {
        menu.item(
            Icon::Glyph("network"),
            "Map network drive…",
            None,
            true,
            act(window, "map-share"),
        );
    }
    if all_folders {
        if !window.desktop {
            menu.item(
                Icon::Glyph("tab-new"),
                "Open in new tab",
                None,
                true,
                act(window, "open-new-tab"),
            );
        }
        menu.item(
            Icon::Glyph("window-new"),
            "Open in new window",
            None,
            true,
            act(window, "open-new-window"),
        );
    } else {
        open_with_submenu(window, &menu, &selected);
    }
    if all_folders {
        let pinned = commands::is_pinned_selection(window);
        menu.item(
            Icon::Glyph(if pinned { "unpin" } else { "pin" }),
            if pinned {
                "Unpin from Quick access"
            } else {
                "Pin to Quick access"
            },
            None,
            true,
            act(window, "pin"),
        );
    } else if !any_folder {
        let favorite = selected
            .iter()
            .filter_map(util::file_of)
            .all(|file| config::is_favorite(&file));
        menu.item(
            Icon::Glyph(if favorite { "star-filled" } else { "star" }),
            if favorite {
                "Remove from Favorites"
            } else {
                "Add to Favorites"
            },
            None,
            true,
            act(window, "favorite"),
        );
    }
    if !drives {
        if single && archive::extractable(&selected[0].name().to_string_lossy()) {
            menu.item(
                Icon::Glyph("extract"),
                "Extract All…",
                None,
                true,
                act(window, "extract"),
            );
        }
        if enabled(window, "extract-here") {
            menu.item(
                Icon::Glyph("extract"),
                "Extract here",
                None,
                true,
                act(window, "extract-here"),
            );
        }
        let w = window.clone();
        menu.submenu(
            Icon::Glyph("zip"),
            "Compress to…",
            enabled(window, "compress-zip"),
            move |sub| {
                sub.item(
                    Icon::Glyph("zip"),
                    "ZIP File",
                    None,
                    true,
                    act(&w, "compress-zip"),
                );
                sub.item(
                    Icon::Glyph("zip"),
                    "TAR File (.tar.gz)",
                    None,
                    true,
                    act(&w, "compress-tar"),
                );
            },
        );
    }
    menu.item(
        Icon::Glyph("copy-path"),
        "Copy as path",
        Some("Ctrl+Shift+C"),
        true,
        act(window, "copy-path"),
    );
    // Launchers on the desktop, as Ubuntu's desktop icons offered them.
    let launchers = selected
        .iter()
        .all(|info| util::content_type(info) == "application/x-desktop");
    if window.desktop && launchers {
        let allowed = selected
            .iter()
            .all(|info| crate::filecell::trusted_launcher(info).is_some());
        menu.check(
            Icon::None,
            "Allow Launching",
            None,
            allowed,
            act(window, "allow-launching"),
        );
    }
    if enabled(window, "set-background") {
        menu.item(
            Icon::Glyph("picture"),
            "Set as desktop background",
            None,
            true,
            act(window, "set-background"),
        );
    }
    if drives {
        drive_items(window, &menu);
    }
    menu.item(
        Icon::Glyph("properties"),
        "Properties",
        Some("Alt+Enter"),
        true,
        act(window, "properties"),
    );
    if single && all_folders && enabled(window, "open-terminal") {
        menu.separator();
        menu.item(
            Icon::Glyph("terminal"),
            "Open in Terminal",
            None,
            true,
            act(window, "open-terminal"),
        );
    }
    menu.separator();
    let w = window.clone();
    let parent_widget = widget.clone();
    menu.item(
        Icon::Glyph("more"),
        "Show more options",
        Some("Shift+F10"),
        true,
        move || {
            classic_item_menu(&w, &parent_widget, x, y);
        },
    );
    menu.popup_at(widget, x, y);
}

/// "Open with": the apps for the type, and "Choose another app".
fn open_with_submenu(window: &Window, menu: &Rc<Menu>, selected: &[gio::FileInfo]) {
    let Some(first) = selected.first() else {
        return;
    };
    let mime = util::content_type(first);
    let files: Vec<gio::File> = selected.iter().filter_map(util::file_of).collect();
    let w = window.clone();
    menu.submenu(Icon::Glyph("open-with"), "Open with", true, move |sub| {
        let default = gio::AppInfo::default_for_type(&mime, false);
        let mut apps = gio::AppInfo::recommended_for_type(&mime);
        if let Some(default) = &default {
            apps.retain(|app| !app.equal(default));
            apps.insert(0, default.clone());
        }
        for app in apps.into_iter().take(12) {
            let icon = app.icon().map(Icon::Gicon).unwrap_or(Icon::None);
            let (w, files, app) = (w.clone(), files.clone(), app.clone());
            sub.item(icon, &app.display_name(), None, true, move || {
                commands::open_with(&w, &app, &files)
            });
        }
        sub.separator();
        sub.item(
            Icon::Glyph("open-with"),
            "Choose another app",
            None,
            true,
            act(&w, "open-with"),
        );
    });
}

/// "Show more options": the full menu, in the classic style.
pub fn classic_item_menu(window: &Window, widget: &gtk::Widget, x: f64, y: f64) {
    let tab = window.tab();
    let selected = tab.selected_infos();
    let menu = Menu::new();
    menu.popover.add_css_class("classic");
    let all_folders = !selected.is_empty() && selected.iter().all(util::is_dir);
    menu.item(Icon::None, "Open", None, true, act(window, "open"));
    if all_folders {
        menu.item(
            Icon::None,
            "Open in new tab",
            None,
            true,
            act(window, "open-new-tab"),
        );
        menu.item(
            Icon::None,
            "Open in new window",
            None,
            true,
            act(window, "open-new-window"),
        );
        menu.item(
            Icon::None,
            if commands::is_pinned_selection(window) {
                "Unpin from Quick access"
            } else {
                "Pin to Quick access"
            },
            None,
            true,
            act(window, "pin"),
        );
        menu.item(
            Icon::Glyph("terminal"),
            "Open in Terminal",
            None,
            enabled(window, "open-terminal"),
            act(window, "open-terminal"),
        );
    } else {
        open_with_submenu(window, &menu, &selected);
        menu.item(
            Icon::None,
            "Add to Favorites",
            None,
            true,
            act(window, "favorite"),
        );
    }
    menu.separator();
    send_to_submenu(window, &menu);
    menu.separator();
    menu.item(
        Icon::None,
        "Cut",
        Some("Ctrl+X"),
        enabled(window, "cut"),
        act(window, "cut"),
    );
    menu.item(
        Icon::None,
        "Copy",
        Some("Ctrl+C"),
        enabled(window, "copy"),
        act(window, "copy"),
    );
    menu.separator();
    menu.item(
        Icon::None,
        "Create shortcut",
        None,
        enabled(window, "create-shortcut"),
        act(window, "create-shortcut"),
    );
    menu.item(
        Icon::None,
        "Delete",
        Some("Delete"),
        enabled(window, "delete"),
        act(window, "delete"),
    );
    menu.item(
        Icon::None,
        "Rename",
        Some("F2"),
        enabled(window, "rename"),
        act(window, "rename"),
    );
    menu.separator();
    menu.item(
        Icon::None,
        "Properties",
        Some("Alt+Enter"),
        true,
        act(window, "properties"),
    );
    menu.popup_at(widget, x, y);
}

/// "Send to": the desktop (as a shortcut), Documents, a ZIP file, and
/// removable drives.
fn send_to_submenu(window: &Window, menu: &Rc<Menu>) {
    let w = window.clone();
    menu.submenu(
        Icon::Glyph("send-to"),
        "Send to",
        enabled(window, "copy"),
        move |sub| {
            let files = w.tab().selected_files();
            sub.item(
                Icon::Glyph("zip"),
                "Compressed (zipped) folder",
                None,
                true,
                act(&w, "compress-zip"),
            );
            if let Some(desktop) = places::user_folder(Special::Desktop) {
                let (w, files) = (w.clone(), files.clone());
                sub.item(
                    Icon::Color("pfm-nav-desktop"),
                    "Desktop (create shortcut)",
                    None,
                    true,
                    move || {
                        let parent = w.dialog_parent();
                        let (files, desktop) = (files.clone(), desktop.clone());
                        glib::spawn_future_local(async move {
                            crate::ops::link(parent, files, desktop, true).await;
                        });
                    },
                );
            }
            if let Some(documents) = places::user_folder(Special::Documents) {
                let (w, files) = (w.clone(), files.clone());
                sub.item(
                    Icon::Color("pfm-nav-documents"),
                    "Documents",
                    None,
                    true,
                    move || {
                        let parent = w.dialog_parent();
                        let (files, documents) = (files.clone(), documents.clone());
                        glib::spawn_future_local(async move {
                            crate::ops::copy(parent, files, documents, true).await;
                        });
                    },
                );
            }
            for mount in places::mounts()
                .into_iter()
                .filter(|m| m.can_eject() || m.can_unmount())
            {
                let (w, files, target) = (w.clone(), files.clone(), mount.root());
                sub.item(
                    Icon::Color(icons::small_drive_icon(&mount)),
                    &mount.name(),
                    None,
                    true,
                    move || {
                        let parent = w.dialog_parent();
                        let (files, target) = (files.clone(), target.clone());
                        glib::spawn_future_local(async move {
                            crate::ops::copy(parent, files, target, true).await;
                        });
                    },
                );
            }
        },
    );
}

/// The right-click menu for empty space.
pub fn background_menu(window: &Window, widget: &gtk::Widget, x: f64, y: f64) {
    let tab = window.tab();
    let menu = Menu::new();
    let page = tab.page();
    if page == Page::Folder {
        let w = window.clone();
        menu.submenu(Icon::Glyph("view"), "View", true, move |sub| {
            view_items(&w, sub)
        });
        let w = window.clone();
        menu.submenu(Icon::Glyph("sort"), "Sort by", true, move |sub| {
            sort_items(&w, sub)
        });
        let w = window.clone();
        menu.submenu(Icon::Glyph("group"), "Group by", true, move |sub| {
            group_items(&w, sub)
        });
        menu.separator();
    }
    if undo::can_undo() {
        menu.item(
            Icon::Glyph("undo"),
            &undo::undo_label(),
            Some("Ctrl+Z"),
            true,
            act(window, "undo"),
        );
    }
    if undo::can_redo() {
        menu.item(
            Icon::Glyph("redo"),
            &undo::redo_label(),
            Some("Ctrl+Y"),
            true,
            act(window, "redo"),
        );
    }
    if tab.location().is_trash() {
        menu.item(
            Icon::Glyph("empty-trash"),
            "Empty Recycle Bin",
            None,
            true,
            act(window, "empty-trash"),
        );
        menu.item(
            Icon::Glyph("restore"),
            "Restore all items",
            None,
            true,
            act(window, "restore-all"),
        );
    } else if tab.location().folder().is_some() {
        menu.item(
            Icon::Glyph("paste"),
            "Paste",
            Some("Ctrl+V"),
            enabled(window, "paste"),
            act(window, "paste"),
        );
        menu.item(
            Icon::Glyph("shortcut"),
            "Paste shortcut",
            None,
            enabled(window, "paste-shortcut"),
            act(window, "paste-shortcut"),
        );
        menu.separator();
        let w = window.clone();
        menu.submenu(
            Icon::Glyph("new"),
            "New",
            enabled(window, "new-folder"),
            move |sub| new_items(&w, sub),
        );
    }
    menu.separator();
    if tab.location().folder().is_some() && !tab.location().is_trash() {
        let pinned = commands::is_pinned_selection(window);
        menu.item(
            Icon::Glyph(if pinned { "unpin" } else { "pin" }),
            if pinned {
                "Unpin from Quick access"
            } else {
                "Pin to Quick access"
            },
            None,
            true,
            act(window, "pin"),
        );
    }
    if page == Page::ThisPc {
        menu.item(
            Icon::Glyph("network"),
            "Map network drive…",
            None,
            true,
            act(window, "map-drive"),
        );
    }
    menu.item(
        Icon::Glyph("properties"),
        "Properties",
        Some("Alt+Enter"),
        true,
        act(window, "properties"),
    );
    if enabled(window, "open-terminal") {
        menu.separator();
        menu.item(
            Icon::Glyph("terminal"),
            "Open in Terminal",
            None,
            true,
            act(window, "open-terminal"),
        );
    }
    menu.popup_at(widget, x, y);
}

/// The desktop's right-click menu, as Windows 11's: View, Sort by, Refresh,
/// Paste, Undo, New, Display settings, Personalize, Open in Terminal (the
/// user, 2026-10-27). With the desktop bubble on it serves the empty
/// desktop and the bubble's empty part, and shows the bubble's files.
pub fn desktop_menu(window: &Window, widget: &gtk::Widget, x: f64, y: f64) {
    let menu = Menu::new();
    let bubble = crate::desktop::bubble_on();
    if bubble {
        menu.item(
            Icon::Color("pfm-nav-desktop"),
            "Open Desktop folder",
            None,
            true,
            crate::desktop::open_desktop_folder,
        );
        menu.separator();
    } else {
        menu.submenu(Icon::Glyph("view"), "View", true, desktop_view_items);
    }
    let w = window.clone();
    menu.submenu(Icon::Glyph("sort"), "Sort by", true, move |sub| {
        desktop_sort_items(&w, sub)
    });
    menu.item(
        Icon::Glyph("refresh"),
        "Refresh",
        Some("F5"),
        true,
        act(window, "refresh"),
    );
    menu.separator();
    menu.item(
        Icon::Glyph("paste"),
        "Paste",
        Some("Ctrl+V"),
        enabled(window, "paste"),
        act(window, "paste"),
    );
    menu.item(
        Icon::Glyph("shortcut"),
        "Paste shortcut",
        None,
        enabled(window, "paste-shortcut"),
        act(window, "paste-shortcut"),
    );
    if undo::can_undo() {
        menu.item(
            Icon::Glyph("undo"),
            &undo::undo_label(),
            Some("Ctrl+Z"),
            true,
            act(window, "undo"),
        );
    }
    if undo::can_redo() {
        menu.item(
            Icon::Glyph("redo"),
            &undo::redo_label(),
            Some("Ctrl+Y"),
            true,
            act(window, "redo"),
        );
    }
    menu.separator();
    let w = window.clone();
    menu.submenu(
        Icon::Glyph("new"),
        "New",
        enabled(window, "new-folder"),
        move |sub| new_items(&w, sub),
    );
    menu.separator();
    if !crate::desktop::bin_shown() {
        menu.item(
            Icon::Color("pfm-nav-trash"),
            "Show Recycle Bin",
            None,
            true,
            crate::desktop::show_bin,
        );
    }
    menu.item(
        Icon::Glyph("computer"),
        "Display settings",
        None,
        true,
        crate::desktop::display_settings,
    );
    menu.item(
        Icon::Glyph("picture"),
        "Personalize",
        None,
        true,
        crate::desktop::personalize,
    );
    menu.separator();
    menu.item(
        Icon::Glyph("terminal"),
        "Open in Terminal",
        None,
        enabled(window, "open-terminal"),
        act(window, "open-terminal"),
    );
    menu.popup_at(widget, x, y);
}

/// The desktop's View: icon size, Auto arrange, and showing the icons.
fn desktop_view_items(menu: &Rc<Menu>) {
    let current = config::string("desktop-icon-size");
    for (id, label, _) in crate::desktopgrid::ICON_SIZES {
        menu.radio(
            Icon::Glyph(match id {
                "large" => "view-large",
                "small" => "view-small",
                _ => "view-medium",
            }),
            label,
            None,
            current == id,
            move || {
                let _ = config::settings().set_string("desktop-icon-size", id);
            },
        );
    }
    menu.separator();
    menu.check(
        Icon::None,
        "Auto arrange icons",
        None,
        config::flag("desktop-auto-arrange"),
        || {
            let on = config::flag("desktop-auto-arrange");
            let _ = config::settings().set_boolean("desktop-auto-arrange", !on);
        },
    );
    menu.separator();
    menu.check(
        Icon::None,
        "Show desktop icons",
        None,
        config::flag("desktop-show-icons"),
        || {
            let on = config::flag("desktop-show-icons");
            let _ = config::settings().set_boolean("desktop-show-icons", !on);
        },
    );
}

/// The desktop's Sort by, as Windows': name, size, type, date modified.
fn desktop_sort_items(window: &Window, menu: &Rc<Menu>) {
    let current = config::string("desktop-sort");
    for column in [Column::Name, Column::Size, Column::Type, Column::Modified] {
        let label = match column {
            Column::Type => "Item type",
            other => other.label(),
        };
        let w = window.clone();
        menu.radio(Icon::None, label, None, current == column.id(), move || {
            crate::desktop::sort_by(&w, column)
        });
    }
}

/// View: the eight views, Compact view, and Show.
fn view_items(window: &Window, menu: &Rc<Menu>) {
    let current = state_string(window, "view");
    for view in VIEWS {
        menu.radio(
            Icon::Glyph(view.glyph()),
            view.label(),
            Some(&view.accel()),
            current == view.id(),
            act_with(window, "view", view.id().to_variant()),
        );
    }
    menu.separator();
    menu.check(
        Icon::Glyph("compact"),
        "Compact view",
        None,
        state_bool(window, "compact"),
        act(window, "compact"),
    );
    let w = window.clone();
    menu.submenu(Icon::Glyph("show"), "Show", true, move |sub| {
        show_items(&w, sub)
    });
}

fn show_items(window: &Window, menu: &Rc<Menu>) {
    menu.check(
        Icon::Glyph("navigation-pane"),
        "Navigation pane",
        None,
        state_bool(window, "navigation-pane"),
        act(window, "navigation-pane"),
    );
    menu.check(
        Icon::Glyph("details-pane"),
        "Details pane",
        Some("Alt+Shift+P"),
        state_bool(window, "details-pane"),
        act(window, "details-pane"),
    );
    menu.check(
        Icon::Glyph("preview-pane"),
        "Preview pane",
        Some("Alt+P"),
        state_bool(window, "preview-pane"),
        act(window, "preview-pane"),
    );
    menu.separator();
    menu.check(
        Icon::Glyph("checkboxes"),
        "Item check boxes",
        None,
        state_bool(window, "item-checkboxes"),
        act(window, "item-checkboxes"),
    );
    menu.check(
        Icon::Glyph("extensions"),
        "File name extensions",
        None,
        state_bool(window, "show-extensions"),
        act(window, "show-extensions"),
    );
    menu.check(
        Icon::Glyph("hidden"),
        "Hidden items",
        None,
        state_bool(window, "show-hidden"),
        act(window, "show-hidden"),
    );
    menu.check(
        Icon::Glyph("info"),
        "System folder hints",
        None,
        state_bool(window, "system-hints"),
        act(window, "system-hints"),
    );
}

/// The columns a place can be sorted or grouped by.
fn sort_columns(window: &Window) -> Vec<Column> {
    let location = window.tab().location();
    if location.is_trash() {
        vec![
            Column::Name,
            Column::Origin,
            Column::Deleted,
            Column::Size,
            Column::Type,
            Column::Modified,
        ]
    } else if matches!(location, Location::Search { .. }) {
        vec![
            Column::Name,
            Column::Modified,
            Column::Type,
            Column::Size,
            Column::Folder,
        ]
    } else {
        vec![
            Column::Name,
            Column::Modified,
            Column::Type,
            Column::Size,
            Column::Created,
            Column::Accessed,
            Column::Owner,
        ]
    }
}

fn sort_items(window: &Window, menu: &Rc<Menu>) {
    let current = state_string(window, "sort");
    for column in sort_columns(window) {
        menu.radio(
            Icon::None,
            column.label(),
            None,
            current == column.id(),
            act_with(window, "sort", column.id().to_variant()),
        );
    }
    menu.separator();
    let descending = state_bool(window, "sort-descending");
    let set_descending = |value: bool| {
        let window = window.downgrade();
        move || {
            if let Some(window) = window.upgrade()
                && state_bool(&window, "sort-descending") != value
            {
                let _ = gtk::prelude::WidgetExt::activate_action(
                    &window.window,
                    "win.sort-descending",
                    None,
                );
            }
        }
    };
    menu.radio(
        Icon::Glyph("ascending"),
        "Ascending",
        None,
        !descending,
        set_descending(false),
    );
    menu.radio(
        Icon::Glyph("descending"),
        "Descending",
        None,
        descending,
        set_descending(true),
    );
}

fn group_items(window: &Window, menu: &Rc<Menu>) {
    let current = state_string(window, "group");
    for column in sort_columns(window) {
        menu.radio(
            Icon::None,
            column.label(),
            None,
            current == column.id(),
            act_with(window, "group", column.id().to_variant()),
        );
    }
    menu.radio(
        Icon::None,
        "(None)",
        None,
        current == "none",
        act_with(window, "group", "none".to_variant()),
    );
}

/// New: a folder, a text document, and whatever is in Templates.
fn new_items(window: &Window, menu: &Rc<Menu>) {
    menu.item(
        Icon::Color("pfm-nav-folder"),
        "Folder",
        Some("Ctrl+Shift+N"),
        enabled(window, "new-folder"),
        act(window, "new-folder"),
    );
    menu.item(
        Icon::Glyph("shortcut"),
        "Shortcut",
        None,
        enabled(window, "paste-shortcut"),
        act(window, "paste-shortcut"),
    );
    menu.separator();
    menu.item(
        Icon::Glyph("file-new"),
        "Text Document",
        None,
        enabled(window, "new-file"),
        act_with(window, "new-file", "text".to_variant()),
    );
    for template in commands::templates() {
        let name = template
            .basename()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let (stem, _) = util::split_extension(&name);
        let kind = icons::kind_of(&gio::content_type_guess(Some(&name), None).0, false);
        let icon = match kind {
            Kind::Document => "pfm-nav-documents",
            _ => "pfm-nav-templates",
        };
        menu.item(
            Icon::Color(icon),
            stem,
            None,
            enabled(window, "new-file"),
            act_with(
                window,
                "new-file",
                format!("template:{}", template.uri()).to_variant(),
            ),
        );
    }
}

/// Filter: show only some kinds of items.
fn filter_items(window: &Window, menu: &Rc<Menu>) {
    let tab = window.tab();
    let current = tab.kind_filter();
    let kinds: &[(Kind, &str, &str)] = &[
        (Kind::Folder, "Folders", "folder"),
        (Kind::Document, "Documents", "file"),
        (Kind::Spreadsheet, "Spreadsheets", "view-details"),
        (Kind::Presentation, "Presentations", "view-tiles"),
        (Kind::Pdf, "PDFs", "file"),
        (Kind::Image, "Pictures", "picture"),
        (Kind::Audio, "Music", "file"),
        (Kind::Video, "Videos", "file"),
        (Kind::Archive, "Archives", "zip"),
        (Kind::Code, "Code", "terminal"),
        (Kind::Text, "Text files", "file"),
    ];
    for (kind, label, glyph) in kinds {
        if tab.page() == Page::Home && *kind == Kind::Folder {
            continue;
        }
        let active = current.contains(kind);
        let w = window.clone();
        let kind = *kind;
        menu.check(Icon::Glyph(glyph), label, None, active, move || {
            let tab = w.tab();
            let mut kinds = tab.kind_filter();
            if let Some(index) = kinds.iter().position(|k| *k == kind) {
                kinds.remove(index);
            } else {
                kinds.push(kind);
            }
            tab.set_kind_filter(kinds);
        });
    }
    menu.separator();
    let w = window.clone();
    menu.item(
        Icon::Glyph("close"),
        "Clear filter",
        None,
        !current.is_empty(),
        move || w.tab().set_kind_filter(Vec::new()),
    );
}

/// "See more" (…).
fn more_items(window: &Window, menu: &Rc<Menu>) {
    menu.item(
        Icon::Glyph("undo"),
        &undo::undo_label(),
        Some("Ctrl+Z"),
        enabled(window, "undo"),
        act(window, "undo"),
    );
    menu.item(
        Icon::Glyph("redo"),
        &undo::redo_label(),
        Some("Ctrl+Y"),
        enabled(window, "redo"),
        act(window, "redo"),
    );
    let pinned = commands::is_pinned_selection(window);
    let can_pin = window.tab().location().folder().is_some()
        || window.tab().selected_infos().iter().any(util::is_dir);
    menu.item(
        Icon::Glyph(if pinned { "unpin" } else { "pin" }),
        if pinned {
            "Unpin from Quick access"
        } else {
            "Pin to Quick access"
        },
        None,
        can_pin,
        act(window, "pin"),
    );
    menu.item(
        Icon::Glyph("copy-path"),
        "Copy path",
        Some("Ctrl+Shift+C"),
        enabled(window, "copy-path"),
        act(window, "copy-path"),
    );
    menu.separator();
    menu.item(
        Icon::Glyph("select-all"),
        "Select all",
        Some("Ctrl+A"),
        enabled(window, "select-all"),
        act(window, "select-all"),
    );
    menu.item(
        Icon::Glyph("select-none"),
        "Select none",
        None,
        enabled(window, "select-none"),
        act(window, "select-none"),
    );
    menu.item(
        Icon::Glyph("invert-selection"),
        "Invert selection",
        None,
        enabled(window, "invert-selection"),
        act(window, "invert-selection"),
    );
    menu.separator();
    if window.tab().page() == Page::ThisPc {
        menu.item(
            Icon::Glyph("network"),
            "Map network drive…",
            None,
            true,
            act(window, "map-drive"),
        );
    }
    menu.item(
        Icon::Glyph("terminal"),
        "Open in Terminal",
        None,
        enabled(window, "open-terminal"),
        act(window, "open-terminal"),
    );
    menu.item(
        Icon::Glyph("properties"),
        "Properties",
        Some("Alt+Enter"),
        true,
        act(window, "properties"),
    );
    menu.item(
        Icon::Glyph("options"),
        "Options",
        None,
        true,
        act(window, "options"),
    );
}

/// Fills a command bar dropdown.
pub fn dropdown(window: &Window, kind: MenuKind, menu: &Rc<Menu>) {
    match kind {
        MenuKind::New => new_items(window, menu),
        MenuKind::Sort => {
            if window.tab().page() == Page::Folder {
                sort_items(window, menu);
                menu.separator();
                let w = window.clone();
                menu.submenu(Icon::Glyph("group"), "Group by", true, move |sub| {
                    group_items(&w, sub)
                });
            }
        }
        MenuKind::View => {
            if window.tab().page() == Page::Folder {
                view_items(window, menu);
            } else {
                show_items(window, menu);
            }
        }
        MenuKind::Filter => filter_items(window, menu),
        MenuKind::More => more_items(window, menu),
    }
}

/// The navigation pane's right-click menu.
pub fn nav_menu(
    window: &Window,
    kind: NodeKind,
    pinned: bool,
    widget: &gtk::Widget,
    x: f64,
    y: f64,
) {
    let menu = Menu::new();
    // A drive that isn't mounted: Open mounts it first.
    if let NodeKind::Drive(drive) = &kind
        && drive.can_mount()
    {
        for (glyph, label, open) in [("open", "Open", true), ("drive", "Mount", false)] {
            let (w, drive) = (window.clone(), drive.clone());
            menu.item(Icon::Glyph(glyph), label, None, true, move || {
                let opened = w.clone();
                crate::thispc::mount_drive(&drive, w.dialog_parent(), move |root| {
                    if open {
                        opened.navigate(Location::Folder(root));
                    }
                });
            });
        }
        // A mapped drive that isn't connected can be forgotten
        if drive.is_network() {
            let (w, drive) = (window.clone(), drive.clone());
            menu.item(Icon::Glyph("close"), "Disconnect", None, true, move || {
                commands::disconnect_drive(&w, &drive)
            });
        }
        menu.popup_at(widget, x, y);
        return;
    }
    let Some(location) = kind.location() else {
        return;
    };
    {
        let (w, location) = (window.clone(), location.clone());
        menu.item(Icon::Glyph("open"), "Open", None, true, move || {
            w.navigate(location.clone())
        });
    }
    {
        let (w, location) = (window.clone(), location.clone());
        menu.item(
            Icon::Glyph("tab-new"),
            "Open in new tab",
            None,
            true,
            move || {
                w.new_tab(location.clone(), true);
            },
        );
    }
    {
        let (w, location) = (window.clone(), location.clone());
        menu.item(
            Icon::Glyph("window-new"),
            "Open in new window",
            None,
            true,
            move || {
                if let Some(app) = w.window.application() {
                    Window::new(&app, location.clone()).present();
                }
            },
        );
    }
    match &kind {
        NodeKind::Trash => {
            menu.separator();
            menu.item(
                Icon::Glyph("empty-trash"),
                "Empty Recycle Bin",
                None,
                true,
                act(window, "empty-trash"),
            );
        }
        NodeKind::Folder(file) => {
            let file = file.clone();
            let is_pinned = pinned || config::is_pinned(&file);
            {
                let file = file.clone();
                menu.item(
                    Icon::Glyph(if is_pinned { "unpin" } else { "pin" }),
                    if is_pinned {
                        "Unpin from Quick access"
                    } else {
                        "Pin to Quick access"
                    },
                    None,
                    true,
                    move || config::set_pinned(&file, !is_pinned),
                );
            }
            if is_pinned {
                let file = file.clone();
                let index = config::pinned()
                    .iter()
                    .position(|p| p.equal(&file))
                    .unwrap_or(0);
                menu.item(Icon::Glyph("up"), "Move up", None, index > 0, move || {
                    config::move_pinned(&file, index.saturating_sub(1))
                });
            }
            let path = util::display_path(&file);
            menu.item(
                Icon::Glyph("copy-path"),
                "Copy as path",
                None,
                true,
                move || crate::clipboard::set_text(&format!("\"{path}\"")),
            );
            if file.is_native() {
                let path = file.path();
                menu.item(
                    Icon::Glyph("terminal"),
                    "Open in Terminal",
                    None,
                    true,
                    move || {
                        if let Some(path) = &path {
                            let _ = std::process::Command::new("xdg-terminal-exec")
                                .current_dir(path)
                                .spawn();
                        }
                    },
                );
            }
        }
        NodeKind::Drive(drive) if drive.is_network() => {
            let (w, drive) = (window.clone(), drive.clone());
            menu.item(Icon::Glyph("close"), "Disconnect", None, true, move || {
                commands::disconnect_drive(&w, &drive)
            });
        }
        NodeKind::Drive(drive)
            if drive
                .mount()
                .is_some_and(|mount| mount.can_eject() || mount.can_unmount()) =>
        {
            let Some(mount) = drive.mount() else { return };
            let label = crate::thispc::unmount_label(std::slice::from_ref(&mount));
            let w = window.clone();
            menu.item(Icon::Glyph("eject"), label, None, true, move || {
                let shown = w.clone();
                let name = mount.name().to_string();
                crate::drives::unmount(&mount, w.dialog_parent().as_ref(), move |result| {
                    if let Err(error) = result
                        && !error.matches(gio::IOErrorEnum::FailedHandled)
                    {
                        crate::dialogs::error(
                            shown.dialog_parent().as_ref(),
                            label,
                            &format!("Can't {} {name}.", label.to_lowercase()),
                            &crate::dialogs::describe(&error),
                        );
                    }
                });
            });
        }
        _ => {}
    }
    if let Some(file) = kind.file() {
        menu.separator();
        let w = window.clone();
        menu.item(
            Icon::Glyph("properties"),
            "Properties",
            Some("Alt+Enter"),
            true,
            move || {
                let w = w.clone();
                let file = file.clone();
                glib::spawn_future_local(async move {
                    if let Some(info) = crate::home::info_for(&file).await {
                        if places::special_of(&file) == Some(Special::Root)
                            || places::mount_root_of(&file).is_some()
                        {
                            info.set_attribute_boolean("pfm::drive", true);
                            info.set_display_name(&places::display_name(&file));
                        }
                        crate::properties::show(&w, vec![info]);
                    }
                });
            },
        );
    }
    menu.popup_at(widget, x, y);
}

/// A tab's right-click menu.
pub fn tab_menu(window: &Window, index: usize, widget: &gtk::Widget, x: f64, y: f64) {
    let menu = Menu::new();
    let count = window.tabs().len();
    menu.item(
        Icon::Glyph("tab-new"),
        "New tab",
        Some("Ctrl+T"),
        true,
        act(window, "new-tab"),
    );
    {
        let w = window.clone();
        menu.item(
            Icon::Glyph("copy"),
            "Duplicate tab",
            None,
            true,
            move || {
                let location = w.tabs()[index].location();
                w.new_tab(location, true);
            },
        );
    }
    {
        let w = window.clone();
        menu.item(
            Icon::Glyph("window-new"),
            "Move to new window",
            None,
            count > 1,
            move || w.move_tab_to(index, None),
        );
    }
    let others: Vec<Window> = crate::window::all()
        .into_iter()
        .filter(|other| !Rc::ptr_eq(&other.0, &window.0))
        .collect();
    if !others.is_empty() {
        let w = window.clone();
        menu.submenu(Icon::None, "Move to window", true, move |sub| {
            for other in &others {
                let title = other
                    .window
                    .title()
                    .map(|title| title.to_string())
                    .unwrap_or_default();
                let title = title.trim_end_matches(" - File Manager").to_owned();
                let (w, other) = (w.clone(), other.clone());
                sub.item(Icon::Glyph("window-new"), &title, None, true, move || {
                    w.move_tab_to(index, Some(&other))
                });
            }
        });
    }
    menu.separator();
    {
        let w = window.clone();
        menu.item(
            Icon::Glyph("close"),
            "Close tab",
            Some("Ctrl+W"),
            true,
            move || w.close_tab(index),
        );
    }
    {
        let w = window.clone();
        menu.item(Icon::None, "Close other tabs", None, count > 1, move || {
            w.close_other_tabs(index)
        });
    }
    {
        let w = window.clone();
        menu.item(
            Icon::None,
            "Close tabs to the right",
            None,
            index + 1 < count,
            move || w.close_tabs_to_the_right(index),
        );
    }
    menu.popup_at(widget, x, y);
}
