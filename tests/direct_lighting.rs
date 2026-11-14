//! Next event estimation changes the noise, never the answer.

use penumbra::accel::SplitMethod;
use penumbra::camera::CameraSpec;
use penumbra::geometry::{Quad, Sphere};
use penumbra::material::Material;
use penumbra::math::{Color, Point3, Vec3};
use penumbra::render::{render, RenderSettings};
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
    b.add(Quad::new(Point3::new(-8.0, 0.0, -4.0), Vec3::new(16.0, 0.0, 0.0), Vec3::new(0.0, 8.0, 0.0), grey));
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

fn mean(direct_lighting: bool, samples_per_pixel: u32) -> Color {
    let settings = RenderSettings {
        width: 60,
        height: 40,
        samples_per_pixel,
        direct_lighting,
        seed: 17,
        ..RenderSettings::default()
    };
    let (film, stats) = render(&scene(), &settings);
    assert_eq!(stats.rejected_samples, 0);
    film.mean()
}

#[test]
fn direct_lighting_matches_plain_path_tracing() {
    // Plain path tracing is the reference; it needs many more samples to settle.
    let reference = mean(false, 3000);
    let sampled = mean(true, 600);
    for (name, r, s) in [("r", reference.x, sampled.x), ("g", reference.y, sampled.y), ("b", reference.z, sampled.z)] {
        let relative = (r - s).abs() / r;
        assert!(relative < 0.02, "{name}: reference {r:.5}, direct {s:.5}, off by {:.2}%", relative * 100.0);
    }
    assert!(reference.x > 0.05, "scene should be lit: {reference:?}");
}
