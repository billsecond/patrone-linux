//! What can be done to processes: end them (alone or with everything they
//! started), change their priority, find their program's file, see their
//! properties; and running a new task. Another account's processes take
//! administrator rights, which pkexec asks for.

use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{gdk, gio, glib};

use crate::row::{APP, Row};
use crate::{Context, format, model, procs, sample};

/// How long ended processes get to close before they're stopped.
const GRACE_SECONDS: u32 = 3;

/// "End task" and "End process tree". Apps end at once, as people expect;
/// anything else asks first, since ending it can upset the desktop.
pub fn end_task(context: &Rc<Context>, row: &Row, tree: bool) {
    let mut targets = row.processes();
    if tree {
        targets = procs::descendants(&context.model.borrow().list, &targets);
    }
    // Never the system's first process or the kernel's threads
    let kernel: Vec<(i32, u64)> = context
        .model
        .borrow()
        .list
        .iter()
        .filter(|process| process.kernel)
        .map(|process| process.key())
        .collect();
    targets.retain(|target| target.0 > 1 && !kernel.contains(target));
    if targets.is_empty() {
        return;
    }
    let name = model::plain_name(row);
    if !tree && row.kind() == APP {
        end(context, targets);
        return;
    }
    let (detail, button) = if tree {
        (
            format!(
                "This ends {name} and every process it started, {} in all. Anything they haven't saved is lost, and ending a background or system process can stop the desktop or the computer from working properly.",
                targets.len()
            ),
            "End process tree",
        )
    } else {
        (
            format!(
                "Ending a background or system process can stop the desktop or the computer from working properly. Anything {name} hasn't saved is lost."
            ),
            "End process",
        )
    };
    let dialog = gtk::AlertDialog::builder()
        .modal(true)
        .message(format!("End {name}?"))
        .detail(detail)
        .buttons(["Cancel", button])
        .cancel_button(0)
        .default_button(0)
        .build();
    let context = context.clone();
    dialog.choose(
        Some(&context.window.clone()),
        gio::Cancellable::NONE,
        move |result| {
            if matches!(result, Ok(1)) {
                end(&context, targets);
            }
        },
    );
}

/// Asks each process to close, then stops those still running a few
/// seconds later. Processes of other accounts are asked through pkexec.
fn end(context: &Rc<Context>, targets: Vec<(i32, u64)>) {
    let mut asked = Vec::new();
    let mut denied = Vec::new();
    for (pid, start) in targets {
        if !procs::still_running(pid, start) {
            continue;
        }
        // SAFETY: kill(2) on a PID just checked to be this process.
        if unsafe { libc::kill(pid, libc::SIGTERM) } == 0 {
            asked.push((pid, start));
        } else if std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM) {
            denied.push(pid);
        }
    }
    if !asked.is_empty() {
        glib::timeout_add_seconds_local_once(GRACE_SECONDS, move || {
            for (pid, start) in asked {
                if procs::still_running(pid, start) {
                    // SAFETY: as above, checked again just now.
                    unsafe { libc::kill(pid, libc::SIGKILL) };
                }
            }
        });
    }
    if !denied.is_empty() {
        let mut command = vec![
            "pkexec".to_string(),
            "kill".to_string(),
            "-TERM".to_string(),
        ];
        command.extend(denied.iter().map(|pid| pid.to_string()));
        run_as_administrator(context, command, "Couldn't end the process");
    }
}

/// Runs a command through pkexec, which asks for an administrator's
/// password, and says so if it fails (not if the person cancels).
pub fn run_as_administrator(context: &Rc<Context>, command: Vec<String>, failure: &'static str) {
    let arguments: Vec<&std::ffi::OsStr> = command.iter().map(std::ffi::OsStr::new).collect();
    let process = match gio::Subprocess::newv(&arguments, gio::SubprocessFlags::STDERR_PIPE) {
        Ok(process) => process,
        Err(error) => {
            message(&context.window, failure, &error.to_string());
            return;
        }
    };
    let window = context.window.clone();
    glib::spawn_future_local(async move {
        let output = process.communicate_utf8_future(None).await;
        let status = process.exit_status();
        // pkexec: 126 when the password dialog was dismissed, 127 when not
        // authorized
        if process.has_exited() && status != 0 && status != 126 && status != 127 {
            let detail = output
                .ok()
                .and_then(|(_, error)| error)
                .map(|e| e.to_string())
                .unwrap_or_default();
            message(&window, failure, detail.trim());
        }
    });
}

/// A message with an OK button.
pub fn message(window: &impl IsA<gtk::Window>, title: &str, detail: &str) {
    gtk::AlertDialog::builder()
        .modal(true)
        .message(title)
        .detail(detail)
        .build()
        .show(Some(window));
}

/// Sets a process's priority (its nice value). Raising it, or changing
/// another account's process, takes administrator rights.
pub fn set_priority(context: &Rc<Context>, row: &Row, nice: i32) {
    let mut denied = Vec::new();
    for (pid, start) in row.processes() {
        if !procs::still_running(pid, start) {
            continue;
        }
        // SAFETY: setpriority(2) on a process just checked.
        if unsafe { libc::setpriority(libc::PRIO_PROCESS, pid as libc::id_t, nice) } != 0 {
            denied.push(pid);
        }
    }
    if !denied.is_empty() {
        let mut command = vec![
            "pkexec".to_string(),
            "renice".to_string(),
            "-n".to_string(),
            nice.to_string(),
            "-p".to_string(),
        ];
        command.extend(denied.iter().map(|pid| pid.to_string()));
        run_as_administrator(context, command, "Couldn't change the priority");
    }
}

/// The priorities offered, as nice values.
pub const PRIORITIES: [(&str, &str, i32); 5] = [
    ("high", "High", -10),
    ("above", "Above normal", -5),
    ("normal", "Normal", 0),
    ("below", "Below normal", 5),
    ("low", "Low", 15),
];

/// The program file a process runs.
fn program_path(pid: i32, command: &str) -> Option<PathBuf> {
    if let Ok(path) = fs::read_link(format!("/proc/{pid}/exe")) {
        let text = path.to_string_lossy();
        let text = text.strip_suffix(" (deleted)").unwrap_or(&text);
        return Some(PathBuf::from(text));
    }
    // Another account's process: from its command line
    let program = command
        .split(' ')
        .next()
        .filter(|program| !program.is_empty())?;
    if program.starts_with('/') {
        return Path::new(program).exists().then(|| PathBuf::from(program));
    }
    glib::find_program_in_path(program)
}

/// "Open file location": shows the program's file in the file manager,
/// or opens its folder.
pub fn open_location(context: &Rc<Context>, row: &Row) {
    let Some(path) = program_path(row.pid(), &row.command()) else {
        message(
            &context.window,
            "Couldn't find the program's file",
            "The process may have ended, or its file is hidden from your account.",
        );
        return;
    };
    let uri = gio::File::for_path(&path).uri().to_string();
    let folder = path
        .parent()
        .map(|folder| gio::File::for_path(folder).uri().to_string());
    let Ok(bus) = gio::bus_get_sync(gio::BusType::Session, gio::Cancellable::NONE) else {
        return;
    };
    bus.call(
        Some("org.freedesktop.FileManager1"),
        "/org/freedesktop/FileManager1",
        "org.freedesktop.FileManager1",
        "ShowItems",
        Some(&(vec![uri], "").to_variant()),
        None,
        gio::DBusCallFlags::NONE,
        5000,
        gio::Cancellable::NONE,
        move |result| {
            if result.is_err()
                && let Some(folder) = folder
            {
                let _ = gio::AppInfo::launch_default_for_uri(&folder, gio::AppLaunchContext::NONE);
            }
        },
    );
}

/// A process's properties, in a window of their own.
pub fn properties(context: &Rc<Context>, row: &Row) {
    let model = context.model.borrow();
    let Some(process) = model.find(row.pid()).cloned() else {
        message(&context.window, "This process has ended", "");
        return;
    };
    let ticks = context.ticks_per_second;
    let mut lines: Vec<(&str, String)> = Vec::new();
    if let Some(children) = row.children() {
        let app_id = row.app_id();
        lines.push(("Name", model::plain_name(row)));
        if !app_id.is_empty() {
            let file = crate::desktop::find(&app_id).map(|entry| entry.path);
            lines.push((
                "App",
                file.map(|file| file.display().to_string())
                    .unwrap_or(format!("{app_id}.desktop")),
            ));
        }
        lines.push(("Processes", children.n_items().max(1).to_string()));
        lines.push(("Memory, all processes", format::megabytes(row.memory())));
        lines.push(("", String::new()));
        lines.push(("Main process", process.name.to_string()));
    } else {
        lines.push(("Name", process.name.to_string()));
    }
    let parent = model
        .find(process.ppid)
        .map(|parent| format!("{} ({})", parent.name, parent.pid))
        .unwrap_or_else(|| process.ppid.to_string());
    let placement = model::place(&process.cgroup);
    let started = sample::boot_time()
        .map(|boot| boot + (process.start as f64 / ticks) as i64)
        .and_then(|when| glib::DateTime::from_unix_local(when).ok())
        .and_then(|when| when.format("%x %X").ok())
        .map(|text| text.to_string())
        .unwrap_or_default();
    let program = fs::read_link(format!("/proc/{}/exe", process.pid))
        .map(|path| path.display().to_string())
        .unwrap_or_else(|_| "Not readable".into());
    let folder = fs::read_link(format!("/proc/{}/cwd", process.pid))
        .map(|path| path.display().to_string())
        .unwrap_or_else(|_| "Not readable".into());
    lines.extend([
        ("Process ID", process.pid.to_string()),
        ("Started by", parent),
        ("Account", process.user.to_string()),
        ("Status", row.description()),
        ("Started", started),
        (
            "Command line",
            if process.cmdline.is_empty() {
                "None (a kernel thread)".into()
            } else {
                process.cmdline.to_string()
            },
        ),
        ("Program", program),
        ("Working folder", folder),
        (
            "Runs in",
            if placement.unit.is_empty() {
                "The kernel".into()
            } else {
                placement.unit.clone()
            },
        ),
        (
            "Processor time",
            format::cpu_time(process.cpu_ticks as f64 / ticks),
        ),
        ("Private memory", format::kilobytes(process.memory)),
        ("Resident memory", format::kilobytes(process.resident)),
        ("Virtual memory", format::kilobytes(process.virtual_size)),
        ("Threads", process.threads.to_string()),
        (
            "Priority",
            PRIORITIES
                .iter()
                .find(|&&(_, _, nice)| nice == process.nice)
                .map(|&(_, name, _)| name.to_string())
                .unwrap_or_else(|| format!("Nice value {}", process.nice)),
        ),
    ]);
    drop(model);

    let grid = gtk::Grid::builder()
        .column_spacing(18)
        .row_spacing(6)
        .build();
    for (index, (caption, value)) in lines.iter().enumerate() {
        grid.attach(
            &gtk::Label::builder()
                .label(*caption)
                .xalign(0.0)
                .yalign(0.0)
                .css_classes(["tm-info-caption"])
                .build(),
            0,
            index as i32,
            1,
            1,
        );
        grid.attach(
            &gtk::Label::builder()
                .label(value)
                .xalign(0.0)
                .wrap(true)
                .wrap_mode(gtk::pango::WrapMode::WordChar)
                .width_chars(36)
                .max_width_chars(60)
                .selectable(true)
                .build(),
            1,
            index as i32,
            1,
            1,
        );
    }
    let window = gtk::Window::builder()
        .transient_for(&context.window)
        .title(format!("{} properties", model::plain_name(row)))
        .default_width(560)
        .build();
    let close = gtk::Button::builder()
        .label("Close")
        .halign(gtk::Align::End)
        .css_classes(["tm-dialog-button"])
        .build();
    close.connect_clicked(glib::clone!(
        #[weak]
        window,
        move |_| window.close()
    ));
    let content = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(18)
        .css_classes(["tm-dialog"])
        .build();
    content.append(&grid);
    content.append(&close);
    window.set_child(Some(&content));
    escape_closes(&window);
    window.present();
}

/// "Run new task": a program, a folder, a file or a web address.
pub fn run_new_task(context: &Rc<Context>) {
    let window = gtk::Window::builder()
        .transient_for(&context.window)
        .modal(true)
        .title("Run new task")
        .resizable(false)
        .default_width(460)
        .build();
    let content = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(12)
        .css_classes(["tm-dialog"])
        .build();
    content.append(
        &gtk::Label::builder()
            .label("Type the name of a program, folder, document or web address, and it opens.")
            .xalign(0.0)
            .wrap(true)
            .max_width_chars(50)
            .build(),
    );
    let entry = gtk::Entry::builder()
        .hexpand(true)
        .activates_default(true)
        .build();
    let line = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    line.append(&gtk::Label::new(Some("Open:")));
    line.append(&entry);
    content.append(&line);
    let admin = gtk::CheckButton::with_label("Run it as an administrator");
    content.append(&admin);
    let error = gtk::Label::builder()
        .xalign(0.0)
        .wrap(true)
        .visible(false)
        .css_classes(["tm-error"])
        .build();
    content.append(&error);

    let buttons = gtk::Box::builder()
        .spacing(8)
        .halign(gtk::Align::End)
        .build();
    let browse = gtk::Button::builder()
        .label("Browse…")
        .css_classes(["tm-dialog-button"])
        .build();
    let cancel = gtk::Button::builder()
        .label("Cancel")
        .css_classes(["tm-dialog-button"])
        .build();
    let ok = gtk::Button::builder()
        .label("OK")
        .css_classes(["tm-dialog-button", "suggested-action"])
        .build();
    buttons.append(&browse);
    buttons.append(&ok);
    buttons.append(&cancel);
    content.append(&buttons);
    window.set_child(Some(&content));
    window.set_default_widget(Some(&ok));

    cancel.connect_clicked(glib::clone!(
        #[weak]
        window,
        move |_| window.close()
    ));
    browse.connect_clicked(glib::clone!(
        #[weak]
        window,
        #[weak]
        entry,
        move |_| {
            gtk::FileDialog::builder()
                .title("Browse")
                .modal(true)
                .build()
                .open(Some(&window), gio::Cancellable::NONE, move |file| {
                    if let Some(path) = file.ok().and_then(|file| file.path()) {
                        entry.set_text(&path.to_string_lossy());
                    }
                });
        }
    ));
    ok.connect_clicked(glib::clone!(
        #[weak]
        window,
        #[weak]
        entry,
        #[weak]
        admin,
        #[weak]
        error,
        move |_| match run(&entry.text(), admin.is_active(), &window) {
            Ok(()) => window.close(),
            Err(text) => {
                error.set_label(&text);
                error.set_visible(true);
            }
        }
    ));
    escape_closes(&window);
    window.present();
}

fn run(text: &str, admin: bool, window: &gtk::Window) -> Result<(), String> {
    let text = text.trim();
    if text.is_empty() {
        return Err("Type something to open.".into());
    }
    let launch = gdk::Display::default().map(|display| display.app_launch_context());
    let expanded = match text.strip_prefix("~/") {
        Some(rest) => glib::home_dir().join(rest),
        None if text == "~" => glib::home_dir(),
        None => PathBuf::from(text),
    };
    // A web address, a folder or a document opens in its app
    if !admin && (text.contains("://") || (expanded.exists() && !is_program(&expanded))) {
        let uri = if text.contains("://") {
            text.to_string()
        } else {
            gio::File::for_path(&expanded).uri().to_string()
        };
        return gio::AppInfo::launch_default_for_uri(&uri, launch.as_ref())
            .map_err(|error| error.to_string());
    }
    let command = if admin {
        format!("pkexec {text}")
    } else {
        text.to_string()
    };
    let program = glib::shell_parse_argv(&command).map_err(|error| error.to_string())?;
    let first = program
        .first()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default();
    if !first.contains('/') && glib::find_program_in_path(&first).is_none() {
        return Err(format!("There's no program called “{first}”."));
    }
    let info = gio::AppInfo::create_from_commandline(&command, None, gio::AppInfoCreateFlags::NONE)
        .map_err(|error| error.to_string())?;
    let _ = window;
    info.launch(&[], launch.as_ref())
        .map_err(|error| error.to_string())
}

fn is_program(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    path.is_file() && fs::metadata(path).is_ok_and(|meta| meta.permissions().mode() & 0o111 != 0)
}

/// Escape closes a dialog window.
pub fn escape_closes(window: &gtk::Window) {
    let keys = gtk::EventControllerKey::new();
    keys.connect_key_pressed(glib::clone!(
        #[weak]
        window,
        #[upgrade_or]
        glib::Propagation::Proceed,
        move |_, key, _, _| {
            if key == gdk::Key::Escape {
                window.close();
                glib::Propagation::Stop
            } else {
                glib::Propagation::Proceed
            }
        }
    ));
    window.add_controller(keys);
}
