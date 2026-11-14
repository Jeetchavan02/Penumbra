use crate::math::{Interval, Point3, Ray, Vec3};

/// Axis-aligned bounding box.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Aabb {
    pub min: Point3,
    pub max: Point3,
}

impl Aabb {
    /// The identity for [`Aabb::union`]: contains nothing.
    pub const EMPTY: Aabb = Aabb { min: Vec3::splat(f64::INFINITY), max: Vec3::splat(f64::NEG_INFINITY) };

    /// Smallest box containing both points, in any order.
    pub fn from_points(a: Point3, b: Point3) -> Self {
        Aabb { min: a.min(b), max: a.max(b) }
    }

    pub fn union(self, o: Aabb) -> Aabb {
        Aabb { min: self.min.min(o.min), max: self.max.max(o.max) }
    }

    pub fn grow(self, p: Point3) -> Aabb {
        Aabb { min: self.min.min(p), max: self.max.max(p) }
    }

    pub fn extent(self) -> Vec3 {
        self.max - self.min
    }

    pub fn centroid(self) -> Point3 {
        (self.min + self.max) * 0.5
    }

    /// Zero for an empty box, so empty bins cost nothing in the SAH.
    pub fn surface_area(self) -> f64 {
        let e = self.extent();
        if e.min_component() < 0.0 {
            return 0.0;
        }
        2.0 * (e.x * e.y + e.y * e.z + e.z * e.x)
    }

    pub fn longest_axis(self) -> usize {
        let e = self.extent();
        if e.x >= e.y && e.x >= e.z {
            0
        } else if e.y >= e.z {
            1
        } else {
            2
        }
    }

    /// Widen any axis thinner than `min_width`. Flat primitives need this: a
    /// zero-thickness slab is rejected by the box test for grazing rays.
    pub fn padded(self, min_width: f64) -> Aabb {
        let e = self.extent();
        let pad = |w: f64| if w < min_width { (min_width - w) * 0.5 } else { 0.0 };
        let p = Vec3::new(pad(e.x), pad(e.y), pad(e.z));
        Aabb { min: self.min - p, max: self.max + p }
    }

    /// Slab test. Does the ray pass through the box anywhere within `t`?
    ///
    /// When a direction component is zero and the origin lies exactly on a slab
    /// plane, `0 * inf` gives NaN. `f64::max`/`min` return the other operand in
    /// that case, so the NaN bound is ignored and the test stays conservative.
    pub fn hit(&self, ray: &Ray, t: Interval) -> bool {
        let (origin, inv) = (ray.origin(), ray.inv_dir());
        let (mut t_min, mut t_max) = (t.min, t.max);
        for axis in 0..3 {
            let t0 = (self.min[axis] - origin[axis]) * inv[axis];
            let t1 = (self.max[axis] - origin[axis]) * inv[axis];
            let (near, far) = if inv[axis] < 0.0 { (t1, t0) } else { (t0, t1) };
            t_min = t_min.max(near);
            t_max = t_max.min(far);
            if t_max < t_min {
                return false;
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unit_box() -> Aabb {
        Aabb::from_points(Vec3::splat(-1.0), Vec3::splat(1.0))
    }

    const FOREVER: Interval = Interval::new(0.0, f64::INFINITY);

    #[test]
    fn hits_and_misses() {
        let b = unit_box();
        let toward = Ray::new(Vec3::new(0.0, 0.0, 5.0), Vec3::new(0.0, 0.0, -1.0));
        let away = Ray::new(Vec3::new(0.0, 0.0, 5.0), Vec3::new(0.0, 0.0, 1.0));
        let beside = Ray::new(Vec3::new(3.0, 0.0, 5.0), Vec3::new(0.0, 0.0, -1.0));
        assert!(b.hit(&toward, FOREVER));
        assert!(!b.hit(&away, FOREVER));
        assert!(!b.hit(&beside, FOREVER));
        assert!(!b.hit(&toward, Interval::new(0.0, 3.0)));
    }

    #[test]
    fn ray_in_a_slab_plane_does_not_produce_nan_misses() {
        // Origin on the x = 1 face, direction with a zero x component.
        let r = Ray::new(Vec3::new(1.0, 0.0, 5.0), Vec3::new(0.0, 0.0, -1.0));
        assert!(unit_box().hit(&r, FOREVER));
    }

    #[test]
    fn union_with_empty_is_identity() {
        assert_eq!(Aabb::EMPTY.union(unit_box()), unit_box());
        assert_eq!(Aabb::EMPTY.surface_area(), 0.0);
        assert_eq!(unit_box().surface_area(), 24.0);
    }

    #[test]
    fn padding_only_touches_thin_axes() {
        let flat = Aabb::from_points(Vec3::new(0.0, 2.0, 0.0), Vec3::new(1.0, 2.0, 1.0)).padded(0.1);
        assert!((flat.extent().y - 0.1).abs() < 1e-12);
        assert_eq!(flat.extent().x, 1.0);
        assert_eq!(flat.longest_axis(), 0);
    }
}
