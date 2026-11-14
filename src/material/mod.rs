//! Surface response: how a ray continues after a hit, and what the surface emits.

mod dielectric;
mod emissive;
mod lambertian;
mod metal;

pub use dielectric::Dielectric;
pub use emissive::Emissive;
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
    Dielectric(Dielectric),
    Emissive(Emissive),
}

impl Material {
    pub fn lambertian(albedo: Color) -> Self {
        Material::Lambertian(Lambertian { albedo })
    }

    pub fn metal(albedo: Color, fuzz: f64) -> Self {
        Material::Metal(Metal { albedo, fuzz: fuzz.clamp(0.0, 1.0) })
    }

    /// Clear glass-like material with the given refractive index.
    pub fn dielectric(ior: f64) -> Self {
        assert!(ior > 0.0 && ior.is_finite(), "refractive index must be positive");
        Material::Dielectric(Dielectric { ior, tint: Color::ONE })
    }

    pub fn emissive(radiance: Color) -> Self {
        Material::Emissive(Emissive { radiance })
    }

    /// Radiance the surface emits on its own. Zero for everything but lights.
    pub fn emitted(&self) -> Color {
        match self {
            Material::Emissive(m) => m.radiance,
            _ => Color::ZERO,
        }
    }

    pub fn is_emissive(&self) -> bool {
        matches!(self, Material::Emissive(_))
    }

    /// Continue the path, or return `None` if the ray is absorbed.
    ///
    /// Inputs are borrowed read-only; the sampler is the only mutable state and
    /// belongs to the calling pixel. The result is new, owned data.
    pub fn scatter(&self, ray_in: &Ray, hit: &HitRecord, sampler: &mut Sampler) -> Option<Scatter> {
        match self {
            Material::Lambertian(m) => m.scatter(hit, sampler),
            Material::Metal(m) => m.scatter(ray_in, hit, sampler),
            Material::Dielectric(m) => m.scatter(ray_in, hit, sampler),
            Material::Emissive(_) => None,
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

    #[test]
    fn matched_index_passes_straight_through() {
        let m = Material::dielectric(1.0);
        let (ray, hit) = hit_from(Vec3::new(1.0, -1.0, 0.0));
        let mut sampler = Sampler::new(1);
        for _ in 0..100 {
            let s = m.scatter(&ray, &hit, &mut sampler).unwrap();
            assert!((s.ray.dir() - ray.dir()).length() < 1e-12);
        }
    }

    #[test]
    fn glass_bends_toward_the_normal_on_entry() {
        let m = Material::dielectric(1.5);
        let (ray, hit) = hit_from(Vec3::new(1.0, -1.0, 0.0));
        let mut sampler = Sampler::new(1);
        let (mut refracted, mut reflected) = (0, 0);
        for _ in 0..2_000 {
            let d = m.scatter(&ray, &hit, &mut sampler).unwrap().ray.dir();
            assert!((d.length() - 1.0).abs() < 1e-9);
            if d.y < 0.0 {
                refracted += 1;
                // sin(out) = sin(45 deg) / 1.5
                assert!((d.x - 0.5_f64.sqrt() / 1.5).abs() < 1e-12);
            } else {
                reflected += 1;
            }
        }
        assert!(refracted > reflected && reflected > 0);
    }

    #[test]
    fn total_internal_reflection_beyond_the_critical_angle() {
        let m = Material::dielectric(1.5);
        // Leaving glass at 60 degrees from the normal; the critical angle is ~41.8.
        let dir = Vec3::new(3.0_f64.sqrt(), 1.0, 0.0).normalized();
        let ray = Ray::new(Vec3::ZERO - dir, dir);
        let hit = HitRecord::new(&ray, 1.0, Vec3::ZERO, Vec3::new(0.0, 1.0, 0.0), MaterialId(0));
        assert!(!hit.front_face);
        let mut sampler = Sampler::new(1);
        for _ in 0..500 {
            let d = m.scatter(&ray, &hit, &mut sampler).unwrap().ray.dir();
            assert!(d.is_finite());
            assert!((d - dir.reflect(hit.normal)).length() < 1e-12);
        }
    }

    #[test]
    fn exactly_grazing_and_exactly_normal_incidence_stay_finite() {
        let m = Material::dielectric(1.5);
        let mut sampler = Sampler::new(1);
        for dir in [Vec3::new(0.0, -1.0, 0.0), Vec3::new(1.0, -1e-12, 0.0), Vec3::new(1.0, 0.0, 0.0)] {
            let (ray, hit) = hit_from(dir);
            for _ in 0..100 {
                if let Some(s) = m.scatter(&ray, &hit, &mut sampler) {
                    assert!(s.ray.dir().is_finite());
                }
            }
        }
    }

    #[test]
    fn lights_emit_and_do_not_scatter() {
        let light = Material::emissive(Color::splat(4.0));
        let (ray, hit) = hit_from(Vec3::new(0.0, -1.0, 0.0));
        assert!(light.scatter(&ray, &hit, &mut Sampler::new(1)).is_none());
        assert_eq!(light.emitted(), Color::splat(4.0));
        assert!(light.is_emissive());
        assert_eq!(Material::lambertian(Color::ONE).emitted(), Color::ZERO);
    }
}
