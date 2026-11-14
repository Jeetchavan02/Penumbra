use super::Aabb;
use crate::math::{Interval, Point3, Ray, Vec3};

/// Index into the scene's material table.
///
/// A hit names its material instead of borrowing it. That keeps `HitRecord`
/// free of lifetimes and keeps this module independent of `material`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MaterialId(pub(crate) u32);

impl MaterialId {
    pub fn index(self) -> usize {
        self.0 as usize
    }
}

/// Everything a material needs to know about an intersection.
#[derive(Debug, Clone, Copy)]
pub struct HitRecord {
    /// Distance along the ray.
    pub t: f64,
    pub point: Point3,
    /// Unit normal, always on the side the ray arrived from.
    pub normal: Vec3,
    /// True when the ray struck the outward-facing side of the surface.
    pub front_face: bool,
    pub material: MaterialId,
}

impl HitRecord {
    /// `outward_normal` is the unit geometric normal; it is flipped here if the
    /// ray arrived from behind it.
    pub fn new(ray: &Ray, t: f64, point: Point3, outward_normal: Vec3, material: MaterialId) -> Self {
        let front_face = ray.dir().dot(outward_normal) < 0.0;
        HitRecord { t, point, normal: if front_face { outward_normal } else { -outward_normal }, front_face, material }
    }
}

/// Anything a ray can intersect.
///
/// `Send + Sync` are supertraits so that every implementor can be shared
/// across render threads; forgetting them should fail here, not at the
/// `par_iter` call site.
pub trait Hittable: Send + Sync {
    /// Nearest intersection with `t` strictly inside the interval.
    fn hit(&self, ray: &Ray, t: Interval) -> Option<HitRecord>;

    fn bounding_box(&self) -> Aabb;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normal_always_opposes_the_ray() {
        let up = Vec3::new(0.0, 1.0, 0.0);
        let down_ray = Ray::new(Vec3::new(0.0, 1.0, 0.0), -up);
        let up_ray = Ray::new(Vec3::new(0.0, -1.0, 0.0), up);

        let front = HitRecord::new(&down_ray, 1.0, Vec3::ZERO, up, MaterialId(0));
        assert!(front.front_face);
        assert_eq!(front.normal, up);

        let back = HitRecord::new(&up_ray, 1.0, Vec3::ZERO, up, MaterialId(0));
        assert!(!back.front_face);
        assert_eq!(back.normal, -up);
    }
}
