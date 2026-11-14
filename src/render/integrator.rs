//! The path tracing loop.

use std::f64::consts::PI;

use super::RenderSettings;
use crate::geometry::HitRecord;
use crate::math::{Color, Interval, Ray, Sampler};
use crate::scene::Scene;

/// Minimum distance a ray must travel before it can hit anything.
///
/// Ray directions are unit length, so this is a world-space distance: large
/// enough to step off the surface a ray just left, small enough not to tunnel
/// through real geometry at the scales the scene library uses.
pub const RAY_EPSILON: f64 = 1e-4;

/// Bounces a path always gets before Russian roulette may end it.
const ROULETTE_START: u32 = 4;

/// Radiance arriving at the origin of `ray` from the direction it points in.
///
/// Iterative rather than recursive: `throughput` carries the product of all
/// attenuations so far, so depth costs no stack and a path can stop anywhere.
pub fn radiance(scene: &Scene, mut ray: Ray, settings: &RenderSettings, sampler: &mut Sampler) -> Color {
    let sample_lights = settings.direct_lighting && !scene.lights().is_empty();
    let mut radiance = Color::ZERO;
    let mut throughput = Color::ONE;
    // Light reached by a diffuse bounce was already counted by the direct
    // sample taken at that bounce. Counting the emitter again would double it.
    let mut count_emission = true;

    for bounce in 0..settings.max_depth {
        let Some(hit) = scene.hit(&ray, Interval::new(RAY_EPSILON, f64::INFINITY)) else {
            radiance += throughput * scene.background.radiance(ray.dir());
            break;
        };
        let material = scene.material(hit.material);
        if count_emission {
            radiance += throughput * material.emitted();
        }

        let Some(scatter) = material.scatter(&ray, &hit, sampler) else {
            break;
        };

        // Mirrors and glass have no chance of sampling a light by luck of
        // direction, so they find emitters only by following their one ray.
        if sample_lights && !scatter.specular {
            radiance += throughput * direct_light(scene, &hit, scatter.attenuation, sampler);
            count_emission = false;
        } else {
            count_emission = true;
        }

        throughput *= scatter.attenuation;
        ray = scatter.ray;

        // Russian roulette: end dim paths at random, and divide the survivors
        // by their survival probability so the estimate stays unbiased.
        if bounce >= ROULETTE_START {
            let survive = throughput.max_component().min(1.0);
            if sampler.uniform() >= survive {
                break;
            }
            throughput /= survive;
        }
    }
    radiance
}

/// One-sample estimate of the light arriving directly from the scene's
/// emitters and leaving a diffuse surface of the given `albedo`.
fn direct_light(scene: &Scene, hit: &HitRecord, albedo: Color, sampler: &mut Sampler) -> Color {
    let lights = scene.lights();
    let pick = ((sampler.uniform() * lights.len() as f64) as usize).min(lights.len() - 1);
    let light = &lights[pick];

    let Some(sample) = light.sample_toward(hit.point, sampler) else {
        return Color::ZERO;
    };
    let cos_surface = hit.normal.dot(sample.dir);
    if cos_surface <= 0.0 {
        return Color::ZERO;
    }

    // The shadow ray stops just short of the light so the light itself does
    // not count as its own occluder.
    let shadow_ray = Ray::new(hit.point, sample.dir);
    let reach = sample.distance - RAY_EPSILON.max(sample.distance * 1e-9);
    if scene.hit(&shadow_ray, Interval::new(RAY_EPSILON, reach)).is_some() {
        return Color::ZERO;
    }

    // Lambertian BRDF (albedo / pi) * cosine / (pdf of the direction * chance
    // of having picked this light).
    let emitted = scene.material(light.material()).emitted();
    albedo * emitted * (cos_surface * lights.len() as f64 / (PI * sample.pdf))
}
