//! Patrone Task Manager: the apps and processes running and what they use,
//! how hard the computer is working, each app's use since it started, what
//! starts at sign-in, who is signed in, and the system's services. It works
//! the way people who have used Windows expect, in Patrone's own words.
//! Ctrl+Shift+Esc opens it (packaging/patrone-task-manager sets that up).
//!
//!   patrone-task-manager [--page processes|performance|history|startup|users|details|services]

mod actions;
mod desktop;
mod details;
mod format;
mod graph;
mod history;
mod live_style;
mod model;
mod netdiag;
mod performance;
mod processes;
mod procs;
mod row;
mod sample;
mod services;
mod startup;
mod table;
mod users;

use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::{Rc, Weak};
use std::time::Duration;

use gtk::prelude::*;
use gtk::{gdk, gio, glib};

use row::Row;

const APP_ID: &str = "org.patrone.TaskManager";

/// The pages: name, title, icon.
const PAGES: [(&str, &str, &str); 7] = [
    ("processes", "Processes", "view-app-grid-symbolic"),
    (
        "performance",
        "Performance",
        "power-profile-performance-symbolic",
    ),
    ("history", "Usage history", "document-open-recent-symbolic"),
    ("startup", "Startup applications", "system-run-symbolic"),
    ("users", "Users", "system-users-symbolic"),
    ("details", "Details", "view-list-symbolic"),
    ("services", "Services", "emblem-system-symbolic"),
];

/// How often it updates: setting, name, milliseconds (0 is paused).
const SPEEDS: [(&str, &str, u64); 4] = [
    ("high", "High (twice a second)", 500),
    ("normal", "Normal (once a second)", 1000),
    ("low", "Low (every 4 seconds)", 4000),
    ("paused", "Paused", 0),
];

/// What the pages share: the window, the model of what's running, the
/// search, and the row a menu was opened on.
pub struct Context {
    pub window: gtk::ApplicationWindow,
    pub model: RefCell<model::Model>,
    pub query: Rc<RefCell<String>>,
    pub counts: Rc<Cell<[u32; 3]>>,
    pub target: RefCell<Option<Row>>,
    pub ticks_per_second: f64,
}

/// What a window action does, and one that's given a word (a priority, a
/// service command).
type Handler = Box<dyn Fn(&Rc<Window>)>;
type TextHandler = Box<dyn Fn(&Rc<Window>, &str)>;

struct Window {
    context: Rc<Context>,
    stack: gtk::Stack,
    nav: gtk::ListBox,
    title: gtk::Label,
    toolbars: gtk::Stack,
    search: gtk::SearchEntry,
    sampler: RefCell<sample::Sampler>,
    scanner: RefCell<procs::Scanner>,
    cpus: usize,
    processes: processes::Page,
    details: details::Page,
    performance: Rc<performance::Page>,
    history: Rc<history::Page>,
    startup: Rc<startup::Page>,
    users: Rc<users::Page>,
    services: Rc<services::Page>,
    timer: RefCell<Option<glib::SourceId>>,
    speed: RefCell<String>,
}

thread_local! {
    static WINDOW: RefCell<Option<Rc<Window>>> = const { RefCell::new(None) };
}

fn main() -> glib::ExitCode {
    let arguments: Vec<String> = std::env::args().collect();
    if arguments
        .iter()
        .any(|argument| argument == "--help" || argument == "-h")
    {
        println!(
            "Usage: patrone-task-manager [--page processes|performance|history|startup|users|details|services]"
        );
        return glib::ExitCode::SUCCESS;
    }
    let app = gtk::Application::builder()
        .application_id(APP_ID)
        .flags(gio::ApplicationFlags::HANDLES_COMMAND_LINE)
        .build();
    app.connect_startup(|app| {
        load_style();
        app.set_accels_for_action("win.run-task", &["<Control>n"]);
        app.set_accels_for_action("win.search", &["<Control>f"]);
    });
    app.connect_command_line(|app, command_line| {
        let arguments: Vec<String> = command_line
            .arguments()
            .iter()
            .map(|argument| argument.to_string_lossy().into_owned())
            .collect();
        present(app, option_value(&arguments, "--page").as_deref());
        glib::ExitCode::SUCCESS
    });
    app.run()
}

/// The value of an option given as `NAME VALUE` or `NAME=VALUE`.
fn option_value(arguments: &[String], name: &str) -> Option<String> {
    let mut iter = arguments.iter().skip(1);
    while let Some(argument) = iter.next() {
        if let Some(value) = argument
            .strip_prefix(name)
            .and_then(|rest| rest.strip_prefix('='))
        {
            return Some(value.to_string());
        }
        if argument == name {
            return iter.next().cloned();
        }
    }
    None
}

fn load_style() {
    let provider = gtk::CssProvider::new();
    provider.load_from_string(include_str!("../data/style.css"));
    if let Some(display) = gdk::Display::default() {
        gtk::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
        live_style::follow(&display);
    }
}

/// Ctrl+Shift+Esc while it's open brings the window forward.
fn present(app: &gtk::Application, page: Option<&str>) {
    let window = WINDOW
        .with(|cell| cell.borrow().clone())
        .unwrap_or_else(|| {
            let window = Window::build(app);
            WINDOW.with(|cell| *cell.borrow_mut() = Some(window.clone()));
            window
        });
    if let Some(page) = page {
        window.show_page(page);
    }
    window.context.window.present();
}

/// A command in a page's bar: an icon and a label.
pub fn command(icon: &str, label: &str) -> gtk::Button {
    let content = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    content.append(&gtk::Image::from_icon_name(icon));
    content.append(&gtk::Label::new(Some(label)));
    gtk::Button::builder()
        .child(&content)
        .css_classes(["tm-command"])
        .build()
}

/// Whether a row matches the search: its name, command line or PID.
pub fn matcher(query: Rc<RefCell<String>>) -> impl Fn(&Row) -> bool + 'static {
    move |row| {
        let query = query.borrow();
        query.is_empty()
            || row.name().to_lowercase().contains(query.as_str())
            || row.command().to_lowercase().contains(query.as_str())
            || row.description().to_lowercase().contains(query.as_str())
            || row.pid_text() == *query
    }
}

/// The window's size, page and update speed, kept between runs.
struct State {
    width: i32,
    height: i32,
    maximized: bool,
    page: String,
    speed: String,
}

fn state_file() -> PathBuf {
    glib::user_config_dir()
        .join("patrone-task-manager")
        .join("state.ini")
}

impl State {
    fn load() -> Self {
        let file = glib::KeyFile::new();
        let _ = file.load_from_file(state_file(), glib::KeyFileFlags::NONE);
        State {
            width: file.integer("window", "width").unwrap_or(1180).max(600),
            height: file.integer("window", "height").unwrap_or(740).max(400),
            maximized: file.boolean("window", "maximized").unwrap_or(false),
            page: file
                .string("window", "page")
                .map(|page| page.to_string())
                .unwrap_or_else(|_| "processes".into()),
            speed: file
                .string("window", "speed")
                .map(|speed| speed.to_string())
                .unwrap_or_else(|_| "normal".into()),
        }
    }

    fn save(&self) {
        let file = glib::KeyFile::new();
        file.set_integer("window", "width", self.width);
        file.set_integer("window", "height", self.height);
        file.set_boolean("window", "maximized", self.maximized);
        file.set_string("window", "page", &self.page);
        file.set_string("window", "speed", &self.speed);
        let path = state_file();
        if let Some(folder) = path.parent() {
            let _ = std::fs::create_dir_all(folder);
        }
        let _ = file.save_to_file(path);
    }
}

impl Window {
    fn build(app: &gtk::Application) -> Rc<Self> {
        let state = State::load();
        let window = gtk::ApplicationWindow::builder()
            .application(app)
            .title("Task Manager")
            .icon_name(APP_ID)
            .default_width(state.width)
            .default_height(state.height)
            .build();
        if state.maximized {
            window.maximize();
        }
        let scanner = procs::Scanner::new();
        let context = Rc::new(Context {
            window: window.clone(),
            model: RefCell::new(model::Model::new(scanner.my_uid)),
            query: Rc::default(),
            counts: Rc::default(),
            target: RefCell::new(None),
            ticks_per_second: scanner.ticks_per_second(),
        });

        // The title bar: the app's name, the search, the menu
        let header = gtk::HeaderBar::new();
        let name = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        name.append(
            &gtk::Image::builder()
                .icon_name(APP_ID)
                .pixel_size(18)
                .build(),
        );
        name.append(
            &gtk::Label::builder()
                .label("Task Manager")
                .css_classes(["tm-app-title"])
                .build(),
        );
        header.pack_start(&name);
        let search = gtk::SearchEntry::builder()
            .placeholder_text("Type a name, command or PID to search")
            .width_chars(40)
            .build();
        header.set_title_widget(Some(&search));
        let menu = gio::Menu::new();
        let speeds = gio::Menu::new();
        for (id, label, _) in SPEEDS {
            speeds.append(Some(label), Some(&format!("win.speed::{id}")));
        }
        menu.append_submenu(Some("Update speed"), &speeds);
        let about = gio::Menu::new();
        about.append(Some("About Task Manager"), Some("win.about"));
        menu.append_section(None, &about);
        header.pack_end(
            &gtk::MenuButton::builder()
                .icon_name("open-menu-symbolic")
                .menu_model(&menu)
                .tooltip_text("Menu")
                .build(),
        );
        window.set_titlebar(Some(&header));

        let processes = processes::Page::new(&context);
        let details = details::Page::new(&context);
        let performance = performance::Page::new();
        let history = history::Page::new(&context);
        let startup = startup::Page::new(&context);
        let users = users::Page::new(&context);
        let services = services::Page::new(&context);

        let stack = gtk::Stack::builder()
            .transition_type(gtk::StackTransitionType::Crossfade)
            .hexpand(true)
            .vexpand(true)
            .build();
        let toolbars = gtk::Stack::builder()
            .hhomogeneous(false)
            .halign(gtk::Align::End)
            .build();
        let no_commands = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        let pages: [(&str, gtk::Widget, &gtk::Box); 7] = [
            (
                "processes",
                processes.table.widget.clone().upcast(),
                &processes.toolbar,
            ),
            (
                "performance",
                performance.widget.clone().upcast(),
                &no_commands,
            ),
            ("history", history.widget.clone().upcast(), &history.toolbar),
            ("startup", startup.widget.clone().upcast(), &startup.toolbar),
            ("users", users.table.widget.clone().upcast(), &users.toolbar),
            (
                "details",
                details.table.widget.clone().upcast(),
                &details.toolbar,
            ),
            (
                "services",
                services.table.widget.clone().upcast(),
                &services.toolbar,
            ),
        ];
        for (name, widget, toolbar) in pages {
            stack.add_named(&widget, Some(name));
            let bar = gtk::Box::new(gtk::Orientation::Horizontal, 4);
            let run = command("list-add-symbolic", "Run a program");
            run.set_action_name(Some("win.run-task"));
            bar.append(&run);
            bar.append(toolbar);
            toolbars.add_named(&bar, Some(name));
        }

        let nav = gtk::ListBox::builder().css_classes(["tm-nav"]).build();
        for (name, title, icon) in PAGES {
            let content = gtk::Box::new(gtk::Orientation::Horizontal, 12);
            content.append(&gtk::Image::from_icon_name(icon));
            content.append(&gtk::Label::new(Some(title)));
            let row = gtk::ListBoxRow::builder()
                .child(&content)
                .tooltip_text(title)
                .build();
            row.set_widget_name(name);
            nav.append(&row);
        }
        let side = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .width_request(200)
            .css_classes(["tm-side"])
            .build();
        side.append(&nav);

        let title = gtk::Label::builder()
            .xalign(0.0)
            .hexpand(true)
            .css_classes(["tm-page-title"])
            .build();
        let top = gtk::Box::builder()
            .spacing(12)
            .css_classes(["tm-top"])
            .build();
        top.append(&title);
        top.append(&toolbars);
        let content = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .css_classes(["tm-content"])
            .build();
        content.append(&top);
        content.append(&stack);
        let layout = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        layout.append(&side);
        layout.append(&content);
        window.set_child(Some(&layout));

        let cpus = sample::CpuInfo::read().logical;
        let this = Rc::new(Window {
            context,
            stack,
            nav,
            title,
            toolbars,
            search,
            sampler: RefCell::new(sample::Sampler::new()),
            scanner: RefCell::new(scanner),
            cpus,
            processes,
            details,
            performance,
            history,
            startup,
            users,
            services,
            timer: RefCell::new(None),
            speed: RefCell::new(state.speed.clone()),
        });

        let weak = Rc::downgrade(&this);
        this.nav.connect_row_selected(move |_, row| {
            let (Some(this), Some(row)) = (weak.upgrade(), row) else {
                return;
            };
            this.page_changed(&row.widget_name());
        });
        let weak = Rc::downgrade(&this);
        this.search.connect_search_changed(move |search| {
            let Some(this) = weak.upgrade() else { return };
            *this.context.query.borrow_mut() = search.text().trim().to_lowercase();
            for table in [
                &this.processes.table,
                &this.details.table,
                &this.users.table,
                &this.services.table,
                &this.startup.table,
                &this.history.table,
            ] {
                table.refilter();
            }
        });
        this.add_actions();
        let weak = Rc::downgrade(&this);
        this.context.window.connect_close_request(move |window| {
            if let Some(this) = weak.upgrade() {
                let (width, height) = window.default_size();
                State {
                    width,
                    height,
                    maximized: window.is_maximized(),
                    page: this
                        .stack
                        .visible_child_name()
                        .map(|name| name.to_string())
                        .unwrap_or_default(),
                    speed: this.speed.borrow().clone(),
                }
                .save();
            }
            glib::Propagation::Proceed
        });

        // A first reading, so nothing starts empty; the second, a moment
        // later, has the first rates
        this.tick();
        let weak = Rc::downgrade(&this);
        glib::timeout_add_local_once(Duration::from_millis(250), move || {
            if let Some(this) = weak.upgrade() {
                this.tick();
                this.restart_timer();
            }
        });
        this.show_page(if PAGES.iter().any(|(name, _, _)| *name == state.page) {
            &state.page
        } else {
            "processes"
        });
        this
    }

    fn show_page(&self, name: &str) {
        let mut index = 0;
        while let Some(row) = self.nav.row_at_index(index) {
            if row.widget_name() == name {
                self.nav.select_row(Some(&row));
                return;
            }
            index += 1;
        }
    }

    fn page_changed(&self, name: &str) {
        self.stack.set_visible_child_name(name);
        self.toolbars.set_visible_child_name(name);
        let title = PAGES
            .iter()
            .find(|(page, _, _)| *page == name)
            .map(|(_, title, _)| *title)
            .unwrap_or("");
        self.title.set_label(title);
        // Pages that read or sort only while shown catch up at once
        if name == "details" {
            self.details.table.resort();
        }
        self.users.update(&self.context, name == "users");
        self.services.update(&self.context, name == "services");
        self.history.update(&self.context, name == "history");
    }

    fn restart_timer(self: &Rc<Self>) {
        if let Some(source) = self.timer.borrow_mut().take() {
            source.remove();
        }
        let speed = self.speed.borrow().clone();
        let milliseconds = SPEEDS
            .iter()
            .find(|(id, _, _)| *id == speed)
            .map(|(_, _, ms)| *ms)
            .unwrap_or(1000);
        if milliseconds == 0 {
            return;
        }
        let weak: Weak<Window> = Rc::downgrade(self);
        let source = glib::timeout_add_local(Duration::from_millis(milliseconds), move || {
            let Some(this) = weak.upgrade() else {
                return glib::ControlFlow::Break;
            };
            this.tick();
            glib::ControlFlow::Continue
        });
        *self.timer.borrow_mut() = Some(source);
    }

    /// Reads everything again and updates the pages.
    fn tick(&self) {
        let sample = self.sampler.borrow_mut().sample();
        let list = self.scanner.borrow_mut().scan(self.cpus);
        {
            let mut model = self.context.model.borrow_mut();
            model.update(list);
            self.context.counts.set(model.counts);
        }
        let totals = self.context.model.borrow().totals;
        let shown = self
            .stack
            .visible_child_name()
            .map(|name| name.to_string())
            .unwrap_or_default();
        self.processes.update(&sample);
        if shown == "details" {
            self.details.table.resort();
        }
        self.performance.update(&sample, &totals);
        self.users.update(&self.context, shown == "users");
        self.services.update(&self.context, shown == "services");
        self.history.update(&self.context, shown == "history");
    }

    fn target(&self) -> Option<Row> {
        self.context.target.borrow().clone()
    }

    /// The menus' and buttons' actions. Process actions work on the row a
    /// menu was opened on.
    fn add_actions(self: &Rc<Self>) {
        let window = &self.context.window;
        let simple = |name: &str, handler: Handler| {
            let action = gio::SimpleAction::new(name, None);
            let weak = Rc::downgrade(self);
            action.connect_activate(move |_, _| {
                if let Some(this) = weak.upgrade() {
                    handler(&this);
                }
            });
            window.add_action(&action);
        };
        simple(
            "end-task",
            Box::new(|this| {
                if let Some(row) = this.target() {
                    actions::end_task(&this.context, &row, false);
                }
            }),
        );
        simple(
            "end-tree",
            Box::new(|this| {
                if let Some(row) = this.target() {
                    actions::end_task(&this.context, &row, true);
                }
            }),
        );
        simple(
            "open-location",
            Box::new(|this| {
                if let Some(row) = this.target() {
                    actions::open_location(&this.context, &row);
                }
            }),
        );
        simple(
            "process-properties",
            Box::new(|this| {
                if let Some(row) = this.target() {
                    actions::properties(&this.context, &row);
                }
            }),
        );
        simple(
            "go-to-details",
            Box::new(|this| {
                let Some(row) = this.target() else { return };
                if row.pid() > 0 {
                    this.search.set_text("");
                    this.show_page("details");
                    this.details.select(row.pid());
                }
            }),
        );
        simple(
            "run-task",
            Box::new(|this| actions::run_new_task(&this.context)),
        );
        simple(
            "sign-out",
            Box::new(|this| {
                if let Some(row) = this.target() {
                    users::end(&this.context, &row, users::Ending::SignOut);
                }
            }),
        );
        simple(
            "disconnect",
            Box::new(|this| {
                if let Some(row) = this.target() {
                    users::end(&this.context, &row, users::Ending::Disconnect);
                }
            }),
        );
        simple(
            "search",
            Box::new(|this| {
                this.search.grab_focus();
            }),
        );
        simple(
            "about",
            Box::new(|this| {
                gtk::AboutDialog::builder()
                .transient_for(&this.context.window)
                .modal(true)
                .program_name("Task Manager")
                .logo_icon_name(APP_ID)
                .version(env!("CARGO_PKG_VERSION"))
                .comments("The apps and processes running, how hard the computer is working, what starts when you sign in, who is signed in, and the system's services. Open it with Ctrl+Shift+Esc.")
                .copyright("© 2026 William Daugherty")
                .license_type(gtk::License::Gpl30)
                .build()
                .present();
            }),
        );

        let with_text = |name: &str, handler: TextHandler| {
            let action = gio::SimpleAction::new(name, Some(glib::VariantTy::STRING));
            let weak = Rc::downgrade(self);
            action.connect_activate(move |_, parameter| {
                let (Some(this), Some(value)) = (
                    weak.upgrade(),
                    parameter.and_then(|p| p.str().map(str::to_string)),
                ) else {
                    return;
                };
                handler(&this, &value);
            });
            window.add_action(&action);
        };
        with_text(
            "priority",
            Box::new(|this, id| {
                let Some(row) = this.target() else { return };
                if let Some(&(_, _, nice)) =
                    actions::PRIORITIES.iter().find(|(name, _, _)| *name == id)
                {
                    actions::set_priority(&this.context, &row, nice);
                }
            }),
        );
        with_text(
            "service",
            Box::new(|this, method| {
                let Some(row) = this.target() else { return };
                let method = match method {
                    "StartUnit" => "StartUnit",
                    "StopUnit" => "StopUnit",
                    _ => "RestartUnit",
                };
                this.services.control(&this.context, &row, method);
            }),
        );
        with_text(
            "startup",
            Box::new(|this, what| {
                let Some(row) = this.target() else { return };
                match what {
                    "on" => this.startup.set_enabled(&this.context, &row, true),
                    "off" => this.startup.set_enabled(&this.context, &row, false),
                    _ => startup::show_file(&row.description()),
                }
            }),
        );

        let speed = gio::SimpleAction::new_stateful(
            "speed",
            Some(glib::VariantTy::STRING),
            &self.speed.borrow().to_variant(),
        );
        let weak = Rc::downgrade(self);
        speed.connect_activate(move |action, parameter| {
            let (Some(this), Some(value)) = (
                weak.upgrade(),
                parameter.and_then(|p| p.str().map(str::to_string)),
            ) else {
                return;
            };
            action.set_state(&value.to_variant());
            *this.speed.borrow_mut() = value;
            this.restart_timer();
        });
        window.add_action(&speed);
    }
}
