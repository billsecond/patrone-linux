//! The Startup apps page: what starts when you sign in. These are the
//! desktop's autostart entries, in /etc/xdg/autostart for everyone and
//! ~/.config/autostart for you, where your entry of the same name takes
//! the place of everyone's. Turning one off writes your own copy of it with
//! Hidden=true, as the autostart standard says; turning it back on removes
//! that copy. Changes take effect the next time you sign in.
//!
//! Entries this desktop never starts as apps aren't listed: those for other
//! desktops (OnlyShowIn, NotShowIn), those systemd runs as a service in
//! their place (X-GNOME-HiddenUnderSystemd), and the desktop's own parts
//! (X-GNOME-Autostart-Phase), which GNOME 50's session hands to systemd.

use std::cell::RefCell;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{gio, glib};

use crate::desktop::{GROUP, flag, for_this_desktop, load};
use crate::row::{self, Row};
use crate::table::{self, Cell, Column, Sort, Table};
use crate::{Context, actions};

const SYSTEM_FOLDER: &str = "/etc/xdg/autostart";
/// Marks the copies Task Manager writes to turn an entry off.
const COPY_KEY: &str = "X-Patrone-Turned-Off";

pub struct Page {
    pub widget: gtk::Box,
    pub table: Table,
    pub toolbar: gtk::Box,
    store: gio::ListStore,
    rows: RefCell<HashMap<String, Row>>,
    /// Which package installed each of everyone's entries
    packages: RefCell<HashMap<String, String>>,
}

impl Page {
    pub fn new(context: &Rc<Context>) -> Rc<Self> {
        let store = gio::ListStore::new::<Row>();
        let table = Table::new(
            &store,
            false,
            None,
            vec![
                Column::new("Name", Cell::Name, Sort::Text("name"), 260),
                Column::new(
                    "Status",
                    Cell::Switch("enabled", "status"),
                    Sort::Text("status"),
                    140,
                ),
                Column::new("From", Cell::Text("extra"), Sort::Text("extra"), 170),
                Column::new("For", Cell::Text("user"), Sort::Text("user"), 90),
                Column::new("Command", Cell::Text("command"), Sort::Text("command"), 300).expand(),
            ],
            crate::matcher(context.query.clone()),
        );
        table
            .view
            .sort_by_column(Some(&table.columns[0]), gtk::SortType::Ascending);

        let note = gtk::Label::builder()
            .label("Apps and helpers that start when you sign in. Changes take effect the next time you sign in.")
            .xalign(0.0)
            .wrap(true)
            .css_classes(["tm-note"])
            .build();
        let widget = gtk::Box::new(gtk::Orientation::Vertical, 0);
        widget.append(&note);
        widget.append(&table.widget);

        let toolbar = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        let enable = crate::command("media-playback-start-symbolic", "Enable");
        let disable = crate::command("media-playback-stop-symbolic", "Disable");
        let location = crate::command("folder-open-symbolic", "Open file location");
        for button in [&enable, &disable, &location] {
            button.set_sensitive(false);
            toolbar.append(button);
        }

        let page = Rc::new(Page {
            widget,
            table,
            toolbar,
            store,
            rows: RefCell::new(HashMap::new()),
            packages: RefCell::new(HashMap::new()),
        });

        let weak_buttons = [
            enable.downgrade(),
            disable.downgrade(),
            location.downgrade(),
        ];
        page.table
            .selection
            .connect_selection_changed(move |selection, _, _| {
                let row = selection.selected_item().and_downcast::<Row>();
                let on = row.as_ref().is_some_and(|row| row.enabled());
                for (index, button) in weak_buttons.iter().enumerate() {
                    if let Some(button) = button.upgrade() {
                        button.set_sensitive(row.is_some() && [!on, on, true][index]);
                    }
                }
            });
        for (button, on) in [(&enable, true), (&disable, false)] {
            let weak = Rc::downgrade(&page);
            let context = context.clone();
            button.connect_clicked(move |_| {
                let Some(page) = weak.upgrade() else { return };
                if let Some(row) = page.table.selected() {
                    page.set_enabled(&context, &row, on);
                }
            });
        }
        let weak = Rc::downgrade(&page);
        location.connect_clicked(move |_| {
            let Some(page) = weak.upgrade() else { return };
            if let Some(row) = page.table.selected() {
                show_file(&row.description());
            }
        });
        let weak = Rc::downgrade(&page);
        let switch_context = context.clone();
        page.table.on_switch(move |row, on| {
            if let Some(page) = weak.upgrade() {
                page.set_enabled(&switch_context, row, on);
            }
        });

        let menu = gio::Menu::new();
        menu.append(Some("Enable"), Some("win.startup::on"));
        menu.append(Some("Disable"), Some("win.startup::off"));
        menu.append(Some("Open file location"), Some("win.startup::location"));
        let menu_context = context.clone();
        let popover = table::context_menu(&menu, &page.table.view);
        page.table.on_menu(move |row, cell, x, y| {
            *menu_context.target.borrow_mut() = Some(row.clone());
            table::popup(&popover, cell, x, y);
        });

        page.reload();
        page.find_packages();
        page
    }

    /// Reads the entries again.
    pub fn reload(&self) {
        let entries = entries();
        let mut rows = self.rows.borrow_mut();
        let packages = self.packages.borrow();
        let mut list = Vec::new();
        for entry in &entries {
            let row = rows
                .entry(entry.id.clone())
                .or_insert_with(|| Row::new(&entry.id))
                .clone();
            row.put_text("name", &entry.name);
            row.put_icon(entry.icon.as_ref());
            if row.enabled() != entry.enabled {
                row.set_enabled(entry.enabled);
            }
            row.put_text("status", if entry.enabled { "Enabled" } else { "Disabled" });
            row.put_text("command", &entry.command);
            row.put_text(
                "user",
                if entry.system.is_some() {
                    "Everyone"
                } else {
                    "You"
                },
            );
            let from = match &entry.system {
                Some(path) => packages
                    .get(&path.to_string_lossy().into_owned())
                    .cloned()
                    .unwrap_or_default(),
                None => "Added by you".to_string(),
            };
            row.put_text("extra", &from);
            // The file that's in effect, for "Open file location"
            let file = entry
                .user
                .as_ref()
                .or(entry.system.as_ref())
                .map(|path| path.to_string_lossy().into_owned())
                .unwrap_or_default();
            row.put_text("description", &file);
            list.push(row);
        }
        let keep: std::collections::HashSet<String> =
            entries.iter().map(|entry| entry.id.clone()).collect();
        rows.retain(|id, _| keep.contains(id));
        drop(rows);
        row::sync_store(&self.store, &list);
    }

    /// Which package installed each of everyone's entries: one dpkg-query
    /// for them all.
    fn find_packages(self: &Rc<Self>) {
        let paths: Vec<String> = entries()
            .into_iter()
            .filter_map(|entry| entry.system.map(|path| path.to_string_lossy().into_owned()))
            .collect();
        if paths.is_empty() {
            return;
        }
        let mut command = vec!["dpkg-query".to_string(), "-S".to_string()];
        command.extend(paths);
        let arguments: Vec<&std::ffi::OsStr> = command.iter().map(std::ffi::OsStr::new).collect();
        let Ok(process) = gio::Subprocess::newv(
            &arguments,
            gio::SubprocessFlags::STDOUT_PIPE | gio::SubprocessFlags::STDERR_SILENCE,
        ) else {
            return;
        };
        let page = Rc::downgrade(self);
        glib::spawn_future_local(async move {
            let Ok((Some(output), _)) = process.communicate_utf8_future(None).await else {
                return;
            };
            let Some(page) = page.upgrade() else { return };
            let mut packages = page.packages.borrow_mut();
            for line in output.lines() {
                // "package: /etc/xdg/autostart/x.desktop", or several
                // packages separated by commas
                if let Some((package, path)) = line.split_once(": ") {
                    packages.insert(
                        path.trim().to_string(),
                        package.split(", ").next().unwrap_or(package).to_string(),
                    );
                }
            }
            drop(packages);
            page.reload();
        });
    }

    pub fn set_enabled(&self, context: &Rc<Context>, row: &Row, on: bool) {
        if let Err(error) = set_entry(&row.key(), on) {
            actions::message(
                &context.window,
                &format!(
                    "Couldn't {} {}",
                    if on { "enable" } else { "disable" },
                    row.name()
                ),
                &error,
            );
        }
        self.reload();
    }
}

/// Shows a file in the file manager.
pub fn show_file(path: &str) {
    if path.is_empty() {
        return;
    }
    let uri = gio::File::for_path(path).uri().to_string();
    let folder = Path::new(path)
        .parent()
        .map(|folder| gio::File::for_path(folder).uri().to_string());
    let Ok(bus) = gio::bus_get_sync(gio::BusType::Session, gio::Cancellable::NONE) else {
        return;
    };
    bus.call(
        Some("org.freedesktop.FileManager1"),
        "/org/freedesktop/FileManager1",
        "org.freedesktop.FileManager1",
        "ShowItems",
        Some(&(vec![uri], "").to_variant()),
        None,
        gio::DBusCallFlags::NONE,
        5000,
        gio::Cancellable::NONE,
        move |result| {
            if result.is_err()
                && let Some(folder) = folder
            {
                let _ = gio::AppInfo::launch_default_for_uri(&folder, gio::AppLaunchContext::NONE);
            }
        },
    );
}

struct Entry {
    id: String,
    name: String,
    icon: Option<gio::Icon>,
    command: String,
    enabled: bool,
    system: Option<PathBuf>,
    user: Option<PathBuf>,
}

fn user_folder() -> PathBuf {
    glib::user_config_dir().join("autostart")
}

fn desktop_files(folder: &Path) -> Vec<String> {
    let Ok(entries) = fs::read_dir(folder) else {
        return Vec::new();
    };
    entries
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.ends_with(".desktop"))
        .collect()
}

/// Every startup entry this desktop starts as an app, yours over
/// everyone's.
fn entries() -> Vec<Entry> {
    let user = user_folder();
    let mut ids: Vec<String> = desktop_files(Path::new(SYSTEM_FOLDER));
    for id in desktop_files(&user) {
        if !ids.contains(&id) {
            ids.push(id);
        }
    }
    let mut list = Vec::new();
    for id in ids {
        let system = Path::new(SYSTEM_FOLDER).join(&id);
        let system = system.exists().then_some(system);
        let mine = user.join(&id);
        let mine = mine.exists().then_some(mine);
        let Some(effective) = mine.as_ref().or(system.as_ref()) else {
            continue;
        };
        let Some(file) = load(effective) else {
            continue;
        };
        // Everyone's entry says what it is, when yours only turns it off
        let base = system.as_ref().and_then(|path| load(path));
        let described = base.as_ref().unwrap_or(&file);
        if !for_this_desktop(described)
            || flag(described, "X-GNOME-HiddenUnderSystemd") == Some(true)
            || described
                .has_key(GROUP, "X-GNOME-Autostart-Phase")
                .unwrap_or(false)
        {
            continue;
        }
        let enabled = flag(&file, "Hidden") != Some(true)
            && flag(&file, "X-GNOME-Autostart-enabled") != Some(false);
        let name = described
            .locale_string(GROUP, "Name", None)
            .map(|name| name.to_string())
            .unwrap_or_else(|_| id.trim_end_matches(".desktop").to_string());
        let icon = described
            .string(GROUP, "Icon")
            .ok()
            .and_then(|icon| gio::Icon::for_string(&icon).ok())
            .or_else(|| Some(gio::ThemedIcon::new("application-x-executable-symbolic").upcast()));
        let command = described
            .string(GROUP, "Exec")
            .map(|exec| exec.to_string())
            .unwrap_or_default();
        list.push(Entry {
            id,
            name,
            icon,
            command,
            enabled,
            system,
            user: mine,
        });
    }
    list
}

/// Turns an entry on or off for you.
fn set_entry(id: &str, on: bool) -> Result<(), String> {
    let system = Path::new(SYSTEM_FOLDER).join(id);
    let mine = user_folder().join(id);
    if on {
        if !mine.exists() {
            return Ok(());
        }
        let file = load(&mine).ok_or("Your copy of it can't be read")?;
        // A copy made only to turn it off: remove it
        if flag(&file, COPY_KEY) == Some(true) && system.exists() {
            return fs::remove_file(&mine).map_err(|error| error.to_string());
        }
        let _ = file.remove_key(GROUP, "Hidden");
        if file
            .has_key(GROUP, "X-GNOME-Autostart-enabled")
            .unwrap_or(false)
        {
            file.set_boolean(GROUP, "X-GNOME-Autostart-enabled", true);
        }
        return file.save_to_file(&mine).map_err(|error| error.to_string());
    }
    // Off: your own entry gets Hidden=true; everyone's gets a copy of yours
    // that says so
    let (file, copy) = if mine.exists() {
        (load(&mine).ok_or("Your copy of it can't be read")?, false)
    } else {
        (load(&system).ok_or("The entry can't be read")?, true)
    };
    file.set_boolean(GROUP, "Hidden", true);
    if copy {
        file.set_boolean(GROUP, COPY_KEY, true);
    }
    fs::create_dir_all(user_folder()).map_err(|error| error.to_string())?;
    file.save_to_file(&mine).map_err(|error| error.to_string())
}
