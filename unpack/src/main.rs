//! Patrone Unpack: Patrone's app for archives and installers. An archive
//! (zip, tar, 7z, rar...) opens as a folder with "Extract all…" and
//! "Extract here"; a .deb package or a Flatpak file opens with what it is
//! and an Install button.

mod archive;
mod browse;
mod deb;
mod flatpak;
mod install;
mod util;

use std::path::{Path, PathBuf};

use gtk::prelude::*;
use gtk::{gdk, gio, glib};

const APP_ID: &str = "org.patrone.Unpack";

const USAGE: &str = "\
Usage: patrone-unpack [OPTION] [FILE...]

Opens each archive to see what's inside and extract it, and each .deb
package or Flatpak file (.flatpakref, .flatpak, .flatpakrepo) to install it.

      --extract-here       Extract each archive next to it, then close
      --extract-to FOLDER  Extract each archive into FOLDER, then close
      --list               Print what's in each archive (no window)
  -h, --help               Show this help
  -V, --version            Show the version
";

/// File types it opens, for the Open dialog (the desktop file lists them
/// for the system).
const TYPES: &[&str] = &[
    "application/zip",
    "application/x-tar",
    "application/x-compressed-tar",
    "application/x-xz-compressed-tar",
    "application/x-bzip2-compressed-tar",
    "application/x-zstd-compressed-tar",
    "application/x-7z-compressed",
    "application/vnd.rar",
    "application/gzip",
    "application/x-xz",
    "application/x-bzip2",
    "application/zstd",
    "application/vnd.debian.binary-package",
    "application/vnd.flatpak",
    "application/vnd.flatpak.ref",
    "application/vnd.flatpak.repo",
];

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
        println!("patrone-unpack {}", env!("CARGO_PKG_VERSION"));
        return glib::ExitCode::SUCCESS;
    }
    if arguments.first().map(String::as_str) == Some("--list") {
        return list(&arguments[1..]);
    }

    let app = gtk::Application::builder()
        .application_id(APP_ID)
        .flags(gio::ApplicationFlags::HANDLES_COMMAND_LINE)
        .build();
    app.connect_startup(|_| startup());
    app.connect_command_line(command_line);
    app.run()
}

/// --list: prints each archive's entries, for scripts and checks.
fn list(files: &[String]) -> glib::ExitCode {
    let mut status = glib::ExitCode::SUCCESS;
    for file in files {
        match archive::list(Path::new(file), None) {
            Ok(listing) => {
                println!("{file}: {}", listing.format);
                for entry in &listing.entries {
                    let kind = match &entry.kind {
                        archive::Kind::File => "file",
                        archive::Kind::Folder => "folder",
                        archive::Kind::Link(_) => "link",
                        archive::Kind::HardLink(_) => "hard link",
                        archive::Kind::Other => "other",
                    };
                    let size = entry.size.map(|size| size.to_string()).unwrap_or_default();
                    let lock = if entry.encrypted { "  (password)" } else { "" };
                    println!("{kind:>9} {size:>12}  {}{lock}", entry.path);
                }
            }
            Err(error) => {
                eprintln!("{file}: {error}");
                status = glib::ExitCode::FAILURE;
            }
        }
    }
    status
}

fn startup() {
    let Some(display) = gdk::Display::default() else {
        return;
    };
    if let Some(settings) = gtk::Settings::default() {
        // GTK's own theme: Ubuntu's Yaru doesn't load in plain GTK 4 apps.
        // Patrone's title bar button style still reaches it, from the
        // account's gtk.css.
        settings.set_gtk_theme_name(Some("Default"));
        // GTK 4.22's Default theme stays light when the desktop prefers
        // dark, though the preference reaches GTK (and the style sheet
        // below); its dark variant has to be asked for.
        #[allow(deprecated)]
        fn follow(settings: &gtk::Settings) {
            let dark = settings.gtk_interface_color_scheme() == gtk::InterfaceColorScheme::Dark;
            settings.set_gtk_application_prefer_dark_theme(dark);
        }
        follow(&settings);
        settings.connect_gtk_interface_color_scheme_notify(follow);
    }
    let css = gtk::CssProvider::new();
    if let Some(settings) = gtk::Settings::default() {
        settings
            .bind_property("gtk-interface-color-scheme", &css, "prefers-color-scheme")
            .sync_create()
            .build();
    }
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
    let arguments = command_line.arguments();
    let mut action = None;
    let mut files: Vec<PathBuf> = Vec::new();
    let mut iter = arguments.iter().skip(1);
    while let Some(argument) = iter.next() {
        let text = argument.to_string_lossy();
        if text == "--extract-here" {
            action = Some(browse::Action::ExtractHere);
        } else if text == "--extract-to" {
            let Some(folder) = iter.next() else {
                command_line.printerr_literal("--extract-to needs a folder\n");
                return glib::ExitCode::FAILURE;
            };
            action = command_line
                .create_file_for_arg(folder)
                .path()
                .map(browse::Action::ExtractTo);
        } else if let Some(folder) = text.strip_prefix("--extract-to=") {
            action = command_line
                .create_file_for_arg(folder)
                .path()
                .map(browse::Action::ExtractTo);
        } else if text.starts_with("--") {
            command_line.printerr_literal(&format!("Unknown option {text}\n\n{USAGE}"));
            return glib::ExitCode::FAILURE;
        } else if let Some(path) = command_line.create_file_for_arg(argument).path() {
            files.push(path);
        }
    }
    if files.is_empty() {
        welcome(app);
    }
    for file in files {
        open_path(app, &file, action.clone());
    }
    glib::ExitCode::SUCCESS
}

/// Which install window a file needs, if it isn't an archive.
fn install_kind(path: &Path) -> Option<install::Kind> {
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let by_name = if name.ends_with(".deb") {
        Some(install::Kind::Deb)
    } else if name.ends_with(".flatpakref") {
        Some(install::Kind::Flatpak(flatpak::Kind::Ref))
    } else if name.ends_with(".flatpakrepo") {
        Some(install::Kind::Flatpak(flatpak::Kind::Repo))
    } else if name.ends_with(".flatpak") {
        Some(install::Kind::Flatpak(flatpak::Kind::Bundle))
    } else {
        None
    };
    if by_name.is_some() {
        return by_name;
    }
    let content_type = gio::File::for_path(path)
        .query_info(
            gio::FILE_ATTRIBUTE_STANDARD_CONTENT_TYPE,
            gio::FileQueryInfoFlags::NONE,
            None::<&gio::Cancellable>,
        )
        .ok()?
        .content_type()?;
    match content_type.as_str() {
        "application/vnd.debian.binary-package" | "application/x-deb" => Some(install::Kind::Deb),
        "application/vnd.flatpak" => Some(install::Kind::Flatpak(flatpak::Kind::Bundle)),
        "application/vnd.flatpak.ref" => Some(install::Kind::Flatpak(flatpak::Kind::Ref)),
        "application/vnd.flatpak.repo" => Some(install::Kind::Flatpak(flatpak::Kind::Repo)),
        _ => None,
    }
}

pub fn open_path(app: &gtk::Application, path: &Path, action: Option<browse::Action>) {
    if !path.is_file() {
        let hold = app.hold();
        gtk::AlertDialog::builder()
            .message("This file can't be opened")
            .detail(format!("{} isn't there, or isn't a file.", path.display()))
            .build()
            .choose(None::<&gtk::Window>, None::<&gio::Cancellable>, move |_| {
                drop(hold)
            });
        return;
    }
    match install_kind(path) {
        Some(kind) => install::open(app, path, kind),
        None => browse::open(app, path, action),
    }
}

/// With no file: a window that offers to open one.
fn welcome(app: &gtk::Application) {
    let window = gtk::ApplicationWindow::builder()
        .application(app)
        .title("Patrone Unpack")
        .default_width(520)
        .default_height(360)
        .build();
    let content = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(14)
        .valign(gtk::Align::Center)
        .margin_start(32)
        .margin_end(32)
        .build();
    content.append(
        &gtk::Image::builder()
            .icon_name(APP_ID)
            .pixel_size(96)
            .build(),
    );
    content.append(
        &gtk::Label::builder()
            .label("Patrone Unpack")
            .css_classes(["unpack-title"])
            .build(),
    );
    content.append(
        &gtk::Label::builder()
            .label(
                "Open an archive (zip, tar, 7z, rar…) to see what's inside and extract it, \
                 or a .deb package or Flatpak file to install it.",
            )
            .wrap(true)
            .justify(gtk::Justification::Center)
            .css_classes(["unpack-dim"])
            .build(),
    );
    let open = gtk::Button::builder()
        .label("Open a file…")
        .halign(gtk::Align::Center)
        .css_classes(["suggested-action"])
        .build();
    content.append(&open);
    window.set_child(Some(&content));

    let app_ref = app.clone();
    let window_ref = window.clone();
    open.connect_clicked(move |_| {
        let filter = gtk::FileFilter::new();
        filter.set_name(Some("Archives and installers"));
        for content_type in TYPES {
            filter.add_mime_type(content_type);
        }
        let everything = gtk::FileFilter::new();
        everything.set_name(Some("All files"));
        everything.add_pattern("*");
        let filters = gio::ListStore::new::<gtk::FileFilter>();
        filters.append(&filter);
        filters.append(&everything);
        let dialog = gtk::FileDialog::builder()
            .title("Open")
            .filters(&filters)
            .default_filter(&filter)
            .initial_folder(&gio::File::for_path(
                glib::user_special_dir(glib::UserDirectory::Downloads)
                    .unwrap_or_else(glib::home_dir),
            ))
            .modal(true)
            .build();
        let app = app_ref.clone();
        let window = window_ref.clone();
        dialog.open(
            Some(&window_ref),
            None::<&gio::Cancellable>,
            move |result| {
                if let Ok(file) = result
                    && let Some(path) = file.path()
                {
                    open_path(&app, &path, None);
                    window.close();
                }
            },
        );
    });
    window.present();
}
