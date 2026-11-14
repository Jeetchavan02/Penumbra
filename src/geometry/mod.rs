//! Shapes, bounding boxes and the record of a ray-surface intersection.

mod aabb;
mod hit;

pub use aabb::Aabb;
pub use hit::{HitRecord, Hittable, MaterialId};
