use super::{Aabb, HitRecord, Hittable, MaterialId};
use crate::math::{Interval, Point3, Ray, Vec3};

/// A parallelogram: the points `q + a*u + b*v` for `a, b` in `[0, 1]`.
#[derive(Debug, Clone, Copy)]
pub struct Quad {
    pub q: Point3,
    pub u: Vec3,
    pub v: Vec3,
    pub material: MaterialId,
    /// Unit normal, `u x v` normalised.
    pub(super) normal: Vec3,
    /// Plane constant: `normal . p = d` for points on the plane.
    pub(super) d: f64,
    /// `n / (n . n)` for the unnormalised `n = u x v`; maps a planar offset to
    /// its `(a, b)` coordinates.
    pub(super) w: Vec3,
    pub(super) area: f64,
}

impl Quad {
    /// Bounding boxes of flat quads are padded to at least this thickness.
    const BOX_PADDING: f64 = 1e-4;

    /// Panics if `u` and `v` are parallel or zero.
    pub fn new(q: Point3, u: Vec3, v: Vec3, material: MaterialId) -> Self {
        let n = u.cross(v);
        let area = n.length();
        assert!(area > 1e-12 && area.is_finite(), "quad edges must span a non-zero area");
        let normal = n / area;
        Quad { q, u, v, material, normal, d: normal.dot(q), w: n / n.dot(n), area }
    }

    pub fn normal(&self) -> Vec3 {
        self.normal
    }

    pub fn area(&self) -> f64 {
        self.area
    }
}

impl Hittable for Quad {
    fn hit(&self, ray: &Ray, t: Interval) -> Option<HitRecord> {
        let denom = self.normal.dot(ray.dir());
        // Parallel rays never hit; near-parallel ones would divide by ~0.
        if denom.abs() < 1e-10 {
            return None;
        }
        let root = (self.d - self.normal.dot(ray.origin())) / denom;
        if !t.surrounds(root) {
            return None;
        }

        let point = ray.at(root);
        let planar = point - self.q;
        let a = self.w.dot(planar.cross(self.v));
        let b = self.w.dot(self.u.cross(planar));
        let unit = Interval::new(0.0, 1.0);
        if !unit.contains(a) || !unit.contains(b) {
            return None;
        }
        Some(HitRecord::new(ray, root, point, self.normal, self.material))
    }

    fn bounding_box(&self) -> Aabb {
        let far = self.q + self.u + self.v;
        Aabb::from_points(self.q, far)
            .union(Aabb::from_points(self.q + self.u, self.q + self.v))
            .padded(Self::BOX_PADDING)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FOREVER: Interval = Interval::new(1e-4, f64::INFINITY);

    /// Unit square in the y = 0 plane with normal +y.
    fn floor() -> Quad {
        Quad::new(Vec3::ZERO, Vec3::new(0.0, 0.0, 1.0), Vec3::new(1.0, 0.0, 0.0), MaterialId(0))
    }

    #[test]
    fn normal_follows_the_right_hand_rule() {
        assert!((floor().normal() - Vec3::new(0.0, 1.0, 0.0)).length() < 1e-12);
        assert!((floor().area() - 1.0).abs() < 1e-12);
    }

    #[test]
    fn hit_inside_and_miss_outside() {
        let down = Vec3::new(0.0, -1.0, 0.0);
        let inside = Ray::new(Vec3::new(0.25, 2.0, 0.75), down);
        let outside = Ray::new(Vec3::new(1.25, 2.0, 0.75), down);
        let hit = floor().hit(&inside, FOREVER).unwrap();
        assert!((hit.t - 2.0).abs() < 1e-12);
        assert!(hit.front_face);
        assert!(floor().hit(&outside, FOREVER).is_none());
    }

    #[test]
    fn both_sides_are_hittable() {
        let up = Ray::new(Vec3::new(0.5, -1.0, 0.5), Vec3::new(0.0, 1.0, 0.0));
        let hit = floor().hit(&up, FOREVER).unwrap();
        assert!(!hit.front_face);
        assert_eq!(hit.normal, Vec3::new(0.0, -1.0, 0.0));
    }

    #[test]
    fn parallel_rays_miss_without_nan() {
        let along = Ray::new(Vec3::new(-1.0, 0.0, 0.5), Vec3::new(1.0, 0.0, 0.0));
        assert!(floor().hit(&along, FOREVER).is_none());
    }

    #[test]
    fn skewed_parallelogram_uses_its_own_coordinates() {
        let q = Quad::new(Vec3::ZERO, Vec3::new(2.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 1.0), MaterialId(0));
        let down = Vec3::new(0.0, -1.0, 0.0);
        // (2.5, 0.5) is inside the sheared shape but (0.1, 0.9) is not.
        assert!(q.hit(&Ray::new(Vec3::new(2.4, 1.0, 0.5), down), FOREVER).is_some());
        assert!(q.hit(&Ray::new(Vec3::new(0.1, 1.0, 0.9), down), FOREVER).is_none());
    }

    #[test]
    fn flat_box_has_thickness() {
        let b = floor().bounding_box();
        assert!(b.extent().y > 0.0);
        assert!(b.hit(&Ray::new(Vec3::new(0.5, 2.0, 0.5), Vec3::new(0.0, -1.0, 0.0)), FOREVER));
    }
}
