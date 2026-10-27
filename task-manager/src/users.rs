//! The Users page: each account signed in, here or remotely, with what its
//! processes use, and its apps and background processes under it. Who is
//! signed in comes from logind, the system's sign-in manager.

use std::collections::HashMap;
use std::rc::Rc;
use std::time::{Duration, Instant};

use gtk::prelude::*;
use gtk::{gio, glib};

use crate::Context;
use crate::model::Login;
use crate::processes::{CPU_HEAT, DISK_HEAT, MEMORY_HEAT, NETWORK_HEAT, process_menu};
use crate::table::{self, Cell, Column, Sort, Table};

/// How often to ask logind again while the page is shown.
const REFRESH: Duration = Duration::from_secs(5);

pub struct Page {
    pub table: Table,
    pub toolbar: gtk::Box,
    asked: std::cell::Cell<Option<Instant>>,
}

impl Page {
    pub fn new(context: &Rc<Context>) -> Rc<Self> {
        let table = Table::new(
            &context.model.borrow().users,
            true,
            None,
            vec![
                Column::new("User", Cell::Name, Sort::Text("name"), 300).expand(),
                Column::new("Status", Cell::Text("status"), Sort::Text("status"), 110),
                Column::new(
                    "CPU",
                    Cell::Number("cpu-text", Some(("cpu", CPU_HEAT))),
                    Sort::Number("cpu"),
                    86,
                ),
                Column::new(
                    "Memory",
                    Cell::Number("memory-text", Some(("memory", MEMORY_HEAT))),
                    Sort::Number("memory"),
                    104,
                ),
                Column::new(
                    "Disk",
                    Cell::Number("disk-text", Some(("disk", DISK_HEAT))),
                    Sort::Number("disk"),
                    92,
                ),
                Column::new(
                    "Network",
                    Cell::Number("network-text", Some(("network", NETWORK_HEAT))),
                    Sort::Number("network"),
                    96,
                ),
                Column::new(
                    "Sessions",
                    Cell::Text("description"),
                    Sort::Text("description"),
                    130,
                ),
            ],
            crate::matcher(context.query.clone()),
        );
        table
            .view
            .sort_by_column(Some(&table.columns[0]), gtk::SortType::Ascending);
        let menu = process_menu(false);
        let menu_context = context.clone();
        let popover = table::context_menu(&menu, &table.view);
        table.on_menu(move |row, cell, x, y| {
            // Accounts themselves have no menu: only their processes
            if row.processes().is_empty() {
                return;
            }
            *menu_context.target.borrow_mut() = Some(row.clone());
            table::popup(&popover, cell, x, y);
        });
        table.view.connect_activate(|view, position| {
            let item = view
                .model()
                .and_then(|model| model.item(position))
                .and_downcast::<gtk::TreeListRow>();
            if let Some(item) = item.filter(|item| item.is_expandable()) {
                item.set_expanded(!item.is_expanded());
            }
        });
        let page = Rc::new(Page {
            table,
            toolbar: gtk::Box::new(gtk::Orientation::Horizontal, 4),
            asked: Default::default(),
        });
        // Once at the start, so the page isn't empty when first shown
        ask_logind(context.clone());
        page.asked.set(Some(Instant::now()));
        page
    }

    pub fn update(&self, context: &Rc<Context>, shown: bool) {
        if shown {
            self.table.resort();
            if self
                .asked
                .get()
                .is_none_or(|when| when.elapsed() >= REFRESH)
            {
                self.asked.set(Some(Instant::now()));
                ask_logind(context.clone());
            }
        }
    }
}

/// Asks logind who is signed in, and how: each session's account, whether
/// it's remote, and its state.
fn ask_logind(context: Rc<Context>) {
    glib::spawn_future_local(async move {
        if let Ok(logins) = logins().await {
            context.model.borrow_mut().logins = logins;
        }
    });
}

async fn logins() -> Result<HashMap<u32, Login>, glib::Error> {
    let bus = gio::bus_get_future(gio::BusType::System).await?;
    let reply = bus
        .call_future(
            Some("org.freedesktop.login1"),
            "/org/freedesktop/login1",
            "org.freedesktop.login1.Manager",
            "ListSessions",
            None,
            Some(glib::VariantTy::new("(a(susso))").unwrap()),
            gio::DBusCallFlags::NONE,
            5000,
        )
        .await?;
    let sessions = reply.child_value(0);
    let mut logins: HashMap<u32, Login> = HashMap::new();
    for index in 0..sessions.n_children() {
        let session = sessions.child_value(index);
        let uid: u32 = session.child_value(1).get().unwrap_or(0);
        let path = session.child_value(4).str().unwrap_or_default().to_string();
        let properties = bus
            .call_future(
                Some("org.freedesktop.login1"),
                &path,
                "org.freedesktop.DBus.Properties",
                "GetAll",
                Some(&("org.freedesktop.login1.Session",).to_variant()),
                Some(glib::VariantTy::new("(a{sv})").unwrap()),
                gio::DBusCallFlags::NONE,
                5000,
            )
            .await;
        let Ok(properties) = properties else { continue };
        let properties = glib::VariantDict::new(Some(&properties.child_value(0)));
        let class: String = properties
            .lookup("Class")
            .ok()
            .flatten()
            .unwrap_or_default();
        // People's sessions, not the login screen's or the account's
        // service manager
        if !class.starts_with("user") {
            continue;
        }
        let remote: bool = properties.lookup("Remote").ok().flatten().unwrap_or(false);
        let state: String = properties
            .lookup("State")
            .ok()
            .flatten()
            .unwrap_or_default();
        let login = logins.entry(uid).or_default();
        login.sessions += 1;
        if remote {
            login.remote += 1;
        }
        // The liveliest of the account's sessions
        let rank = |state: &str| match state {
            "active" => 3,
            "online" => 2,
            "closing" => 1,
            _ => 0,
        };
        if rank(&state) > rank(&login.state) {
            login.state = state;
        }
    }
    Ok(logins)
}
