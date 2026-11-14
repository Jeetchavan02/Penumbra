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
