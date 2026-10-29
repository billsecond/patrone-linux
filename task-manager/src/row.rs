//! The rows of Task Manager's lists. One GObject class serves every page
//! (apps, processes, users, services, startup applications, usage history); each
//! column's value is a property the cells bind to, so a row that changes
//! updates its cells without the list being rebuilt.

use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gio, glib};

/// What a row on the Processes and Users pages stands for: its section.
pub const APP: u32 = 0;
pub const BACKGROUND: u32 = 1;
pub const SYSTEM: u32 = 2;

mod imp {
    use std::cell::{Cell, OnceCell, RefCell};

    use gtk::prelude::*;
    use gtk::subclass::prelude::*;
    use gtk::{gio, glib};

    #[derive(glib::Properties, Default)]
    #[properties(wrapper_type = super::Row)]
    pub struct Row {
        /// What identifies it: a PID and start time, a control group, a
        /// unit name, a file name
        #[property(get, set)]
        key: RefCell<String>,
        #[property(get, set)]
        name: RefCell<String>,
        #[property(get, set, nullable)]
        icon: RefCell<Option<gio::Icon>>,
        #[property(get, set)]
        status: RefCell<String>,
        #[property(get, set)]
        cpu: Cell<f64>,
        #[property(get, set)]
        cpu_text: RefCell<String>,
        #[property(get, set)]
        cpu_detail: RefCell<String>,
        #[property(get, set)]
        memory: Cell<f64>,
        #[property(get, set)]
        memory_text: RefCell<String>,
        #[property(get, set)]
        memory_detail: RefCell<String>,
        #[property(get, set)]
        disk: Cell<f64>,
        #[property(get, set)]
        disk_text: RefCell<String>,
        #[property(get, set)]
        network: Cell<f64>,
        #[property(get, set)]
        network_text: RefCell<String>,
        #[property(get, set)]
        pid: Cell<i32>,
        #[property(get, set)]
        pid_text: RefCell<String>,
        #[property(get, set)]
        user: RefCell<String>,
        #[property(get, set)]
        command: RefCell<String>,
        #[property(get, set)]
        description: RefCell<String>,
        /// A page's own column: a service's start, a startup app's package,
        /// how often an app was opened
        #[property(get, set)]
        extra: RefCell<String>,
        #[property(get, set)]
        extra_value: Cell<f64>,
        #[property(get, set)]
        threads: Cell<u32>,
        /// APP, BACKGROUND or SYSTEM
        #[property(get, set)]
        kind: Cell<u32>,
        /// A group with one process, shown without an expander
        #[property(get, set)]
        leaf: Cell<bool>,
        #[property(get, set)]
        enabled: Cell<bool>,
        /// The desktop entry of an app's group (org.patrone.Terminal)
        #[property(get, set)]
        app_id: RefCell<String>,

        /// What's under it in a tree: a group's processes, a user's groups
        pub children: OnceCell<gio::ListStore>,
        /// The processes a group stands for, by PID and start time
        pub pids: RefCell<Vec<(i32, u64)>>,
        pub start: Cell<u64>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Row {
        const NAME: &'static str = "PatroneTaskManagerRow";
        type Type = super::Row;
    }

    #[glib::derived_properties]
    impl ObjectImpl for Row {}
}

glib::wrapper! {
    pub struct Row(ObjectSubclass<imp::Row>);
}

impl Row {
    pub fn new(key: &str) -> Self {
        glib::Object::builder().property("key", key).build()
    }

    /// A row with rows under it.
    pub fn parent(key: &str) -> Self {
        let row = Self::new(key);
        let _ = row.imp().children.set(gio::ListStore::new::<Row>());
        row
    }

    pub fn children(&self) -> Option<gio::ListStore> {
        self.imp().children.get().cloned()
    }

    /// The processes it stands for: a group's, or a process's own.
    pub fn processes(&self) -> Vec<(i32, u64)> {
        let pids = self.imp().pids.borrow();
        if pids.is_empty() && self.pid() > 0 {
            vec![(self.pid(), self.imp().start.get())]
        } else {
            pids.clone()
        }
    }

    pub fn set_processes(&self, pids: Vec<(i32, u64)>) {
        *self.imp().pids.borrow_mut() = pids;
    }

    pub fn start(&self) -> u64 {
        self.imp().start.get()
    }

    pub fn set_start(&self, start: u64) {
        self.imp().start.set(start);
    }

    /// Sets a text property only when it changes, so cells aren't told to
    /// redraw for nothing.
    pub fn put_text(&self, property: &str, value: &str) {
        if self.property::<String>(property) != value {
            self.set_property(property, value);
        }
    }

    pub fn put_number(&self, property: &str, value: f64) {
        if self.property::<f64>(property) != value {
            self.set_property(property, value);
        }
    }

    pub fn put_icon(&self, icon: Option<&gio::Icon>) {
        if self.icon().as_ref() != icon {
            self.set_icon(icon);
        }
    }
}

/// Makes a list hold exactly these rows, keeping the ones it has where they
/// are (so a tree keeps what's expanded, and a list its selection) and
/// adding the new ones at the end.
pub fn sync_store(store: &gio::ListStore, rows: &[Row]) {
    use std::collections::HashSet;
    let wanted: HashSet<_> = rows.iter().map(|row| row.as_ptr()).collect();
    let mut present = HashSet::with_capacity(rows.len());
    let mut index = store.n_items();
    while index > 0 {
        index -= 1;
        let Some(row) = store.item(index).and_downcast::<Row>() else {
            continue;
        };
        if wanted.contains(&row.as_ptr()) {
            present.insert(row.as_ptr());
        } else {
            store.remove(index);
        }
    }
    let added: Vec<Row> = rows
        .iter()
        .filter(|row| !present.contains(&row.as_ptr()))
        .cloned()
        .collect();
    if !added.is_empty() {
        store.splice(store.n_items(), 0, &added);
    }
}
