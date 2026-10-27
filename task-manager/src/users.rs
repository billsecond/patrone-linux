//! The Users page: each account signed in, here or remotely, with what its
//! processes use, and its apps and background processes under it. Who is
//! signed in comes from logind, the system's sign-in manager.
//!
//! Sign out ends an account's sessions, and Disconnect ends only the
//! sessions that show its desktop on a screen, leaving the desktop running
//! (in Patrone, an account's desktop is a session with no seat that keeps
//! running, and signing in on the screen adds a session that shows it).
//! Both go to logind over D-Bus with interactive authorization allowed, so
//! for another account polkit asks for an administrator's password right
//! then, in its own dialog. Signing out your own account goes through the
//! desktop's own log-out instead.

use std::collections::HashMap;
use std::rc::Rc;
use std::time::{Duration, Instant};

use gtk::prelude::*;
use gtk::{gio, glib};

use crate::Context;
use crate::model::{Login, Session};
use crate::processes::{CPU_HEAT, DISK_HEAT, MEMORY_HEAT, NETWORK_HEAT, process_menu};
use crate::row::Row;
use crate::table::{self, Cell, Column, Sort, Table};

/// How often to ask logind again while the page is shown.
const REFRESH: Duration = Duration::from_secs(5);
/// How long logind may wait for someone to answer polkit's password dialog.
const PASSWORD_WAIT: i32 = 5 * 60 * 1000;

#[derive(Clone, Copy, PartialEq)]
pub enum Ending {
    SignOut,
    Disconnect,
}

pub struct Page {
    pub table: Table,
    pub toolbar: gtk::Box,
    disconnect: gtk::Button,
    sign_out: gtk::Button,
    asked: std::cell::Cell<Option<Instant>>,
}

impl Page {
    pub fn new(context: &Rc<Context>) -> Rc<Self> {
        let table = Table::new(
            &context.model.borrow().users,
            true,
            None,
            vec![
                Column::new("User", Cell::Name, Sort::Text("name"), 280).expand(),
                Column::new("Status", Cell::Text("status"), Sort::Text("status"), 100),
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
                    200,
                ),
            ],
            crate::matcher(context.query.clone()),
        );
        table
            .view
            .sort_by_column(Some(&table.columns[0]), gtk::SortType::Ascending);

        // Accounts get Disconnect and Sign out; their processes the usual
        // process menu
        let account_menu = gio::Menu::new();
        account_menu.append(Some("Disconnect"), Some("win.disconnect"));
        account_menu.append(Some("Sign out"), Some("win.sign-out"));
        let account_popover = table::context_menu(&account_menu, &table.view);
        let process_popover = table::context_menu(&process_menu(false), &table.view);
        let menu_context = context.clone();
        table.on_menu(move |row, cell, x, y| {
            *menu_context.target.borrow_mut() = Some(row.clone());
            if is_account(row) {
                table::popup(&account_popover, cell, x, y);
            } else if !row.processes().is_empty() {
                table::popup(&process_popover, cell, x, y);
            }
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

        let toolbar = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        let disconnect = crate::command("video-display-symbolic", "Disconnect");
        let sign_out = crate::command("system-log-out-symbolic", "Sign out");
        disconnect.set_tooltip_text(Some(
            "Stop showing this account's desktop on the screen. The desktop keeps running, with its apps open.",
        ));
        sign_out.set_tooltip_text(Some("End this account's desktop and every app in it"));
        toolbar.append(&disconnect);
        toolbar.append(&sign_out);

        let page = Rc::new(Page {
            table,
            toolbar,
            disconnect: disconnect.clone(),
            sign_out: sign_out.clone(),
            asked: Default::default(),
        });
        let weak = Rc::downgrade(&page);
        let selection_context = context.clone();
        page.table
            .selection
            .connect_selection_changed(move |_, _, _| {
                if let Some(page) = weak.upgrade() {
                    page.refresh_buttons(&selection_context);
                }
            });
        for (button, ending) in [
            (disconnect, Ending::Disconnect),
            (sign_out, Ending::SignOut),
        ] {
            let weak = Rc::downgrade(&page);
            let context = context.clone();
            button.connect_clicked(move |_| {
                let Some(page) = weak.upgrade() else { return };
                if let Some(row) = page.table.selected().filter(is_account) {
                    end(&context, &row, ending);
                }
            });
        }
        page.refresh_buttons(context);

        // Once at the start, so the page isn't empty when first shown
        ask_logind(context.clone());
        page.asked.set(Some(Instant::now()));
        page
    }

    pub fn update(&self, context: &Rc<Context>, shown: bool) {
        if shown {
            self.table.resort();
            self.refresh_buttons(context);
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

    /// Sign out for a selected account; Disconnect when its desktop is shown
    /// on a screen.
    fn refresh_buttons(&self, context: &Context) {
        let row = self.table.selected().filter(is_account);
        let viewers = row
            .as_ref()
            .and_then(|row| {
                context
                    .model
                    .borrow()
                    .logins
                    .get(&uid_of(row))
                    .map(|login| login.viewers().len())
            })
            .unwrap_or(0);
        self.sign_out.set_sensitive(row.is_some());
        self.disconnect.set_sensitive(viewers > 0);
    }
}

/// An account's row, not one of its apps or processes: accounts stand for
/// no process of their own.
pub fn is_account(row: &Row) -> bool {
    row.children().is_some() && row.processes().is_empty()
}

/// The account's UID, which its row keeps in extra-value.
fn uid_of(row: &Row) -> u32 {
    row.extra_value() as u32
}

/// The account's name for messages: its full name, or else its login.
fn person(row: &Row) -> String {
    let name = row.name();
    let login = row.user();
    name.strip_suffix(&format!(" ({login})"))
        .map(str::to_string)
        .unwrap_or(name)
}

/// Asks first, as Windows does, then signs the account out or disconnects
/// it.
pub fn end(context: &Rc<Context>, row: &Row, ending: Ending) {
    if !is_account(row) {
        return;
    }
    let uid = uid_of(row);
    let own = uid == context.model.borrow().my_uid;
    let name = person(row);
    let viewers: Vec<String> = context
        .model
        .borrow()
        .logins
        .get(&uid)
        .map(|login| {
            login
                .viewers()
                .iter()
                .map(|session| session.id.clone())
                .collect()
        })
        .unwrap_or_default();
    if ending == Ending::Disconnect && viewers.is_empty() {
        crate::actions::message(
            &context.window,
            &format!("{name}'s desktop isn't shown on a screen"),
            "There's nothing to disconnect. Sign out ends the desktop itself.",
        );
        return;
    }
    let their = if own { "your" } else { "their" };
    let (title, detail, button) = match ending {
        Ending::SignOut => (
            format!("Sign out {name}?"),
            format!("Unsaved work in {their} apps will be lost."),
            "Sign out",
        ),
        Ending::Disconnect => (
            format!("Disconnect {name}?"),
            format!(
                "The screen goes back to the sign-in screen. {} desktop keeps running, with {their} apps open, for the next time {} sign in.",
                if own { "Your" } else { "Their" },
                if own { "you" } else { "they" }
            ),
            "Disconnect",
        ),
    };
    let dialog = gtk::AlertDialog::builder()
        .modal(true)
        .message(title)
        .detail(detail)
        .buttons(["Cancel", button])
        .cancel_button(0)
        .default_button(0)
        .build();
    let context = context.clone();
    dialog.choose(
        Some(&context.window.clone()),
        gio::Cancellable::NONE,
        move |answer| {
            if !matches!(answer, Ok(1)) {
                return;
            }
            glib::spawn_future_local(async move {
                let result = match (ending, own) {
                    (Ending::SignOut, true) => log_out().await,
                    (Ending::SignOut, false) => logind("TerminateUser", &(uid,).to_variant()).await,
                    (Ending::Disconnect, _) => {
                        let mut result = Ok(());
                        for id in &viewers {
                            result = logind("TerminateSession", &(id.as_str(),).to_variant()).await;
                            if result.is_err() {
                                break;
                            }
                        }
                        result
                    }
                };
                if let Err(error) = result {
                    report(&context, &name, ending, own, &error);
                }
                ask_logind(context.clone());
            });
        },
    );
}

/// Says plainly what didn't happen, and why.
fn report(context: &Context, name: &str, ending: Ending, own: bool, error: &glib::Error) {
    let text = error.to_string();
    let refused = [
        "AccessDenied",
        "Access denied",
        "InteractiveAuthorizationRequired",
        "not authorized",
    ]
    .iter()
    .any(|sign| text.contains(sign));
    let (done, verb) = match ending {
        Ending::SignOut => ("signed out", "sign out"),
        Ending::Disconnect => ("disconnected", "disconnect"),
    };
    let title = if own {
        format!("You weren't {done}")
    } else {
        format!("{name} wasn't {done}")
    };
    let detail = if refused && !own {
        format!(
            "To {verb} another account, an administrator's password is needed, and it wasn't given. Nothing was changed."
        )
    } else {
        format!("Nothing was changed. The system said: {text}")
    };
    crate::actions::message(&context.window, &title, &detail);
}

/// Calls a logind manager method, letting polkit ask for a password.
async fn logind(method: &str, arguments: &glib::Variant) -> Result<(), glib::Error> {
    let bus = gio::bus_get_future(gio::BusType::System).await?;
    bus.call_future(
        Some("org.freedesktop.login1"),
        "/org/freedesktop/login1",
        "org.freedesktop.login1.Manager",
        method,
        Some(arguments),
        None,
        gio::DBusCallFlags::ALLOW_INTERACTIVE_AUTHORIZATION,
        PASSWORD_WAIT,
    )
    .await
    .map(|_| ())
}

/// Signs out of this desktop through its own session manager, which lets
/// apps with unsaved work speak up. Mode 1: no second confirmation, since
/// Task Manager has asked.
async fn log_out() -> Result<(), glib::Error> {
    let bus = gio::bus_get_future(gio::BusType::Session).await?;
    bus.call_future(
        Some("org.gnome.SessionManager"),
        "/org/gnome/SessionManager",
        "org.gnome.SessionManager",
        "Logout",
        Some(&(1u32,).to_variant()),
        None,
        gio::DBusCallFlags::NONE,
        60_000,
    )
    .await
    .map(|_| ())
}

/// Asks logind who is signed in, and how: each session's account, seat,
/// type and state.
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
        let id = session.child_value(0).str().unwrap_or_default().to_string();
        let uid: u32 = session.child_value(1).get().unwrap_or(0);
        let seat = session.child_value(3).str().unwrap_or_default().to_string();
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
        let text =
            |key: &str| -> String { properties.lookup(key).ok().flatten().unwrap_or_default() };
        // People's sessions, not the login screen's or the account's
        // service manager
        if !text("Class").starts_with("user") {
            continue;
        }
        let state = text("State");
        let kind = text("Type");
        let login = logins.entry(uid).or_default();
        login.sessions.push(Session {
            id,
            seat,
            graphical: matches!(kind.as_str(), "wayland" | "x11" | "mir"),
        });
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
