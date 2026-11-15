use crate::camera::CameraSpec;
use crate::geometry::Quad;
use crate::material::Material;
use crate::math::{Color, Point3, Vec3};
use crate::mesh::Mesh;
use crate::scene::{Background, SceneBuilder};

/// Surface finishes offered for a mesh shown in the studio.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Finish {
    #[default]
    Clay,
    Gold,
    Glass,
}

impl Finish {
    pub const NAMES: &'static [&'static str] = &["clay", "gold", "glass"];

    pub fn from_name(name: &str) -> Option<Finish> {
        match name {
            "clay" => Some(Finish::Clay),
            "gold" => Some(Finish::Gold),
            "glass" => Some(Finish::Glass),
            _ => None,
        }
    }

    fn material(self) -> Material {
        match self {
            Finish::Clay => Material::lambertian(Color::new(0.74, 0.62, 0.52)),
            Finish::Gold => Material::glossy(Color::new(1.0, 0.77, 0.34), 0.3),
            Finish::Glass => Material::dielectric(1.5),
        }
    }
}

/// Height the displayed mesh is scaled to.
const SUBJECT_HEIGHT: f64 = 2.4;

/// A neutral photo studio around any mesh: floor, back wall, a large warm key
/// light overhead and a cool rim light off frame to the right. The mesh is scaled to a fixed
/// height and stood on the floor, so models of any size and origin frame the
/// same way.
pub fn studio(mesh: &Mesh, finish: Finish, turn_degrees: f64) -> SceneBuilder {
    let mut b = SceneBuilder::default();

    let floor = b.material(Material::lambertian(Color::splat(0.55)));
    let wall = b.material(Material::lambertian(Color::new(0.42, 0.44, 0.50)));
    b.add(Quad::new(Point3::new(-30.0, 0.0, -30.0), Vec3::new(0.0, 0.0, 60.0), Vec3::new(60.0, 0.0, 0.0), floor));
    b.add(Quad::new(Point3::new(-30.0, 0.0, -5.0), Vec3::new(60.0, 0.0, 0.0), Vec3::new(0.0, 30.0, 0.0), wall));

    let key = b.material(Material::emissive(Color::new(17.0, 15.0, 12.5)));
    let rim = b.material(Material::emissive(Color::new(8.0, 12.0, 20.0)));
    b.add(Quad::new(Point3::new(-4.5, 6.0, 0.5), Vec3::new(3.5, 0.0, 0.0), Vec3::new(0.0, 0.0, 3.5), key));
    b.add(Quad::new(Point3::new(7.0, 0.6, -2.0), Vec3::new(1.2, 0.0, 1.6), Vec3::new(0.0, 2.6, 0.0), rim));

    let mut subject = mesh.clone();
    subject.transform(1.0, turn_degrees, Vec3::ZERO).stand_on_floor(SUBJECT_HEIGHT);
    let surface = b.material(finish.material());
    b.add_mesh(&subject, surface);

    b.background(Background::Solid(Color::splat(0.02)));
    let look_from = Point3::new(0.0, 2.3, 8.5);
    let look_at = Point3::new(0.0, SUBJECT_HEIGHT * 0.48, 0.0);
    b.camera(CameraSpec {
        look_from,
        look_at,
        vfov: 30.0,
        aspect_ratio: 16.0 / 9.0,
        focus_distance: (look_from - look_at).length(),
        ..CameraSpec::default()
    });
    b
}

/// The library's mesh scene: a trefoil knot of about a hundred thousand
/// triangles, in gold.
pub fn knot() -> SceneBuilder {
    studio(&crate::mesh::torus_knot(2, 3, 1024, 48, 0.42), Finish::Gold, 0.0)
}
