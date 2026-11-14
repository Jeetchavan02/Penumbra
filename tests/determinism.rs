//! A render is a pure function of the scene, the settings and the seed.

use penumbra::accel::SplitMethod;
use penumbra::camera::CameraSpec;
use penumbra::geometry::{Quad, Sphere};
use penumbra::material::Material;
use penumbra::math::{Color, Point3, Vec3};
use penumbra::render::{render, Film, RenderSettings};
use penumbra::scene::{Background, Scene};

/// Small scene with every material and both primitive types.
fn scene(split: SplitMethod) -> Scene {
    let mut b = Scene::builder();
    let ground = b.material(Material::lambertian(Color::splat(0.5)));
    let red = b.material(Material::lambertian(Color::new(0.7, 0.2, 0.2)));
    let steel = b.material(Material::metal(Color::splat(0.8), 0.2));
    let glass = b.material(Material::dielectric(1.5));
    let lamp = b.material(Material::emissive(Color::splat(6.0)));
    b.add(Quad::new(Point3::new(-10.0, 0.0, -10.0), Vec3::new(0.0, 0.0, 20.0), Vec3::new(20.0, 0.0, 0.0), ground));
    b.add(Sphere::new(Point3::new(-2.1, 1.0, 0.0), 1.0, red));
    b.add(Sphere::new(Point3::new(0.0, 1.0, 0.0), 1.0, glass));
    b.add(Sphere::new(Point3::new(2.1, 1.0, 0.0), 1.0, steel));
    b.add(Sphere::new(Point3::new(0.0, 5.0, 2.0), 1.0, lamp));
    b.background(Background::Sky { horizon: Color::splat(0.3), zenith: Color::new(0.1, 0.15, 0.3) });
    b.camera(CameraSpec {
        look_from: Point3::new(0.0, 2.5, 9.0),
        look_at: Point3::new(0.0, 1.0, 0.0),
        aspect_ratio: 2.0,
        defocus_angle: 1.0,
        focus_distance: 9.0,
        ..CameraSpec::default()
    });
    b.build(split)
}

fn settings(seed: u64) -> RenderSettings {
    RenderSettings { width: 64, height: 32, samples_per_pixel: 8, seed, ..RenderSettings::default() }
}

fn render_with_threads(threads: usize, seed: u64, split: SplitMethod) -> Film {
    let pool = rayon::ThreadPoolBuilder::new().num_threads(threads).build().unwrap();
    let (film, stats) = pool.install(|| render(&scene(split), &settings(seed)));
    assert_eq!(stats.rejected_samples, 0);
    film
}

#[test]
fn thread_count_does_not_change_the_image() {
    let one = render_with_threads(1, 11, SplitMethod::Sah);
    let many = render_with_threads(7, 11, SplitMethod::Sah);
    assert_eq!(one.pixels(), many.pixels());
}

#[test]
fn different_seeds_give_different_noise() {
    let a = render_with_threads(4, 1, SplitMethod::Sah);
    let b = render_with_threads(4, 2, SplitMethod::Sah);
    assert_ne!(a.pixels(), b.pixels());
}

#[test]
fn split_method_does_not_change_the_image() {
    // Both trees must return the same nearest hit for every ray, so the whole
    // random walk, and therefore every pixel, is identical.
    let midpoint = render_with_threads(4, 5, SplitMethod::Midpoint);
    let sah = render_with_threads(4, 5, SplitMethod::Sah);
    assert_eq!(midpoint.pixels(), sah.pixels());
}

#[test]
fn every_pixel_is_finite_and_non_negative() {
    let film = render_with_threads(4, 3, SplitMethod::Sah);
    for p in film.pixels() {
        assert!(p.is_finite() && p.min_component() >= 0.0);
    }
    assert!(film.mean().max_component() > 0.01, "image should not be black");
}
