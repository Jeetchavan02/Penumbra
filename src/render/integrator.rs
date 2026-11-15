//! The path tracing loop.

use super::{Lighting, RenderSettings};
use crate::geometry::HitRecord;
use crate::material::Material;
use crate::math::{Color, Interval, Point3, Ray, Sampler};
use crate::scene::Scene;

/// Minimum distance a ray must travel before it can hit anything.
///
/// Ray directions are unit length, so this is a world-space distance: large
/// enough to step off the surface a ray just left, small enough not to tunnel
/// through real geometry at the scales the scene library uses.
pub const RAY_EPSILON: f64 = 1e-4;

/// Bounces a path always gets before Russian roulette may end it.
const ROULETTE_START: u32 = 4;

/// How much of a light's emission to count when the current ray hits one.
///
/// A light can be reached two ways: by a ray aimed at it on purpose, or by a
/// scattered ray that happens to arrive. Counting both in full would double
/// the light, so each bounce records how the next ray was chosen.
#[derive(Clone, Copy)]
enum Emission {
    /// Nothing else could have found this light: a camera ray, or a mirror or
    /// glass bounce.
    Full,
    /// A light sample at the previous bounce already accounted for it.
    Skip,
    /// A light sample was also taken at `origin`; share the credit according
    /// to how likely each strategy was to choose this direction.
    Shared { origin: Point3, bsdf_pdf: f64 },
}

/// The power heuristic: the weight for a sample drawn with density `chosen`
/// when another strategy would have drawn it with density `other`.
fn power_heuristic(chosen: f64, other: f64) -> f64 {
    // An infinite density is a certain choice; avoid inf / inf.
    if chosen.is_infinite() {
        return 1.0;
    }
    let (a, b) = (chosen * chosen, other * other);
    if a + b > 0.0 {
        a / (a + b)
    } else {
        0.0
    }
}

/// Radiance arriving at the origin of `ray` from the direction it points in.
///
/// Iterative rather than recursive: `throughput` carries the product of all
/// attenuations so far, so depth costs no stack and a path can stop anywhere.
pub fn radiance(scene: &Scene, mut ray: Ray, settings: &RenderSettings, sampler: &mut Sampler) -> Color {
    let lighting = if scene.lights().is_empty() { Lighting::BsdfOnly } else { settings.lighting };
    let light_count = scene.lights().len() as f64;
    let mut radiance = Color::ZERO;
    let mut throughput = Color::ONE;
    let mut emission = Emission::Full;

    for bounce in 0..settings.max_depth {
        let Some(hit) = scene.hit(&ray, Interval::new(RAY_EPSILON, f64::INFINITY)) else {
            // The background is never sampled as a light, so it always counts.
            radiance += throughput * scene.background.radiance(ray.dir());
            break;
        };
        let material = scene.material(hit.material);

        if material.is_emissive() {
            let weight = match emission {
                Emission::Full => 1.0,
                Emission::Skip => 0.0,
                Emission::Shared { origin, bsdf_pdf } => {
                    let light_pdf = scene.primitive(hit.primitive).pdf_toward(origin, ray.dir()) / light_count;
                    power_heuristic(bsdf_pdf, light_pdf)
                }
            };
            radiance += throughput * material.emitted() * weight;
        }

        let Some(scatter) = material.scatter(&ray, &hit, sampler) else {
            break;
        };

        // Mirrors and glass reflect a single direction, which a light sample
        // has no chance of matching, so they find lights only by following it.
        if scatter.specular || lighting == Lighting::BsdfOnly {
            emission = Emission::Full;
        } else {
            radiance += throughput * direct_light(scene, material, &ray, &hit, lighting, sampler);
            emission = match lighting {
                Lighting::Mis => Emission::Shared { origin: hit.point, bsdf_pdf: scatter.pdf },
                _ => Emission::Skip,
            };
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

/// One-sample estimate of the light that arrives at `hit` straight from the
/// scene's emitters and leaves back along `ray_in`.
fn direct_light(
    scene: &Scene,
    material: &Material,
    ray_in: &Ray,
    hit: &HitRecord,
    lighting: Lighting,
    sampler: &mut Sampler,
) -> Color {
    let lights = scene.lights();
    let pick = ((sampler.uniform() * lights.len() as f64) as usize).min(lights.len() - 1);
    let light = &lights[pick];

    let Some(sample) = light.sample_toward(hit.point, sampler) else {
        return Color::ZERO;
    };
    let Some(bsdf) = material.eval(ray_in, hit, sample.dir) else {
        return Color::ZERO;
    };

    // The shadow ray stops just short of the light so the light itself does
    // not count as its own occluder.
    let shadow_ray = Ray::new(hit.point, sample.dir);
    let reach = sample.distance - RAY_EPSILON.max(sample.distance * 1e-9);
    if scene.hit(&shadow_ray, Interval::new(RAY_EPSILON, reach)).is_some() {
        return Color::ZERO;
    }

    // Density of this direction: the chance of picking this light times the
    // light's own density.
    let light_pdf = sample.pdf / lights.len() as f64;
    let weight = match lighting {
        Lighting::Mis => power_heuristic(light_pdf, bsdf.pdf),
        _ => 1.0,
    };
    let emitted = scene.material(light.material()).emitted();
    bsdf.value * emitted * (weight / light_pdf)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heuristic_weights_sum_to_one() {
        for (a, b) in [(1.0, 1.0), (0.1, 30.0), (1e-9, 1e9), (5.0, 0.0)] {
            assert!((power_heuristic(a, b) + power_heuristic(b, a) - 1.0).abs() < 1e-12);
        }
        assert_eq!(power_heuristic(3.0, 0.0), 1.0);
        assert_eq!(power_heuristic(0.0, 0.0), 0.0);
        assert_eq!(power_heuristic(f64::INFINITY, 2.0), 1.0);
        // The better-suited strategy dominates.
        assert!(power_heuristic(10.0, 1.0) > 0.98);
    }
}
