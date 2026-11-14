//! Shapes, bounding boxes and the record of a ray-surface intersection.

mod aabb;
mod hit;
mod light;
mod primitive;
mod quad;
mod sphere;

pub use aabb::Aabb;
pub use hit::{HitRecord, Hittable, MaterialId};
pub use light::LightSample;
pub use primitive::Primitive;
pub use quad::Quad;
pub use sphere::Sphere;
