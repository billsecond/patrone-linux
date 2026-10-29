//! Patrone Virtual Machine Setup. At each desktop login in a Hyper-V virtual
//! machine (patrone-hyperv-setup.service), this checks what Patrone Linux
//! needs there and offers to set up whatever isn't: Hyper-V's integration
//! services (patrone-guest-tools) and VMConnect's enhanced session
//! (patrone-remote-desktop), with one password, and the PowerShell command
//! for what has to change on Windows. With everything set up, or "Don't
//! show this again" ticked, it shows nothing.
//!
//! Microsoft's trademark guidelines bar its marks, Hyper-V among them, from
//! product names (docs/COMPLIANCE.md, item 15), so the name the user sees
//! is "Virtual Machine Setup". The package, binary, units and app ID keep
//! "hyperv": renaming those would need a migration.

mod checks;

use std::cell::RefCell;
use std::ffi::OsStr;
use std::fs;
use std::path::Path;
use std::process::Command;
use std::rc::Rc;
use std::time::{Duration, Instant};

use gtk::prelude::*;
use gtk::{gdk, gio, glib};

use checks::{Session, Status};

const APP_ID: &str = "org.patrone.HyperVSetup";

/// Sets up what's missing, as root, through pkexec
/// (org.patrone.hyperv-setup.policy)
const HELPER: &str = "/usr/libexec/patrone-hyperv-setup/apply";

/// How long, at login, to give Hyper-V to open an enhanced session before
/// saying the VM isn't set up for one: on the dev VM, it did 26 seconds
/// after the service for it started
const HOST_GRACE: Duration = Duration::from_secs(30);

/// What --pretend-missing takes
const PARTS: [&str; 4] = ["guest-tools", "enhanced-session", "transport", "services"];

/// Shown wherever the dialog names them
const TRADEMARKS: &str = "Hyper-V and Windows are trademarks of the Microsoft group of companies.";

const USAGE: &str = "\
Usage: patrone-hyperv-setup [OPTION...]

Virtual Machine Setup: shows what Patrone Linux needs in a Hyper-V virtual
machine, and sets up what's missing. It runs at each desktop login with
--at-login.

      --at-login              Show only if something's missing, and not in
                              the installer's live session or with \"Don't
                              show this again\" ticked
      --status                Print what's set up and what isn't
      --pretend-missing=LIST  Take these as missing, for testing:
                              guest-tools, enhanced-session, transport,
                              services (comma-separated)
      --dry-run               Set nothing up: say what would be done
  -h, --help                  Show this help
  -V, --version               Show the version
";

#[derive(Default)]
struct Options {
    at_login: bool,
    pretend: Vec<String>,
    dry_run: bool,
}

struct Setup {
    window: gtk::ApplicationWindow,
    stack: gtk::Stack,
    /// "Don't show this again", saved as the window closes
    hide: gtk::CheckButton,
    options: Rc<Options>,
    status: RefCell<Status>,
}

thread_local! {
    /// The open window's state, which its buttons reach through weak
    /// references, until it closes
    static SETUP: RefCell<Option<Rc<Setup>>> = const { RefCell::new(None) };
}

fn main() -> glib::ExitCode {
    let mut options = Options::default();
    let mut print = false;
    for argument in std::env::args().skip(1) {
        match argument.as_str() {
            "-h" | "--help" => {
                print!("{USAGE}");
                return glib::ExitCode::SUCCESS;
            }
            "-V" | "--version" => {
                println!("patrone-hyperv-setup {}", env!("CARGO_PKG_VERSION"));
                return glib::ExitCode::SUCCESS;
            }
            "--at-login" => options.at_login = true,
            "--status" => print = true,
            "--dry-run" => options.dry_run = true,
            _ => {
                let list = argument.strip_prefix("--pretend-missing=");
                let parts: Vec<&str> = list.unwrap_or_default().split(',').collect();
                if list.is_none() || parts.iter().any(|part| !PARTS.contains(part)) {
                    eprint!("Unknown option {argument}\n\n{USAGE}");
                    return glib::ExitCode::FAILURE;
                }
                options.pretend.extend(parts.into_iter().map(String::from));
            }
        }
    }

    let status = Status::check(&options.pretend);
    if print {
        print_status(&status);
        return glib::ExitCode::SUCCESS;
    }
    // Most logins end here, before GTK starts
    if options.at_login && !wanted_at_login(&status) {
        return glib::ExitCode::SUCCESS;
    }

    let options = Rc::new(options);
    let app = gtk::Application::builder().application_id(APP_ID).build();
    app.connect_startup(|_| startup());
    app.connect_activate(move |app| activate(app, &options));
    app.run_with_args(&[] as &[&str])
}

fn wanted_at_login(status: &Status) -> bool {
    status.hyper_v && !status.live && !status.set_up() && !checks::dont_show_file().exists()
}

/// --status: a line each, NAME=VALUE
fn print_status(status: &Status) {
    let yes_no = |value: bool| if value { "yes" } else { "no" };
    println!("hyper-v={}", yes_no(status.hyper_v));
    println!("live-session={}", yes_no(status.live));
    println!("guest-tools={}", yes_no(status.guest_tools));
    let session = match status.session {
        Session::On => "on",
        Session::Off => "off",
        Session::Old => "old",
        Session::Missing => "missing",
    };
    println!("enhanced-session={session}");
    println!("transport={}", yes_no(status.transport));
    println!("services-off={}", status.services_off.join(","));
    println!("vm-name={}", status.vm_name.as_deref().unwrap_or(""));
    println!("set-up={}", yes_no(status.set_up()));
    println!("to-set-up={}", status.to_set_up().join(","));
    println!(
        "windows={}",
        status
            .windows_command()
            .unwrap_or_default()
            .replace('\n', "; ")
    );
    println!("dont-show={}", yes_no(checks::dont_show_file().exists()));
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

fn activate(app: &gtk::Application, options: &Rc<Options>) {
    if let Some(window) = app.active_window() {
        window.present();
        return;
    }
    let started = Instant::now();
    let waiting = move |options: &Options| {
        // A new account's first login: first-login setup comes first
        initial_setup_running()
            // Just after the VM starts, Hyper-V may not have opened its
            // first enhanced session yet, which it does when the VM is set
            // up for one
            || (!Path::new(checks::SEEN).exists()
                && !options.pretend.iter().any(|part| part == "transport")
                && started.elapsed() < HOST_GRACE)
    };
    if options.at_login && waiting(options.as_ref()) {
        let hold = app.hold();
        let app = app.clone();
        let options = options.clone();
        glib::timeout_add_seconds_local(2, move || {
            // The app stays up while this waits, and no longer
            let _hold = &hold;
            if waiting(options.as_ref()) {
                return glib::ControlFlow::Continue;
            }
            show(&app, &options);
            glib::ControlFlow::Break
        });
        return;
    }
    show(app, options);
}

/// First-login setup (GNOME Initial Setup), which the login unit starts
/// after
fn initial_setup_running() -> bool {
    !checks::config_dir()
        .join("gnome-initial-setup-done")
        .exists()
        && Command::new("systemctl")
            .args([
                "--user",
                "is-active",
                "--quiet",
                "gnome-initial-setup-first-login.service",
            ])
            .status()
            .is_ok_and(|status| status.success())
}

fn show(app: &gtk::Application, options: &Rc<Options>) {
    let status = Status::check(&options.pretend);
    if options.at_login && !wanted_at_login(&status) {
        return;
    }
    let this = build(app, options, status);
    show_check(&this);
    this.window.present();
    SETUP.with_borrow_mut(|setup| *setup = Some(this));
}

fn build(app: &gtk::Application, options: &Rc<Options>, status: Status) -> Rc<Setup> {
    let window = gtk::ApplicationWindow::builder()
        .application(app)
        .title("Virtual Machine Setup")
        .resizable(false)
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
        .vhomogeneous(false)
        .hhomogeneous(false)
        .interpolate_size(true)
        .build();
    window.set_child(Some(&stack));

    let hide = gtk::CheckButton::builder()
        .label("Don't show this again")
        .tooltip_text("To see it again, run patrone-hyperv-setup in a terminal.")
        .active(checks::dont_show_file().exists())
        .hexpand(true)
        .valign(gtk::Align::Center)
        .build();

    let this = Rc::new(Setup {
        window,
        stack,
        hide,
        options: options.clone(),
        status: RefCell::new(status),
    });
    let weak = Rc::downgrade(&this);
    this.window.connect_close_request(move |_| {
        if let Some(this) = weak.upgrade() {
            save_dont_show(this.hide.is_active());
        }
        SETUP.with_borrow_mut(|setup| *setup = None);
        glib::Propagation::Proceed
    });
    this
}

fn label(text: &str, class: &str) -> gtk::Label {
    let label = gtk::Label::builder()
        .label(text)
        .xalign(0.0)
        .wrap(true)
        .max_width_chars(62)
        .build();
    if !class.is_empty() {
        label.add_css_class(class);
    }
    label
}

fn button(text: &str, suggested: bool) -> gtk::Button {
    let button = gtk::Button::with_label(text);
    if suggested {
        button.add_css_class("suggested-action");
    }
    button
}

fn page() -> gtk::Box {
    gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(10)
        .margin_top(18)
        .margin_bottom(16)
        .margin_start(22)
        .margin_end(22)
        .build()
}

/// The buttons along the bottom, at the right, with "Don't show this
/// again" at the left on the first page
fn bottom(this: &Setup, with_hide: bool, buttons: &[&gtk::Button]) -> gtk::Box {
    let row = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(8)
        .margin_top(6)
        .build();
    if with_hide {
        if let Some(parent) = this.hide.parent().and_downcast::<gtk::Box>() {
            parent.remove(&this.hide);
        }
        row.append(&this.hide);
    } else {
        row.append(&gtk::Box::builder().hexpand(true).build());
    }
    for button in buttons {
        row.append(*button);
    }
    row
}

fn put_page(this: &Setup, name: &str, page: &gtk::Box) {
    if let Some(old) = this.stack.child_by_name(name) {
        this.stack.remove(&old);
    }
    this.stack.add_named(page, Some(name));
    this.stack.set_visible_child_name(name);
}

/// A row: an icon, a name over a line or two about it, and where it stands.
fn add_row(list: &gtk::ListBox, icon: &str, name: &str, about: &str, state: &str, done: bool) {
    let image = gtk::Image::from_icon_name(icon);
    image.set_pixel_size(32);
    image.set_valign(gtk::Align::Center);
    let text = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(2)
        .hexpand(true)
        .valign(gtk::Align::Center)
        .build();
    text.append(&label(name, "setup-name"));
    let about = label(about, "setup-dim");
    about.set_max_width_chars(46);
    text.append(&about);
    let stands = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(4)
        .valign(gtk::Align::Center)
        .build();
    if done {
        stands.append(&gtk::Image::from_icon_name("object-select-symbolic"));
    }
    let state = label(state, if done { "setup-done" } else { "setup-todo" });
    state.set_wrap(false);
    stands.append(&state);
    let row = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(14)
        .margin_top(8)
        .margin_bottom(8)
        .margin_start(12)
        .margin_end(12)
        .build();
    row.append(&image);
    row.append(&text);
    row.append(&stands);
    list.append(&row);
}

/// The first page: each part, where it stands, and what Windows needs.
fn show_check(this: &Rc<Setup>) {
    let status = this.status.borrow().clone();
    let page = page();
    if !status.hyper_v {
        page.append(&label(
            "This isn't a Hyper-V virtual machine",
            "setup-title",
        ));
        page.append(&label(
            "Patrone Linux isn't running in Hyper-V here, so there's nothing to set up.",
            "",
        ));
        page.append(&label(TRADEMARKS, "setup-legal"));
        let close = button("Close", true);
        let window = this.window.clone();
        close.connect_clicked(move |_| window.close());
        page.append(&bottom(this, false, &[&close]));
        put_page(this, "check", &page);
        return;
    }

    if status.set_up() {
        page.append(&label("This virtual machine is set up", "setup-title"));
        page.append(&label(
            "Patrone Linux is running in a Hyper-V virtual machine, and everything it \
             needs there is set up.",
            "",
        ));
    } else {
        page.append(&label("Set up this virtual machine", "setup-title"));
        page.append(&label(
            "Patrone Linux is running in a Hyper-V virtual machine, and some of what it \
             needs there isn't set up yet.",
            "",
        ));
    }

    let list = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .show_separators(true)
        .build();
    list.add_css_class("setup-list");
    let (state, done) = if status.guest_tools {
        ("Installed", true)
    } else {
        ("Not installed", false)
    };
    add_row(
        &list,
        "computer-symbolic",
        "Integration services",
        "Shutting down and backing up from Windows, the clock, and copying files in.",
        state,
        done,
    );
    let (state, done) = match status.session {
        Session::On => ("On", true),
        Session::Off => ("Off", false),
        Session::Old => ("Needs an update", false),
        Session::Missing => ("Not installed", false),
    };
    add_row(
        &list,
        "preferences-desktop-remote-desktop-symbolic",
        "Enhanced session",
        "Patrone's login screen and your desktop in VMConnect, with copy and paste, \
         sound and resizing.",
        state,
        done,
    );
    let command = status.windows_command();
    let (state, done) = if command.is_some() {
        ("Needs a change", false)
    } else {
        ("Set", true)
    };
    add_row(
        &list,
        "emblem-system-symbolic",
        "This VM's settings in Hyper-V",
        &windows_about(&status),
        state,
        done,
    );
    page.append(&list);

    if let Some(command) = command {
        page.append(&windows_box(&status, &command));
    }

    let parts = status.to_set_up();
    if parts.is_empty() {
        page.append(&label(TRADEMARKS, "setup-legal"));
        let close = button("Close", true);
        let window = this.window.clone();
        close.connect_clicked(move |_| window.close());
        page.append(&bottom(this, true, &[&close]));
    } else {
        page.append(&label(
            "Setting up asks for your password once.",
            "setup-dim",
        ));
        page.append(&label(TRADEMARKS, "setup-legal"));
        let not_now = button("Not Now", false);
        let set_up = button("Set Up", true);
        let window = this.window.clone();
        not_now.connect_clicked(move |_| window.close());
        let weak = Rc::downgrade(this);
        set_up.connect_clicked(move |_| {
            if let Some(this) = weak.upgrade() {
                set_up_parts(&this);
            }
        });
        page.append(&bottom(this, true, &[&not_now, &set_up]));
        this.window.set_default_widget(Some(&set_up));
    }
    put_page(this, "check", &page);
}

/// What Windows' side lacks, or that it has it all
fn windows_about(status: &Status) -> String {
    let mut about = Vec::new();
    if !status.transport {
        about.push(
            "Hyper-V hasn't opened an enhanced session yet: Linux needs the Hyper-V \
             socket transport."
                .to_string(),
        );
    }
    if !status.services_off.is_empty() {
        about.push(format!("Turned off: {}.", status.services_off.join(", ")));
    }
    if about.is_empty() {
        about.push(
            "Hyper-V gives this VM the enhanced session and every integration service.".to_string(),
        );
    }
    about.join(" ")
}

/// What to run on Windows, with a Copy button
fn windows_box(status: &Status, command: &str) -> gtk::Box {
    let section = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(6)
        .margin_top(4)
        .build();
    section.append(&label("On Windows", "setup-heading"));
    section.append(&label(
        if status.windows_needs_shutdown() {
            "Shut down this VM, then run this in PowerShell as an administrator, and \
             start the VM again:"
        } else {
            "Run this in PowerShell as an administrator:"
        },
        "",
    ));

    // A paragraph each command, wrapped only between words: Pango would
    // also break after a hyphen, in Get-VMIntegrationService, which reads
    // like two words to someone typing it
    let code = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(6)
        .hexpand(true)
        .valign(gtk::Align::Center)
        .build();
    for line in command.lines() {
        let markup: Vec<String> = line
            .split(' ')
            .map(|word| {
                format!(
                    "<span allow_breaks=\"false\">{}</span>",
                    glib::markup_escape_text(word)
                )
            })
            .collect();
        let line = gtk::Label::builder()
            .label(markup.join(" "))
            .use_markup(true)
            .xalign(0.0)
            .wrap(true)
            .wrap_mode(gtk::pango::WrapMode::Word)
            .max_width_chars(52)
            .selectable(true)
            .build();
        line.add_css_class("monospace");
        code.append(&line);
    }
    let copy = gtk::Button::builder()
        .label("Copy")
        .valign(gtk::Align::Center)
        .build();
    let command = command.to_string();
    copy.connect_clicked(move |copy| {
        copy.clipboard().set_text(&command);
        copy.set_label("Copied");
        let copy = copy.clone();
        glib::timeout_add_seconds_local_once(2, move || copy.set_label("Copy"));
    });
    let line = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(12)
        .build();
    line.add_css_class("setup-command");
    line.append(&code);
    line.append(&copy);
    section.append(&line);

    let mut notes = Vec::new();
    if status.vm_name.is_none() {
        notes.push("Put the VM's name, as Hyper-V Manager shows it, in place of NAME.");
    }
    if !status.transport {
        // No enhanced session yet means, most likely, VMConnect's basic
        // window, whose clipboard only goes the other way
        notes.push("VMConnect's basic window can't copy to Windows, so you may need to type it.");
    }
    if !notes.is_empty() {
        section.append(&label(&notes.join(" "), "setup-dim"));
    }
    section
}

/// "Set Up": the helper, through pkexec, for what's missing.
fn set_up_parts(this: &Rc<Setup>) {
    let status = this.status.borrow().clone();
    let parts = status.to_set_up();
    show_working(this, &status);

    let this = this.clone();
    glib::spawn_future_local(async move {
        let mut arguments = if this.options.dry_run {
            vec![HELPER, "--dry-run"]
        } else {
            vec!["pkexec", HELPER]
        };
        arguments.extend(&parts);
        let arguments: Vec<&OsStr> = arguments.iter().map(OsStr::new).collect();
        let flags = gio::SubprocessFlags::STDOUT_PIPE | gio::SubprocessFlags::STDERR_MERGE;
        let process = match gio::Subprocess::newv(&arguments, flags) {
            Ok(process) => process,
            Err(error) => {
                show_failed(&this, &format!("The setup didn't start: {error}"), "");
                return;
            }
        };
        let output = match process.communicate_utf8_future(None).await {
            Ok((output, _)) => output.map(|output| output.to_string()).unwrap_or_default(),
            Err(error) => error.to_string(),
        };
        let code = process.has_exited().then(|| process.exit_status());
        match code {
            Some(0) => {}
            // The password dialog, cancelled
            Some(126) => {
                show_check(&this);
                return;
            }
            Some(127) => {
                show_failed(
                    &this,
                    "Patrone Linux couldn't get permission to set it up. It needs an \
                     administrator's password.",
                    &output,
                );
                return;
            }
            _ => {
                show_failed(&this, "Something went wrong while setting it up.", &output);
                return;
            }
        }

        // What's set up now; what was only pretended missing, set up or not
        let pretend: Vec<String> = this
            .options
            .pretend
            .iter()
            .filter(|part| !parts.contains(&part.as_str()))
            .cloned()
            .collect();
        let fresh = Status::check(&pretend);
        let left = fresh.to_set_up();
        if !left.is_empty() {
            show_failed(
                &this,
                &format!(
                    "The setup finished, but this still isn't set up: {}.",
                    describe_parts(&fresh, &left)
                ),
                &output,
            );
            return;
        }
        show_done(&this, fresh, status.restart_after());
    });
}

/// "installing the integration services and turning on the enhanced
/// session"
fn describe_parts(status: &Status, parts: &[&str]) -> String {
    let mut doing = Vec::new();
    if parts.contains(&"guest-tools") {
        doing.push("installing the integration services");
    }
    if parts.contains(&"enhanced-session") {
        doing.push(match status.session {
            Session::Missing => "installing the enhanced session",
            Session::Old => "updating Remote Desktop for the enhanced session",
            _ => "turning on the enhanced session",
        });
    }
    doing.join(" and ")
}

fn sentence(text: &str) -> String {
    let mut chars = text.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(chars).collect()
    })
}

fn show_working(this: &Setup, status: &Status) {
    let page = page();
    page.set_valign(gtk::Align::Center);
    page.set_margin_top(36);
    page.set_margin_bottom(36);
    let spinner = gtk::Spinner::builder()
        .spinning(true)
        .width_request(48)
        .height_request(48)
        .halign(gtk::Align::Center)
        .build();
    page.append(&spinner);
    let title = label("Setting up this virtual machine", "setup-title");
    title.set_xalign(0.5);
    page.append(&title);
    let text = label(
        &format!(
            "{}. This can take a few minutes.",
            sentence(&describe_parts(status, &status.to_set_up()))
        ),
        "",
    );
    text.set_xalign(0.5);
    text.set_justify(gtk::Justification::Center);
    page.append(&text);
    put_page(this, "working", &page);
}

fn show_done(this: &Rc<Setup>, fresh: Status, restart: bool) {
    let command = fresh.windows_command();
    let shutdown = fresh.windows_needs_shutdown();
    let page = page();
    if let Some(command) = &command {
        page.append(&label("One more step, on Windows", "setup-title"));
        page.append(&label(
            if restart && !shutdown {
                "Patrone Linux's side is set up: restart it to finish. What's left is on \
                 Windows' side."
            } else {
                "Patrone Linux's side is set up. What's left is on Windows' side."
            },
            "",
        ));
        page.append(&windows_box(&fresh, command));
    } else {
        page.append(&label("This virtual machine is set up", "setup-title"));
        page.append(&label(
            if restart {
                "Restart Patrone Linux to finish setting it up."
            } else {
                "Everything Patrone Linux needs in Hyper-V is set up. In VMConnect, the \
                 enhanced session is on the View menu."
            },
            "",
        ));
    }
    page.append(&label(TRADEMARKS, "setup-legal"));

    let mut buttons = Vec::new();
    let power = if command.is_some() && shutdown {
        Some(("Shut Down…", "--power-off"))
    } else if restart {
        Some(("Restart…", "--reboot"))
    } else {
        None
    };
    let close = button(
        if power.is_some() { "Later" } else { "Close" },
        power.is_none(),
    );
    let window = this.window.clone();
    close.connect_clicked(move |_| window.close());
    buttons.push(close);
    if let Some((text, how)) = power {
        let power = button(text, true);
        let window = this.window.clone();
        power.connect_clicked(move |_| {
            // GNOME's own dialog, which asks first
            let arguments = [OsStr::new("gnome-session-quit"), OsStr::new(how)];
            if let Err(error) = gio::Subprocess::newv(&arguments, gio::SubprocessFlags::NONE) {
                eprintln!("patrone-hyperv-setup: gnome-session-quit didn't start: {error}");
            }
            window.close();
        });
        buttons.push(power);
    }
    let buttons: Vec<&gtk::Button> = buttons.iter().collect();
    page.append(&bottom(this, false, &buttons));
    *this.status.borrow_mut() = fresh;
    put_page(this, "done", &page);
}

fn show_failed(this: &Rc<Setup>, text: &str, output: &str) {
    let page = page();
    page.append(&label("Setup didn't finish", "setup-title"));
    page.append(&label(text, ""));
    // The last of what apt and the rest said
    let lines: Vec<&str> = output
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect();
    let tail = lines[lines.len().saturating_sub(12)..].join("\n");
    if !tail.is_empty() {
        let log = gtk::Label::builder()
            .label(&tail)
            .xalign(0.0)
            .yalign(0.0)
            .wrap(true)
            .wrap_mode(gtk::pango::WrapMode::WordChar)
            .max_width_chars(62)
            .selectable(true)
            .build();
        log.add_css_class("monospace");
        log.add_css_class("setup-command");
        let scrolled = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .min_content_height(120)
            .max_content_height(240)
            .propagate_natural_height(true)
            .child(&log)
            .build();
        page.append(&scrolled);
    }
    let close = button("Close", false);
    let again = button("Try Again", true);
    let window = this.window.clone();
    close.connect_clicked(move |_| window.close());
    let weak = Rc::downgrade(this);
    again.connect_clicked(move |_| {
        if let Some(this) = weak.upgrade() {
            *this.status.borrow_mut() = Status::check(&this.options.pretend);
            show_check(&this);
        }
    });
    page.append(&bottom(this, false, &[&close, &again]));
    put_page(this, "failed", &page);
}

/// "Don't show this again": a file the login unit checks for
fn save_dont_show(hidden: bool) {
    let file = checks::dont_show_file();
    if !hidden {
        if let Err(error) = fs::remove_file(&file)
            && error.kind() != std::io::ErrorKind::NotFound
        {
            eprintln!(
                "patrone-hyperv-setup: couldn't remove {}: {error}",
                file.display()
            );
        }
        return;
    }
    if file.exists() {
        return;
    }
    let written = file
        .parent()
        .map_or(Ok(()), fs::create_dir_all)
        .and_then(|()| {
            fs::write(
                &file,
                "# Virtual Machine Setup's \"Don't show this again\": while this file\n\
                 # is here, it doesn't show at login. Delete it, or untick the box in\n\
                 # patrone-hyperv-setup, to see it again.\n",
            )
        });
    if let Err(error) = written {
        eprintln!(
            "patrone-hyperv-setup: couldn't write {}: {error}",
            file.display()
        );
    }
}
