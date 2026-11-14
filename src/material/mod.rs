//! Surface response: how a ray continues after a hit, and what the surface emits.

mod lambertian;
mod metal;

pub use lambertian::Lambertian;
pub use metal::Metal;

use crate::geometry::HitRecord;
use crate::math::{Color, Ray, Sampler};

/// The outcome of a scattering event. Owned, plain data: nothing here borrows
/// from the incoming ray, the hit or the material.
#[derive(Debug, Clone, Copy)]
pub struct Scatter {
    /// Fraction of radiance carried along `ray`, per colour channel.
    pub attenuation: Color,
    pub ray: Ray,
    /// True when the direction was chosen from a delta-like lobe (mirror,
    /// glass). Direct light sampling is skipped for these bounces.
    pub specular: bool,
}

/// The closed set of materials. `Copy` plain data, so it is `Send + Sync` for
/// free and lives in one table indexed by `MaterialId`.
#[derive(Debug, Clone, Copy)]
pub enum Material {
    Lambertian(Lambertian),
    Metal(Metal),
}

impl Material {
    pub fn lambertian(albedo: Color) -> Self {
        Material::Lambertian(Lambertian { albedo })
    }

    pub fn metal(albedo: Color, fuzz: f64) -> Self {
        Material::Metal(Metal { albedo, fuzz: fuzz.clamp(0.0, 1.0) })
    }

    /// Continue the path, or return `None` if the ray is absorbed.
    ///
    /// Inputs are borrowed read-only; the sampler is the only mutable state and
    /// belongs to the calling pixel. The result is new, owned data.
    pub fn scatter(&self, ray_in: &Ray, hit: &HitRecord, sampler: &mut Sampler) -> Option<Scatter> {
        match self {
            Material::Lambertian(m) => m.scatter(hit, sampler),
            Material::Metal(m) => m.scatter(ray_in, hit, sampler),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::MaterialId;
    use crate::math::Vec3;

    /// A hit at the origin on a surface facing +y, struck by `dir`.
    fn hit_from(dir: Vec3) -> (Ray, HitRecord) {
        let dir = dir.normalized();
        let ray = Ray::new(Vec3::ZERO - dir, dir);
        (ray, HitRecord::new(&ray, 1.0, Vec3::ZERO, Vec3::new(0.0, 1.0, 0.0), MaterialId(0)))
    }

    #[test]
    fn lambertian_scatters_into_the_upper_hemisphere() {
        let m = Material::lambertian(Color::splat(0.5));
        let (ray, hit) = hit_from(Vec3::new(1.0, -1.0, 0.0));
        let mut sampler = Sampler::new(1);
        let mut mean_cos = 0.0;
        let n = 50_000;
        for _ in 0..n {
            let s = m.scatter(&ray, &hit, &mut sampler).unwrap();
            let cos = s.ray.dir().dot(hit.normal);
            assert!(cos >= 0.0 && s.ray.dir().is_finite());
            assert!(!s.specular);
            mean_cos += cos;
        }
        // E[cos] under a cosine-weighted pdf is 2/3.
        assert!((mean_cos / n as f64 - 2.0 / 3.0).abs() < 0.01);
    }

    #[test]
    fn polished_metal_mirrors_the_ray() {
        let m = Material::metal(Color::splat(0.9), 0.0);
        let (ray, hit) = hit_from(Vec3::new(1.0, -1.0, 0.0));
        let s = m.scatter(&ray, &hit, &mut Sampler::new(1)).unwrap();
        assert!((s.ray.dir() - Vec3::new(1.0, 1.0, 0.0).normalized()).length() < 1e-12);
        assert!(s.specular);
    }

    #[test]
    fn rough_metal_never_scatters_below_the_surface() {
        let m = Material::metal(Color::splat(0.9), 1.0);
        // A grazing ray: about half the fuzzed directions point into the surface.
        let (ray, hit) = hit_from(Vec3::new(1.0, -0.05, 0.0));
        let mut sampler = Sampler::new(1);
        let mut absorbed = 0;
        for _ in 0..10_000 {
            match m.scatter(&ray, &hit, &mut sampler) {
                Some(s) => assert!(s.ray.dir().dot(hit.normal) > 0.0),
                None => absorbed += 1,
            }
        }
        assert!(absorbed > 0);
    }
}
