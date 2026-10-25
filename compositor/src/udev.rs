//! Runs the compositor on the screen itself: frames go straight to the
//! display through DRM/KMS, input comes from libinput, and logind (through
//! libseat) hands over the screen and input devices when started from a
//! virtual terminal or the login screen.
//!
//! One GPU and one screen for now. Without a GPU, as on Hyper-V, Mesa draws
//! OpenGL ES on the CPU (kms_swrast) into buffers the display can show.
//! Based on Smithay's anvil example (MIT), cut down to that case.

use std::error::Error;
use std::path::PathBuf;

use smithay::backend::allocator::format::FormatSet;
use smithay::backend::allocator::gbm::{GbmAllocator, GbmBufferFlags, GbmDevice};
use smithay::backend::allocator::{Fourcc, Modifier};
use smithay::backend::drm::compositor::FrameFlags;
use smithay::backend::drm::exporter::gbm::GbmFramebufferExporter;
use smithay::backend::drm::output::{DrmOutput, DrmOutputManager, DrmOutputRenderElements};
use smithay::backend::drm::{DrmDevice, DrmDeviceFd, DrmEvent};
use smithay::backend::egl::{EGLContext, EGLDevice, EGLDisplay};
use smithay::backend::libinput::{LibinputInputBackend, LibinputSessionInterface};
use smithay::backend::renderer::gles::GlesRenderer;
use smithay::backend::session::libseat::LibSeatSession;
use smithay::backend::session::{Event as SessionEvent, Session};
use smithay::backend::udev::{all_gpus, primary_gpu};
use smithay::output::{Mode as OutputMode, Output, PhysicalProperties};
use smithay::reexports::calloop::LoopHandle;
use smithay::reexports::drm::Device as _;
use smithay::reexports::drm::control::{Device as _, ModeTypeFlags, connector, crtc};
use smithay::reexports::input::Libinput;
use smithay::reexports::rustix::fs::OFlags;
use smithay::utils::DeviceFd;
use tracing::{error, info, warn};

use crate::cli::Options;
use crate::cursor::Cursors;
use crate::render::{OutputElement, output_elements, save_screenshot};
use crate::state::Patrone;
use crate::wallpaper::{self, Wallpaper};
use crate::{App, Backend as AnyBackend};

/// The formats the screen's buffers may have, best first.
const COLOR_FORMATS: [Fourcc; 2] = [Fourcc::Xrgb8888, Fourcc::Argb8888];

type Allocator = GbmAllocator<DrmDeviceFd>;
type Exporter = GbmFramebufferExporter<DrmDeviceFd>;

pub struct Backend {
    session: LibSeatSession,
    libinput: Libinput,
    drm: DrmOutputManager<Allocator, Exporter, (), DrmDeviceFd>,
    screen: DrmOutput<Allocator, Exporter, (), DrmDeviceFd>,
    renderer: GlesRenderer,
    output: Output,
    wallpaper: Option<Wallpaper>,
    cursors: Cursors,
    event_loop: LoopHandle<'static, App>,
    /// A frame waits to be shown; the next is drawn after its vblank.
    frame_pending: bool,
    redraw_after_vblank: bool,
    render_scheduled: bool,
    screenshot: Option<PathBuf>,
}

/// Takes over the screen and the input devices, and adds their events to
/// the event loop.
pub fn init(
    event_loop: LoopHandle<'static, App>,
    state: &mut Patrone,
    options: &Options,
) -> Result<Backend, Box<dyn Error>> {
    let (mut session, session_events) = LibSeatSession::new().map_err(|err| {
        format!("no session for the screen ({err}); start it from a virtual terminal or the login screen")
    })?;
    let seat = session.seat();
    let path = gpu_path(&seat)?;
    let fd = session
        .open(
            &path,
            OFlags::RDWR | OFlags::CLOEXEC | OFlags::NOCTTY | OFlags::NONBLOCK,
        )
        .map_err(|err| {
            format!(
                "couldn't open {} ({err}). Only the session on the screen can: start the \
                 compositor from a virtual terminal (Ctrl+Alt+F3) or the login screen, not \
                 over SSH or inside another desktop",
                path.display()
            )
        })?;
    let fd = DrmDeviceFd::new(DeviceFd::from(fd));
    let (drm, drm_events) = DrmDevice::new(fd.clone(), true)?;
    let gbm = GbmDevice::new(fd)?;
    // Safety: the GBM device stays open as long as the display uses it.
    let display = unsafe { EGLDisplay::new(gbm.clone())? };
    let software = EGLDevice::device_for_display(&display).is_ok_and(|device| device.is_software());
    let context = EGLContext::new(&display)?;
    // Safety: the context is only used from this thread.
    let mut renderer = unsafe { GlesRenderer::new(context)? };
    let driver = drm.get_driver()?.name().to_string_lossy().into_owned();
    info!(
        "using {} ({driver}){}",
        path.display(),
        if software { ", drawing on the CPU" } else { "" }
    );

    let (connector, crtc) = connected_screen(&drm)?;
    let modes: Vec<ModeInfo> = connector.modes().iter().map(ModeInfo::from).collect();
    let mode_index = choose_mode(&modes, options.mode, driver == "hyperv_drm")
        .ok_or("the screen offers no modes")?;
    let mode = connector.modes()[mode_index];
    let output = new_output(&connector, mode);
    output.create_global::<Patrone>(&state.display_handle);
    state.space.map_output(&output, (0, 0));
    info!(
        "screen {} at {}x{}",
        output.name(),
        mode.size().0,
        mode.size().1
    );

    // Without a GPU the buffers can only be linear, which the display reads.
    let render_formats: FormatSet = renderer
        .egl_context()
        .dmabuf_render_formats()
        .iter()
        .filter(|format| !software || format.modifier == Modifier::Linear)
        .copied()
        .collect();
    let allocator = GbmAllocator::new(
        gbm.clone(),
        GbmBufferFlags::RENDERING | GbmBufferFlags::SCANOUT,
    );
    let exporter = GbmFramebufferExporter::new(gbm.clone(), None);
    let mut drm = DrmOutputManager::new(
        drm,
        allocator,
        exporter,
        Some(gbm),
        COLOR_FORMATS,
        render_formats,
    );
    let planes = drm.device().planes(&crtc).ok();
    let screen = drm.initialize_output::<_, OutputElement>(
        crtc,
        mode,
        &[connector.handle()],
        &output,
        planes,
        &mut renderer,
        &DrmOutputRenderElements::default(),
    )?;

    event_loop.insert_source(drm_events, |event, _, app| match event {
        DrmEvent::VBlank(_) => {
            if let AnyBackend::Udev(backend) = &mut app.backend {
                backend.on_vblank();
            }
        }
        DrmEvent::Error(err) => error!("display error: {err}"),
    })?;

    let mut libinput =
        Libinput::new_with_udev::<LibinputSessionInterface<LibSeatSession>>(session.clone().into());
    libinput
        .udev_assign_seat(&seat)
        .map_err(|()| format!("libinput couldn't use {seat}"))?;
    event_loop.insert_source(
        LibinputInputBackend::new(libinput.clone()),
        |event, _, app| {
            app.state.process_input_event(event);
            if let (Some(vt), AnyBackend::Udev(backend)) =
                (app.state.vt_switch.take(), &mut app.backend)
                && let Err(err) = backend.session.change_vt(vt)
            {
                warn!("couldn't switch to virtual terminal {vt}: {err}");
            }
        },
    )?;

    event_loop.insert_source(session_events, |event, _, app| {
        if let AnyBackend::Udev(backend) = &mut app.backend {
            backend.on_session_event(event, &mut app.state);
        }
    })?;

    Ok(Backend {
        session,
        libinput,
        drm,
        screen,
        renderer,
        output,
        wallpaper: wallpaper::load_or_warn(&options.wallpaper),
        cursors: Cursors::new(),
        event_loop,
        frame_pending: false,
        redraw_after_vblank: false,
        render_scheduled: false,
        screenshot: None,
    })
}

impl Backend {
    /// Draws a frame soon: right away, or after the vblank of the frame
    /// that's waiting to be shown.
    pub fn schedule_frame(&mut self) {
        if self.frame_pending {
            self.redraw_after_vblank = true;
            return;
        }
        if self.render_scheduled {
            return;
        }
        self.render_scheduled = true;
        self.event_loop.insert_idle(|app| {
            if let AnyBackend::Udev(backend) = &mut app.backend {
                backend.render_scheduled = false;
                backend.render(&mut app.state);
            }
        });
    }

    /// Saves the next frame to `path`.
    pub fn request_screenshot(&mut self, path: PathBuf) {
        self.screenshot = Some(path);
    }

    fn render(&mut self, state: &mut Patrone) {
        // Another virtual terminal has the screen.
        if !self.session.is_active() {
            return;
        }
        let elements = output_elements(
            &mut self.renderer,
            state,
            &self.output,
            self.wallpaper.as_ref(),
            Some(&mut self.cursors),
        );
        match self.screen.render_frame(
            &mut self.renderer,
            &elements,
            wallpaper::BACKGROUND,
            FrameFlags::DEFAULT,
        ) {
            Ok(frame) if !frame.is_empty => match self.screen.queue_frame(()) {
                Ok(()) => self.frame_pending = true,
                Err(err) => warn!("couldn't show a frame: {err}"),
            },
            Ok(_) => {}
            Err(err) => error!("couldn't draw a frame: {err}"),
        }

        if let Some(path) = self.screenshot.take() {
            let size = self
                .output
                .current_mode()
                .map(|mode| mode.size)
                .unwrap_or_default();
            let scale = self.output.current_scale().fractional_scale();
            match save_screenshot(&mut self.renderer, &elements, size, scale, &path) {
                Ok(()) => info!("saved a screenshot to {}", path.display()),
                Err(err) => error!("couldn't save a screenshot to {}: {err}", path.display()),
            }
        }
        state.send_frames(&self.output);
    }

    /// The frame waiting to be shown is on screen now.
    fn on_vblank(&mut self) {
        self.frame_pending = false;
        if let Err(err) = self.screen.frame_submitted() {
            warn!("frame error: {err}");
        }
        if std::mem::take(&mut self.redraw_after_vblank) {
            self.schedule_frame();
        }
    }

    /// Switching to another virtual terminal pauses the session: logind takes
    /// the screen and input devices away until we're switched back to.
    fn on_session_event(&mut self, event: SessionEvent, state: &mut Patrone) {
        match event {
            SessionEvent::PauseSession => {
                info!("pausing: another virtual terminal has the screen");
                self.libinput.suspend();
                self.drm.pause();
                self.frame_pending = false;
            }
            SessionEvent::ActivateSession => {
                info!("resuming");
                if let Err(err) = self.libinput.resume() {
                    error!("couldn't resume input: {err:?}");
                }
                if let Err(err) = self.drm.activate(false) {
                    error!("couldn't take the screen back: {err}");
                }
                state.redraw_needed = true;
            }
        }
    }
}

/// The display device: the boot GPU, or else the first one.
fn gpu_path(seat: &str) -> Result<PathBuf, Box<dyn Error>> {
    if let Some(path) = primary_gpu(seat)? {
        return Ok(path);
    }
    all_gpus(seat)?
        .into_iter()
        .next()
        .ok_or_else(|| "no display device (/dev/dri/card*)".into())
}

/// The first connected screen, and a CRTC that can drive it.
fn connected_screen(drm: &DrmDevice) -> Result<(connector::Info, crtc::Handle), Box<dyn Error>> {
    let resources = drm.resource_handles()?;
    for handle in resources.connectors() {
        let connector = drm.get_connector(*handle, false)?;
        if connector.state() != connector::State::Connected {
            continue;
        }
        for encoder in connector.encoders() {
            let Ok(encoder) = drm.get_encoder(*encoder) else {
                continue;
            };
            if let Some(crtc) = resources.filter_crtcs(encoder.possible_crtcs()).first() {
                return Ok((connector, *crtc));
            }
        }
    }
    Err("no connected screen".into())
}

fn new_output(connector: &connector::Info, mode: smithay::reexports::drm::control::Mode) -> Output {
    let name = format!(
        "{}-{}",
        connector.interface().as_str(),
        connector.interface_id()
    );
    let (width_mm, height_mm) = connector.size().unwrap_or((0, 0));
    let output = Output::new(
        name,
        PhysicalProperties {
            size: (width_mm as i32, height_mm as i32).into(),
            subpixel: connector.subpixel().into(),
            make: "Unknown".into(),
            model: "Unknown".into(),
        },
    );
    let mode = OutputMode::from(mode);
    output.set_preferred(mode);
    output.change_current_state(Some(mode), None, None, Some((0, 0).into()));
    output
}

/// What choosing a mode needs to know about one.
#[derive(Clone, Copy, Debug, PartialEq)]
struct ModeInfo {
    size: (u16, u16),
    refresh: u32,
    preferred: bool,
}

impl From<&smithay::reexports::drm::control::Mode> for ModeInfo {
    fn from(mode: &smithay::reexports::drm::control::Mode) -> Self {
        ModeInfo {
            size: mode.size(),
            refresh: mode.vrefresh(),
            preferred: mode.mode_type().contains(ModeTypeFlags::PREFERRED),
        }
    }
}

/// The index of the mode to use: `wanted` if the screen has it, else the
/// preferred one, else the largest. Hyper-V marks its 1024×768 boot mode as
/// preferred, so there `prefer_largest` skips straight to the largest.
fn choose_mode(
    modes: &[ModeInfo],
    wanted: Option<(u16, u16)>,
    prefer_largest: bool,
) -> Option<usize> {
    let largest = |modes: &mut dyn Iterator<Item = (usize, &ModeInfo)>| {
        modes
            .max_by_key(|(_, mode)| (mode.size.0 as u32 * mode.size.1 as u32, mode.refresh))
            .map(|(index, _)| index)
    };
    if let Some(wanted) = wanted {
        let found = largest(
            &mut modes
                .iter()
                .enumerate()
                .filter(|(_, mode)| mode.size == wanted),
        );
        if found.is_some() {
            return found;
        }
        warn!("the screen has no {}x{} mode", wanted.0, wanted.1);
    }
    if !prefer_largest && let Some(index) = modes.iter().position(|mode| mode.preferred) {
        return Some(index);
    }
    largest(&mut modes.iter().enumerate())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mode(width: u16, height: u16, refresh: u32, preferred: bool) -> ModeInfo {
        ModeInfo {
            size: (width, height),
            refresh,
            preferred,
        }
    }

    /// What hyperv_drm reports: the boot mode first and preferred.
    fn hyperv_modes() -> Vec<ModeInfo> {
        vec![
            mode(1024, 768, 60, true),
            mode(1920, 1080, 60, false),
            mode(1600, 1200, 60, false),
        ]
    }

    #[test]
    fn the_preferred_mode_is_used_normally() {
        assert_eq!(choose_mode(&hyperv_modes(), None, false), Some(0));
    }

    #[test]
    fn hyper_v_gets_the_largest_mode() {
        assert_eq!(choose_mode(&hyperv_modes(), None, true), Some(1));
    }

    #[test]
    fn a_wanted_mode_wins_if_the_screen_has_it() {
        let modes = hyperv_modes();
        assert_eq!(choose_mode(&modes, Some((1600, 1200)), true), Some(2));
        assert_eq!(choose_mode(&modes, Some((3840, 2160)), false), Some(0));
    }

    #[test]
    fn the_faster_of_two_same_sized_modes() {
        let modes = [mode(1920, 1080, 30, false), mode(1920, 1080, 60, false)];
        assert_eq!(choose_mode(&modes, None, false), Some(1));
    }
}
