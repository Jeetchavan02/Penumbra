use super::Scatter;
use crate::geometry::HitRecord;
use std::f64::consts::PI;

use crate::math::{Color, Ray, Sampler};

/// Ideal diffuse reflector.
#[derive(Debug, Clone, Copy)]
pub struct Lambertian {
    pub albedo: Color,
}

impl Lambertian {
    pub fn scatter(&self, hit: &HitRecord, sampler: &mut Sampler) -> Option<Scatter> {
        // normal + uniform unit vector is cosine-distributed about the normal,
        // so the cosine term and the pdf cancel and the weight is the albedo.
        let candidate = hit.normal + sampler.unit_vector();
        // The two can cancel exactly; normalising that would be 0/0.
        let dir = if candidate.near_zero() { hit.normal } else { candidate.normalized() };
        let pdf = hit.normal.dot(dir).max(0.0) / PI;
        Some(Scatter { attenuation: self.albedo, ray: Ray::new(hit.point, dir), specular: false, pdf })
    }
}
