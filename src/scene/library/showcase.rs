use std::f64::consts::TAU;

use super::hue;
use crate::camera::CameraSpec;
use crate::geometry::Sphere;
use crate::material::Material;
use crate::math::{Color, Point3};
use crate::scene::{Background, SceneBuilder};

/// Twenty spheres under an open sky: three large ones, one per material, and
/// two rings of small ones. Shallow depth of field.
pub fn build() -> SceneBuilder {
    let mut b = SceneBuilder::default();

    let ground = b.material(Material::lambertian(Color::splat(0.5)));
    b.add(Sphere::new(Point3::new(0.0, -1000.0, 0.0), 1000.0, ground));

    let glass = b.material(Material::dielectric(1.5));
    let clay = b.material(Material::lambertian(Color::new(0.45, 0.22, 0.12)));
    let chrome = b.material(Material::metal(Color::new(0.75, 0.68, 0.58), 0.0));
    b.add(Sphere::new(Point3::new(0.0, 1.0, 0.0), 1.0, glass));
    b.add(Sphere::new(Point3::new(-4.0, 1.0, 0.0), 1.0, clay));
    b.add(Sphere::new(Point3::new(4.0, 1.0, 0.0), 1.0, chrome));

    for (ring, (ring_radius, sphere_radius)) in [(2.3, 0.32), (6.2, 0.42)].into_iter().enumerate() {
        for i in 0..8 {
            let turn = (i as f64 + 0.5 * ring as f64) / 8.0;
            let color = hue(turn + 0.1 * ring as f64);
            let material = match (i + ring) % 3 {
                0 => Material::lambertian(color),
                1 => Material::metal(Color::lerp(color, Color::ONE, 0.5), 0.15 * ring as f64),
                _ => Material::dielectric(1.5),
            };
            let id = b.material(material);
            let (sin, cos) = (TAU * turn).sin_cos();
            b.add(Sphere::new(Point3::new(ring_radius * cos, sphere_radius, ring_radius * sin), sphere_radius, id));
        }
    }

    b.background(Background::Sky { horizon: Color::ONE, zenith: Color::new(0.45, 0.65, 1.0) });
    let look_from = Point3::new(11.0, 2.4, 6.0);
    let look_at = Point3::new(0.0, 0.7, 0.0);
    b.camera(CameraSpec {
        look_from,
        look_at,
        vfov: 24.0,
        aspect_ratio: 16.0 / 9.0,
        defocus_angle: 0.5,
        focus_distance: (look_from - look_at).length(),
        ..CameraSpec::default()
    });
    b
}
