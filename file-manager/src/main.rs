//! Patrone File Manager: the file manager of Patrone Linux, in the image
//! of Windows 11's File Explorer. GTK4, written in Rust.

mod addressbar;
mod archive;
mod clipboard;
mod commandbar;
mod commands;
mod config;
mod conflicts;
mod dbus;
mod dialogs;
mod dnd;
mod filecell;
mod folderview;
mod gallery;
mod glyph;
mod hints;
mod home;
mod icons;
mod location;
mod menu;
mod menus;
mod navpane;
mod openwith;
mod ops;
mod options;
mod permissions;
mod places;
mod progress;
mod properties;
mod run;
mod search;
mod sidepane;
mod tab;
mod tabbar;
mod thispc;
mod thumbs;
mod undo;
mod util;
mod viewstate;
mod window;

use gtk::{gdk, gio, glib, graphene, prelude::*};

use crate::location::Location;
use crate::window::Window;

/// Also the icon name and the desktop file's name.
pub const APP_ID: &str = "org.patrone.FileManager";

const USAGE: &str = "\
Usage: patrone-file-manager [OPTION...] [LOCATION...]

Opens a File Manager window for each folder. A file opens in its app.

  -w, --new-window   Open a new window (the default with no locations)
  -s, --select       Show each location in its folder, selected
      --restore      Reopen the folders open at the last sign-out, if
                     Folder Options says to
      --quit         Close every File Manager window
  -h, --help         Show this help
  -V, --version      Show the version
";

fn main() -> glib::ExitCode {
    gio::resources_register_include!("resources.gresource").expect("the app's resources");
    let app = gtk::Application::builder()
        .application_id(APP_ID)
        .flags(gio::ApplicationFlags::HANDLES_COMMAND_LINE)
        .build();
    app.connect_startup(startup);
    app.connect_command_line(command_line);
    app.run()
}

fn startup(app: &gtk::Application) {
    let display = gdk::Display::default().expect("File Manager needs a display");
    if let Some(settings) = gtk::Settings::default() {
        // GTK's own theme: Ubuntu's Yaru doesn't load in plain GTK4 apps,
        // and the style sheet below gives the Windows 11 look.
        settings.set_gtk_theme_name(Some("Default"));
        match std::env::var("PATRONE_FM_COLOR_SCHEME").as_deref() {
            Ok("dark") => settings.set_gtk_interface_color_scheme(gtk::InterfaceColorScheme::Dark),
            Ok("light") => {
                settings.set_gtk_interface_color_scheme(gtk::InterfaceColorScheme::Light)
            }
            _ => {}
        }
    }
    let css = gtk::CssProvider::new();
    // The style sheet's dark palette follows the desktop's light or dark
    // preference (or PATRONE_FM_COLOR_SCHEME, above).
    if let Some(settings) = gtk::Settings::default() {
        settings
            .bind_property("gtk-interface-color-scheme", &css, "prefers-color-scheme")
            .sync_create()
            .build();
    }
    css.load_from_resource("/org/patrone/FileManager/style.css");
    gtk::style_context_add_provider_for_display(
        &display,
        &css,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
    gtk::Window::set_default_icon_name(APP_ID);
    window::set_accels(app);
    dbus::register(app);
    thumbs::set_enabled(config::flag("show-thumbnails"));
}

/// Runs for the first launch and, through D-Bus, for every later one, so
/// every window shares one process.
fn command_line(
    app: &gtk::Application,
    command_line: &gio::ApplicationCommandLine,
) -> glib::ExitCode {
    let arguments = command_line.arguments();
    let mut select = false;
    let mut restore = false;
    let mut locations = Vec::new();
    for argument in arguments.iter().skip(1) {
        match argument.to_str() {
            Some("-h" | "--help") => {
                command_line.print_literal(USAGE);
                return glib::ExitCode::SUCCESS;
            }
            Some("-V" | "--version") => {
                command_line.print_literal(&format!(
                    "patrone-file-manager {}\n",
                    env!("CARGO_PKG_VERSION")
                ));
                return glib::ExitCode::SUCCESS;
            }
            Some("--quit") => {
                app.quit();
                return glib::ExitCode::SUCCESS;
            }
            Some("-w" | "--new-window") => {}
            Some("-s" | "--select") => select = true,
            Some("--restore") => restore = true,
            Some(option) if option.starts_with('-') && option.len() > 1 => {
                command_line.printerr_literal(&format!(
                    "patrone-file-manager: unknown option {option}\n\n{USAGE}"
                ));
                return glib::ExitCode::FAILURE;
            }
            _ => locations.push(command_line.create_file_for_arg(argument)),
        }
    }

    if restore {
        if config::flag("restore-windows") {
            for uri in config::settings().strv("open-windows") {
                Window::new(app, Location::Folder(gio::File::for_uri(&uri))).present();
            }
        }
        return glib::ExitCode::SUCCESS;
    }

    if locations.is_empty() {
        // As on Windows, launching it again opens another window.
        let window = Window::new(app, window::start_location());
        window.present();
        take_screenshot_if_asked(&window);
        return glib::ExitCode::SUCCESS;
    }
    if select {
        let uris: Vec<String> = locations
            .iter()
            .map(|file| file.uri().to_string())
            .collect();
        dbus::show_items(app, &uris);
        return glib::ExitCode::SUCCESS;
    }
    for file in locations {
        let file_type = file.query_file_type(gio::FileQueryInfoFlags::NONE, gio::Cancellable::NONE);
        if file_type == gio::FileType::Regular {
            let launched =
                gio::AppInfo::launch_default_for_uri(&file.uri(), None::<&gio::AppLaunchContext>);
            if launched.is_ok() {
                continue;
            }
        }
        let window = Window::new(app, Location::Folder(file));
        window.present();
        take_screenshot_if_asked(&window);
    }
    glib::ExitCode::SUCCESS
}

/// For testing: with `PATRONE_FM_SCREENSHOT=file.png`, the first window is
/// drawn into that file after `PATRONE_FM_SCREENSHOT_DELAY` milliseconds
/// (2000 by default), then the app quits.
fn take_screenshot_if_asked(window: &Window) {
    let Ok(path) = std::env::var("PATRONE_FM_SCREENSHOT") else {
        return;
    };
    let delay = std::env::var("PATRONE_FM_SCREENSHOT_DELAY")
        .ok()
        .and_then(|ms| ms.parse().ok())
        .unwrap_or(2000);
    let window = window.gtk_window();
    glib::timeout_add_local_once(std::time::Duration::from_millis(delay), move || {
        // With PATRONE_FM_SCREENSHOT_DIALOG set, a dialog open by then
        // (Properties, Folder Options) is drawn instead.
        let dialog = std::env::var_os("PATRONE_FM_SCREENSHOT_DIALOG").and_then(|_| {
            gtk::Window::list_toplevels()
                .into_iter()
                .filter_map(|widget| widget.downcast::<gtk::Window>().ok())
                .find(|other| other != &window && other.is_visible())
        });
        let app = window.application();
        let window = dialog.unwrap_or(window);
        // PATRONE_FM_SCREENSHOT_PAGE=N shows a dialog's Nth tab first.
        let page = std::env::var("PATRONE_FM_SCREENSHOT_PAGE")
            .ok()
            .and_then(|page| page.parse::<u32>().ok());
        let notebook = page.and_then(|_| find_notebook(window.upcast_ref()));
        if let (Some(page), Some(notebook)) = (page, &notebook) {
            notebook.set_current_page(Some(page));
        }
        let settle = if notebook.is_some() { 500 } else { 0 };
        glib::timeout_add_local_once(std::time::Duration::from_millis(settle), move || {
            draw_to_png(&window, &path);
            if let Some(app) = app {
                app.quit();
            }
        });
    });
}

fn find_notebook(widget: &gtk::Widget) -> Option<gtk::Notebook> {
    if let Some(notebook) = widget.downcast_ref::<gtk::Notebook>() {
        return Some(notebook.clone());
    }
    let mut child = widget.first_child();
    while let Some(current) = child {
        if let Some(notebook) = find_notebook(&current) {
            return Some(notebook);
        }
        child = current.next_sibling();
    }
    None
}

fn draw_to_png(window: &gtk::Window, path: &str) {
    let (width, height) = (window.width(), window.height());
    let paintable = gtk::WidgetPaintable::new(Some(window));
    let snapshot = gtk::Snapshot::new();
    paintable.snapshot(&snapshot, width as f64, height as f64);
    let renderer = window.native().and_then(|native| native.renderer());
    if let (Some(node), Some(renderer)) = (snapshot.to_node(), renderer) {
        let texture = renderer.render_texture(
            &node,
            Some(&graphene::Rect::new(0.0, 0.0, width as f32, height as f32)),
        );
        if let Err(error) = texture.save_to_png(path) {
            eprintln!("patrone-file-manager: could not save {path}: {error}");
        }
    }
}
