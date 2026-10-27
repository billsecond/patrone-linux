//! The Processes page: apps, then background processes, then system
//! processes, each group with its processes under it, and how much
//! processor, memory, disk and network each uses. Column headers carry the
//! whole computer's figures, as "12%" above "CPU".

use std::rc::Rc;

use gtk::prelude::*;
use gtk::{gdk, gio, glib};

use crate::model;
use crate::sample::Sample;
use crate::table::{self, Cell, Column, Sort, Table};
use crate::{Context, actions};

/// Where each column's shade gets darker: CPU in percent, memory in bytes,
/// disk and network in bytes per second.
pub const CPU_HEAT: [f64; 4] = [0.5, 5.0, 20.0, 50.0];
pub const MEMORY_HEAT: [f64; 4] = [50e6, 200e6, 500e6, 1.5e9];
pub const DISK_HEAT: [f64; 4] = [0.1e6, 1e6, 10e6, 50e6];
pub const NETWORK_HEAT: [f64; 4] = [12_500.0, 125_000.0, 1.25e6, 6.25e6];

pub struct Page {
    pub table: Table,
    pub toolbar: gtk::Box,
}

impl Page {
    pub fn new(context: &Rc<Context>) -> Self {
        let counts = context.counts.clone();
        let table = Table::new(
            &context.model.borrow().groups,
            true,
            Some(Rc::new(move |kind| {
                model::section_title(kind, &counts.get())
            })),
            vec![
                Column::new("Name", Cell::Name, Sort::Text("name"), 300).expand(),
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
            if row.processes().is_empty() {
                return;
            }
            *menu_context.target.borrow_mut() = Some(row.clone());
            table::popup(&popover, cell, x, y);
        });

        // Double-click or Enter opens a group
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
        let end = crate::command("process-stop-symbolic", "End task");
        toolbar.append(&end);
        end_button(&end, &table, context, true);
        delete_ends(&table, context, true);
        Page { table, toolbar }
    }

    /// Column headers with the whole computer's figures, and the rows
    /// sorted again.
    pub fn update(&self, sample: &Sample) {
        let disk = sample
            .disks
            .iter()
            .map(|disk| disk.active)
            .fold(0.0, f64::max);
        let network: f64 = sample.nets.iter().map(|net| net.send + net.receive).sum();
        let memory = sample.memory.used() as f64 / sample.memory.total.max(1) as f64;
        let titles = [
            (2, format!("{:.0}%\nCPU", sample.cpu * 100.0)),
            (3, format!("{:.0}%\nMemory", memory * 100.0)),
            (4, format!("{:.0}%\nDisk", disk * 100.0)),
            (5, format!("{}\nNetwork", crate::format::bits_rate(network))),
        ];
        for (index, title) in titles {
            let column = &self.table.columns[index];
            if column.title().as_deref() != Some(title.as_str()) {
                column.set_title(Some(&title));
            }
        }
        self.table.refresh_headers();
        self.table.resort();
    }
}

/// The right-click menu for processes. The Details page's also sets
/// priorities; the others can go to the process on the Details page.
pub fn process_menu(details: bool) -> gio::Menu {
    let menu = gio::Menu::new();
    let ending = gio::Menu::new();
    ending.append(Some("End task"), Some("win.end-task"));
    ending.append(Some("End process tree"), Some("win.end-tree"));
    menu.append_section(None, &ending);
    if details {
        let priorities = gio::Menu::new();
        for (id, name, _) in actions::PRIORITIES {
            priorities.append(Some(name), Some(&format!("win.priority::{id}")));
        }
        let section = gio::Menu::new();
        section.append_submenu(Some("Set priority"), &priorities);
        menu.append_section(None, &section);
    }
    let more = gio::Menu::new();
    if !details {
        more.append(Some("Go to details"), Some("win.go-to-details"));
    }
    more.append(Some("Open file location"), Some("win.open-location"));
    more.append(Some("Properties"), Some("win.process-properties"));
    menu.append_section(None, &more);
    menu
}

/// An "End task" button for a list: on while a process or group is
/// selected.
pub fn end_button(button: &gtk::Button, table: &Table, context: &Rc<Context>, tree: bool) {
    button.set_sensitive(false);
    let selection = table.selection.clone();
    let weak = button.downgrade();
    table
        .selection
        .connect_selection_changed(move |selection, _, _| {
            let row = selection
                .selected_item()
                .and_then(|item| table::row_of(&item, tree));
            if let Some(button) = weak.upgrade() {
                button.set_sensitive(row.is_some_and(|row| !row.processes().is_empty()));
            }
        });
    let context = context.clone();
    button.connect_clicked(move |_| {
        if let Some(row) = selection
            .selected_item()
            .and_then(|item| table::row_of(&item, tree))
            && !row.processes().is_empty()
        {
            actions::end_task(&context, &row, false);
        }
    });
}

/// The Delete key ends the selected task.
pub fn delete_ends(table: &Table, context: &Rc<Context>, tree: bool) {
    let keys = gtk::EventControllerKey::new();
    let selection = table.selection.clone();
    let context = context.clone();
    keys.connect_key_pressed(move |_, key, _, _| {
        if key != gdk::Key::Delete {
            return glib::Propagation::Proceed;
        }
        if let Some(row) = selection
            .selected_item()
            .and_then(|item| table::row_of(&item, tree))
            && !row.processes().is_empty()
        {
            actions::end_task(&context, &row, false);
        }
        glib::Propagation::Stop
    });
    table.view.add_controller(keys);
}
