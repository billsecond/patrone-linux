//! The Patrone compositor, the Wayland compositor of Patrone Desktop.
//!
//! It runs as a window inside another desktop, or on the screen itself from
//! a virtual terminal or the login screen. It draws the Patrone wallpaper,
//! starts a terminal, and lets windows be moved, resized, maximized and
//! closed.

mod cli;
mod cursor;
mod grabs;
mod handlers;
mod input;
mod launch;
mod maximize;
mod render;
mod state;
mod udev;
mod wallpaper;
mod winit;

use std::error::Error;
use std::path::PathBuf;
use std::process::ExitCode;

use smithay::reexports::calloop::EventLoop;
use smithay::reexports::calloop::timer::{TimeoutAction, Timer};
use smithay::reexports::wayland_server::Display;
use tracing_subscriber::EnvFilter;

use crate::state::Patrone;

/// Everything the event loop works on.
pub struct App {
    pub state: Patrone,
    pub backend: Backend,
}

/// Where the compositor draws and gets its input from.
pub enum Backend {
    /// A window inside another desktop
    Winit(winit::Backend),
    /// The screen itself, with libinput for input
    Udev(udev::Backend),
}

impl Backend {
    /// Draws a frame soon, unless one is already on the way.
    pub fn schedule_frame(&mut self) {
        match self {
            Backend::Winit(backend) => backend.schedule_frame(),
            Backend::Udev(backend) => backend.schedule_frame(),
        }
    }

    /// Saves the next frame to `path`.
    pub fn request_screenshot(&mut self, path: PathBuf) {
        match self {
            Backend::Winit(backend) => backend.request_screenshot(path),
            Backend::Udev(backend) => backend.request_screenshot(path),
        }
    }
}

fn main() -> ExitCode {
    let options = match cli::parse(std::env::args_os()) {
        Ok(cli::Command::Run(options)) => options,
        Ok(cli::Command::Help) => {
            print!("{}", cli::USAGE);
            return ExitCode::SUCCESS;
        }
        Ok(cli::Command::Version) => {
            println!("patrone-compositor {}", env!("CARGO_PKG_VERSION"));
            return ExitCode::SUCCESS;
        }
        Err(message) => {
            eprint!("patrone-compositor: {message}\n\n{}", cli::USAGE);
            return ExitCode::FAILURE;
        }
    };
    init_logging();
    match run(options) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            tracing::error!("{err}");
            ExitCode::FAILURE
        }
    }
}

/// Logs to stderr. `RUST_LOG` overrides the default levels.
fn init_logging() {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("warn,patrone_compositor=info"));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .init();
}

fn run(options: cli::Options) -> Result<(), Box<dyn Error>> {
    let mut event_loop = EventLoop::<App>::try_new()?;
    let display = Display::<Patrone>::new()?;
    let mut state = Patrone::new(display, &event_loop, &options)?;
    let backend = if options.backend.nested() {
        tracing::info!("running as a window inside the desktop (--backend drm takes the screen)");
        Backend::Winit(winit::init(event_loop.handle(), &mut state, &options)?)
    } else {
        tracing::info!("taking over the screen");
        Backend::Udev(udev::init(event_loop.handle(), &mut state, &options)?)
    };

    state.launcher.launch();
    event_loop
        .handle()
        .insert_source(Timer::from_duration(launch::CHECK_AFTER), |_, _, app| {
            if app.state.launcher.retry_if_failed() {
                TimeoutAction::ToDuration(launch::CHECK_AFTER)
            } else {
                TimeoutAction::Drop
            }
        })?;

    if let Some(path) = options.screenshot {
        event_loop.handle().insert_source(
            Timer::from_duration(options.screenshot_after),
            move |_, _, app| {
                app.backend.request_screenshot(path.clone());
                app.state.redraw_needed = true;
                TimeoutAction::Drop
            },
        )?;
    }

    let mut app = App { state, backend };
    event_loop.run(None, &mut app, |app| {
        if std::mem::take(&mut app.state.redraw_needed) {
            app.backend.schedule_frame();
        }
        if let Err(err) = app.state.display_handle.flush_clients() {
            tracing::warn!("couldn't flush clients: {err}");
        }
    })?;
    Ok(())
}
