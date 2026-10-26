//! The pointer's images from the cursor theme, for drawing the cursor
//! ourselves when no desktop around the compositor draws it.

use std::collections::HashMap;

use smithay::backend::allocator::Fourcc;
use smithay::backend::renderer::element::Kind;
use smithay::backend::renderer::element::memory::{
    MemoryRenderBuffer, MemoryRenderBufferRenderElement,
};
use smithay::backend::renderer::gles::GlesRenderer;
use smithay::input::pointer::CursorIcon;
use smithay::utils::{Physical, Point, Transform};
use tracing::warn;

const DEFAULT_SIZE: u32 = 24;

struct Image {
    buffer: MemoryRenderBuffer,
    hotspot: Point<f64, Physical>,
}

/// Cursor images, loaded the first time each one is shown. Follows
/// `XCURSOR_THEME` and `XCURSOR_SIZE` like other Wayland compositors.
pub struct Cursors {
    theme_name: String,
    theme: xcursor::CursorTheme,
    size: u32,
    loaded: HashMap<CursorIcon, Option<Image>>,
    warned: bool,
}

impl Cursors {
    pub fn new() -> Self {
        let name = std::env::var("XCURSOR_THEME").unwrap_or_else(|_| "default".to_owned());
        let size = std::env::var("XCURSOR_SIZE")
            .ok()
            .and_then(|size| size.parse().ok())
            .unwrap_or(DEFAULT_SIZE);
        Cursors {
            theme: xcursor::CursorTheme::load(&name),
            theme_name: name,
            size,
            loaded: HashMap::new(),
            warned: false,
        }
    }

    /// The cursor `icon` with its hotspot at `location`, or the default
    /// arrow if the theme doesn't have it.
    pub fn element(
        &mut self,
        renderer: &mut GlesRenderer,
        icon: CursorIcon,
        location: Point<f64, Physical>,
    ) -> Option<MemoryRenderBufferRenderElement<GlesRenderer>> {
        for icon in [icon, CursorIcon::Default] {
            let (theme, size) = (&self.theme, self.size);
            let image = self
                .loaded
                .entry(icon)
                .or_insert_with(|| load(theme, icon, size));
            if let Some(image) = image {
                return MemoryRenderBufferRenderElement::from_buffer(
                    renderer,
                    location - image.hotspot,
                    &image.buffer,
                    None,
                    None,
                    None,
                    Kind::Cursor,
                )
                .inspect_err(|err| warn!("couldn't draw the cursor: {err}"))
                .ok();
            }
        }
        if !self.warned {
            warn!(
                "the cursor theme \"{}\" has no default cursor; set XCURSOR_THEME",
                self.theme_name
            );
            self.warned = true;
        }
        None
    }
}

/// The first frame of `icon` at the size nearest to `size`.
fn load(theme: &xcursor::CursorTheme, icon: CursorIcon, size: u32) -> Option<Image> {
    let path = std::iter::once(icon.name())
        .chain(icon.alt_names().iter().copied())
        .find_map(|name| theme.load_icon(name))?;
    let data = std::fs::read(&path)
        .inspect_err(|err| warn!("couldn't read {}: {err}", path.display()))
        .ok()?;
    let images = xcursor::parser::parse_xcursor(&data)?;
    let image = images
        .iter()
        .min_by_key(|image| image.size.abs_diff(size))?;
    let buffer = MemoryRenderBuffer::from_slice(
        &image.pixels_rgba,
        // RGBA bytes in memory, in DRM's little-endian naming.
        Fourcc::Abgr8888,
        (image.width as i32, image.height as i32),
        1,
        Transform::Normal,
        None,
    );
    Some(Image {
        buffer,
        hotspot: (image.xhot as f64, image.yhot as f64).into(),
    })
}
