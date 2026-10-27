//! The Details page: every process on its own line, kernel threads too,
//! with its PID, account, processor and memory use, threads and command
//! line.

use std::rc::Rc;

use gtk::prelude::*;

use crate::Context;
use crate::processes::{delete_ends, end_button, process_menu};
use crate::table::{self, Cell, Column, Sort, Table};

pub struct Page {
    pub table: Table,
    pub toolbar: gtk::Box,
}

impl Page {
    pub fn new(context: &Rc<Context>) -> Self {
        let table = Table::new(
            &context.model.borrow().processes,
            false,
            None,
            vec![
                Column::new("Name", Cell::Name, Sort::Text("name"), 220),
                Column::new(
                    "PID",
                    Cell::Number("pid-text", None),
                    Sort::Number("pid"),
                    72,
                ),
                Column::new(
                    "Status",
                    Cell::Text("description"),
                    Sort::Text("description"),
                    90,
                ),
                Column::new("User name", Cell::Text("user"), Sort::Text("user"), 110),
                Column::new(
                    "CPU",
                    Cell::Number("cpu-detail", None),
                    Sort::Number("cpu"),
                    56,
                ),
                Column::new(
                    "Memory (private)",
                    Cell::Number("memory-detail", None),
                    Sort::Number("memory"),
                    130,
                ),
                Column::new(
                    "Threads",
                    Cell::Number("threads", None),
                    Sort::Number("threads"),
                    70,
                ),
                Column::new(
                    "Command line",
                    Cell::Text("command"),
                    Sort::Text("command"),
                    300,
                )
                .expand(),
            ],
            crate::matcher(context.query.clone()),
        );
        table
            .view
            .sort_by_column(Some(&table.columns[0]), gtk::SortType::Ascending);

        let menu = process_menu(true);
        let menu_context = context.clone();
        let popover = table::context_menu(&menu, &table.view);
        table.on_menu(move |row, cell, x, y| {
            *menu_context.target.borrow_mut() = Some(row.clone());
            table::popup(&popover, cell, x, y);
        });
        // Double-click or Enter: the process's properties
        let activate_context = context.clone();
        table.view.connect_activate(move |view, position| {
            let row = view
                .model()
                .and_then(|model| model.item(position))
                .and_then(|item| table::row_of(&item, false));
            if let Some(row) = row {
                crate::actions::properties(&activate_context, &row);
            }
        });

        let toolbar = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        let end = crate::command("process-stop-symbolic", "End task");
        toolbar.append(&end);
        end_button(&end, &table, context, false);
        let properties = crate::command("document-properties-symbolic", "Properties");
        properties.set_sensitive(false);
        let weak = properties.downgrade();
        table
            .selection
            .connect_selection_changed(move |selection, _, _| {
                if let Some(button) = weak.upgrade() {
                    button.set_sensitive(selection.selected_item().is_some());
                }
            });
        let selection = table.selection.clone();
        let properties_context = context.clone();
        properties.connect_clicked(move |_| {
            if let Some(row) = selection
                .selected_item()
                .and_then(|item| table::row_of(&item, false))
            {
                crate::actions::properties(&properties_context, &row);
            }
        });
        toolbar.append(&properties);
        delete_ends(&table, context, false);
        Page { table, toolbar }
    }

    /// Selects a process, for "Go to details".
    pub fn select(&self, pid: i32) -> bool {
        self.table.select(|row| row.pid() == pid)
    }
}
