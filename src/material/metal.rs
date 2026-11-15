use super::Scatter;
use crate::geometry::HitRecord;
use crate::math::{Color, Ray, Sampler};

/// Mirror reflection, optionally roughened.
#[derive(Debug, Clone, Copy)]
pub struct Metal {
    pub albedo: Color,
    /// 0 is a perfect mirror; 1 is very rough. Clamped to `[0, 1]`.
    pub fuzz: f64,
}

impl Metal {
    pub fn scatter(&self, ray_in: &Ray, hit: &HitRecord, sampler: &mut Sampler) -> Option<Scatter> {
        let reflected = ray_in.dir().reflect(hit.normal);
        let dir = (reflected + sampler.unit_vector() * self.fuzz).try_normalized()?;
        // Fuzz can push the direction below the surface. Tracing it would start
        // a ray inside the object, so that energy is absorbed instead.
        if dir.dot(hit.normal) <= 0.0 {
            return None;
        }
        Some(Scatter { attenuation: self.albedo, ray: Ray::new(hit.point, dir), specular: true, pdf: 0.0 })
    }
}
