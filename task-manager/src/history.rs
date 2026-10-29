//! The Usage history page: how much processor time each app has used since
//! the computer started. Apps opened from the desktop run in a systemd
//! scope of their own, and when a scope ends systemd writes what it used to
//! your journal ("Consumed 2.3s CPU time, 120M memory peak"). Those
//! records, and the scopes still running, add up to each app's use.

use std::cell::{Cell as StdCell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::time::{Duration, Instant};

use gtk::prelude::*;
use gtk::{gio, glib};

use crate::row::{self, APP, Row};
use crate::table::{Cell, Column, Sort, Table};
use crate::{Context, format, model, sample};

/// systemd's "unit consumed resources" message.
const RESOURCES_MESSAGE: &str = "MESSAGE_ID=ae8f7b866b0347b9af31fe1c80b127c0";
const REFRESH: Duration = Duration::from_secs(5);
const REREAD_JOURNAL: Duration = Duration::from_secs(60);

#[derive(Default, Clone)]
struct Usage {
    cpu: f64,
    peak: f64,
    opened: u32,
}

pub struct Page {
    pub widget: gtk::Box,
    pub table: Table,
    pub toolbar: gtk::Box,
    store: gio::ListStore,
    rows: RefCell<HashMap<String, Row>>,
    /// Apps that have closed, from the journal, by desktop entry
    ended: RefCell<HashMap<String, Usage>>,
    read: StdCell<Option<Instant>>,
    refreshed: StdCell<Option<Instant>>,
    reading: StdCell<bool>,
}

impl Page {
    pub fn new(context: &Rc<Context>) -> Rc<Self> {
        let store = gio::ListStore::new::<Row>();
        let table = Table::new(
            &store,
            false,
            None,
            vec![
                Column::new("Name", Cell::Name, Sort::Text("name"), 300).expand(),
                Column::new(
                    "CPU time",
                    Cell::Number("cpu-text", None),
                    Sort::Number("cpu"),
                    110,
                ),
                Column::new(
                    "Peak memory",
                    Cell::Number("memory-text", None),
                    Sort::Number("memory"),
                    120,
                ),
                Column::new(
                    "Times opened",
                    Cell::Number("extra", None),
                    Sort::Number("extra-value"),
                    110,
                ),
            ],
            crate::matcher(context.query.clone()),
        );
        table
            .view
            .sort_by_column(Some(&table.columns[1]), gtk::SortType::Ascending);

        let since = sample::boot_time()
            .and_then(|boot| glib::DateTime::from_unix_local(boot).ok())
            .and_then(|when| when.format("%A %x, %X").ok())
            .map(|text| {
                format!("Use since the computer started, {text}, by apps opened from the desktop.")
            })
            .unwrap_or_else(|| {
                "Use since the computer started, by apps opened from the desktop.".into()
            });
        let note = gtk::Label::builder()
            .label(&since)
            .xalign(0.0)
            .wrap(true)
            .css_classes(["tm-note"])
            .build();
        let widget = gtk::Box::new(gtk::Orientation::Vertical, 0);
        widget.append(&note);
        widget.append(&table.widget);

        Rc::new(Page {
            widget,
            table,
            toolbar: gtk::Box::new(gtk::Orientation::Horizontal, 4),
            store,
            rows: RefCell::new(HashMap::new()),
            ended: RefCell::new(HashMap::new()),
            read: StdCell::new(None),
            refreshed: StdCell::new(None),
            reading: StdCell::new(false),
        })
    }

    pub fn update(self: &Rc<Self>, context: &Rc<Context>, shown: bool) {
        if !shown {
            return;
        }
        if !self.reading.get()
            && self
                .read
                .get()
                .is_none_or(|when| when.elapsed() >= REREAD_JOURNAL)
        {
            self.read_journal(context);
        }
        if self
            .refreshed
            .get()
            .is_none_or(|when| when.elapsed() >= REFRESH)
        {
            self.refreshed.set(Some(Instant::now()));
            self.fill(context);
        }
    }

    fn read_journal(self: &Rc<Self>, context: &Rc<Context>) {
        self.reading.set(true);
        let arguments = [
            "journalctl",
            "--user",
            "--boot",
            "--output=json",
            "--output-fields=USER_UNIT,CPU_USAGE_NSEC,MEMORY_PEAK",
            "--no-pager",
            RESOURCES_MESSAGE,
        ];
        let arguments: Vec<&std::ffi::OsStr> = arguments.iter().map(std::ffi::OsStr::new).collect();
        let Ok(process) = gio::Subprocess::newv(
            &arguments,
            gio::SubprocessFlags::STDOUT_PIPE | gio::SubprocessFlags::STDERR_SILENCE,
        ) else {
            self.reading.set(false);
            return;
        };
        let page = Rc::downgrade(self);
        let context = context.clone();
        glib::spawn_future_local(async move {
            let output = process.communicate_utf8_future(None).await;
            let Some(page) = page.upgrade() else { return };
            page.reading.set(false);
            page.read.set(Some(Instant::now()));
            let Ok((Some(output), _)) = output else {
                return;
            };
            let mut ended: HashMap<String, Usage> = HashMap::new();
            let mut model = context.model.borrow_mut();
            for line in output.lines() {
                let Some(unit) = json_field(line, "USER_UNIT") else {
                    continue;
                };
                let Some(id) = model::app_id(&unit) else {
                    continue;
                };
                if model.app(&id).is_none() {
                    continue;
                }
                let usage = ended.entry(id).or_default();
                usage.cpu += json_field(line, "CPU_USAGE_NSEC")
                    .and_then(|v| v.parse::<f64>().ok())
                    .unwrap_or(0.0)
                    / 1e9;
                usage.peak = usage.peak.max(
                    json_field(line, "MEMORY_PEAK")
                        .and_then(|v| v.parse::<f64>().ok())
                        .unwrap_or(0.0),
                );
                usage.opened += 1;
            }
            drop(model);
            *page.ended.borrow_mut() = ended;
            page.fill(&context);
        });
    }

    /// Ended apps' use plus what the running ones have used so far.
    fn fill(&self, context: &Rc<Context>) {
        let mut usage = self.ended.borrow().clone();
        let mut model = context.model.borrow_mut();
        let mut names: HashMap<String, (String, Option<gio::Icon>)> = HashMap::new();
        let groups = model.groups.clone();
        for index in 0..groups.n_items() {
            let Some(group) = groups.item(index).and_downcast::<Row>() else {
                continue;
            };
            if group.kind() != APP || group.app_id().is_empty() {
                continue;
            }
            let entry = usage.entry(group.app_id()).or_default();
            entry.opened += 1;
            // The scope's own count includes its processes that have ended
            let stat = std::fs::read_to_string(format!("/sys/fs/cgroup{}/cpu.stat", group.key()))
                .unwrap_or_default();
            let used = stat
                .lines()
                .find_map(|line| {
                    line.strip_prefix("usage_usec ")
                        .and_then(|v| v.trim().parse::<f64>().ok())
                })
                .unwrap_or(0.0)
                / 1e6;
            entry.cpu += used;
            let peak =
                std::fs::read_to_string(format!("/sys/fs/cgroup{}/memory.peak", group.key()))
                    .ok()
                    .and_then(|text| text.trim().parse::<f64>().ok())
                    .unwrap_or(group.memory());
            entry.peak = entry.peak.max(peak);
        }
        for id in usage.keys() {
            if let Some(app) = model.app(id) {
                names.insert(id.clone(), (app.name, app.icon));
            }
        }
        drop(model);

        let mut rows = self.rows.borrow_mut();
        let mut list = Vec::new();
        for (id, usage) in &usage {
            let Some((name, icon)) = names.get(id) else {
                continue;
            };
            let row = rows
                .entry(id.clone())
                .or_insert_with(|| Row::new(id))
                .clone();
            row.put_text("name", name);
            row.put_icon(icon.as_ref());
            row.put_number("cpu", usage.cpu);
            row.put_text("cpu-text", &format::cpu_time(usage.cpu));
            row.put_number("memory", usage.peak);
            row.put_text(
                "memory-text",
                &if usage.peak > 0.0 {
                    format::megabytes(usage.peak)
                } else {
                    String::new()
                },
            );
            row.put_number("extra-value", usage.opened as f64);
            row.put_text("extra", &usage.opened.to_string());
            list.push(row);
        }
        drop(rows);
        row::sync_store(&self.store, &list);
        self.table.resort();
    }
}

/// A string field of one line of journalctl's JSON: `"KEY":"value"`.
/// systemd writes unit names with backslashes (\x2d), escaped in JSON.
fn json_field(line: &str, key: &str) -> Option<String> {
    let start = line.find(&format!("\"{key}\":\""))? + key.len() + 4;
    let rest = &line[start..];
    let mut value = String::new();
    let mut chars = rest.chars();
    while let Some(c) = chars.next() {
        match c {
            '"' => return Some(value),
            '\\' => match chars.next()? {
                'n' => value.push('\n'),
                't' => value.push('\t'),
                other => value.push(other),
            },
            c => value.push(c),
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn journal_lines() {
        let line = r#"{"USER_UNIT":"app-gnome-gnome\\x2dpatrone\\x2dpanel-325676.scope","CPU_USAGE_NSEC":"3877000000","MEMORY_PEAK":"320000000"}"#;
        assert_eq!(
            json_field(line, "USER_UNIT").as_deref(),
            Some(r"app-gnome-gnome\x2dpatrone\x2dpanel-325676.scope")
        );
        assert_eq!(
            json_field(line, "CPU_USAGE_NSEC").as_deref(),
            Some("3877000000")
        );
        assert_eq!(json_field(line, "MISSING"), None);
        assert_eq!(
            model::app_id(&json_field(line, "USER_UNIT").unwrap()).as_deref(),
            Some("gnome-patrone-panel")
        );
    }
}
