//! The install window, for .deb packages and Flatpak files: what the
//! package is, whether it's installed already, what else installing it
//! changes, then Install. A .deb is installed by apt, through a small root
//! helper run by pkexec (polkit asks for an administrator's password); a
//! Flatpak by Flatpak itself, for the account that opened it, with no
//! password.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{gio, glib};

use crate::{deb, flatpak, util};

/// The root helper that installs a .deb with apt (PATRONE_UNPACK_HELPER
/// replaces it when running from the source tree).
const HELPER: &str = "/usr/libexec/patrone-unpack/install-deb";

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Kind {
    Deb,
    Flatpak(flatpak::Kind),
}

enum Details {
    Deb(deb::Deb, deb::Installed),
    Flatpak(flatpak::Info, FlatpakState),
}

/// A Flatpak file against what's installed.
struct FlatpakState {
    installed: bool,
    /// The bundle's runtime isn't installed yet, so installing downloads it.
    runtime_missing: bool,
    /// ...from Flathub, which has to be set up first.
    add_flathub: bool,
}

struct Window {
    window: gtk::ApplicationWindow,
    path: PathBuf,
    kind: Kind,
    stack: gtk::Stack,
    icon: gtk::Image,
    title: gtk::Label,
    subtitle: gtk::Label,
    status: gtk::Label,
    description: gtk::Label,
    facts: gtk::Grid,
    permissions: gtk::Box,
    changes: gtk::Label,
    progress: gtk::ProgressBar,
    log: gtk::TextView,
    expander: gtk::Expander,
    install: gtk::Button,
    open: gtk::Button,
    close: gtk::Button,
    details: RefCell<Option<Details>>,
}

pub fn open(app: &gtk::Application, path: &Path, kind: Kind) {
    let window = gtk::ApplicationWindow::builder()
        .application(app)
        .title(file_name(path))
        .default_width(600)
        .default_height(720)
        .build();

    let icon = gtk::Image::builder()
        .icon_name(match kind {
            Kind::Deb => "package-x-generic",
            Kind::Flatpak(_) => "system-software-install",
        })
        .pixel_size(64)
        .valign(gtk::Align::Start)
        .build();
    let title = label(&["unpack-title"]);
    let subtitle = label(&["unpack-dim"]);
    let heading = gtk::Box::new(gtk::Orientation::Vertical, 4);
    heading.append(&title);
    heading.append(&subtitle);
    let top = gtk::Box::new(gtk::Orientation::Horizontal, 18);
    top.append(&icon);
    top.append(&heading);

    let status = label(&["unpack-status"]);
    let description = label(&[]);
    let facts = gtk::Grid::builder()
        .column_spacing(18)
        .row_spacing(6)
        .margin_top(6)
        .build();
    let permissions = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(4)
        .visible(false)
        .build();
    let changes = label(&[]);
    let body = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(14)
        .margin_end(12)
        .build();
    body.append(&description);
    body.append(&facts);
    body.append(&permissions);
    body.append(&changes);
    let scroller = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vexpand(true)
        .child(&body)
        .build();

    let progress = gtk::ProgressBar::builder()
        .show_text(true)
        .visible(false)
        .build();
    let log = gtk::TextView::builder()
        .editable(false)
        .cursor_visible(false)
        .monospace(true)
        .wrap_mode(gtk::WrapMode::WordChar)
        .top_margin(6)
        .bottom_margin(6)
        .left_margin(6)
        .right_margin(6)
        .build();
    let expander = gtk::Expander::builder()
        .label("Details")
        .visible(false)
        .child(
            &gtk::ScrolledWindow::builder()
                .min_content_height(160)
                .child(&log)
                .css_classes(["unpack-log"])
                .build(),
        )
        .build();

    let close = gtk::Button::with_label("Close");
    let open_button = gtk::Button::builder().label("Open").visible(false).build();
    let install = gtk::Button::builder()
        .label("Install")
        .sensitive(false)
        .css_classes(["suggested-action"])
        .build();
    let buttons = gtk::Box::builder()
        .spacing(8)
        .halign(gtk::Align::End)
        .build();
    buttons.append(&close);
    buttons.append(&open_button);
    buttons.append(&install);

    let page = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(16)
        .build();
    page.append(&top);
    page.append(&status);
    page.append(&scroller);
    page.append(&progress);
    page.append(&expander);
    page.append(&buttons);

    let loading = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(12)
        .valign(gtk::Align::Center)
        .build();
    loading.append(
        &gtk::Spinner::builder()
            .spinning(true)
            .width_request(32)
            .height_request(32)
            .build(),
    );
    loading.append(&gtk::Label::new(Some("Reading the package…")));

    let stack = gtk::Stack::builder()
        .margin_top(24)
        .margin_bottom(20)
        .margin_start(24)
        .margin_end(24)
        .build();
    stack.add_named(&loading, Some("loading"));
    stack.add_named(&page, Some("page"));
    window.set_child(Some(&stack));

    let this = Rc::new(Window {
        window: window.clone(),
        path: path.to_path_buf(),
        kind,
        stack,
        icon,
        title,
        subtitle,
        status,
        description,
        facts,
        permissions,
        changes,
        progress,
        log,
        expander,
        install: install.clone(),
        open: open_button.clone(),
        close: close.clone(),
        details: RefCell::new(None),
    });

    let window_ref = window.clone();
    close.connect_clicked(move |_| window_ref.close());
    let weak = Rc::downgrade(&this);
    install.connect_clicked(move |_| {
        if let Some(this) = weak.upgrade() {
            start_install(&this);
        }
    });
    let weak = Rc::downgrade(&this);
    open_button.connect_clicked(move |_| {
        if let Some(this) = weak.upgrade() {
            launch(&this);
        }
    });
    // The window keeps its state until it's closed.
    let keep = RefCell::new(Some(this.clone()));
    window.connect_close_request(move |_| {
        keep.borrow_mut().take();
        glib::Propagation::Proceed
    });

    window.present();
    load(this);
}

fn label(classes: &[&str]) -> gtk::Label {
    gtk::Label::builder()
        .wrap(true)
        .wrap_mode(gtk::pango::WrapMode::WordChar)
        .xalign(0.0)
        .selectable(true)
        .css_classes(
            classes
                .iter()
                .map(|class| class.to_string())
                .collect::<Vec<_>>(),
        )
        .build()
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Reads the package off the main thread, then fills the window.
fn load(this: Rc<Window>) {
    let path = this.path.clone();
    let kind = this.kind;
    glib::spawn_future_local(async move {
        let result = gio::spawn_blocking(move || -> Result<Details, String> {
            match kind {
                Kind::Deb => {
                    let package = deb::read(&path)?;
                    let installed = deb::installed(&package);
                    Ok(Details::Deb(package, installed))
                }
                Kind::Flatpak(kind) => {
                    let info = flatpak::read(&path, kind)?;
                    let available = flatpak::available();
                    let installed = available
                        && match kind {
                            flatpak::Kind::Repo => flatpak::has_repository(&info.id),
                            _ => flatpak::installed(&info.id),
                        };
                    let runtime_missing = available
                        && kind == flatpak::Kind::Bundle
                        && info
                            .needs
                            .as_deref()
                            .is_some_and(|runtime| !flatpak::runtime_installed(runtime));
                    let add_flathub = runtime_missing && flatpak::needs_flathub(&info);
                    Ok(Details::Flatpak(
                        info,
                        FlatpakState {
                            installed,
                            runtime_missing,
                            add_flathub,
                        },
                    ))
                }
            }
        })
        .await
        .unwrap_or_else(|_| Err("Reading the package failed.".into()));
        this.stack.set_visible_child_name("page");
        match result {
            Ok(details) => {
                fill(&this, &details);
                // No gap where a package says nothing about itself
                this.description
                    .set_visible(!this.description.text().is_empty());
                *this.details.borrow_mut() = Some(details);
                if this.kind == Kind::Deb {
                    check_changes(this);
                }
            }
            Err(message) => {
                this.icon.set_icon_name(Some("dialog-error-symbolic"));
                this.title.set_text("This package can't be opened");
                this.subtitle.set_text(&file_name(&this.path));
                this.status.set_text(&message);
                this.status.add_css_class("unpack-error");
            }
        }
    });
}

fn add_fact(this: &Window, name: &str, value: &str) {
    if value.is_empty() {
        return;
    }
    let row = this.facts.observe_children().n_items() as i32 / 2;
    let name_label = gtk::Label::builder()
        .label(name)
        .xalign(0.0)
        .valign(gtk::Align::Start)
        .css_classes(["unpack-dim"])
        .build();
    let value_label = gtk::Label::builder()
        .label(value)
        .xalign(0.0)
        .wrap(true)
        .wrap_mode(gtk::pango::WrapMode::WordChar)
        .selectable(true)
        .hexpand(true)
        .build();
    this.facts.attach(&name_label, 0, row, 1, 1);
    this.facts.attach(&value_label, 1, row, 1, 1);
}

fn add_link(this: &Window, name: &str, uri: &str) {
    let row = this.facts.observe_children().n_items() as i32 / 2;
    let name_label = gtk::Label::builder()
        .label(name)
        .xalign(0.0)
        .css_classes(["unpack-dim"])
        .build();
    let link = gtk::LinkButton::builder()
        .uri(uri)
        .label(uri)
        .halign(gtk::Align::Start)
        .css_classes(["unpack-link"])
        .build();
    this.facts.attach(&name_label, 0, row, 1, 1);
    this.facts.attach(&link, 1, row, 1, 1);
}

fn file_size(path: &Path) -> String {
    std::fs::metadata(path)
        .map(|metadata| util::size(metadata.len()))
        .unwrap_or_default()
}

/// The host part of an address: "dl.flathub.org".
fn host(url: &str) -> String {
    let rest = url.split_once("://").map_or(url, |(_, rest)| rest);
    rest.split('/').next().unwrap_or(rest).to_string()
}

fn fill(this: &Window, details: &Details) {
    match details {
        Details::Deb(package, installed) => {
            this.window
                .set_title(Some(&format!("Install {}", package.package)));
            this.title.set_text(&package.package);
            this.subtitle.set_text(&package.summary);
            this.description.set_text(&package.description);
            add_fact(this, "Version", &package.version);
            add_fact(this, "Download size", &file_size(&this.path));
            if let Some(kb) = package.installed_size_kb {
                add_fact(this, "Size installed", &util::size(kb * 1024));
            }
            add_fact(
                this,
                "Made for",
                &match package.architecture.as_str() {
                    "all" => "Any computer".to_string(),
                    arch => arch.to_string(),
                },
            );
            add_fact(this, "Packaged by", &package.maintainer);
            add_fact(this, "Category", &package.section);
            if let Some(homepage) = &package.homepage {
                add_link(this, "Website", homepage);
            }
            add_fact(this, "File", &util::display_path(&this.path));
            let (status, button) = match installed {
                deb::Installed::No => (
                    "Not installed. Installing it makes it available to everyone on this computer."
                        .to_string(),
                    "Install",
                ),
                deb::Installed::Same => (
                    format!("Version {} is already installed.", package.version),
                    "Reinstall",
                ),
                deb::Installed::Older(old) => (
                    format!(
                        "Version {old} is installed. Installing updates it to {}.",
                        package.version
                    ),
                    "Update",
                ),
                deb::Installed::Newer(new) => (
                    format!(
                        "A newer version, {new}, is installed. Installing this one goes back to {}.",
                        package.version
                    ),
                    "Install older version",
                ),
            };
            this.status.set_text(&status);
            this.install.set_label(button);
            this.changes.set_text("Checking what else is needed…");
            this.changes.add_css_class("unpack-dim");
        }
        Details::Flatpak(info, state) => {
            let flatpak::Kind::Repo = kind_of(this) else {
                return fill_flatpak_app(this, info, state);
            };
            this.window.set_title(Some(&format!("Add {}", info.title)));
            this.title.set_text(&info.title);
            this.subtitle.set_text("Software source for Flatpak apps");
            this.description.set_text(if info.description.is_empty() {
                &info.summary
            } else {
                &info.description
            });
            add_fact(this, "Name", &info.id);
            if let Some(url) = &info.url {
                add_fact(this, "Address", url);
            }
            if let Some(homepage) = &info.homepage {
                add_link(this, "Website", homepage);
            }
            this.install.set_label("Add");
            if !flatpak::available() {
                no_flatpak(this);
            } else if state.installed {
                this.status.set_text(&format!(
                    "{} is already set up for you as “{}”.",
                    info.title, info.id
                ));
                this.install.set_sensitive(false);
            } else {
                this.status.set_text(&format!(
                    "Adds {} as a place your Flatpak apps can come from. No password is needed.",
                    info.title
                ));
                this.install.set_sensitive(true);
            }
        }
    }
}

fn kind_of(this: &Window) -> flatpak::Kind {
    match this.kind {
        Kind::Flatpak(kind) => kind,
        Kind::Deb => flatpak::Kind::Ref,
    }
}

fn fill_flatpak_app(this: &Window, info: &flatpak::Info, state: &FlatpakState) {
    this.window
        .set_title(Some(&format!("Install {}", info.title)));
    if let Some(png) = &info.icon
        && let Ok(texture) = gtk::gdk::Texture::from_bytes(&glib::Bytes::from(png.as_slice()))
    {
        this.icon.set_paintable(Some(&texture));
    }
    this.title.set_text(&info.title);
    this.subtitle.set_text(if info.summary.is_empty() {
        if info.is_runtime {
            "Flatpak runtime"
        } else {
            "Flatpak app"
        }
    } else {
        &info.summary
    });
    this.description.set_text(&info.description);
    add_fact(this, "Version", &info.version);
    add_fact(this, "Made by", &info.developer);
    if kind_of(this) == flatpak::Kind::Bundle {
        add_fact(this, "Download size", &file_size(&this.path));
    }
    if let Some(size) = info.installed_size {
        add_fact(this, "Size installed", &util::size(size));
    }
    if let Some(needs) = &info.needs {
        let label = flatpak::runtime_label(needs);
        let text = if state.runtime_missing {
            let from = match info.runtime_repo.as_deref() {
                Some(repo) if !repo.contains("dl.flathub.org") => host(repo),
                _ => "Flathub".to_string(),
            };
            format!("{label}, downloaded from {from} the first time")
        } else {
            label
        };
        add_fact(this, "Needs", &text);
    }
    if let Some(url) = &info.url {
        add_fact(this, "Comes from", &host(url));
    }
    add_fact(this, "App ID", &info.id);
    add_fact(this, "Branch", &info.branch);
    if let Some(homepage) = &info.homepage {
        add_link(this, "Website", homepage);
    }
    add_fact(this, "File", &util::display_path(&this.path));
    if let Some(permissions) = &info.permissions {
        show_permissions(this, permissions);
    }
    if !flatpak::available() {
        no_flatpak(this);
        return;
    }
    if state.installed {
        this.status
            .set_text(&format!("{} is already installed.", info.title));
        this.install.set_label("Reinstall");
        this.open.set_visible(!info.is_runtime);
    } else {
        let mut text = "Not installed. Installing it for you needs no password.".to_string();
        if kind_of(this) == flatpak::Kind::Ref
            && let Some(url) = &info.url
        {
            text = format!(
                "Not installed. Installing downloads it from {} for you, with anything \
                 else it needs. No password is needed.",
                host(url)
            );
        }
        if state.runtime_missing {
            text.push_str(
                " It also downloads the runtime it needs, which Flatpak apps share; the \
                 first one can be a few hundred MB.",
            );
        }
        this.status.set_text(&text);
    }
    this.install.set_sensitive(true);
}

/// What the app may use beyond its own sandbox.
fn show_permissions(this: &Window, permissions: &[flatpak::Permission]) {
    this.permissions.append(
        &gtk::Label::builder()
            .label("What it can use")
            .xalign(0.0)
            .css_classes(["unpack-heading"])
            .build(),
    );
    if permissions.is_empty() {
        let line = label(&["unpack-dim"]);
        line.set_text("Only its own files: nothing beyond Flatpak's sandbox.");
        this.permissions.append(&line);
    }
    for permission in permissions {
        let line = label(if permission.warning {
            &["unpack-warning"]
        } else {
            &[]
        });
        line.set_text(&format!("•  {}", permission.text));
        this.permissions.append(&line);
    }
    this.permissions.set_visible(true);
}

fn no_flatpak(this: &Window) {
    this.status.set_text(
        "Flatpak isn't installed on this computer, so Flatpak apps can't be installed yet. \
         An administrator can add it with: sudo apt install flatpak",
    );
    this.status.add_css_class("unpack-error");
    this.install.set_sensitive(false);
}

/// Asks apt what else installing the .deb changes.
fn check_changes(this: Rc<Window>) {
    let path = this.path.clone();
    let package = match &*this.details.borrow() {
        Some(Details::Deb(package, _)) => package.package.clone(),
        _ => return,
    };
    glib::spawn_future_local(async move {
        let result = gio::spawn_blocking(move || deb::simulate(&path, &package))
            .await
            .unwrap_or_else(|_| Err("apt couldn't check the package.".into()));
        this.changes.remove_css_class("unpack-dim");
        match result {
            Ok(changes) => {
                let mut lines = Vec::new();
                if changes.installs.is_empty() && changes.upgrades.is_empty() {
                    lines.push("Nothing else needs to be installed.".to_string());
                }
                if !changes.installs.is_empty() {
                    lines.push(format!(
                        "Also installs {} {} it needs: {}.",
                        changes.installs.len(),
                        if changes.installs.len() == 1 {
                            "package"
                        } else {
                            "packages"
                        },
                        changes.installs.join(", ")
                    ));
                }
                if !changes.upgrades.is_empty() {
                    lines.push(format!("Also updates: {}.", changes.upgrades.join(", ")));
                }
                if !changes.removes.is_empty() {
                    lines.push(format!(
                        "Removes {} installed {}: {}.",
                        changes.removes.len(),
                        if changes.removes.len() == 1 {
                            "package"
                        } else {
                            "packages"
                        },
                        changes.removes.join(", ")
                    ));
                    this.changes.add_css_class("unpack-warning");
                }
                this.changes.set_text(&lines.join("\n"));
                this.install.set_sensitive(true);
            }
            Err(message) => {
                this.changes
                    .set_text(&format!("This package can't be installed:\n{message}"));
                this.changes.add_css_class("unpack-error");
                this.install.set_sensitive(false);
            }
        }
    });
}

fn append_log(this: &Window, line: &str) {
    let buffer = this.log.buffer();
    let mut end = buffer.end_iter();
    buffer.insert(&mut end, line);
    buffer.insert(&mut end, "\n");
    let mark = buffer.create_mark(None, &buffer.end_iter(), false);
    this.log.scroll_mark_onscreen(&mark);
    buffer.delete_mark(&mark);
}

/// apt's machine-readable progress ("pmstatus:PACKAGE:PERCENT:MESSAGE",
/// the package sometimes with ":ARCH"): the share done and the message.
fn apt_progress(line: &str) -> Option<(f64, String)> {
    let rest = line
        .strip_prefix("pmstatus:")
        .or_else(|| line.strip_prefix("dlstatus:"))?;
    let fields: Vec<&str> = rest.split(':').collect();
    let at = (1..fields.len()).find(|&i| fields[i].parse::<f64>().is_ok())?;
    let percent: f64 = fields[at].parse().ok()?;
    Some((percent / 100.0, fields[at + 1..].join(":")))
}

/// The last "NN%" in a line of Flatpak's output.
fn percent(line: &str) -> Option<f64> {
    let at = line.rfind('%')?;
    let digits: String = line[..at]
        .chars()
        .rev()
        .take_while(|c| c.is_ascii_digit())
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    digits
        .parse::<f64>()
        .ok()
        .map(|value| (value / 100.0).min(1.0))
}

fn start_install(this: &Rc<Window>) {
    let command: Vec<String> = match (&this.kind, &*this.details.borrow()) {
        (Kind::Deb, _) => {
            let helper = std::env::var("PATRONE_UNPACK_HELPER").unwrap_or_else(|_| HELPER.into());
            vec![
                "pkexec".into(),
                helper,
                this.path.to_string_lossy().into_owned(),
            ]
        }
        (Kind::Flatpak(kind), Some(Details::Flatpak(info, state))) => {
            let mut commands = flatpak::install_commands(
                *kind,
                &this.path,
                info,
                state.installed,
                state.add_flathub,
            );
            // Flathub first, if it has to be set up
            let last = commands.pop().unwrap_or_default();
            run_all(this, commands, last);
            return;
        }
        _ => return,
    };
    run_all(this, Vec::new(), command);
}

/// Runs commands one after another, then the main one, with their output
/// and progress in the window.
fn run_all(this: &Rc<Window>, before: Vec<Vec<String>>, command: Vec<String>) {
    this.install.set_sensitive(false);
    this.close.set_sensitive(false);
    this.status.remove_css_class("unpack-error");
    this.status.set_text(match this.kind {
        Kind::Deb => "Waiting for an administrator's password…",
        Kind::Flatpak(_) => "Starting…",
    });
    this.progress.set_visible(true);
    this.progress.set_fraction(0.0);
    this.progress.set_text(Some(""));
    this.expander.set_visible(true);
    let pulse = {
        let progress = this.progress.clone();
        glib::timeout_add_local(std::time::Duration::from_millis(150), move || {
            if progress.fraction() == 0.0 {
                progress.pulse();
            }
            glib::ControlFlow::Continue
        })
    };
    let this = this.clone();
    glib::spawn_future_local(async move {
        for step in before.iter().chain(std::iter::once(&command)) {
            let code = run_one(&this, step).await;
            if code != 0 {
                pulse.remove();
                finish(&this, Err(failure(this.kind, code)));
                return;
            }
        }
        pulse.remove();
        finish(&this, Ok(()));
    });
}

/// Why a command failed, for the status line.
fn failure(kind: Kind, code: i32) -> String {
    match (kind, code) {
        // pkexec: the password dialog was dismissed
        (Kind::Deb, 126) => String::new(),
        (Kind::Deb, 127) => "You aren't allowed to install software on this computer, \
                             or the password wasn't accepted."
            .into(),
        (_, -1) => "Installing couldn't start. The details below say why.".into(),
        (_, code) => format!("Installing failed (exit status {code}). The details below say why."),
    }
}

/// Runs one command and returns its exit status (-1 if it didn't start).
async fn run_one(this: &Rc<Window>, command: &[String]) -> i32 {
    append_log(this, &format!("$ {}", command.join(" ")));
    let arguments: Vec<&std::ffi::OsStr> = command.iter().map(std::ffi::OsStr::new).collect();
    let process = match gio::Subprocess::newv(
        &arguments,
        gio::SubprocessFlags::STDOUT_PIPE | gio::SubprocessFlags::STDERR_MERGE,
    ) {
        Ok(process) => process,
        Err(error) => {
            append_log(this, &format!("{} couldn't start: {error}", command[0]));
            return -1;
        }
    };
    {
        let this = this.clone();
        let mut started = false;
        read_lines(&process, |line| {
            if !started {
                started = true;
                this.status.set_text("Installing…");
            }
            if let Some((fraction, message)) = apt_progress(line) {
                this.progress.set_fraction(fraction.clamp(0.01, 1.0));
                this.progress.set_text(Some(&message));
                return;
            }
            if matches!(this.kind, Kind::Flatpak(_))
                && let Some(fraction) = percent(line)
            {
                this.progress.set_fraction(fraction.max(0.01));
            }
            append_log(&this, line);
        })
        .await;
    }
    let _ = process.wait_future().await;
    if process.has_exited() {
        process.exit_status()
    } else {
        -1
    }
}

/// Reads a process's output a line at a time, splitting on carriage returns
/// too (progress bars redraw a line with them).
async fn read_lines(process: &gio::Subprocess, mut each: impl FnMut(&str)) {
    let Some(stdout) = process.stdout_pipe() else {
        return;
    };
    let mut pending: Vec<u8> = Vec::new();
    loop {
        match stdout
            .read_bytes_future(8192, glib::Priority::DEFAULT)
            .await
        {
            Ok(bytes) if bytes.is_empty() => break,
            Ok(bytes) => {
                pending.extend_from_slice(&bytes);
                while let Some(at) = pending.iter().position(|&b| b == b'\n' || b == b'\r') {
                    let line: Vec<u8> = pending.drain(..=at).collect();
                    let text = String::from_utf8_lossy(&line[..line.len() - 1]);
                    if !text.trim().is_empty() {
                        each(text.trim_end());
                    }
                }
            }
            Err(_) => break,
        }
    }
    if !pending.is_empty() {
        each(String::from_utf8_lossy(&pending).trim_end());
    }
}

fn finish(this: &Rc<Window>, result: Result<(), String>) {
    this.close.set_sensitive(true);
    match result {
        Ok(()) => {
            this.progress.set_visible(false);
            this.status.set_text(match this.kind {
                Kind::Flatpak(flatpak::Kind::Repo) => "Added. Its apps can be installed now.",
                // Flatpak was only just installed: the session finds its apps
                // from the next sign-in
                Kind::Flatpak(_) if !flatpak::session_sees_flatpak_apps() => {
                    "Installed. Open starts it now; it appears in Start after you next sign in."
                }
                _ => "Installed.",
            });
            this.status.add_css_class("unpack-success");
            this.install.set_visible(false);
            this.open.set_visible(has_launcher(this));
            this.open.add_css_class("suggested-action");
            append_log(this, "Done.");
        }
        Err(message) if message.is_empty() => {
            this.progress.set_visible(false);
            this.status
                .set_text("Not installed: no administrator's password was given.");
            this.install.set_sensitive(true);
            append_log(this, "Cancelled.");
        }
        Err(message) => {
            this.progress.set_visible(false);
            this.status.set_text(&message);
            this.status.add_css_class("unpack-error");
            this.expander.set_expanded(true);
            this.install.set_label("Try again");
            this.install.set_sensitive(true);
        }
    }
}

fn has_launcher(this: &Window) -> bool {
    match &*this.details.borrow() {
        Some(Details::Deb(package, _)) => !deb::launchers(&package.package).is_empty(),
        Some(Details::Flatpak(info, _)) => kind_of(this) != flatpak::Kind::Repo && !info.is_runtime,
        None => false,
    }
}

/// Opens the app just installed.
fn launch(this: &Window) {
    let context = gtk::prelude::WidgetExt::display(&this.window).app_launch_context();
    match &*this.details.borrow() {
        Some(Details::Deb(package, _)) => {
            // Launcher entries by their IDs, the files' names
            let ids: Vec<String> = deb::launchers(&package.package)
                .iter()
                .filter_map(|path| Path::new(path).file_name())
                .map(|name| name.to_string_lossy().into_owned())
                .collect();
            for app in gio::AppInfo::all() {
                if app
                    .id()
                    .is_some_and(|id| ids.iter().any(|wanted| *wanted == id.as_str()))
                    && app.should_show()
                    && app.launch(&[], Some(&context)).is_ok()
                {
                    return;
                }
            }
        }
        Some(Details::Flatpak(info, _)) => {
            let _ = std::process::Command::new("flatpak")
                .args(["run", &info.id])
                .spawn();
        }
        None => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_lines() {
        assert_eq!(
            apt_progress("pmstatus:hello:20:Preparing hello (amd64)"),
            Some((0.2, "Preparing hello (amd64)".into()))
        );
        assert_eq!(
            apt_progress("pmstatus:libc6:amd64:57.5:Unpacking libc6:amd64"),
            Some((0.575, "Unpacking libc6:amd64".into()))
        );
        assert_eq!(apt_progress("Setting up hello (2.10) ..."), None);
        assert_eq!(percent("Installing 1/2… ████▌  45%  2.3 MB/s"), Some(0.45));
        assert_eq!(percent("no progress"), None);
        assert_eq!(host("https://dl.flathub.org/repo/"), "dl.flathub.org");
    }
}
