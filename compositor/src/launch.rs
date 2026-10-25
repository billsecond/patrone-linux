//! Starting terminals inside the compositor.

use std::ffi::OsString;
use std::io;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use tracing::{info, warn};

/// Tried in order. `--standalone` makes the GTK terminals start a new process
/// instead of opening a window in an instance already running elsewhere.
const TERMINALS: &[&[&str]] = &[
    &["patrone-terminal", "--standalone"],
    &["ptyxis", "--standalone"],
    &["foot"],
];

/// How long a terminal gets to fail before the next one is tried. A
/// patrone-terminal from before `--standalone` exits at once with an error.
pub const CHECK_AFTER: Duration = Duration::from_secs(2);

pub struct Launcher {
    socket_name: OsString,
    commands: Vec<Vec<String>>,
    /// The command in use: the first one that started and kept running.
    current: usize,
    children: Vec<Child>,
}

impl Launcher {
    /// `custom` is a shell command that replaces the list of terminals.
    pub fn new(socket_name: OsString, custom: Option<&str>) -> Self {
        let commands = match custom {
            Some(command) => vec![vec!["sh".into(), "-c".into(), command.into()]],
            None => TERMINALS
                .iter()
                .map(|command| command.iter().map(|arg| arg.to_string()).collect())
                .collect(),
        };
        Launcher {
            socket_name,
            commands,
            current: 0,
            children: Vec::new(),
        }
    }

    /// Starts a terminal: the current command, or the next one that exists.
    pub fn launch(&mut self) {
        self.children
            .retain_mut(|child| matches!(child.try_wait(), Ok(None)));
        while let Some(command) = self.commands.get(self.current) {
            match self.spawn(command) {
                Ok(child) => {
                    info!("started {}", command.join(" "));
                    self.children.push(child);
                    return;
                }
                Err(err) => {
                    info!("couldn't start {}: {err}", command[0]);
                    self.current += 1;
                }
            }
        }
        warn!("no terminal could be started");
        self.current = 0;
    }

    /// If the terminal started last has already failed, starts the next one
    /// and returns true, so it gets checked in turn.
    pub fn retry_if_failed(&mut self) -> bool {
        let Some(child) = self.children.last_mut() else {
            return false;
        };
        let Ok(Some(status)) = child.try_wait() else {
            return false;
        };
        let failed = &self.commands[self.current];
        if status.success() || self.current + 1 >= self.commands.len() {
            if !status.success() {
                warn!("{} exited: {status}", failed.join(" "));
            }
            return false;
        }
        info!("{} exited ({status}), trying the next terminal", failed[0]);
        self.children.pop();
        self.current += 1;
        self.launch();
        true
    }

    fn spawn(&self, command: &[String]) -> io::Result<Child> {
        Command::new(&command[0])
            .args(&command[1..])
            .env("WAYLAND_DISPLAY", &self.socket_name)
            // Keep clients from opening on the desktop around us instead.
            .env_remove("DISPLAY")
            .env_remove("WAYLAND_SOCKET")
            .env_remove("XDG_ACTIVATION_TOKEN")
            .env_remove("DESKTOP_STARTUP_ID")
            .stdin(Stdio::null())
            .spawn()
    }
}
