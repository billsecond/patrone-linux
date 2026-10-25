//! The Patrone compositor, the Wayland compositor of Patrone Desktop.
//!
//! For now it runs as a window inside another desktop: it draws the Patrone
//! wallpaper, starts a terminal, and lets windows be moved, resized and closed.

mod cli;
mod grabs;
mod handlers;
mod input;
mod launch;
mod maximize;
mod state;
mod wallpaper;
mod winit;

use std::error::Error;
use std::process::ExitCode;

use smithay::reexports::calloop::EventLoop;
use smithay::reexports::calloop::timer::{TimeoutAction, Timer};
use smithay::reexports::wayland_server::Display;
use tracing_subscriber::EnvFilter;

use crate::state::Patrone;

/// Everything the event loop works on.
pub struct App {
    pub state: Patrone,
    pub backend: winit::Backend,
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
    tracing_subscriber::fmt().with_env_filter(filter).init();
}

fn run(options: cli::Options) -> Result<(), Box<dyn Error>> {
    let mut event_loop = EventLoop::<App>::try_new()?;
    let display = Display::<Patrone>::new()?;
    let mut state = Patrone::new(display, &event_loop, &options)?;
    let backend = winit::init(event_loop.handle(), &mut state, &options)?;

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
