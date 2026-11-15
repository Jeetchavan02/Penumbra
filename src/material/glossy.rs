use std::f64::consts::{PI, TAU};

use super::{Bsdf, Scatter};
use crate::geometry::HitRecord;
use crate::math::{Color, Ray, Sampler, Vec3};

/// A rough reflective surface: a GGX microfacet conductor.
///
/// Unlike [`super::Metal`], whose fuzz has no closed-form density, this lobe
/// can be both sampled and evaluated. That is what allows a light sample and
/// a reflection sample to be weighed against each other.
///
/// Single scattering only: light that would bounce between microfacets is
/// lost, so very rough surfaces render slightly darker than they should.
#[derive(Debug, Clone, Copy)]
pub struct Glossy {
    /// Reflectance at normal incidence.
    pub color: Color,
    /// GGX width, the square of the perceptual roughness.
    alpha: f64,
}

impl Glossy {
    /// `roughness` runs from 0 (mirror-like) to 1 (nearly diffuse). It is
    /// clamped away from zero: a true mirror is a different material, and a
    /// vanishing lobe would overflow the density.
    pub fn new(color: Color, roughness: f64) -> Self {
        Glossy { color, alpha: roughness.clamp(0.03, 1.0).powi(2) }
    }

    pub fn alpha(&self) -> f64 {
        self.alpha
    }

    /// Density of microfacet normals at angle `cos_h` from the surface normal.
    fn distribution(&self, cos_h: f64) -> f64 {
        let a2 = self.alpha * self.alpha;
        let k = cos_h * cos_h * (a2 - 1.0) + 1.0;
        a2 / (PI * k * k)
    }

    /// Smith masking: the fraction of microfacets visible from a direction at
    /// angle `cos_v` from the normal.
    fn masking(&self, cos_v: f64) -> f64 {
        let a2 = self.alpha * self.alpha;
        2.0 * cos_v / (cos_v + (a2 + (1.0 - a2) * cos_v * cos_v).sqrt())
    }

    fn fresnel(&self, cos: f64) -> Color {
        self.color + (Color::ONE - self.color) * (1.0 - cos.clamp(0.0, 1.0)).powi(5)
    }

    /// BSDF times cosine, and the density with which [`Glossy::scatter`]
    /// chooses `wi`. `wo` points back along the incoming ray; all are unit.
    pub fn eval(&self, wo: Vec3, normal: Vec3, wi: Vec3) -> Option<Bsdf> {
        let (cos_o, cos_i) = (normal.dot(wo), normal.dot(wi));
        if cos_o <= 1e-9 || cos_i <= 1e-9 {
            return None;
        }
        let half = (wo + wi).try_normalized()?;
        let d = self.distribution(normal.dot(half));
        let visible = self.masking(cos_o);
        // f * cos_i = F D G / (4 cos_o); pdf = G1(wo) D / (4 cos_o).
        let pdf = visible * d / (4.0 * cos_o);
        if !(pdf > 0.0 && pdf.is_finite()) {
            return None;
        }
        Some(Bsdf { value: self.fresnel(wo.dot(half)) * (pdf * self.masking(cos_i)), pdf })
    }

    pub fn scatter(&self, ray_in: &Ray, hit: &HitRecord, sampler: &mut Sampler) -> Option<Scatter> {
        let wo = -ray_in.dir();
        let half = self.sample_visible_normal(wo, hit.normal, sampler);
        let wi = ray_in.dir().reflect(half).try_normalized()?;
        let bsdf = self.eval(wo, hit.normal, wi)?;
        // value / pdf = F * G1(wi), which never exceeds 1: no fireflies.
        Some(Scatter {
            attenuation: bsdf.value / bsdf.pdf,
            ray: Ray::new(hit.point, wi),
            specular: false,
            pdf: bsdf.pdf,
        })
    }

    /// Sample a microfacet normal from the distribution of normals visible
    /// from `wo` (Heitz 2018). Every sample is usable, unlike sampling the
    /// full distribution, which wastes samples on facets that face away.
    fn sample_visible_normal(&self, wo: Vec3, normal: Vec3, sampler: &mut Sampler) -> Vec3 {
        let (tangent, bitangent) = normal.orthonormal_basis();
        let local = Vec3::new(wo.dot(tangent), wo.dot(bitangent), wo.dot(normal));

        // Stretch the view direction so the surface behaves as roughness 1.
        let view = Vec3::new(self.alpha * local.x, self.alpha * local.y, local.z).normalized();
        let len_sq = view.x * view.x + view.y * view.y;
        let t1 =
            if len_sq > 1e-18 { Vec3::new(-view.y, view.x, 0.0) / len_sq.sqrt() } else { Vec3::new(1.0, 0.0, 0.0) };
        let t2 = view.cross(t1);

        // A point on the disk, warped onto the half-disks visible from `view`.
        let r = sampler.uniform().sqrt();
        let (sin_phi, cos_phi) = (TAU * sampler.uniform()).sin_cos();
        let p1 = r * cos_phi;
        let s = 0.5 * (1.0 + view.z);
        let p2 = (1.0 - s) * (1.0 - p1 * p1).max(0.0).sqrt() + s * r * sin_phi;

        let stretched = t1 * p1 + t2 * p2 + view * (1.0 - p1 * p1 - p2 * p2).max(0.0).sqrt();
        // Undo the stretch.
        let h = Vec3::new(self.alpha * stretched.x, self.alpha * stretched.y, stretched.z.max(1e-12)).normalized();
        tangent * h.x + bitangent * h.y + normal * h.z
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::MaterialId;

    const UP: Vec3 = Vec3::new(0.0, 1.0, 0.0);

    fn setup(incoming: Vec3) -> (Ray, HitRecord) {
        let dir = incoming.normalized();
        let ray = Ray::new(Vec3::ZERO - dir, dir);
        (ray, HitRecord::new(&ray, 1.0, Vec3::ZERO, UP, MaterialId(0)))
    }

    #[test]
    fn sample_weight_equals_value_over_pdf_and_never_exceeds_one() {
        for roughness in [0.05, 0.2, 0.5, 1.0] {
            let m = Glossy::new(Color::ONE, roughness);
            for incoming in [Vec3::new(0.0, -1.0, 0.0), Vec3::new(1.0, -1.0, 0.3), Vec3::new(1.0, -0.05, 0.0)] {
                let (ray, hit) = setup(incoming);
                let mut sampler = Sampler::new(5);
                for _ in 0..2_000 {
                    let Some(s) = m.scatter(&ray, &hit, &mut sampler) else { continue };
                    let bsdf = m.eval(-ray.dir(), hit.normal, s.ray.dir()).unwrap();
                    assert!((bsdf.pdf - s.pdf).abs() <= 1e-9 * s.pdf);
                    assert!((s.attenuation - bsdf.value / bsdf.pdf).length() < 1e-9);
                    assert!(s.attenuation.max_component() <= 1.0 + 1e-9 && s.attenuation.min_component() >= 0.0);
                    assert!(s.ray.dir().dot(UP) > 0.0 && !s.specular);
                }
            }
        }
    }

    #[test]
    fn density_integrates_to_at_most_one() {
        // Uniform directions over the sphere; the estimate of the integral of
        // the pdf is 4 pi times its mean. Mass aimed below the surface is lost.
        for (roughness, incoming) in
            [(0.3, Vec3::new(0.0, -1.0, 0.0)), (0.5, Vec3::new(1.0, -1.0, 0.0)), (1.0, Vec3::new(1.0, -0.3, 0.2))]
        {
            let m = Glossy::new(Color::ONE, roughness);
            let wo = -incoming.normalized();
            let mut sampler = Sampler::new(8);
            let n = 400_000;
            let sum: f64 = (0..n).map(|_| m.eval(wo, UP, sampler.unit_vector()).map_or(0.0, |b| b.pdf)).sum();
            let integral = 4.0 * PI * sum / n as f64;
            assert!(integral <= 1.02 && integral > 0.6, "roughness {roughness}: {integral}");
        }
    }

    #[test]
    fn sampling_matches_the_evaluated_density() {
        // The fraction of samples landing in a cone must equal the integral of
        // the pdf over that cone.
        let m = Glossy::new(Color::ONE, 0.4);
        let (ray, hit) = setup(Vec3::new(1.0, -1.0, 0.0));
        let wo = -ray.dir();
        let axis = ray.dir().reflect(UP);
        let cos_cone = 0.9;

        let mut sampler = Sampler::new(21);
        let n = 300_000;
        let landed = (0..n)
            .filter(|_| m.scatter(&ray, &hit, &mut sampler).is_some_and(|s| s.ray.dir().dot(axis) > cos_cone))
            .count() as f64
            / n as f64;

        let mut inside = 0.0;
        for _ in 0..n {
            let dir = sampler.unit_vector();
            if dir.dot(axis) > cos_cone {
                inside += m.eval(wo, UP, dir).map_or(0.0, |b| b.pdf);
            }
        }
        let expected = 4.0 * PI * inside / n as f64;
        assert!((landed - expected).abs() < 0.01, "sampled {landed}, density says {expected}");
    }

    #[test]
    fn smoother_surfaces_have_tighter_lobes() {
        let (ray, _) = setup(Vec3::new(1.0, -1.0, 0.0));
        let mirror_dir = ray.dir().reflect(UP);
        let peak = |roughness| Glossy::new(Color::ONE, roughness).eval(-ray.dir(), UP, mirror_dir).unwrap().pdf;
        assert!(peak(0.1) > peak(0.3) && peak(0.3) > peak(0.8));
    }

    #[test]
    fn nothing_is_reflected_below_the_surface() {
        let m = Glossy::new(Color::ONE, 0.5);
        assert!(m.eval(UP, UP, Vec3::new(0.0, -1.0, 0.0)).is_none());
        assert!(m.eval(Vec3::new(0.0, -1.0, 0.0), UP, UP).is_none());
    }
}
