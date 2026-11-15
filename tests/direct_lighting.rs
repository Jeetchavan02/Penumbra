//! Light sampling and multiple importance sampling change the noise, never
//! the answer.

use penumbra::accel::SplitMethod;
use penumbra::camera::CameraSpec;
use penumbra::geometry::{Quad, Sphere};
use penumbra::material::Material;
use penumbra::math::{Color, Point3, Vec3};
use penumbra::render::{render, Lighting, RenderSettings};
use penumbra::scene::Scene;

/// A floor lit by a sphere light and a quad light, with a diffuse blocker, a
/// mirror and a glass ball so that every emission-counting rule is exercised.
fn scene() -> Scene {
    let mut b = Scene::builder();
    let grey = b.material(Material::lambertian(Color::splat(0.7)));
    let green = b.material(Material::lambertian(Color::new(0.2, 0.7, 0.3)));
    let mirror = b.material(Material::metal(Color::splat(0.9), 0.0));
    let glass = b.material(Material::dielectric(1.5));
    let warm = b.material(Material::emissive(Color::new(12.0, 9.0, 6.0)));
    let cool = b.material(Material::emissive(Color::new(3.0, 4.0, 6.0)));

    b.add(Quad::new(Point3::new(-8.0, 0.0, -8.0), Vec3::new(0.0, 0.0, 16.0), Vec3::new(16.0, 0.0, 0.0), grey));
    // A glossy back wall: the case where the two strategies trade places.
    let brushed = b.material(Material::glossy(Color::new(0.9, 0.7, 0.4), 0.35));
    b.add(Quad::new(Point3::new(-8.0, 0.0, -4.0), Vec3::new(16.0, 0.0, 0.0), Vec3::new(0.0, 8.0, 0.0), brushed));
    b.add(Sphere::new(Point3::new(-1.6, 0.8, 0.0), 0.8, green));
    b.add(Sphere::new(Point3::new(0.2, 0.8, 0.6), 0.8, mirror));
    b.add(Sphere::new(Point3::new(2.0, 0.8, 0.0), 0.8, glass));
    b.add(Sphere::new(Point3::new(-2.0, 4.5, 1.0), 1.0, warm));
    b.add(Quad::new(Point3::new(1.5, 4.0, -1.0), Vec3::new(2.0, 0.0, 0.0), Vec3::new(0.0, 0.5, 2.0), cool));
    b.camera(CameraSpec {
        look_from: Point3::new(0.0, 3.0, 9.0),
        look_at: Point3::new(0.0, 1.2, 0.0),
        aspect_ratio: 1.5,
        vfov: 45.0,
        focus_distance: 9.0,
        ..CameraSpec::default()
    });
    b.build(SplitMethod::Sah)
}

fn mean(lighting: Lighting, samples_per_pixel: u32) -> Color {
    let settings =
        RenderSettings { width: 60, height: 40, samples_per_pixel, lighting, seed: 17, ..RenderSettings::default() };
    let (film, stats) = render(&scene(), &settings);
    assert_eq!(stats.rejected_samples, 0);
    film.mean()
}

fn assert_close(what: &str, reference: Color, other: Color, tolerance: f64) {
    for (name, r, o) in [("r", reference.x, other.x), ("g", reference.y, other.y), ("b", reference.z, other.z)] {
        let relative = (r - o).abs() / r;
        assert!(relative < tolerance, "{what} {name}: reference {r:.5}, got {o:.5}, off by {:.2}%", relative * 100.0);
    }
}

#[test]
fn all_three_lighting_modes_converge_to_the_same_image() {
    // Plain path tracing is the reference; it needs many more samples to settle.
    let reference = mean(Lighting::BsdfOnly, 3000);
    assert!(reference.x > 0.05, "scene should be lit: {reference:?}");
    assert_close("light sampling", reference, mean(Lighting::LightOnly, 600), 0.02);
    assert_close("mis", reference, mean(Lighting::Mis, 600), 0.02);
}
