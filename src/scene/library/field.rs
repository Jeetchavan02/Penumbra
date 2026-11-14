use super::hue;
use crate::camera::CameraSpec;
use crate::geometry::Sphere;
use crate::material::Material;
use crate::math::{Color, Point3, Sampler};
use crate::scene::{Background, SceneBuilder};

/// Several hundred small spheres scattered over a plain around three large
/// ones. Mostly here to give the BVH real work: sizes span three orders of
/// magnitude and the distribution is flat in one axis.
pub fn build() -> SceneBuilder {
    let mut b = SceneBuilder::default();
    // Fixed seed: the layout is part of the scene, not of the render.
    let mut rng = Sampler::new(0x5EED);

    let ground = b.material(Material::lambertian(Color::new(0.48, 0.5, 0.46)));
    b.add(Sphere::new(Point3::new(0.0, -1000.0, 0.0), 1000.0, ground));

    let big = [
        (Point3::new(0.0, 1.0, 0.0), Material::dielectric(1.5)),
        (Point3::new(-4.0, 1.0, 0.0), Material::lambertian(Color::new(0.4, 0.2, 0.1))),
        (Point3::new(4.0, 1.0, 0.0), Material::metal(Color::new(0.7, 0.6, 0.5), 0.0)),
    ];
    for (center, material) in big {
        let id = b.material(material);
        b.add(Sphere::new(center, 1.0, id));
    }

    for gx in -11..11 {
        for gz in -11..11 {
            let radius = rng.range(0.14, 0.24);
            let center = Point3::new(gx as f64 + 0.8 * rng.uniform(), radius, gz as f64 + 0.8 * rng.uniform());
            // Keep clear of the large spheres so nothing interpenetrates.
            if big.iter().any(|(c, _)| (center - *c).length() < 1.0 + radius + 0.05) {
                continue;
            }
            let choice = rng.uniform();
            let material = if choice < 0.7 {
                Material::lambertian(hue(rng.uniform()) * hue(rng.uniform()))
            } else if choice < 0.9 {
                Material::metal(Color::lerp(Color::ONE, hue(rng.uniform()), 0.4), rng.range(0.0, 0.4))
            } else {
                Material::dielectric(1.5)
            };
            let id = b.material(material);
            b.add(Sphere::new(center, radius, id));
        }
    }

    b.background(Background::Sky { horizon: Color::ONE, zenith: Color::new(0.5, 0.7, 1.0) });
    b.camera(CameraSpec {
        look_from: Point3::new(13.0, 2.0, 3.0),
        look_at: Point3::ZERO,
        vfov: 20.0,
        aspect_ratio: 16.0 / 9.0,
        defocus_angle: 0.6,
        focus_distance: 10.0,
        ..CameraSpec::default()
    });
    b
}
