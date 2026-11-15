//! Sampling directions toward a primitive, for direct lighting.

use std::f64::consts::TAU;

use super::{Hittable, Primitive, Quad, Sphere, Triangle};
use crate::math::{Interval, Point3, Ray, Sampler, Vec3};

/// A direction toward a light, chosen at random.
#[derive(Debug, Clone, Copy)]
pub struct LightSample {
    /// Unit direction from the shading point to the light.
    pub dir: Vec3,
    /// Distance to the sampled point on the light.
    pub distance: f64,
    /// Probability density of `dir`, per unit solid angle.
    pub pdf: f64,
}

impl Sphere {
    /// Sample the cone of directions the sphere subtends as seen from `origin`,
    /// uniformly in solid angle. `None` if `origin` is inside the sphere.
    pub fn sample_toward(&self, origin: Point3, sampler: &mut Sampler) -> Option<LightSample> {
        let to_center = self.center - origin;
        let dist_sq = to_center.length_squared();
        let radius_sq = self.radius * self.radius;
        if dist_sq <= radius_sq * (1.0 + 1e-9) {
            return None;
        }

        let sin_sq_max = radius_sq / dist_sq;
        let cos_max = (1.0 - sin_sq_max).sqrt();
        // 1 - cos_max, computed without cancellation for small, distant lights.
        let one_minus_cos_max = sin_sq_max / (1.0 + cos_max);

        let cos_theta = 1.0 - sampler.uniform() * one_minus_cos_max;
        let sin_theta = (1.0 - cos_theta * cos_theta).max(0.0).sqrt();
        let (sin_phi, cos_phi) = (TAU * sampler.uniform()).sin_cos();

        let dist = dist_sq.sqrt();
        let w = to_center / dist;
        let (u, v) = w.orthonormal_basis();
        let dir = (u * (sin_theta * cos_phi) + v * (sin_theta * sin_phi) + w * cos_theta).normalized();

        // Distance to the near surface. A direction on the very rim of the cone
        // can miss by rounding; the tangent distance is the right limit there.
        let ray = Ray::new(origin, dir);
        let distance = self.hit(&ray, Interval::new(0.0, f64::INFINITY)).map_or(dist * cos_theta, |hit| hit.t);

        Some(LightSample { dir, distance, pdf: 1.0 / (TAU * one_minus_cos_max) })
    }
}

impl Quad {
    /// Sample a point uniformly on the quad and express its density in solid
    /// angle from `origin`. `None` if the quad is seen exactly edge-on.
    pub fn sample_toward(&self, origin: Point3, sampler: &mut Sampler) -> Option<LightSample> {
        let point = self.q + self.u * sampler.uniform() + self.v * sampler.uniform();
        area_sample(origin, point, self.normal, self.area)
    }
}

impl Triangle {
    /// Sample a point uniformly on the triangle and express its density in
    /// solid angle from `origin`. `None` if it is seen exactly edge-on.
    pub fn sample_toward(&self, origin: Point3, sampler: &mut Sampler) -> Option<LightSample> {
        // Square-root warp of the unit square onto the triangle.
        let r = sampler.uniform().sqrt();
        let (a, b) = (1.0 - r, r * sampler.uniform());
        let point = self.p0 + self.e1 * a + self.e2 * b;
        area_sample(origin, point, self.face_normal(), self.area())
    }
}

/// Convert a uniformly chosen `point` on a flat light of the given `area` into
/// a direction from `origin` with its solid-angle density.
fn area_sample(origin: Point3, point: Point3, normal: Vec3, area: f64) -> Option<LightSample> {
    let offset = point - origin;
    let dist_sq = offset.length_squared();
    let distance = dist_sq.sqrt();
    if distance < 1e-9 {
        return None;
    }
    let dir = offset / distance;
    let cos_light = normal.dot(dir).abs();
    if cos_light < 1e-9 {
        return None;
    }
    // dA = dw * dist^2 / cos, so pdf_w = pdf_A * dist^2 / cos.
    Some(LightSample { dir, distance, pdf: dist_sq / (cos_light * area) })
}

/// Density that a flat light of the given `area`, sampled uniformly, assigns
/// to the direction `dir` from `origin`. Zero if the ray does not reach it.
fn area_pdf(shape: &impl Hittable, normal: Vec3, area: f64, origin: Point3, dir: Vec3) -> f64 {
    let Some(hit) = shape.hit(&Ray::new(origin, dir), Interval::new(0.0, f64::INFINITY)) else {
        return 0.0;
    };
    let cos_light = normal.dot(dir).abs();
    if cos_light < 1e-9 {
        return 0.0;
    }
    hit.t * hit.t / (cos_light * area)
}

impl Primitive {
    /// The density with which [`Primitive::sample_toward`] would have chosen
    /// `dir` from `origin`, for a `dir` already known to reach a sphere. Zero
    /// if it never would: the ray misses a flat light, or `origin` is inside
    /// a sphere.
    pub fn pdf_toward(&self, origin: Point3, dir: Vec3) -> f64 {
        match self {
            Primitive::Sphere(s) => {
                let dist_sq = (s.center - origin).length_squared();
                let radius_sq = s.radius * s.radius;
                if dist_sq <= radius_sq * (1.0 + 1e-9) {
                    return 0.0;
                }
                let sin_sq_max = radius_sq / dist_sq;
                let one_minus_cos_max = sin_sq_max / (1.0 + (1.0 - sin_sq_max).sqrt());
                1.0 / (TAU * one_minus_cos_max)
            }
            Primitive::Quad(q) => area_pdf(q, q.normal, q.area, origin, dir),
            Primitive::Triangle(t) => area_pdf(t, t.face_normal(), t.area(), origin, dir),
        }
    }

    pub fn sample_toward(&self, origin: Point3, sampler: &mut Sampler) -> Option<LightSample> {
        match self {
            Primitive::Sphere(s) => s.sample_toward(origin, sampler),
            Primitive::Quad(q) => q.sample_toward(origin, sampler),
            Primitive::Triangle(t) => t.sample_toward(origin, sampler),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::MaterialId;
    use std::f64::consts::PI;

    const RANGE: Interval = Interval::new(1e-6, f64::INFINITY);

    /// Monte Carlo estimate of the solid angle: E[1 / pdf] over samples that
    /// actually reach the shape.
    fn estimated_solid_angle(shape: &Primitive, origin: Point3) -> f64 {
        let mut s = Sampler::new(9);
        let n = 100_000;
        let mut sum = 0.0;
        for _ in 0..n {
            let ls = shape.sample_toward(origin, &mut s).unwrap();
            assert!(ls.pdf > 0.0 && ls.pdf.is_finite());
            assert!((ls.dir.length() - 1.0).abs() < 1e-9);
            let hit = shape.hit(&Ray::new(origin, ls.dir), RANGE).expect("sampled direction must reach the light");
            // Evaluating the density afterwards gives the value sampling reported.
            let pdf = shape.pdf_toward(origin, ls.dir);
            assert!((pdf - ls.pdf).abs() < 1e-6 * ls.pdf, "{pdf} vs {}", ls.pdf);
            assert!((hit.t - ls.distance).abs() < 1e-6 * ls.distance.max(1.0));
            sum += 1.0 / ls.pdf;
        }
        sum / n as f64
    }

    #[test]
    fn sphere_samples_cover_exactly_its_solid_angle() {
        let sphere = Sphere::new(Point3::new(0.0, 5.0, 0.0), 1.0, MaterialId(0));
        // Cap of half-angle asin(r/d): 2*pi*(1 - cos).
        let expected = TAU * (1.0 - (1.0 - 1.0f64 / 25.0).sqrt());
        let got = estimated_solid_angle(&sphere.into(), Point3::ZERO);
        assert!((got - expected).abs() < 1e-9, "{got} vs {expected}");
    }

    #[test]
    fn quad_samples_integrate_to_its_solid_angle() {
        // A 2x2 square centred one unit above the origin is one face of a cube
        // seen from its centre: a sixth of the sphere.
        let quad =
            Quad::new(Point3::new(-1.0, 1.0, -1.0), Vec3::new(2.0, 0.0, 0.0), Vec3::new(0.0, 0.0, 2.0), MaterialId(0));
        let got = estimated_solid_angle(&quad.into(), Point3::ZERO);
        assert!((got - 4.0 * PI / 6.0).abs() < 0.01, "{got}");
    }

    #[test]
    fn triangle_samples_integrate_to_its_solid_angle() {
        // Half of the cube face above: a twelfth of the sphere.
        let corners = [Point3::new(-1.0, 1.0, -1.0), Point3::new(1.0, 1.0, -1.0), Point3::new(-1.0, 1.0, 1.0)];
        let triangle = Triangle::flat(corners, MaterialId(0)).unwrap();
        let got = estimated_solid_angle(&triangle.into(), Point3::ZERO);
        assert!((got - 4.0 * PI / 12.0).abs() < 0.01, "{got}");
    }

    #[test]
    fn no_sample_from_inside_a_sphere() {
        let sphere = Sphere::new(Point3::ZERO, 2.0, MaterialId(0));
        assert!(sphere.sample_toward(Point3::new(0.5, 0.0, 0.0), &mut Sampler::new(1)).is_none());
    }

    #[test]
    fn distant_small_light_keeps_precision() {
        let sphere = Sphere::new(Point3::new(0.0, 1e6, 0.0), 1.0, MaterialId(0));
        let ls = sphere.sample_toward(Point3::ZERO, &mut Sampler::new(1)).unwrap();
        // Solid angle ~ pi * r^2 / d^2.
        assert!((1.0 / ls.pdf / (PI * 1e-12) - 1.0).abs() < 1e-6);
    }
}
