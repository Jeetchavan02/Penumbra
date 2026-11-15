//! PENUMBRA: a CPU path tracer written from scratch.
//!
//! The crate is a library with a thin binary on top so that integration tests
//! can build scenes and render them without going through the CLI.

pub mod accel;
pub mod camera;
pub mod geometry;
pub mod material;
pub mod math;
pub mod mesh;
pub mod render;
pub mod scene;
