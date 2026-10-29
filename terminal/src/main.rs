//! Patrone Terminal, the default terminal of PatroneOS.
//!
//! GTK4 and VTE, with copy on select, paste on right-click, tabs, and the
//! Patrone Dark theme.

mod cli;
mod preferences;
mod settings;
mod theme;
mod window;

use gtk::{gdk, gio, glib, prelude::*};

/// Also the icon name and the desktop file's name.
pub const APP_ID: &str = "org.patrone.Terminal";

const ACCELS: &[(&str, &[&str])] = &[
    ("win.preferences", &["<Ctrl>comma"]),
    ("win.copy", &["<Ctrl><Shift>c"]),
    ("win.paste", &["<Ctrl><Shift>v", "<Shift>Insert"]),
    ("win.new-tab", &["<Ctrl><Shift>t"]),
    ("win.new-window", &["<Ctrl><Shift>n"]),
    ("win.close-tab", &["<Ctrl><Shift>w"]),
    ("win.next-tab", &["<Ctrl>Page_Down"]),
    ("win.previous-tab", &["<Ctrl>Page_Up"]),
    (
        "win.zoom-in",
        &["<Ctrl>plus", "<Ctrl>equal", "<Ctrl>KP_Add"],
    ),
    ("win.zoom-out", &["<Ctrl>minus", "<Ctrl>KP_Subtract"]),
    ("win.zoom-reset", &["<Ctrl>0", "<Ctrl>KP_0"]),
];

fn main() -> glib::ExitCode {
    let mut flags = gio::ApplicationFlags::HANDLES_COMMAND_LINE;
    if cli::wants_standalone(std::env::args_os()) {
        flags |= gio::ApplicationFlags::NON_UNIQUE;
    }
    let app = gtk::Application::builder()
        .application_id(APP_ID)
        .flags(flags)
        .build();
    app.connect_startup(startup);
    app.connect_command_line(command_line);
    app.run()
}

fn startup(app: &gtk::Application) {
    let display = gdk::Display::default().expect("Patrone Terminal needs a display");
    let css = gtk::CssProvider::new();
    css.load_from_string(theme::CSS);
    gtk::style_context_add_provider_for_display(
        &display,
        &css,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
    if let Some(settings) = gtk::Settings::default() {
        // GTK's built-in theme. Ubuntu's Yaru fails to load in plain GTK4
        // apps, and Patrone will bring its own styling.
        settings.set_gtk_theme_name(Some("Default"));
        settings.set_gtk_interface_color_scheme(gtk::InterfaceColorScheme::Dark);
    }
    gtk::Window::set_default_icon_name(APP_ID);
    for (action, accels) in ACCELS {
        app.set_accels_for_action(action, accels);
    }
}

/// Runs for the first launch and, through D-Bus, for every later one, so all
/// windows share one process.
fn command_line(
    app: &gtk::Application,
    command_line: &gio::ApplicationCommandLine,
) -> glib::ExitCode {
    let cwd = command_line.cwd();
    match cli::parse(&command_line.arguments(), cwd.as_deref()) {
        Ok(cli::Command::Open(launch)) => {
            window::open(app, launch);
            glib::ExitCode::SUCCESS
        }
        Ok(cli::Command::Help) => {
            command_line.print_literal(cli::USAGE);
            glib::ExitCode::SUCCESS
        }
        Ok(cli::Command::Version) => {
            command_line
                .print_literal(&format!("patrone-terminal {}\n", env!("CARGO_PKG_VERSION")));
            glib::ExitCode::SUCCESS
        }
        Err(message) => {
            command_line
                .printerr_literal(&format!("patrone-terminal: {message}\n\n{}", cli::USAGE));
            glib::ExitCode::FAILURE
        }
    }
}
