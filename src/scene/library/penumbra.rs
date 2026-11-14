use crate::camera::CameraSpec;
use crate::geometry::{Quad, Sphere};
use crate::material::Material;
use crate::math::{Color, Point3, Vec3};
use crate::scene::{Background, SceneBuilder};

/// The scene the renderer is named for: one spherical lamp in a dark room, and
/// three spheres hovering at increasing heights above the floor.
///
/// Each shadow has a dark core (umbra) where the lamp is fully hidden and a
/// soft edge (penumbra) where it is partly visible. The higher the sphere, the
/// wider the penumbra, until the umbra disappears altogether.
pub fn build() -> SceneBuilder {
    let mut b = SceneBuilder::default();

    let floor = b.material(Material::lambertian(Color::new(0.75, 0.73, 0.70)));
    let wall = b.material(Material::lambertian(Color::new(0.55, 0.57, 0.65)));
    b.add(Quad::new(Point3::new(-40.0, 0.0, -40.0), Vec3::new(0.0, 0.0, 80.0), Vec3::new(80.0, 0.0, 0.0), floor));
    b.add(Quad::new(Point3::new(-40.0, 0.0, -9.0), Vec3::new(80.0, 0.0, 0.0), Vec3::new(0.0, 40.0, 0.0), wall));

    let lamp = b.material(Material::emissive(Color::new(95.0, 82.0, 64.0)));
    b.add(Sphere::new(Point3::new(-5.0, 7.0, 4.5), 0.9, lamp));

    let coral = b.material(Material::lambertian(Color::new(0.80, 0.30, 0.22)));
    let ivory = b.material(Material::lambertian(Color::new(0.82, 0.78, 0.68)));
    let cobalt = b.material(Material::lambertian(Color::new(0.22, 0.38, 0.80)));
    b.add(Sphere::new(Point3::new(-2.8, 0.7, 0.0), 0.7, coral));
    b.add(Sphere::new(Point3::new(-0.2, 1.9, 0.0), 0.7, ivory));
    b.add(Sphere::new(Point3::new(2.4, 3.3, 0.0), 0.7, cobalt));

    let glass = b.material(Material::dielectric(1.5));
    let steel = b.material(Material::metal(Color::splat(0.85), 0.05));
    b.add(Sphere::new(Point3::new(-1.3, 0.45, 2.6), 0.45, glass));
    b.add(Sphere::new(Point3::new(1.5, 0.55, 2.9), 0.55, steel));

    b.background(Background::Solid(Color::splat(0.01)));
    let look_from = Point3::new(0.6, 3.4, 13.0);
    let look_at = Point3::new(0.6, 1.7, 0.0);
    b.camera(CameraSpec {
        look_from,
        look_at,
        vfov: 34.0,
        aspect_ratio: 16.0 / 9.0,
        focus_distance: (look_from - look_at).length(),
        ..CameraSpec::default()
    });
    b
}
