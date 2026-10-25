//! Runs the compositor as a window inside another desktop, through winit,
//! and draws each frame with OpenGL ES. In the dev VM that's Mesa's llvmpipe,
//! on the CPU.

use std::error::Error;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use smithay::backend::allocator::Fourcc;
use smithay::backend::egl::ffi::egl as egl_ffi;
use smithay::backend::renderer::damage::OutputDamageTracker;
use smithay::backend::renderer::element::memory::MemoryRenderBufferRenderElement;
use smithay::backend::renderer::element::surface::{
    WaylandSurfaceRenderElement, render_elements_from_surface_tree,
};
use smithay::backend::renderer::element::{Kind, render_elements};
use smithay::backend::renderer::gles::{GlesRenderer, GlesTexture};
use smithay::backend::renderer::{Bind, ExportMem, Frame, Offscreen, Renderer, TextureMapping};
use smithay::backend::winit::{self as smithay_winit, WinitEvent, WinitGraphicsBackend};
use smithay::desktop::space::SpaceRenderElements;
use smithay::desktop::utils::send_frames_surface_tree;
use smithay::input::pointer::{CursorIcon, CursorImageStatus, CursorImageSurfaceData};
use smithay::output::{Mode, Output, PhysicalProperties, Scale, Subpixel};
use smithay::reexports::calloop::LoopHandle;
use smithay::reexports::calloop::timer::{TimeoutAction, Timer};
use smithay::reexports::winit::dpi::LogicalSize;
use smithay::reexports::winit::platform::wayland::WindowAttributesExtWayland;
use smithay::reexports::winit::window::Window as WinitWindow;
use smithay::utils::{Buffer, Physical, Rectangle, Size, Transform};
use smithay::wayland::compositor::with_states;
use tracing::{error, info, warn};

use crate::App;
use crate::cli::Options;
use crate::state::Patrone;
use crate::wallpaper::{self, Wallpaper};

const TITLE: &str = "Patrone Desktop";
const APP_ID: &str = "org.patrone.Compositor";
const WINDOW_SIZE: LogicalSize<f64> = LogicalSize::new(1600.0, 900.0);
const REFRESH_MHZ: i32 = 60_000;
/// Frames are drawn at most this often, timed by our own event loop. GNOME
/// doesn't always send the frame callbacks winit waits for, which would stop
/// winit's redraws for good.
const FRAME_INTERVAL: Duration = Duration::from_micros(16_667);

render_elements! {
    OutputElement<=GlesRenderer>;
    Space = SpaceRenderElements<GlesRenderer, WaylandSurfaceRenderElement<GlesRenderer>>,
    Cursor = WaylandSurfaceRenderElement<GlesRenderer>,
    Wallpaper = MemoryRenderBufferRenderElement<GlesRenderer>,
}

pub struct Backend {
    winit: WinitGraphicsBackend<GlesRenderer>,
    event_loop: LoopHandle<'static, App>,
    output: Output,
    damage_tracker: OutputDamageTracker,
    wallpaper: Option<Wallpaper>,
    /// The cursor last set on our window: `None` when hidden.
    host_cursor: Option<CursorIcon>,
    screenshot: Option<PathBuf>,
    frame_scheduled: bool,
    last_frame: Instant,
}

/// Opens the window and adds its events to the event loop.
pub fn init(
    event_loop: LoopHandle<'static, App>,
    state: &mut Patrone,
    options: &Options,
) -> Result<Backend, Box<dyn Error>> {
    let attributes = WinitWindow::default_attributes()
        .with_title(TITLE)
        .with_name(APP_ID, "")
        .with_inner_size(WINDOW_SIZE);
    let (mut winit, events) = smithay_winit::init_from_attributes::<GlesRenderer>(attributes)?;
    disable_vsync(&mut winit)?;

    let output = Output::new(
        "nested".into(),
        PhysicalProperties {
            size: (0, 0).into(),
            subpixel: Subpixel::Unknown,
            make: "Patrone".into(),
            model: "Nested window".into(),
        },
    );
    output.create_global::<Patrone>(&state.display_handle);
    let mode = Mode {
        size: winit.window_size(),
        refresh: REFRESH_MHZ,
    };
    output.change_current_state(
        Some(mode),
        Some(Transform::Normal),
        Some(Scale::Fractional(winit.scale_factor())),
        Some((0, 0).into()),
    );
    output.set_preferred(mode);
    state.space.map_output(&output, (0, 0));

    let wallpaper = match Wallpaper::load(&options.wallpaper) {
        Ok(wallpaper) => Some(wallpaper),
        Err(err) => {
            warn!("no wallpaper: {}: {err}", options.wallpaper.display());
            None
        }
    };

    event_loop.insert_source(events, |event, _, app| {
        let App { state, backend } = app;
        backend.on_event(event, state);
    })?;

    Ok(Backend {
        damage_tracker: damage_tracker(mode.size, winit.scale_factor()),
        winit,
        event_loop,
        output,
        wallpaper,
        host_cursor: Some(CursorIcon::Default),
        screenshot: None,
        frame_scheduled: false,
        last_frame: Instant::now(),
    })
}

/// Makes presenting a frame return at once. Mesa otherwise waits for the
/// desktop around us to show the last one, and GNOME doesn't while the
/// screen is locked, which would freeze the compositor and its clients.
fn disable_vsync(winit: &mut WinitGraphicsBackend<GlesRenderer>) -> Result<(), Box<dyn Error>> {
    // The interval applies to the surface current on this thread.
    make_surface_current(winit)?;
    let display = winit
        .renderer()
        .egl_context()
        .display()
        .get_display_handle();
    // Safety: `display` is the live EGL display of the current context.
    if unsafe { egl_ffi::SwapInterval(**display, 0) } == egl_ffi::FALSE {
        warn!("couldn't turn off vsync; frames may stall while the desktop isn't drawing");
    }
    Ok(())
}

/// Makes our window's surface the current one on this thread, as setting the
/// swap interval and asking for the buffer age need. Smithay only does that
/// when a frame starts, so this starts one that draws nothing.
fn make_surface_current(
    winit: &mut WinitGraphicsBackend<GlesRenderer>,
) -> Result<(), Box<dyn Error>> {
    let size = winit.window_size();
    let (renderer, mut framebuffer) = winit.bind()?;
    let frame = renderer.render(&mut framebuffer, size, Transform::Normal)?;
    let _ = frame.finish()?;
    Ok(())
}

/// The GL framebuffer is upside down, so frames are drawn flipped.
fn damage_tracker(size: Size<i32, Physical>, scale: f64) -> OutputDamageTracker {
    OutputDamageTracker::new(size, scale, Transform::Flipped180)
}

impl Backend {
    /// Draws a frame soon, unless one is already on the way.
    pub fn schedule_frame(&mut self) {
        if self.frame_scheduled {
            return;
        }
        let delay = FRAME_INTERVAL.saturating_sub(self.last_frame.elapsed());
        let timer = self
            .event_loop
            .insert_source(Timer::from_duration(delay), |_, _, app| {
                let App { state, backend } = app;
                backend.frame_scheduled = false;
                backend.render(state);
                TimeoutAction::Drop
            });
        match timer {
            Ok(_) => self.frame_scheduled = true,
            Err(err) => error!("couldn't schedule a frame: {err}"),
        }
    }

    /// Saves the next frame to `path`.
    pub fn request_screenshot(&mut self, path: PathBuf) {
        self.screenshot = Some(path);
    }

    fn on_event(&mut self, event: WinitEvent, state: &mut Patrone) {
        match event {
            WinitEvent::Resized { size, scale_factor } => {
                let mode = Mode {
                    size,
                    refresh: REFRESH_MHZ,
                };
                self.output.change_current_state(
                    Some(mode),
                    None,
                    Some(Scale::Fractional(scale_factor)),
                    None,
                );
                self.damage_tracker = damage_tracker(size, scale_factor);
                state.output_resized();
                state.redraw_needed = true;
            }
            WinitEvent::Input(event) => state.process_input_event(event),
            WinitEvent::Redraw => self.schedule_frame(),
            WinitEvent::CloseRequested => state.loop_signal.stop(),
            WinitEvent::Focus(_) => {}
        }
    }

    fn render(&mut self, state: &mut Patrone) {
        self.last_frame = Instant::now();
        self.update_host_cursor(&state.cursor);
        let age = self.winit.buffer_age().unwrap_or(0);
        let (renderer, mut framebuffer) = match self.winit.bind() {
            Ok(bound) => bound,
            Err(err) => {
                error!("couldn't start a frame: {err}");
                return;
            }
        };
        let elements = output_elements(renderer, state, &self.output, self.wallpaper.as_ref());
        let damage = match self.damage_tracker.render_output(
            renderer,
            &mut framebuffer,
            age,
            &elements,
            wallpaper::BACKGROUND,
        ) {
            Ok(result) => result.damage.cloned(),
            Err(err) => {
                error!("couldn't draw a frame: {err:?}");
                None
            }
        };
        drop(framebuffer);
        if let Some(damage) = damage
            && let Err(err) = self.winit.submit(Some(&damage))
        {
            error!("couldn't show a frame: {err}");
        }

        if let Some(path) = self.screenshot.take() {
            let size = self.winit.window_size();
            let scale = self.output.current_scale().fractional_scale();
            match save_screenshot(self.winit.renderer(), &elements, size, scale, &path) {
                Ok(()) => info!("saved a screenshot to {}", path.display()),
                Err(err) => error!("couldn't save a screenshot to {}: {err}", path.display()),
            }
            if let Err(err) = make_surface_current(&mut self.winit) {
                error!("couldn't get back to drawing in the window: {err}");
            }
        }
        self.send_frames(state);
    }

    /// Tells clients they can draw their next frame.
    fn send_frames(&self, state: &mut Patrone) {
        let time = state.start_time.elapsed();
        let output = &self.output;
        for window in state.space.elements() {
            window.send_frame(output, time, Some(Duration::ZERO), |_, _| {
                Some(output.clone())
            });
        }
        if let CursorImageStatus::Surface(surface) = &state.cursor {
            send_frames_surface_tree(surface, output, time, Some(Duration::ZERO), |_, _| {
                Some(output.clone())
            });
        }
        state.space.refresh();
        state.popups.cleanup();
    }

    /// Named cursors are drawn by the desktop around us. Cursors that clients
    /// draw themselves are drawn into the frame, with the desktop's hidden.
    fn update_host_cursor(&mut self, cursor: &CursorImageStatus) {
        let wanted = match cursor {
            CursorImageStatus::Named(icon) => Some(*icon),
            CursorImageStatus::Hidden | CursorImageStatus::Surface(_) => None,
        };
        if wanted == self.host_cursor {
            return;
        }
        let window = self.winit.window();
        match wanted {
            Some(icon) => {
                window.set_cursor(icon);
                window.set_cursor_visible(true);
            }
            None => window.set_cursor_visible(false),
        }
        self.host_cursor = wanted;
    }
}

/// Everything on the output, front to back: the cursor, the windows, then
/// the wallpaper.
fn output_elements(
    renderer: &mut GlesRenderer,
    state: &Patrone,
    output: &Output,
    wallpaper: Option<&Wallpaper>,
) -> Vec<OutputElement> {
    let mut elements = Vec::new();
    let scale = output.current_scale().fractional_scale();
    if let CursorImageStatus::Surface(surface) = &state.cursor
        && let Some(pointer) = state.seat.get_pointer()
    {
        let hotspot = with_states(surface, |states| {
            states
                .data_map
                .get::<CursorImageSurfaceData>()
                .map(|data| data.lock().unwrap().hotspot)
                .unwrap_or_default()
        });
        let location = (pointer.current_location() - hotspot.to_f64()).to_physical(scale);
        let cursor = render_elements_from_surface_tree(
            renderer,
            surface,
            location.to_i32_round(),
            scale,
            1.0,
            Kind::Cursor,
        );
        elements.extend(cursor.into_iter().map(OutputElement::Cursor));
    }
    match state
        .space
        .render_elements_for_output(renderer, output, 1.0)
    {
        Ok(windows) => elements.extend(windows.into_iter().map(OutputElement::Space)),
        Err(err) => warn!("couldn't draw the windows: {err:?}"),
    }
    if let Some(wallpaper) = wallpaper {
        let output_size = state.output_geometry().unwrap_or_default().size;
        match wallpaper.element(renderer, output_size) {
            Ok(element) => elements.push(OutputElement::Wallpaper(element)),
            Err(err) => warn!("couldn't draw the wallpaper: {err}"),
        }
    }
    elements
}

/// Draws `elements` again, off screen, and writes them to a PNG file.
fn save_screenshot(
    renderer: &mut GlesRenderer,
    elements: &[OutputElement],
    size: Size<i32, Physical>,
    scale: f64,
    path: &Path,
) -> Result<(), Box<dyn Error>> {
    let buffer_size = Size::<i32, Buffer>::from((size.w, size.h));
    let mut texture: GlesTexture = renderer.create_buffer(Fourcc::Abgr8888, buffer_size)?;
    let mut target = renderer.bind(&mut texture)?;
    damage_tracker(size, scale).render_output(
        renderer,
        &mut target,
        0,
        elements,
        wallpaper::BACKGROUND,
    )?;
    let region = Rectangle::from_size(buffer_size);
    let mapping = renderer.copy_framebuffer(&target, region, Fourcc::Abgr8888)?;
    let flipped = mapping.flipped();
    let pixels = renderer.map_texture(&mapping)?;
    let mut image = image::RgbaImage::from_raw(size.w as u32, size.h as u32, pixels.to_vec())
        .ok_or("the frame is smaller than the window")?;
    if flipped {
        image::imageops::flip_vertical_in_place(&mut image);
    }
    for pixel in image.pixels_mut() {
        pixel.0[3] = u8::MAX;
    }
    image.save(path)?;
    Ok(())
}
