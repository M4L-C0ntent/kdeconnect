//! Renders contact photos as a pre-masked circular RGBA buffer.
//!
//! A plain `Container` with `border.radius` + `.clip(true)` was tried
//! first and didn't actually clip the image content to the rounded
//! shape in testing — clipping in this renderer (or this iced version)
//! only affects the container's own background/border quad, not
//! arbitrary child content drawn inside it. Baking the mask into the
//! pixel data itself sidesteps that entirely: the image is circular
//! before it ever reaches the renderer, so there's nothing left for the
//! renderer to get wrong.

/// Fixed bake resolution — comfortably above every on-screen avatar size
/// this app uses (32–40px), so downscaling at render time stays crisp.
pub const BAKE_SIZE: u32 = 128;

/// A decoded, circularly-masked avatar ready for `image::Handle::from_rgba`.
#[derive(Debug, Clone)]
pub struct Avatar {
    pub width: u32,
    pub height: u32,
    /// RGBA8, row-major, alpha already zeroed outside the circle.
    pub rgba: Vec<u8>,
}

/// Decodes arbitrary image bytes (JPEG/PNG/whatever the phone sent),
/// crops+scales to a `size`x`size` square, and zeroes alpha outside an
/// inscribed circle with a 1px antialiased edge. Returns `None` on
/// decode failure — a broken photo shouldn't take down the contact list.
pub fn make_circular(raw: &[u8], size: u32) -> Option<Avatar> {
    let decoded = image::load_from_memory(raw).ok()?;
    let mut rgba = decoded
        .resize_to_fill(size, size, image::imageops::FilterType::Lanczos3)
        .to_rgba8();

    let radius = size as f32 / 2.0;
    let center = radius;
    for y in 0..size {
        for x in 0..size {
            let dx = x as f32 + 0.5 - center;
            let dy = y as f32 + 0.5 - center;
            let dist = (dx * dx + dy * dy).sqrt();
            let coverage = if dist <= radius - 1.0 {
                1.0
            } else if dist >= radius {
                0.0
            } else {
                radius - dist
            };
            if coverage < 1.0 {
                let pixel = rgba.get_pixel_mut(x, y);
                pixel.0[3] = (f32::from(pixel.0[3]) * coverage) as u8;
            }
        }
    }

    Some(Avatar {
        width: size,
        height: size,
        rgba: rgba.into_raw(),
    })
}
