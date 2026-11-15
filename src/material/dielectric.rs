use super::Scatter;
use crate::geometry::HitRecord;
use crate::math::{Color, Ray, Sampler};

/// Clear refractive material such as glass or water.
#[derive(Debug, Clone, Copy)]
pub struct Dielectric {
    /// Refractive index relative to the medium outside the surface. Values
    /// below 1 describe a less dense inclusion, e.g. an air bubble in glass.
    pub ior: f64,
    pub tint: Color,
}

impl Dielectric {
    pub fn scatter(&self, ray_in: &Ray, hit: &HitRecord, sampler: &mut Sampler) -> Option<Scatter> {
        // Entering goes from outside (1) to inside (ior); leaving is the inverse.
        let ratio = if hit.front_face { 1.0 / self.ior } else { self.ior };

        let dir = ray_in.dir();
        // Rounding can push the dot product past 1, and sqrt(1 - cos^2) of
        // that is NaN.
        let cos_theta = (-dir).dot(hit.normal).clamp(0.0, 1.0);
        let sin_theta = (1.0 - cos_theta * cos_theta).sqrt();

        // Total internal reflection must be decided before refracting.
        let cannot_refract = ratio * sin_theta > 1.0;
        let out = if cannot_refract || sampler.uniform() < reflectance(cos_theta, ratio) {
            dir.reflect(hit.normal)
        } else {
            dir.refract(hit.normal, ratio)
        };

        Some(Scatter {
            attenuation: self.tint,
            ray: Ray::new(hit.point, out.try_normalized()?),
            specular: true,
            pdf: 0.0,
        })
    }
}

/// Schlick's approximation to the Fresnel reflectance.
fn reflectance(cos_theta: f64, ratio: f64) -> f64 {
    let r0 = ((1.0 - ratio) / (1.0 + ratio)).powi(2);
    r0 + (1.0 - r0) * (1.0 - cos_theta).powi(5)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schlick_limits() {
        // Normal incidence on glass reflects about 4%.
        assert!((reflectance(1.0, 1.0 / 1.5) - 0.04).abs() < 1e-12);
        // Grazing incidence reflects everything.
        assert!((reflectance(0.0, 1.0 / 1.5) - 1.0).abs() < 1e-12);
        // Matched indices reflect nothing head-on.
        assert_eq!(reflectance(1.0, 1.0), 0.0);
    }
}
