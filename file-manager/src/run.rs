//! Commands typed in the address bar, run in the folder on screen, as
//! Windows' File Explorer runs `cmd` or `notepad` typed there. Programs with
//! a window of their own start by themselves; anything else runs in a
//! terminal, through the user's shell (so pipes and `sudo` work), and the
//! terminal stays open on a shell in that folder afterwards.

use std::path::{Path, PathBuf};
use std::process::Command;

use gtk::{gio, glib, prelude::*};

/// Windows programs people type, and what stands in for them here when no
/// program of that name is installed.
const STAND_INS: &[(&str, &[&str])] = &[
    (
        "notepad",
        &["gnome-text-editor", "gedit", "mousepad", "kate"],
    ),
    ("calc", &["gnome-calculator", "kcalc"]),
    ("taskmgr", &["gnome-system-monitor", "plasma-systemmonitor"]),
    ("control", &["gnome-control-center"]),
    ("regedit", &["dconf-editor"]),
    ("mspaint", &["pinta", "drawing"]),
];

/// Windows' command prompts: typed alone, they open a terminal here.
const PROMPTS: &[&str] = &["cmd", "powershell", "wt", "terminal", "command"];

/// Runs `text` in `folder`. `Ok(false)` means it isn't a command at all;
/// `Err` says why a command didn't start.
pub fn run(text: &str, folder: &Path) -> Result<bool, String> {
    let Ok(argv) = glib::shell_parse_argv(text) else {
        return Ok(false);
    };
    let Some(first) = argv.first().map(|arg| arg.to_string_lossy().into_owned()) else {
        return Ok(false);
    };
    let name = first
        .strip_suffix(".exe")
        .or_else(|| first.strip_suffix(".EXE"))
        .unwrap_or(&first)
        .to_owned();
    let program = find(&name, folder).or_else(|| {
        STAND_INS
            .iter()
            .find(|(windows, _)| name.eq_ignore_ascii_case(windows))
            .and_then(|(_, programs)| programs.iter().find_map(glib::find_program_in_path))
    });
    let Some(program) = program else {
        if argv.len() == 1 && PROMPTS.iter().any(|p| name.eq_ignore_ascii_case(p)) {
            return terminal(folder, None)
                .then_some(true)
                .ok_or_else(no_terminal);
        }
        return Ok(false);
    };
    let started = if argv.len() == 1 && is_shell(&program) {
        // A shell typed alone: a terminal running it, here.
        terminal(folder, Some(&[program.to_string_lossy().into_owned()]))
    } else if has_window(&program) {
        let mut command = Command::new(&program);
        command.args(&argv[1..]).current_dir(folder);
        launch(command)
    } else {
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into());
        let script = format!("{text}\nexec {shell}");
        terminal(folder, Some(&[shell, "-c".into(), script]))
    };
    if started {
        Ok(true)
    } else {
        Err(format!("'{name}' couldn't be started."))
    }
}

fn no_terminal() -> String {
    "No terminal app was found.".into()
}

/// A program by name, or by a path relative to the folder.
fn find(name: &str, folder: &Path) -> Option<PathBuf> {
    if name.contains('/') {
        let path = folder.join(name);
        let executable = std::fs::metadata(&path).is_ok_and(|meta| {
            use std::os::unix::fs::PermissionsExt;
            meta.is_file() && meta.permissions().mode() & 0o111 != 0
        });
        return executable.then_some(path);
    }
    glib::find_program_in_path(name)
}

fn is_shell(program: &Path) -> bool {
    std::fs::read_to_string("/etc/shells").is_ok_and(|shells| {
        shells
            .lines()
            .filter_map(|line| Path::new(line.trim()).file_name())
            .any(|shell| Some(shell) == program.file_name())
    })
}

/// Whether an app menu entry starts this program without a terminal.
fn has_window(program: &Path) -> bool {
    gio::AppInfo::all().iter().any(|app| {
        let runs_it = glib::shell_parse_argv(app.commandline().unwrap_or_default())
            .unwrap_or_default()
            .iter()
            .filter(|arg| {
                let arg = arg.to_string_lossy();
                !arg.starts_with('-') && !arg.contains('=') && !arg.starts_with('%')
            })
            .take(2)
            .any(|arg| glib::find_program_in_path(arg).as_deref() == Some(program));
        runs_it && !runs_in_terminal(app)
    })
}

fn runs_in_terminal(app: &gio::AppInfo) -> bool {
    let Some(id) = app.id() else { return false };
    let file = glib::KeyFile::new();
    file.load_from_data_dirs(format!("applications/{id}"), glib::KeyFileFlags::NONE)
        .is_ok()
        && file.boolean("Desktop Entry", "Terminal").unwrap_or(false)
}

/// Opens a terminal in `folder`, running `command` if there is one.
pub fn terminal(folder: &Path, command: Option<&[String]>) -> bool {
    let mut xdg = Command::new("xdg-terminal-exec");
    xdg.current_dir(folder);
    if let Some(command) = command {
        xdg.args(command);
    }
    if launch(xdg) {
        return true;
    }
    let mut debian = Command::new("x-terminal-emulator");
    debian.current_dir(folder);
    if let Some(command) = command {
        debian.arg("-e").args(command);
    }
    launch(debian)
}

/// Starts a program and reaps it when it exits.
fn launch(mut command: Command) -> bool {
    match command.spawn() {
        Ok(mut child) => {
            std::thread::spawn(move || {
                let _ = child.wait();
            });
            true
        }
        Err(_) => false,
    }
}
