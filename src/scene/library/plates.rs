use crate::camera::CameraSpec;
use crate::geometry::{Quad, Sphere};
use crate::material::Material;
use crate::math::{Color, Point3, Vec3};
use crate::scene::{Background, SceneBuilder};

/// The classic test for multiple importance sampling, after Veach's thesis:
/// four glossy plates, smooth at the back and rough at the front, each
/// reflecting four lights of equal power that range from tiny to large.
///
/// Sampling the lights handles a small light on a rough plate and fails on a
/// large light in a smooth plate. Sampling the reflection lobe is the reverse.
/// Every combination appears in one image, so each strategy alone leaves a
/// noisy corner and only the combination is clean everywhere.
pub fn build() -> SceneBuilder {
    let mut b = SceneBuilder::default();

    // Lights: equal power, so radiance falls with the square of the radius.
    let lights = [
        (-3.75, 0.03333, Color::new(1.0, 0.75, 0.55)),
        (-1.25, 0.1, Color::new(0.95, 1.0, 0.7)),
        (1.25, 0.3, Color::new(0.65, 1.0, 0.9)),
        (3.75, 0.9, Color::new(0.7, 0.8, 1.0)),
    ];
    for (x, radius, tint) in lights {
        let lamp = b.material(Material::emissive(tint * (1.0 / (radius * radius))));
        b.add(Sphere::new(Point3::new(x, 0.0, 0.0), radius, lamp));
    }

    // Plates: each is tilted so the camera sees the row of lights mirrored in it.
    let plates = [
        ((-2.70651, 0.25609), (-2.08375, -0.526323), 0.07),
        ((-3.28825, 1.36972), (-2.83856, 0.476536), 0.14),
        ((-3.73096, 2.70046), (-3.43378, 1.74564), 0.22),
        ((-3.99615, 4.0667), (-3.82069, 3.08221), 0.32),
    ];
    for ((near_y, near_z), (far_y, far_z), roughness) in plates {
        let plate = b.material(Material::glossy(Color::splat(0.9), roughness));
        let corner = Point3::new(-4.0, near_y, near_z);
        b.add(Quad::new(corner, Vec3::new(0.0, far_y - near_y, far_z - near_z), Vec3::new(8.0, 0.0, 0.0), plate));
    }

    // A floor and back wall, with one distant lamp so they are not black.
    let surround = b.material(Material::lambertian(Color::splat(0.4)));
    b.add(Quad::new(Point3::new(-12.0, -4.15, -12.0), Vec3::new(0.0, 0.0, 30.0), Vec3::new(24.0, 0.0, 0.0), surround));
    b.add(Quad::new(Point3::new(-12.0, -4.15, -2.0), Vec3::new(24.0, 0.0, 0.0), Vec3::new(0.0, 14.0, 0.0), surround));
    let fill = b.material(Material::emissive(Color::splat(60.0)));
    b.add(Sphere::new(Point3::new(10.0, 10.0, 4.0), 0.5, fill));

    b.background(Background::Solid(Color::ZERO));
    let look_from = Point3::new(0.0, 2.0, 15.0);
    let look_at = Point3::new(0.0, -2.0, 2.5);
    b.camera(CameraSpec {
        look_from,
        look_at,
        vfov: 25.0,
        aspect_ratio: 1.5,
        focus_distance: (look_from - look_at).length(),
        ..CameraSpec::default()
    });
    b
}
