//! Pixel storage and output. The only file that touches `image`.

use std::path::Path;

use crate::math::Color;

/// A rectangle of linear-light RGB pixels, row-major from the top-left.
pub struct Film {
    width: u32,
    height: u32,
    pixels: Vec<Color>,
}

impl Film {
    pub fn new(width: u32, height: u32) -> Self {
        assert!(width > 0 && height > 0, "film must have at least one pixel");
        Film { width, height, pixels: vec![Color::ZERO; width as usize * height as usize] }
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn pixels(&self) -> &[Color] {
        &self.pixels
    }

    pub fn pixels_mut(&mut self) -> &mut [Color] {
        &mut self.pixels
    }

    pub fn get(&self, x: u32, y: u32) -> Color {
        self.pixels[y as usize * self.width as usize + x as usize]
    }

    /// Average linear radiance over the whole image.
    pub fn mean(&self) -> Color {
        self.pixels.iter().copied().sum::<Color>() / self.pixels.len() as f64
    }

    /// 8-bit sRGB bytes, three per pixel.
    pub fn to_srgb8(&self) -> Vec<u8> {
        self.pixels.iter().flat_map(|c| [encode(c.x), encode(c.y), encode(c.z)]).collect()
    }

    pub fn save_png(&self, path: impl AsRef<Path>) -> image::ImageResult<()> {
        image::RgbImage::from_raw(self.width, self.height, self.to_srgb8())
            .expect("buffer length matches dimensions")
            .save_with_format(path, image::ImageFormat::Png)
    }
}

/// The sRGB transfer function: linear light to display-encoded, both in `[0, 1]`.
pub fn linear_to_srgb(linear: f64) -> f64 {
    if linear <= 0.003_130_8 {
        12.92 * linear
    } else {
        1.055 * linear.powf(1.0 / 2.4) - 0.055
    }
}

fn encode(linear: f64) -> u8 {
    // A non-finite value should never reach the film, but if one does it must
    // not become an arbitrary byte.
    if !linear.is_finite() {
        return 0;
    }
    (linear_to_srgb(linear.clamp(0.0, 1.0)) * 255.0).round() as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn srgb_curve_fixed_points() {
        assert_eq!(linear_to_srgb(0.0), 0.0);
        assert!((linear_to_srgb(1.0) - 1.0).abs() < 1e-12);
        // 18% grey encodes to roughly 46%.
        assert!((linear_to_srgb(0.18) - 0.4614).abs() < 1e-3);
        // The two branches meet without a visible step.
        assert!((linear_to_srgb(0.003_130_8) - linear_to_srgb(0.003_130_9)).abs() < 1e-5);
    }

    #[test]
    fn encoding_clamps_and_survives_bad_values() {
        assert_eq!(encode(-1.0), 0);
        assert_eq!(encode(0.0), 0);
        assert_eq!(encode(1.0), 255);
        assert_eq!(encode(50.0), 255);
        assert_eq!(encode(f64::NAN), 0);
        assert_eq!(encode(f64::INFINITY), 0);
    }

    #[test]
    fn bytes_are_row_major_rgb() {
        let mut film = Film::new(2, 1);
        film.pixels_mut()[1] = Color::new(1.0, 0.0, 1.0);
        assert_eq!(film.to_srgb8(), vec![0, 0, 0, 255, 0, 255]);
        assert_eq!(film.get(1, 0), Color::new(1.0, 0.0, 1.0));
        assert_eq!(film.mean(), Color::new(0.5, 0.0, 0.5));
    }
}
