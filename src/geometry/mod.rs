//! Shapes, bounding boxes and the record of a ray-surface intersection.

mod aabb;
mod hit;
mod light;
mod primitive;
mod quad;
mod sphere;
mod triangle;

pub use aabb::Aabb;
pub use hit::{HitRecord, Hittable, MaterialId};
pub use light::LightSample;
pub use primitive::Primitive;
pub use quad::Quad;
pub use sphere::Sphere;
pub use triangle::Triangle;
