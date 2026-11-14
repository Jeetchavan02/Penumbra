//! Shapes, bounding boxes and the record of a ray-surface intersection.

mod aabb;
mod hit;
mod quad;
mod sphere;

pub use aabb::Aabb;
pub use hit::{HitRecord, Hittable, MaterialId};
pub use quad::Quad;
pub use sphere::Sphere;
