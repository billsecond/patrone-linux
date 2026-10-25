//! The background: a picture scaled to cover the whole output.

use std::path::Path;

use smithay::backend::allocator::Fourcc;
use smithay::backend::renderer::element::Kind;
use smithay::backend::renderer::element::memory::{
    MemoryRenderBuffer, MemoryRenderBufferRenderElement,
};
use smithay::backend::renderer::gles::{GlesError, GlesRenderer};
use smithay::utils::{Logical, Rectangle, Size, Transform};

/// Patrone's near-black, shown where there's no wallpaper.
pub const BACKGROUND: [f32; 4] = [3.0 / 255.0, 3.0 / 255.0, 3.0 / 255.0, 1.0];

pub struct Wallpaper {
    buffer: MemoryRenderBuffer,
    size: Size<i32, Logical>,
}

impl Wallpaper {
    pub fn load(path: &Path) -> Result<Self, image::ImageError> {
        let image = image::open(path)?.into_rgba8();
        let (width, height) = (image.width() as i32, image.height() as i32);
        let opaque = image.pixels().all(|pixel| pixel.0[3] == u8::MAX);
        let buffer = MemoryRenderBuffer::from_slice(
            image.as_raw(),
            // RGBA bytes in memory, in DRM's little-endian naming.
            Fourcc::Abgr8888,
            (width, height),
            1,
            Transform::Normal,
            opaque.then(|| vec![Rectangle::from_size((width, height).into())]),
        );
        Ok(Wallpaper {
            buffer,
            size: (width, height).into(),
        })
    }

    /// The wallpaper covering an output of `output_size`.
    pub fn element(
        &self,
        renderer: &mut GlesRenderer,
        output_size: Size<i32, Logical>,
    ) -> Result<MemoryRenderBufferRenderElement<GlesRenderer>, GlesError> {
        MemoryRenderBufferRenderElement::from_buffer(
            renderer,
            (0.0, 0.0),
            &self.buffer,
            None,
            Some(cover(self.size, output_size)),
            Some(output_size),
            Kind::Unspecified,
        )
    }
}

/// The part of an image of `image` size that fills `output` without
/// stretching: all of one dimension, and the middle of the other.
fn cover(image: Size<i32, Logical>, output: Size<i32, Logical>) -> Rectangle<f64, Logical> {
    let image = image.to_f64();
    let output = output.to_f64();
    let scale = (output.w / image.w).max(output.h / image.h);
    let width = output.w / scale;
    let height = output.h / scale;
    Rectangle::new(
        ((image.w - width) / 2.0, (image.h - height) / 2.0).into(),
        (width, height).into(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_wider_output_crops_the_top_and_bottom() {
        let crop = cover((1600, 900).into(), (1600, 600).into());
        assert_eq!(
            crop,
            Rectangle::new((0.0, 150.0).into(), (1600.0, 600.0).into())
        );
    }

    #[test]
    fn a_taller_output_crops_the_sides() {
        let crop = cover((1600, 900).into(), (900, 900).into());
        assert_eq!(
            crop,
            Rectangle::new((350.0, 0.0).into(), (900.0, 900.0).into())
        );
    }

    #[test]
    fn a_larger_output_with_the_same_shape_uses_the_whole_image() {
        let crop = cover((1600, 900).into(), (3840, 2160).into());
        assert_eq!(
            crop,
            Rectangle::new((0.0, 0.0).into(), (1600.0, 900.0).into())
        );
    }
}
