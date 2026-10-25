//! What goes on the output, for both backends: the cursor, the windows and
//! the wallpaper, drawn with OpenGL ES. Also screenshots of it.

use std::error::Error;
use std::path::Path;

use smithay::backend::allocator::Fourcc;
use smithay::backend::renderer::damage::OutputDamageTracker;
use smithay::backend::renderer::element::memory::MemoryRenderBufferRenderElement;
use smithay::backend::renderer::element::surface::{
    WaylandSurfaceRenderElement, render_elements_from_surface_tree,
};
use smithay::backend::renderer::element::{Kind, render_elements};
use smithay::backend::renderer::gles::{GlesRenderer, GlesTexture};
use smithay::backend::renderer::{Bind, ExportMem, Offscreen, TextureMapping};
use smithay::desktop::space::SpaceRenderElements;
use smithay::input::pointer::{CursorImageStatus, CursorImageSurfaceData};
use smithay::output::Output;
use smithay::utils::{Buffer, Physical, Rectangle, Size, Transform};
use smithay::wayland::compositor::with_states;
use tracing::warn;

use crate::cursor::Cursors;
use crate::state::Patrone;
use crate::wallpaper::{self, Wallpaper};

render_elements! {
    pub OutputElement<=GlesRenderer>;
    Space = SpaceRenderElements<GlesRenderer, WaylandSurfaceRenderElement<GlesRenderer>>,
    Surface = WaylandSurfaceRenderElement<GlesRenderer>,
    /// The wallpaper, and cursors from the cursor theme
    Memory = MemoryRenderBufferRenderElement<GlesRenderer>,
}

/// Everything on `output`, front to back: the cursor, the windows, then the
/// wallpaper. Named cursors are drawn from `cursors` when given; nested in
/// another desktop, that desktop draws them instead.
pub fn output_elements(
    renderer: &mut GlesRenderer,
    state: &Patrone,
    output: &Output,
    wallpaper: Option<&Wallpaper>,
    cursors: Option<&mut Cursors>,
) -> Vec<OutputElement> {
    let mut elements = Vec::new();
    let scale = output.current_scale().fractional_scale();
    if let Some(pointer) = state.seat.get_pointer() {
        let location = pointer.current_location().to_physical(scale);
        match &state.cursor {
            CursorImageStatus::Surface(surface) => {
                let hotspot = with_states(surface, |states| {
                    states
                        .data_map
                        .get::<CursorImageSurfaceData>()
                        .map(|data| data.lock().unwrap().hotspot)
                        .unwrap_or_default()
                });
                let location = location - hotspot.to_f64().to_physical(scale);
                let cursor = render_elements_from_surface_tree(
                    renderer,
                    surface,
                    location.to_i32_round(),
                    scale,
                    1.0,
                    Kind::Cursor,
                );
                elements.extend(cursor.into_iter().map(OutputElement::Surface));
            }
            CursorImageStatus::Named(icon) => {
                if let Some(element) = cursors.and_then(|c| c.element(renderer, *icon, location)) {
                    elements.push(OutputElement::Memory(element));
                }
            }
            CursorImageStatus::Hidden => {}
        }
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
            Ok(element) => elements.push(OutputElement::Memory(element)),
            Err(err) => warn!("couldn't draw the wallpaper: {err}"),
        }
    }
    elements
}

/// Draws `elements` again, off screen, and writes them to a PNG file.
pub fn save_screenshot(
    renderer: &mut GlesRenderer,
    elements: &[OutputElement],
    size: Size<i32, Physical>,
    scale: f64,
    path: &Path,
) -> Result<(), Box<dyn Error>> {
    let buffer_size = Size::<i32, Buffer>::from((size.w, size.h));
    let mut texture: GlesTexture = renderer.create_buffer(Fourcc::Abgr8888, buffer_size)?;
    let mut target = renderer.bind(&mut texture)?;
    // Drawn flipped like a GL framebuffer; the mapping says whether to undo it.
    OutputDamageTracker::new(size, scale, Transform::Flipped180).render_output(
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
        .ok_or("the frame is smaller than the output")?;
    if flipped {
        image::imageops::flip_vertical_in_place(&mut image);
    }
    for pixel in image.pixels_mut() {
        pixel.0[3] = u8::MAX;
    }
    image.save(path)?;
    Ok(())
}
