//! The compositor's state: the Wayland globals, the windows and the seat.

use std::ffi::OsString;
use std::io;
use std::sync::Arc;
use std::time::Instant;

use smithay::backend::renderer::utils::with_renderer_surface_state;
use smithay::desktop::{PopupManager, Space, Window, WindowSurfaceType};
use smithay::input::keyboard::{Keycode, XkbConfig};
use smithay::input::pointer::{CursorIcon, CursorImageStatus};
use smithay::input::{Seat, SeatState};
use smithay::reexports::calloop::generic::Generic;
use smithay::reexports::calloop::{EventLoop, Interest, LoopHandle, LoopSignal, Mode, PostAction};
use smithay::reexports::wayland_server::backend::{ClientData, ClientId, DisconnectReason};
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::reexports::wayland_server::{Display, DisplayHandle};
use smithay::utils::{Logical, Point, Rectangle, SERIAL_COUNTER, Serial};
use smithay::wayland::compositor::{CompositorClientState, CompositorState};
use smithay::wayland::cursor_shape::CursorShapeManagerState;
use smithay::wayland::output::OutputManagerState;
use smithay::wayland::selection::data_device::DataDeviceState;
use smithay::wayland::shell::xdg::XdgShellState;
use smithay::wayland::shm::ShmState;
use smithay::wayland::socket::ListeningSocketSource;
use tracing::info;

use crate::App;
use crate::cli::{Modifier, Options};
use crate::grabs::Resize;
use crate::launch::Launcher;

/// New windows open this far apart, so they don't hide each other.
const CASCADE_STEP: i32 = 32;
const CASCADE_STEPS: i32 = 8;

pub struct Patrone {
    pub display_handle: DisplayHandle,
    pub loop_signal: LoopSignal,
    pub start_time: Instant,

    pub compositor_state: CompositorState,
    pub xdg_shell_state: XdgShellState,
    pub shm_state: ShmState,
    pub seat_state: SeatState<Self>,
    pub data_device_state: DataDeviceState,
    pub seat: Seat<Self>,

    pub space: Space<Window>,
    pub popups: PopupManager,
    /// Toplevels that haven't drawn anything yet. They're placed once they do.
    pub unmapped: Vec<Window>,
    /// The window being resized, until it has drawn at its final size.
    pub resize: Option<Resize>,
    pub cursor: CursorImageStatus,
    pub modifier: Modifier,
    /// Keys pressed for a shortcut, whose release clients mustn't see either.
    pub suppressed_keys: Vec<Keycode>,
    pub launcher: Launcher,
    pub redraw_needed: bool,
}

impl Patrone {
    pub fn new(
        display: Display<Self>,
        event_loop: &EventLoop<'static, App>,
        options: &Options,
    ) -> io::Result<Self> {
        let dh = display.handle();
        let compositor_state = CompositorState::new::<Self>(&dh);
        let xdg_shell_state = XdgShellState::new::<Self>(&dh);
        let shm_state = ShmState::new::<Self>(&dh, vec![]);
        OutputManagerState::new_with_xdg_output::<Self>(&dh);
        CursorShapeManagerState::new::<Self>(&dh);
        let data_device_state = DataDeviceState::new::<Self>(&dh);
        let mut seat_state = SeatState::new();
        let mut seat = seat_state.new_wl_seat(&dh, "seat0");
        seat.add_keyboard(XkbConfig::default(), 200, 25)
            .map_err(io::Error::other)?;
        seat.add_pointer();

        let socket_name = listen(display, &event_loop.handle())?;
        info!("listening on {}", socket_name.to_string_lossy());

        Ok(Patrone {
            display_handle: dh,
            loop_signal: event_loop.get_signal(),
            start_time: Instant::now(),
            launcher: Launcher::new(socket_name, options.terminal.as_deref()),
            compositor_state,
            xdg_shell_state,
            shm_state,
            seat_state,
            data_device_state,
            seat,
            space: Space::default(),
            popups: PopupManager::default(),
            unmapped: Vec::new(),
            resize: None,
            cursor: CursorImageStatus::Named(CursorIcon::Default),
            modifier: options.modifier,
            suppressed_keys: Vec::new(),
            redraw_needed: true,
        })
    }

    /// The surface under `location` and where that surface is.
    pub fn surface_under(
        &self,
        location: Point<f64, Logical>,
    ) -> Option<(WlSurface, Point<f64, Logical>)> {
        let (window, render_location) = self.space.element_under(location)?;
        window
            .surface_under(location - render_location.to_f64(), WindowSurfaceType::ALL)
            .map(|(surface, offset)| (surface, (offset + render_location).to_f64()))
    }

    pub fn output_geometry(&self) -> Option<Rectangle<i32, Logical>> {
        let output = self.space.outputs().next()?;
        self.space.output_geometry(output)
    }

    /// The mapped window whose toplevel is `surface`.
    pub fn window_for(&self, surface: &WlSurface) -> Option<Window> {
        self.space
            .elements()
            .find(|window| window.toplevel().is_some_and(|t| t.wl_surface() == surface))
            .cloned()
    }

    pub fn focused_window(&self) -> Option<Window> {
        let focus = self.seat.get_keyboard()?.current_focus()?;
        self.window_for(&focus)
    }

    /// Raises `window` and gives it the keyboard, or takes the keyboard away
    /// from every window.
    pub fn focus(&mut self, window: Option<&Window>, serial: Serial) {
        if let Some(window) = window {
            self.space.raise_element(window, true);
        } else {
            for window in self.space.elements() {
                window.set_activated(false);
            }
        }
        let surface = window
            .and_then(|w| w.toplevel())
            .map(|t| t.wl_surface().clone());
        if let Some(keyboard) = self.seat.get_keyboard() {
            keyboard.set_focus(self, surface, serial);
        }
        for toplevel in self.space.elements().filter_map(|w| w.toplevel()) {
            toplevel.send_pending_configure();
        }
    }

    /// Focuses the window on top, if there is one.
    pub fn focus_top_window(&mut self) {
        let top = self.space.elements().next_back().cloned();
        self.focus(top.as_ref(), SERIAL_COUNTER.next_serial());
    }

    /// Called on every commit of a toplevel that isn't mapped yet: sends its
    /// first configure, then places and focuses it once it has drawn.
    pub fn commit_unmapped(&mut self, surface: &WlSurface) {
        let Some(index) = self
            .unmapped
            .iter()
            .position(|w| w.toplevel().is_some_and(|t| t.wl_surface() == surface))
        else {
            return;
        };
        let window = &self.unmapped[index];
        let Some(toplevel) = window.toplevel() else {
            return;
        };
        if !toplevel.is_initial_configure_sent() {
            toplevel.send_configure();
            return;
        }
        window.on_commit();
        let has_buffer = with_renderer_surface_state(surface, |state| state.buffer().is_some());
        if has_buffer == Some(true) {
            let window = self.unmapped.remove(index);
            let location = self.placement(&window);
            self.space.map_element(window.clone(), location, true);
            self.focus(Some(&window), SERIAL_COUNTER.next_serial());
        }
    }

    /// Where a new window goes: centered, a step down and right from the last
    /// one, and never with its top-left corner off screen.
    fn placement(&self, window: &Window) -> Point<i32, Logical> {
        let output = self.output_geometry().unwrap_or_default();
        let size = window.geometry().size;
        let step = CASCADE_STEP * (self.space.elements().len() as i32 % CASCADE_STEPS);
        let x = output.loc.x + (output.size.w - size.w) / 2 + step;
        let y = output.loc.y + (output.size.h - size.h) / 2 + step;
        (x.max(output.loc.x), y.max(output.loc.y)).into()
    }
}

/// Opens the Wayland socket (`wayland-1` or the next free name) and serves
/// clients from the event loop.
fn listen(
    display: Display<Patrone>,
    event_loop: &LoopHandle<'static, App>,
) -> io::Result<OsString> {
    let socket = ListeningSocketSource::new_auto().map_err(io::Error::other)?;
    let socket_name = socket.socket_name().to_os_string();
    event_loop
        .insert_source(socket, |stream, _, app| {
            let client = Arc::new(ClientState::default());
            if let Err(err) = app.state.display_handle.insert_client(stream, client) {
                tracing::warn!("couldn't add a client: {err}");
            }
        })
        .map_err(|err| io::Error::other(err.to_string()))?;
    event_loop
        .insert_source(
            Generic::new(display, Interest::READ, Mode::Level),
            |_, display, app| {
                // Safety: the display is never dropped while the loop runs.
                unsafe { display.get_mut().dispatch_clients(&mut app.state)? };
                Ok(PostAction::Continue)
            },
        )
        .map_err(|err| io::Error::other(err.to_string()))?;
    Ok(socket_name)
}

#[derive(Default)]
pub struct ClientState {
    pub compositor_state: CompositorClientState,
}

impl ClientData for ClientState {
    fn initialized(&self, _client_id: ClientId) {}
    fn disconnected(&self, _client_id: ClientId, _reason: DisconnectReason) {}
}
