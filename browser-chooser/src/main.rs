//! Patrone Browser Chooser. Patrone Linux comes with no web browser, so the
//! first time one's needed, from the taskbar's Web browser button or a link
//! opened in Help, first-login setup or any other app, this asks which one
//! to install. Patrone Software installs it from Flathub; it then becomes
//! the account's default for the web and opens, with the links that were
//! waiting.

mod browsers;

use std::cell::RefCell;
use std::ffi::OsStr;
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{gdk, gio, glib};

use browsers::{BROWSERS, Browser};

const APP_ID: &str = "org.patrone.BrowserChooser";

const USAGE: &str = "\
Usage: patrone-browser-chooser [LINK...]

Asks which web browser to install, installs it through Patrone Software,
makes it the default for web links and opens each LINK with it. While no
browser is installed, links come here (patrone-default-apps).

      --list      Print the browsers it offers, and which are installed
  -h, --help      Show this help
  -V, --version   Show the version
";

struct Chooser {
    window: gtk::ApplicationWindow,
    stack: gtk::Stack,
    /// Which links open once a browser's installed
    link: gtk::Label,
    spinner: gtk::Spinner,
    waiting_title: gtk::Label,
    waiting_text: gtk::Label,
    /// Links waiting for a browser
    uris: RefCell<Vec<String>>,
    /// The browser being installed, and the check for it
    watch: RefCell<Option<(&'static Browser, glib::SourceId)>>,
}

thread_local! {
    /// The open window: a second link while it's open goes to it.
    static CHOOSER: RefCell<Option<Rc<Chooser>>> = const { RefCell::new(None) };
}

fn main() -> glib::ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    if arguments
        .iter()
        .any(|argument| argument == "-h" || argument == "--help")
    {
        print!("{USAGE}");
        return glib::ExitCode::SUCCESS;
    }
    if arguments
        .iter()
        .any(|argument| argument == "-V" || argument == "--version")
    {
        println!("patrone-browser-chooser {}", env!("CARGO_PKG_VERSION"));
        return glib::ExitCode::SUCCESS;
    }
    if arguments.first().map(String::as_str) == Some("--list") {
        list();
        return glib::ExitCode::SUCCESS;
    }

    let app = gtk::Application::builder()
        .application_id(APP_ID)
        .flags(gio::ApplicationFlags::HANDLES_COMMAND_LINE)
        .build();
    app.connect_startup(|_| startup());
    app.connect_command_line(command_line);
    app.run()
}

/// --list: the browsers on offer, and the launcher of each one installed.
fn list() {
    for browser in BROWSERS {
        let installed = browser
            .installed()
            .and_then(|app| app.id())
            .map(|id| id.to_string())
            .unwrap_or_else(|| "-".into());
        println!("{:<16} {:<32} {installed}", browser.name, browser.id);
    }
    for app in browsers::others() {
        println!(
            "{:<16} {:<32} {}",
            app.name(),
            "(not from Flathub)",
            app.id().unwrap_or_default()
        );
    }
}

fn startup() {
    let Some(display) = gdk::Display::default() else {
        return;
    };
    if let Some(settings) = gtk::Settings::default() {
        // GTK's own theme, as in Patrone's other apps: Ubuntu's Yaru
        // doesn't load in plain GTK 4 apps
        settings.set_gtk_theme_name(Some("Default"));
        // GTK 4.22's Default theme stays light when the desktop prefers
        // dark; its dark variant has to be asked for
        #[allow(deprecated)]
        fn follow(settings: &gtk::Settings) {
            let dark = settings.gtk_interface_color_scheme() == gtk::InterfaceColorScheme::Dark;
            settings.set_gtk_application_prefer_dark_theme(dark);
        }
        follow(&settings);
        settings.connect_gtk_interface_color_scheme_notify(follow);
    }
    let css = gtk::CssProvider::new();
    css.load_from_string(include_str!("../data/style.css"));
    gtk::style_context_add_provider_for_display(
        &display,
        &css,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
    gtk::Window::set_default_icon_name(APP_ID);
}

fn command_line(
    app: &gtk::Application,
    command_line: &gio::ApplicationCommandLine,
) -> glib::ExitCode {
    let mut uris = Vec::new();
    for argument in command_line.arguments().iter().skip(1) {
        let text = argument.to_string_lossy();
        if text.starts_with("--") {
            command_line.printerr_literal(&format!("Unknown option {text}\n\n{USAGE}"));
            return glib::ExitCode::FAILURE;
        }
        if glib::Uri::peek_scheme(&text).is_some() {
            uris.push(text.into_owned());
        } else {
            uris.push(command_line.create_file_for_arg(argument).uri().to_string());
        }
    }

    let this = CHOOSER.with_borrow(Clone::clone).unwrap_or_else(|| {
        let this = build(app);
        CHOOSER.with_borrow_mut(|chooser| *chooser = Some(this.clone()));
        this
    });
    this.uris.borrow_mut().extend(uris);
    show_links(&this);
    this.window.present();
    glib::ExitCode::SUCCESS
}

fn label(text: &str, class: &str) -> gtk::Label {
    let label = gtk::Label::builder()
        .label(text)
        .xalign(0.0)
        .wrap(true)
        .build();
    if !class.is_empty() {
        label.add_css_class(class);
    }
    label
}

fn build(app: &gtk::Application) -> Rc<Chooser> {
    let window = gtk::ApplicationWindow::builder()
        .application(app)
        .title("Choose a Web Browser")
        .default_width(560)
        .default_height(700)
        .build();
    // Escape closes it, as it would a dialog
    let keys = gtk::ShortcutController::new();
    keys.add_shortcut(gtk::Shortcut::new(
        gtk::ShortcutTrigger::parse_string("Escape"),
        Some(gtk::NamedAction::new("window.close")),
    ));
    window.add_controller(keys);

    let stack = gtk::Stack::builder()
        .transition_type(gtk::StackTransitionType::Crossfade)
        .build();
    window.set_child(Some(&stack));

    // Choosing
    let page = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(12)
        .margin_top(24)
        .margin_bottom(18)
        .margin_start(24)
        .margin_end(24)
        .build();
    page.append(&label("Choose a web browser", "chooser-title"));
    page.append(&label(
        "Patrone Linux comes without a web browser, so the one you use is up to you. \
         Pick one, and Patrone Software installs it from Flathub and makes it your \
         default browser.",
        "",
    ));
    let link = label("", "chooser-link");
    link.set_visible(false);
    page.append(&link);

    let lists = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(8)
        .build();
    let scrolled = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vexpand(true)
        .child(&lists)
        .build();
    page.append(&scrolled);

    page.append(&label(
        "Installing asks for your password once. Patrone Software keeps the browser \
         up to date.",
        "chooser-dim",
    ));
    let not_now = gtk::Button::with_label("Not now");
    let buttons = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .halign(gtk::Align::End)
        .build();
    buttons.append(&not_now);
    page.append(&buttons);
    stack.add_named(&page, Some("choose"));

    // Waiting for Patrone Software
    let waiting = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(12)
        .valign(gtk::Align::Center)
        .margin_start(36)
        .margin_end(36)
        .build();
    let spinner = gtk::Spinner::builder()
        .spinning(true)
        .width_request(48)
        .height_request(48)
        .halign(gtk::Align::Center)
        .build();
    waiting.append(&spinner);
    let waiting_title = label("", "chooser-title");
    waiting_title.set_xalign(0.5);
    waiting.append(&waiting_title);
    let waiting_text = label("", "");
    waiting_text.set_justify(gtk::Justification::Center);
    waiting_text.set_xalign(0.5);
    waiting.append(&waiting_text);
    let another = gtk::Button::with_label("Choose another browser");
    let close = gtk::Button::with_label("Close");
    let waiting_buttons = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(8)
        .halign(gtk::Align::Center)
        .margin_top(12)
        .build();
    waiting_buttons.append(&another);
    waiting_buttons.append(&close);
    waiting.append(&waiting_buttons);
    stack.add_named(&waiting, Some("waiting"));

    let this = Rc::new(Chooser {
        window,
        stack,
        link,
        spinner,
        waiting_title,
        waiting_text,
        uris: RefCell::default(),
        watch: RefCell::default(),
    });

    let others = browsers::others();
    if !others.is_empty() {
        lists.append(&label("On this computer", "chooser-heading"));
        let list = new_list();
        for app in others {
            let icon = app
                .icon()
                .unwrap_or_else(|| gio::ThemedIcon::new(APP_ID).upcast());
            let button = add_row(
                &list,
                &gtk::Image::from_gicon(&icon),
                &app.name(),
                app.description().as_deref().unwrap_or(""),
                "Use",
            );
            let weak = Rc::downgrade(&this);
            button.connect_clicked(move |_| {
                if let Some(this) = weak.upgrade() {
                    finish(&this, app.upcast_ref());
                }
            });
        }
        lists.append(&list);
        lists.append(&label("Install a browser", "chooser-heading"));
    }
    let list = new_list();
    for browser in BROWSERS {
        let icon = browser.icon().map_or_else(
            || gtk::Image::from_icon_name(APP_ID),
            gtk::Image::from_file,
        );
        let action = if browser.installed().is_some() {
            "Open"
        } else {
            "Install"
        };
        let button = add_row(&list, &icon, browser.name, browser.about, action);
        let weak = Rc::downgrade(&this);
        button.connect_clicked(move |_| {
            if let Some(this) = weak.upgrade() {
                choose(&this, browser);
            }
        });
    }
    lists.append(&list);

    let window = this.window.clone();
    not_now.connect_clicked(move |_| window.close());
    let window = this.window.clone();
    close.connect_clicked(move |_| window.close());
    let weak = Rc::downgrade(&this);
    another.connect_clicked(move |_| {
        if let Some(this) = weak.upgrade() {
            stop_watching(&this);
            this.stack.set_visible_child_name("choose");
        }
    });
    let weak = Rc::downgrade(&this);
    this.window.connect_close_request(move |_| {
        if let Some(this) = weak.upgrade() {
            stop_watching(&this);
        }
        CHOOSER.with_borrow_mut(|chooser| *chooser = None);
        glib::Propagation::Proceed
    });
    this
}

fn new_list() -> gtk::ListBox {
    let list = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .show_separators(true)
        .build();
    list.add_css_class("chooser-list");
    // Activating a row presses its button
    list.connect_row_activated(|_, row| {
        if let Some(button) = row
            .child()
            .and_then(|child| child.last_child())
            .and_then(|last| last.downcast::<gtk::Button>().ok())
        {
            button.emit_clicked();
        }
    });
    list
}

/// A row: the browser's icon, its name over one line about it, and a button.
fn add_row(
    list: &gtk::ListBox,
    icon: &gtk::Image,
    name: &str,
    about: &str,
    action: &str,
) -> gtk::Button {
    icon.set_pixel_size(48);
    let text = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(2)
        .hexpand(true)
        .valign(gtk::Align::Center)
        .build();
    text.append(&label(name, "chooser-name"));
    if !about.is_empty() {
        text.append(&label(about, "chooser-dim"));
    }
    let button = gtk::Button::builder()
        .label(action)
        .valign(gtk::Align::Center)
        .build();
    button.add_css_class("suggested-action");
    let row = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(14)
        .margin_top(10)
        .margin_bottom(10)
        .margin_start(12)
        .margin_end(12)
        .build();
    row.append(icon);
    row.append(&text);
    row.append(&button);
    list.append(&row);
    button
}

/// Says which links open once a browser's installed.
fn show_links(this: &Chooser) {
    let uris = this.uris.borrow();
    let text = match uris.as_slice() {
        [] => String::new(),
        [uri] => format!("Then it opens {}.", browsers::describe(uri)),
        many => format!("Then it opens the {} links you followed.", many.len()),
    };
    this.link.set_visible(!text.is_empty());
    this.link.set_text(&text);
}

/// Opens the browser if it's installed, or has Patrone Software install it.
fn choose(this: &Rc<Chooser>, browser: &'static Browser) {
    if let Some(app) = browser.installed() {
        finish(this, app.upcast_ref());
        return;
    }
    this.spinner.set_visible(true);
    this.waiting_title
        .set_text(&format!("Installing {}", browser.name));
    let opens = match this.uris.borrow().as_slice() {
        [] => String::new(),
        [uri] => format!(", with {}", browsers::describe(uri)),
        _ => ", with the links you followed".into(),
    };
    this.waiting_text.set_text(&format!(
        "Patrone Software is installing {name} from Flathub, and may ask for your \
         password. Once {name} is installed, it becomes your default browser and \
         opens here{opens}.",
        name = browser.name
    ));
    this.stack.set_visible_child_name("waiting");
    watch(this, browser);

    if browsers::catalog_ready() {
        open_in_software(this, browser);
        return;
    }
    // A new system's first sign-in: Patrone Software hasn't fetched
    // Flathub's list of apps yet, and can't find the browser without it
    let this = this.clone();
    glib::spawn_future_local(async move {
        let arguments = ["flatpak", "update", "--system", "--appstream", "flathub"];
        let arguments: Vec<&OsStr> = arguments.iter().map(OsStr::new).collect();
        match gio::Subprocess::newv(&arguments, gio::SubprocessFlags::NONE) {
            Ok(process) => {
                let _ = process.wait_future().await;
            }
            Err(error) => eprintln!("patrone-browser-chooser: flatpak couldn't start: {error}"),
        }
        if watching(&this).is_some_and(|watched| std::ptr::eq(watched, browser)) {
            open_in_software(&this, browser);
        }
    });
}

/// Opens Patrone Software (GNOME Software) on the browser's page, already
/// installing it.
fn open_in_software(this: &Chooser, browser: &Browser) {
    let install = format!("--install={}", browser.data_id());
    let arguments = ["gnome-software", install.as_str(), "--interaction=full"];
    let arguments: Vec<&OsStr> = arguments.iter().map(OsStr::new).collect();
    if let Err(error) = gio::Subprocess::newv(&arguments, gio::SubprocessFlags::NONE) {
        stop_watching(this);
        this.spinner.set_visible(false);
        this.waiting_title.set_text("Patrone Software didn't open");
        this.waiting_text.set_text(&format!(
            "{error}\n\nPatrone Linux's browsers come from Patrone Software. If it's been \
             removed, an administrator can put it back with “sudo apt install patrone-software”."
        ));
    }
}

fn watching(this: &Chooser) -> Option<&'static Browser> {
    this.watch.borrow().as_ref().map(|(browser, _)| *browser)
}

/// Checks every second for the browser's launcher, which Flatpak adds as
/// the install finishes, then opens it.
fn watch(this: &Rc<Chooser>, browser: &'static Browser) {
    stop_watching(this);
    let weak = Rc::downgrade(this);
    let source = glib::timeout_add_seconds_local(1, move || {
        let Some(this) = weak.upgrade() else {
            return glib::ControlFlow::Break;
        };
        let Some(app) = browser.installed() else {
            return glib::ControlFlow::Continue;
        };
        // This check ends here, as it returns
        this.watch.borrow_mut().take();
        finish(&this, app.upcast_ref());
        glib::ControlFlow::Break
    });
    *this.watch.borrow_mut() = Some((browser, source));
}

fn stop_watching(this: &Chooser) {
    if let Some((_, source)) = this.watch.borrow_mut().take() {
        source.remove();
    }
}

/// Makes the browser the default for the web, opens it with the waiting
/// links, and closes.
fn finish(this: &Rc<Chooser>, app: &gio::AppInfo) {
    stop_watching(this);
    if let Err(error) = browsers::make_default(app) {
        eprintln!(
            "patrone-browser-chooser: couldn't make {} the default browser: {error}",
            app.name()
        );
    }
    let uris = this.uris.take();
    let uris: Vec<&str> = uris.iter().map(String::as_str).collect();
    let context = WidgetExt::display(&this.window).app_launch_context();
    let opened = if uris.is_empty() {
        app.launch(&[], Some(&context))
    } else {
        app.launch_uris(&uris, Some(&context))
    };
    match opened {
        Ok(()) => this.window.close(),
        Err(error) => {
            this.spinner.set_visible(false);
            this.waiting_title
                .set_text(&format!("{} didn't open", app.name()));
            this.waiting_text.set_text(&error.to_string());
            this.stack.set_visible_child_name("waiting");
        }
    }
}
