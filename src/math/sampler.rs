//! The only file that touches `rand`.

use std::f64::consts::TAU;

use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};

use super::Vec3;

/// A stream of random numbers owned by exactly one pixel.
///
/// Seeding per pixel rather than per thread makes a render independent of how
/// rayon schedules the work: the same seed gives the same image on any machine.
pub struct Sampler {
    rng: SmallRng,
}

impl Sampler {
    pub fn new(seed: u64) -> Self {
        Sampler { rng: SmallRng::seed_from_u64(seed) }
    }

    /// Decorrelated stream for one pixel of one render.
    pub fn for_pixel(seed: u64, pixel: u64) -> Self {
        Sampler::new(splitmix64(splitmix64(seed) ^ pixel.wrapping_mul(0x9E37_79B9_7F4A_7C15)))
    }

    /// Uniform in `[0, 1)`.
    pub fn uniform(&mut self) -> f64 {
        self.rng.random::<f64>()
    }

    /// Uniform in `[lo, hi)`.
    pub fn range(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.uniform()
    }

    /// Uniform on the unit sphere. Sampled analytically: no rejection loop, and
    /// the result is unit length by construction rather than by division.
    pub fn unit_vector(&mut self) -> Vec3 {
        let z = 1.0 - 2.0 * self.uniform();
        let r = (1.0 - z * z).max(0.0).sqrt();
        let (sin_phi, cos_phi) = (TAU * self.uniform()).sin_cos();
        Vec3::new(r * cos_phi, r * sin_phi, z)
    }

    /// Uniform on the unit disk.
    pub fn in_unit_disk(&mut self) -> (f64, f64) {
        let r = self.uniform().sqrt();
        let (sin_phi, cos_phi) = (TAU * self.uniform()).sin_cos();
        (r * cos_phi, r * sin_phi)
    }
}

fn splitmix64(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_same_stream() {
        let mut a = Sampler::for_pixel(7, 123);
        let mut b = Sampler::for_pixel(7, 123);
        for _ in 0..100 {
            assert_eq!(a.uniform(), b.uniform());
        }
    }

    #[test]
    fn neighbouring_pixels_differ() {
        let mut a = Sampler::for_pixel(7, 0);
        let mut b = Sampler::for_pixel(7, 1);
        assert_ne!(a.uniform(), b.uniform());
    }

    #[test]
    fn uniform_stays_in_range_and_averages_a_half() {
        let mut s = Sampler::new(1);
        let n = 100_000;
        let mut sum = 0.0;
        for _ in 0..n {
            let u = s.uniform();
            assert!((0.0..1.0).contains(&u));
            sum += u;
        }
        assert!((sum / n as f64 - 0.5).abs() < 0.01);
    }

    #[test]
    fn unit_vectors_are_unit_and_unbiased() {
        let mut s = Sampler::new(2);
        let n = 100_000;
        let mut mean = Vec3::ZERO;
        for _ in 0..n {
            let v = s.unit_vector();
            assert!((v.length() - 1.0).abs() < 1e-12);
            mean += v;
        }
        assert!((mean / n as f64).length() < 0.01);
    }

    #[test]
    fn disk_samples_stay_inside() {
        let mut s = Sampler::new(3);
        for _ in 0..10_000 {
            let (x, y) = s.in_unit_disk();
            assert!(x * x + y * y <= 1.0 + 1e-12);
        }
    }
}
