use super::{Aabb, HitRecord, Hittable, MaterialId, Quad, Sphere, Triangle};
use crate::math::{Interval, Ray};

/// The closed set of shapes a scene can contain.
///
/// Stored by value in one contiguous `Vec`, so a BVH leaf is a slice and
/// dispatch is a `match` the optimiser can inline. Adding a shape means adding
/// a variant here; the compiler then points at every `match` that needs an arm.
#[derive(Debug, Clone, Copy)]
pub enum Primitive {
    Sphere(Sphere),
    Quad(Quad),
    Triangle(Triangle),
}

impl Primitive {
    pub fn material(&self) -> MaterialId {
        match self {
            Primitive::Sphere(s) => s.material,
            Primitive::Quad(q) => q.material,
            Primitive::Triangle(t) => t.material,
        }
    }
}

impl Hittable for Primitive {
    #[inline]
    fn hit(&self, ray: &Ray, t: Interval) -> Option<HitRecord> {
        match self {
            Primitive::Sphere(s) => s.hit(ray, t),
            Primitive::Quad(q) => q.hit(ray, t),
            Primitive::Triangle(tri) => tri.hit(ray, t),
        }
    }

    fn bounding_box(&self) -> Aabb {
        match self {
            Primitive::Sphere(s) => s.bounding_box(),
            Primitive::Quad(q) => q.bounding_box(),
            Primitive::Triangle(t) => t.bounding_box(),
        }
    }
}

impl From<Sphere> for Primitive {
    fn from(s: Sphere) -> Self {
        Primitive::Sphere(s)
    }
}

impl From<Quad> for Primitive {
    fn from(q: Quad) -> Self {
        Primitive::Quad(q)
    }
}

impl From<Triangle> for Primitive {
    fn from(t: Triangle) -> Self {
        Primitive::Triangle(t)
    }
}
