//! Furnace tests: objects inside a uniformly white environment.
//!
//! With radiance 1 arriving from every direction, a surface of albedo `a`
//! must look exactly `a` bright, and a lossless one must be invisible. Any
//! energy gained or lost, any shadow acne and any NaN shows up as a number
//! that is not the expected one.

use penumbra::accel::SplitMethod;
use penumbra::camera::CameraSpec;
use penumbra::geometry::Sphere;
use penumbra::material::Material;
use penumbra::math::{Color, Point3};
use penumbra::render::{render, Film, RenderSettings};
use penumbra::scene::{Background, Scene};

fn furnace(material: Material) -> Film {
    let mut b = Scene::builder();
    let m = b.material(material);
    b.add(Sphere::new(Point3::ZERO, 1.0, m));
    b.background(Background::Solid(Color::ONE));
    b.camera(CameraSpec {
        look_from: Point3::new(0.0, 0.0, 4.0),
        aspect_ratio: 1.0,
        focus_distance: 4.0,
        ..CameraSpec::default()
    });
    let scene = b.build(SplitMethod::Sah);
    let settings = RenderSettings { width: 48, height: 48, samples_per_pixel: 32, ..RenderSettings::default() };
    let (film, stats) = render(&scene, &settings);
    assert_eq!(stats.rejected_samples, 0, "no sample may be NaN, infinite or negative");
    film
}

#[test]
fn diffuse_sphere_reflects_exactly_its_albedo() {
    let film = furnace(Material::lambertian(Color::splat(0.8)));
    // A convex object cannot see itself, so every path is camera -> sphere ->
    // environment (0.8) or camera -> environment (1.0). Edge pixels mix the two.
    for (i, p) in film.pixels().iter().enumerate() {
        assert!(p.x >= 0.8 - 1e-12 && p.x <= 1.0 + 1e-12, "pixel {i} = {p:?}");
    }
    assert!((film.get(24, 24).x - 0.8).abs() < 1e-12, "centre = {:?}", film.get(24, 24));
    assert!((film.get(0, 0).x - 1.0).abs() < 1e-12);
}

#[test]
fn white_diffuse_sphere_is_invisible() {
    let film = furnace(Material::lambertian(Color::ONE));
    for p in film.pixels() {
        assert!((*p - Color::ONE).length() < 1e-12, "{p:?}");
    }
}

#[test]
fn perfect_mirror_is_invisible() {
    let film = furnace(Material::metal(Color::ONE, 0.0));
    for p in film.pixels() {
        assert!((*p - Color::ONE).length() < 1e-12, "{p:?}");
    }
}

#[test]
fn clear_glass_conserves_energy() {
    // Paths inside glass can bounce many times; with a depth limit of 50 the
    // truncated remainder is far below the tolerance.
    let film = furnace(Material::dielectric(1.5));
    for (i, p) in film.pixels().iter().enumerate() {
        assert!((p.x - 1.0).abs() < 1e-3, "pixel {i} = {p:?}");
    }
}

/// Two large facing diffuse plates of albedo 0.9 trap paths for many bounces,
/// well past the point where Russian roulette starts.
///
/// Roulette may only end paths, never brighten a pixel beyond the environment,
/// and two independent estimates of the image mean must agree.
#[test]
fn roulette_is_unbiased_on_long_paths() {
    use penumbra::geometry::Quad;
    use penumbra::math::Vec3;

    let build = || {
        let mut b = Scene::builder();
        let wall = b.material(Material::lambertian(Color::splat(0.9)));
        b.add(Quad::new(Point3::new(-5.0, -1.0, -5.0), Vec3::new(0.0, 0.0, 10.0), Vec3::new(10.0, 0.0, 0.0), wall));
        b.add(Quad::new(Point3::new(-5.0, 1.0, -5.0), Vec3::new(10.0, 0.0, 0.0), Vec3::new(0.0, 0.0, 10.0), wall));
        b.background(Background::Solid(Color::ONE));
        b.camera(CameraSpec {
            look_from: Point3::new(0.0, 0.0, 3.0),
            aspect_ratio: 1.0,
            vfov: 70.0,
            focus_distance: 3.0,
            ..CameraSpec::default()
        });
        b.build(SplitMethod::Sah)
    };
    let mean = |seed| {
        let settings = RenderSettings {
            width: 32,
            height: 32,
            samples_per_pixel: 256,
            max_depth: 200,
            seed,
            ..RenderSettings::default()
        };
        let (film, stats) = render(&build(), &settings);
        assert_eq!(stats.rejected_samples, 0);
        for p in film.pixels() {
            assert!(p.x <= 1.0 + 1e-9, "roulette must never create energy: {p:?}");
        }
        film.mean().x
    };
    let (a, b) = (mean(1), mean(2));
    assert!((a - b).abs() < 0.01, "independent estimates disagree: {a} vs {b}");
    assert!(a > 0.3 && a < 1.0, "corridor should be dimmer than the furnace but lit: {a}");
}
