//! Vectors, rays, intervals and random sampling.

mod interval;
mod ray;
mod sampler;
mod vec3;

pub use interval::Interval;
pub use ray::Ray;
pub use sampler::Sampler;
pub use vec3::{Color, Point3, Vec3};
