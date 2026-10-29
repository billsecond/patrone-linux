//! The Services page: the system's services (systemd units), or the
//! account's own, each with its state, how it starts and its main process.
//! Start, Stop and Restart go to systemd, which asks for an administrator's
//! password where a system service needs one.

use std::cell::{Cell as StdCell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::time::{Duration, Instant};

use gtk::prelude::*;
use gtk::{gio, glib};

use crate::row::{self, Row};
use crate::table::{self, Cell, Column, Sort, Table};
use crate::{Context, actions};

/// How often the list is read again while it's shown, and how often the
/// installed services (which takes systemd longer) are.
const REFRESH: Duration = Duration::from_secs(3);
const REFRESH_FILES: Duration = Duration::from_secs(30);

/// An installed service's file and its state (enabled, disabled, static...).
struct UnitFile {
    path: String,
    state: String,
}

struct Unit {
    name: String,
    description: String,
    active: String,
    sub: String,
}

pub struct Page {
    pub table: Table,
    pub toolbar: gtk::Box,
    store: gio::ListStore,
    rows: RefCell<HashMap<String, Row>>,
    /// System services (true) or the account's own
    system: StdCell<bool>,
    files: RefCell<HashMap<String, UnitFile>>,
    descriptions: RefCell<HashMap<String, String>>,
    asked: StdCell<Option<Instant>>,
    files_asked: StdCell<Option<Instant>>,
    busy: StdCell<bool>,
}

impl Page {
    pub fn new(context: &Rc<Context>) -> Rc<Self> {
        let store = gio::ListStore::new::<Row>();
        let table = Table::new(
            &store,
            false,
            None,
            vec![
                Column::new("Name", Cell::Text("name"), Sort::Text("name"), 240),
                Column::new(
                    "PID",
                    Cell::Number("pid-text", None),
                    Sort::Number("pid"),
                    72,
                ),
                Column::new(
                    "Description",
                    Cell::Text("description"),
                    Sort::Text("description"),
                    320,
                )
                .expand(),
                Column::new("Status", Cell::Text("status"), Sort::Text("status"), 100),
                Column::new("Starts", Cell::Text("extra"), Sort::Text("extra"), 130),
            ],
            crate::matcher(context.query.clone()),
        );
        table
            .view
            .sort_by_column(Some(&table.columns[0]), gtk::SortType::Ascending);

        let toolbar = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        let scope = gtk::DropDown::from_strings(&["System services", "Your services"]);
        scope.add_css_class("tm-scope");
        let start = crate::command("media-playback-start-symbolic", "Start");
        let stop = crate::command("media-playback-stop-symbolic", "Stop");
        let restart = crate::command("view-refresh-symbolic", "Restart");
        toolbar.append(&scope);
        for button in [&start, &stop, &restart] {
            button.set_sensitive(false);
            toolbar.append(button);
        }

        let page = Rc::new(Page {
            table,
            toolbar,
            store,
            rows: RefCell::new(HashMap::new()),
            system: StdCell::new(true),
            files: RefCell::new(HashMap::new()),
            descriptions: RefCell::new(HashMap::new()),
            asked: StdCell::new(None),
            files_asked: StdCell::new(None),
            busy: StdCell::new(false),
        });

        let weak = Rc::downgrade(&page);
        let scope_context = context.clone();
        scope.connect_selected_notify(move |scope| {
            let Some(page) = weak.upgrade() else { return };
            page.system.set(scope.selected() == 0);
            page.rows.borrow_mut().clear();
            page.store.remove_all();
            page.files.borrow_mut().clear();
            page.files_asked.set(None);
            page.refresh(&scope_context);
        });

        let buttons = [
            (start, "StartUnit"),
            (stop, "StopUnit"),
            (restart, "RestartUnit"),
        ];
        let weak_buttons: Vec<_> = buttons
            .iter()
            .map(|(button, _)| button.downgrade())
            .collect();
        page.table
            .selection
            .connect_selection_changed(move |selection, _, _| {
                let row = selection.selected_item().and_downcast::<Row>();
                let running = row.as_ref().is_some_and(|row| row.enabled());
                for (index, button) in weak_buttons.iter().enumerate() {
                    if let Some(button) = button.upgrade() {
                        // Start when stopped; Stop and Restart when running
                        button.set_sensitive(
                            row.is_some() && if index == 0 { !running } else { running },
                        );
                    }
                }
            });
        for (button, method) in buttons {
            let weak = Rc::downgrade(&page);
            let context = context.clone();
            button.connect_clicked(move |_| {
                let Some(page) = weak.upgrade() else { return };
                if let Some(row) = page.table.selected() {
                    page.control(&context, &row, method);
                }
            });
        }

        let menu = gio::Menu::new();
        let section = gio::Menu::new();
        section.append(Some("Start"), Some("win.service::StartUnit"));
        section.append(Some("Stop"), Some("win.service::StopUnit"));
        section.append(Some("Restart"), Some("win.service::RestartUnit"));
        menu.append_section(None, &section);
        let more = gio::Menu::new();
        more.append(Some("Show details"), Some("win.go-to-details"));
        menu.append_section(None, &more);
        let menu_context = context.clone();
        let popover = table::context_menu(&menu, &page.table.view);
        page.table.on_menu(move |row, cell, x, y| {
            *menu_context.target.borrow_mut() = Some(row.clone());
            table::popup(&popover, cell, x, y);
        });
        page
    }

    pub fn update(self: &Rc<Self>, context: &Rc<Context>, shown: bool) {
        if shown
            && self
                .asked
                .get()
                .is_none_or(|when| when.elapsed() >= REFRESH)
        {
            self.refresh(context);
        }
    }

    /// Start, stop or restart a service: systemd asks for a password where
    /// it's needed.
    pub fn control(self: &Rc<Self>, context: &Rc<Context>, row: &Row, method: &'static str) {
        let system = self.system.get();
        let unit = row.key();
        let page = Rc::downgrade(self);
        let context = context.clone();
        glib::spawn_future_local(async move {
            let result = async {
                let bus = gio::bus_get_future(bus_type(system)).await?;
                bus.call_future(
                    Some("org.freedesktop.systemd1"),
                    "/org/freedesktop/systemd1",
                    "org.freedesktop.systemd1.Manager",
                    method,
                    Some(&(unit.as_str(), "replace").to_variant()),
                    None,
                    gio::DBusCallFlags::ALLOW_INTERACTIVE_AUTHORIZATION,
                    120_000,
                )
                .await
            }
            .await;
            if let Err(error) = result {
                let text = error.to_string();
                // Not when the person cancelled the password dialog
                if !text.contains("AccessDenied")
                    && !text.contains("InteractiveAuthorizationRequired")
                {
                    let verb = match method {
                        "StartUnit" => "start",
                        "StopUnit" => "stop",
                        _ => "restart",
                    };
                    actions::message(
                        &context.window,
                        &format!("Couldn't {verb} {}", unit.trim_end_matches(".service")),
                        &text,
                    );
                }
            }
            if let Some(page) = page.upgrade() {
                page.asked.set(None);
                page.refresh(&context);
            }
        });
    }

    fn refresh(self: &Rc<Self>, context: &Rc<Context>) {
        if self.busy.get() {
            return;
        }
        self.busy.set(true);
        self.asked.set(Some(Instant::now()));
        let system = self.system.get();
        let with_files = self
            .files_asked
            .get()
            .is_none_or(|when| when.elapsed() >= REFRESH_FILES);
        if with_files {
            self.files_asked.set(Some(Instant::now()));
        }
        let page = Rc::downgrade(self);
        let context = context.clone();
        glib::spawn_future_local(async move {
            let units = list_units(system).await;
            let files = if with_files {
                list_unit_files(system).await.ok()
            } else {
                None
            };
            let Some(page) = page.upgrade() else { return };
            page.busy.set(false);
            // The list was switched meanwhile
            if page.system.get() != system {
                return;
            }
            if let Some(files) = files {
                *page.files.borrow_mut() = files;
            }
            if let Ok(units) = units {
                page.fill(&context, &units);
            }
        });
    }

    fn fill(&self, context: &Rc<Context>, units: &[Unit]) {
        let system = self.system.get();
        let files = self.files.borrow();
        let loaded: HashMap<&str, &Unit> = units
            .iter()
            .map(|unit| (unit.name.as_str(), unit))
            .collect();
        let mut names: Vec<&str> = loaded.keys().copied().collect();
        // Installed but not loaded; templates (getty@.service) only run as
        // instances
        names.extend(
            files
                .keys()
                .map(String::as_str)
                .filter(|name| !loaded.contains_key(name) && !name.contains("@.")),
        );

        let model = context.model.borrow();
        let mut rows = self.rows.borrow_mut();
        let mut list = Vec::with_capacity(names.len());
        for name in names {
            let unit = loaded.get(name);
            let file = files.get(name);
            let row = rows
                .entry(name.to_string())
                .or_insert_with(|| Row::new(name))
                .clone();
            row.put_text(
                "name",
                &crate::model::unescape(name.trim_end_matches(".service")),
            );
            let description = match unit {
                Some(unit) if !unit.description.is_empty() => unit.description.clone(),
                _ => file
                    .map(|file| self.description(&file.path))
                    .unwrap_or_default(),
            };
            row.put_text("description", &description);
            let (active, sub) = unit
                .map(|unit| (unit.active.as_str(), unit.sub.as_str()))
                .unwrap_or(("inactive", "dead"));
            row.put_text("status", status(active, sub));
            let running = matches!(active, "active" | "activating" | "reloading");
            if row.enabled() != running {
                row.set_enabled(running);
            }
            let pid = if running {
                model
                    .service_pids
                    .get(&(system, name.to_string()))
                    .copied()
                    .unwrap_or(0)
            } else {
                0
            };
            if row.pid() != pid {
                row.set_pid(pid);
                row.set_pid_text(if pid > 0 {
                    pid.to_string()
                } else {
                    String::new()
                });
            }
            row.put_text(
                "extra",
                starts(file.map(|file| file.state.as_str()).unwrap_or("")),
            );
            list.push(row);
        }
        let keep: std::collections::HashSet<String> = list.iter().map(|row| row.key()).collect();
        rows.retain(|name, _| keep.contains(name));
        drop(rows);
        drop(model);
        row::sync_store(&self.store, &list);
        self.table.resort();
    }

    /// The Description= line of a unit file that isn't loaded.
    fn description(&self, path: &str) -> String {
        if let Some(text) = self.descriptions.borrow().get(path) {
            return text.clone();
        }
        let text = std::fs::read_to_string(path)
            .ok()
            .and_then(|text| {
                text.lines().find_map(|line| {
                    line.strip_prefix("Description=")
                        .map(|d| d.trim().to_string())
                })
            })
            .unwrap_or_default();
        self.descriptions
            .borrow_mut()
            .insert(path.to_string(), text.clone());
        text
    }
}

/// How a service starts, from its unit file's state.
fn starts(state: &str) -> &'static str {
    match state {
        "enabled" | "enabled-runtime" | "alias" => "Automatically",
        "disabled" => "Manually",
        "static" | "indirect" | "generated" | "transient" => "When needed",
        "masked" | "masked-runtime" => "Never",
        _ => "",
    }
}

fn status(active: &str, sub: &str) -> &'static str {
    match (active, sub) {
        ("active", "exited") => "Finished",
        ("active", _) => "Running",
        ("activating", _) => "Starting",
        ("deactivating", _) => "Stopping",
        ("reloading", _) => "Reloading",
        ("failed", _) => "Failed",
        _ => "Stopped",
    }
}

fn bus_type(system: bool) -> gio::BusType {
    if system {
        gio::BusType::System
    } else {
        gio::BusType::Session
    }
}

/// The loaded services: name, description and state.
async fn list_units(system: bool) -> Result<Vec<Unit>, glib::Error> {
    let bus = gio::bus_get_future(bus_type(system)).await?;
    let reply = bus
        .call_future(
            Some("org.freedesktop.systemd1"),
            "/org/freedesktop/systemd1",
            "org.freedesktop.systemd1.Manager",
            "ListUnitsByPatterns",
            Some(&(Vec::<String>::new(), vec!["*.service".to_string()]).to_variant()),
            Some(glib::VariantTy::new("(a(ssssssouso))").unwrap()),
            gio::DBusCallFlags::NONE,
            10_000,
        )
        .await?;
    let list = reply.child_value(0);
    let mut units = Vec::with_capacity(list.n_children());
    for index in 0..list.n_children() {
        let unit = list.child_value(index);
        let text = |field: usize| {
            unit.child_value(field)
                .str()
                .unwrap_or_default()
                .to_string()
        };
        // Units whose files are gone (not-found) aren't listed
        if text(2) == "not-found" {
            continue;
        }
        units.push(Unit {
            name: text(0),
            description: text(1),
            active: text(3),
            sub: text(4),
        });
    }
    Ok(units)
}

/// The installed services, loaded or not, by name.
async fn list_unit_files(system: bool) -> Result<HashMap<String, UnitFile>, glib::Error> {
    let bus = gio::bus_get_future(bus_type(system)).await?;
    let reply = bus
        .call_future(
            Some("org.freedesktop.systemd1"),
            "/org/freedesktop/systemd1",
            "org.freedesktop.systemd1.Manager",
            "ListUnitFilesByPatterns",
            Some(&(Vec::<String>::new(), vec!["*.service".to_string()]).to_variant()),
            Some(glib::VariantTy::new("(a(ss))").unwrap()),
            gio::DBusCallFlags::NONE,
            20_000,
        )
        .await?;
    let list = reply.child_value(0);
    let mut files = HashMap::new();
    for index in 0..list.n_children() {
        let entry = list.child_value(index);
        let path = entry.child_value(0).str().unwrap_or_default().to_string();
        let state = entry.child_value(1).str().unwrap_or_default().to_string();
        let name = path.rsplit('/').next().unwrap_or_default().to_string();
        files.insert(name, UnitFile { path, state });
    }
    Ok(files)
}
