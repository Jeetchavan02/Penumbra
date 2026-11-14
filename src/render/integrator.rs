//! The path tracing loop.

use super::RenderSettings;
use crate::math::{Color, Interval, Ray, Sampler};
use crate::scene::Scene;

/// Minimum distance a ray must travel before it can hit anything.
///
/// Ray directions are unit length, so this is a world-space distance: large
/// enough to step off the surface a ray just left, small enough not to tunnel
/// through real geometry at the scales the scene library uses.
pub const RAY_EPSILON: f64 = 1e-4;

/// Radiance arriving at the origin of `ray` from the direction it points in.
///
/// Iterative rather than recursive: `throughput` carries the product of all
/// attenuations so far, so depth costs no stack and a path can stop anywhere.
pub fn radiance(scene: &Scene, mut ray: Ray, settings: &RenderSettings, sampler: &mut Sampler) -> Color {
    let mut radiance = Color::ZERO;
    let mut throughput = Color::ONE;

    for _ in 0..settings.max_depth {
        let Some(hit) = scene.hit(&ray, Interval::new(RAY_EPSILON, f64::INFINITY)) else {
            radiance += throughput * scene.background.radiance(ray.dir());
            break;
        };
        let material = scene.material(hit.material);
        radiance += throughput * material.emitted();

        let Some(scatter) = material.scatter(&ray, &hit, sampler) else {
            break;
        };
        throughput *= scatter.attenuation;
        ray = scatter.ray;
    }
    radiance
}
