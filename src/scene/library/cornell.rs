use crate::camera::CameraSpec;
use crate::geometry::{Quad, Sphere};
use crate::material::Material;
use crate::math::{Color, Point3, Vec3};
use crate::scene::{Background, SceneBuilder};

/// A Cornell box: a closed room with one red wall, one green wall and a
/// rectangular ceiling light, holding a mirror ball and a glass ball.
///
/// Lit entirely by one small emitter, so it shows colour bleeding between
/// walls and a caustic under the glass.
pub fn build() -> SceneBuilder {
    const SIZE: f64 = 555.0;
    let mut b = SceneBuilder::default();

    let red = b.material(Material::lambertian(Color::new(0.65, 0.05, 0.05)));
    let green = b.material(Material::lambertian(Color::new(0.12, 0.45, 0.15)));
    let white = b.material(Material::lambertian(Color::splat(0.73)));
    let light = b.material(Material::emissive(Color::new(17.0, 14.0, 10.0)));

    let (x, y, z) = (Vec3::new(SIZE, 0.0, 0.0), Vec3::new(0.0, SIZE, 0.0), Vec3::new(0.0, 0.0, SIZE));
    // The camera looks down +z, which puts +x on the left of the image.
    b.add(Quad::new(Point3::new(SIZE, 0.0, 0.0), y, z, red));
    b.add(Quad::new(Point3::ZERO, y, z, green));
    b.add(Quad::new(Point3::ZERO, x, z, white));
    b.add(Quad::new(Point3::new(0.0, SIZE, 0.0), x, z, white));
    b.add(Quad::new(Point3::new(0.0, 0.0, SIZE), x, y, white));
    b.add(Quad::new(Point3::new(213.0, SIZE - 1.0, 227.0), Vec3::new(130.0, 0.0, 0.0), Vec3::new(0.0, 0.0, 105.0), light));

    let mirror = b.material(Material::metal(Color::splat(0.9), 0.0));
    let glass = b.material(Material::dielectric(1.5));
    b.add(Sphere::new(Point3::new(380.0, 110.0, 370.0), 110.0, mirror));
    b.add(Sphere::new(Point3::new(170.0, 90.0, 170.0), 90.0, glass));

    b.background(Background::Solid(Color::ZERO));
    b.camera(CameraSpec {
        look_from: Point3::new(278.0, 278.0, -800.0),
        look_at: Point3::new(278.0, 278.0, 0.0),
        vfov: 40.0,
        aspect_ratio: 1.0,
        focus_distance: 800.0,
        ..CameraSpec::default()
    });
    b
}
