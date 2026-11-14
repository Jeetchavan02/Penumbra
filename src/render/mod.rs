//! Turning a scene into pixels.

mod film;
mod integrator;

pub use film::{linear_to_srgb, Film};
pub use integrator::{radiance, RAY_EPSILON};

use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use rayon::prelude::*;

use crate::camera::Camera;
use crate::math::{Color, Sampler};
use crate::scene::Scene;

#[derive(Debug, Clone)]
pub struct RenderSettings {
    pub width: u32,
    pub height: u32,
    pub samples_per_pixel: u32,
    /// Maximum number of surface interactions along one path.
    pub max_depth: u32,
    /// Same seed, same image, regardless of thread count.
    pub seed: u64,
    /// Sample lights directly at every diffuse bounce (next event estimation).
    /// Converges to the same image as plain path tracing, with far less noise
    /// when lights are small.
    pub direct_lighting: bool,
    /// Report progress on stderr.
    pub progress: bool,
}

impl Default for RenderSettings {
    fn default() -> Self {
        RenderSettings { width: 400, height: 225, samples_per_pixel: 64, max_depth: 50, seed: 0, direct_lighting: true, progress: false }
    }
}

#[derive(Debug, Clone, Default)]
pub struct RenderStats {
    /// Samples discarded because they were NaN, infinite or negative. Anything
    /// other than zero means a numerical bug somewhere upstream.
    pub rejected_samples: u64,
    pub elapsed: Duration,
}

/// A sample is usable only if every channel is a finite, non-negative number.
fn is_valid(sample: Color) -> bool {
    sample.is_finite() && sample.min_component() >= 0.0
}

/// Estimate one pixel. Returns its colour and how many samples were rejected.
fn render_pixel(scene: &Scene, camera: &Camera, settings: &RenderSettings, x: u32, y: u32) -> (Color, u64) {
    let index = y as u64 * settings.width as u64 + x as u64;
    let mut sampler = Sampler::for_pixel(settings.seed, index);
    let mut sum = Color::ZERO;
    let mut accepted = 0u32;
    for _ in 0..settings.samples_per_pixel {
        let sample = radiance(scene, camera.ray(x, y, &mut sampler), settings, &mut sampler);
        // One bad sample added to the sum would poison the pixel for good.
        if is_valid(sample) {
            sum += sample;
            accepted += 1;
        }
    }
    let rejected = (settings.samples_per_pixel - accepted) as u64;
    (if accepted > 0 { sum / accepted as f64 } else { Color::ZERO }, rejected)
}

/// Render on rayon's current thread pool, one task per scanline.
///
/// Rows are disjoint `&mut` slices of the film, so no locking is needed. The
/// scene is only ever borrowed immutably. Each pixel owns its random stream,
/// which is what makes the output independent of scheduling.
pub fn render(scene: &Scene, settings: &RenderSettings) -> (Film, RenderStats) {
    let start = Instant::now();
    let camera = Camera::new(&scene.camera, settings.width, settings.height);
    let mut film = Film::new(settings.width, settings.height);
    let rejected_samples = AtomicU64::new(0);
    let rows_done = AtomicUsize::new(0);
    let total_rows = settings.height as usize;

    film.pixels_mut().par_chunks_mut(settings.width as usize).enumerate().for_each(|(y, row)| {
        let mut rejected_in_row = 0;
        for (x, pixel) in row.iter_mut().enumerate() {
            let (color, rejected) = render_pixel(scene, &camera, settings, x as u32, y as u32);
            *pixel = color;
            rejected_in_row += rejected;
        }
        rejected_samples.fetch_add(rejected_in_row, Ordering::Relaxed);

        let done = rows_done.fetch_add(1, Ordering::Relaxed) + 1;
        if settings.progress && (done * 20 / total_rows) != ((done - 1) * 20 / total_rows) {
            eprint!("\r  {:3}%", done * 100 / total_rows);
        }
    });
    if settings.progress {
        eprintln!();
    }

    let stats = RenderStats { rejected_samples: rejected_samples.into_inner(), elapsed: start.elapsed() };
    (film, stats)
}
