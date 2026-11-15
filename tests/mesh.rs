//! Triangle meshes rendered end to end.

use penumbra::accel::SplitMethod;
use penumbra::camera::CameraSpec;
use penumbra::geometry::Sphere;
use penumbra::material::Material;
use penumbra::math::{Color, Point3};
use penumbra::mesh::{parse_obj, Mesh};
use penumbra::render::{render, Film, RenderSettings};
use penumbra::scene::{Background, Scene};

/// A sphere approximated by subdividing an octahedron `levels` times.
fn geodesic_sphere(levels: u32) -> Mesh {
    let mut positions = vec![
        Point3::new(1.0, 0.0, 0.0),
        Point3::new(-1.0, 0.0, 0.0),
        Point3::new(0.0, 1.0, 0.0),
        Point3::new(0.0, -1.0, 0.0),
        Point3::new(0.0, 0.0, 1.0),
        Point3::new(0.0, 0.0, -1.0),
    ];
    let mut triangles: Vec<[u32; 3]> =
        vec![[0, 2, 4], [2, 1, 4], [1, 3, 4], [3, 0, 4], [2, 0, 5], [1, 2, 5], [3, 1, 5], [0, 3, 5]];
    for _ in 0..levels {
        let mut next = Vec::with_capacity(triangles.len() * 4);
        for [a, b, c] in triangles {
            // Shared edges get duplicate midpoints; harmless for this test.
            let mut midpoint = |i: u32, j: u32| {
                positions.push(((positions[i as usize] + positions[j as usize]) * 0.5).normalized());
                positions.len() as u32 - 1
            };
            let (ab, bc, ca) = (midpoint(a, b), midpoint(b, c), midpoint(c, a));
            next.extend([[a, ab, ca], [ab, b, bc], [ca, bc, c], [ab, bc, ca]]);
        }
        triangles = next;
    }
    Mesh::from_triangles(positions, &triangles)
}

fn camera() -> CameraSpec {
    CameraSpec {
        look_from: Point3::new(0.0, 0.0, 4.0),
        aspect_ratio: 1.0,
        focus_distance: 4.0,
        ..CameraSpec::default()
    }
}

fn render_scene(scene: &Scene, samples_per_pixel: u32) -> Film {
    let settings = RenderSettings { width: 48, height: 48, samples_per_pixel, ..RenderSettings::default() };
    let (film, stats) = render(scene, &settings);
    assert_eq!(stats.rejected_samples, 0);
    film
}

fn mesh_scene(mesh: &Mesh, material: Material, background: Color, split: SplitMethod) -> Scene {
    let mut b = Scene::builder();
    let id = b.material(material);
    b.add_mesh(mesh, id);
    b.background(Background::Solid(background));
    b.camera(camera());
    b.build(split)
}

#[test]
fn white_mesh_is_invisible_in_the_furnace() {
    let mesh = geodesic_sphere(3);
    assert_eq!(mesh.triangle_count(), 8 * 64);
    let film = render_scene(&mesh_scene(&mesh, Material::lambertian(Color::ONE), Color::ONE, SplitMethod::Sah), 64);
    // Interpolated normals let a few scattered rays start below the true
    // surface and bounce inside until the depth limit, so this is not exact
    // the way the analytic sphere is. It must still be very close.
    for (i, p) in film.pixels().iter().enumerate() {
        assert!(p.x <= 1.0 + 1e-9 && p.x > 0.97, "pixel {i} = {p:?}");
    }
    assert!((film.mean().x - 1.0).abs() < 2e-3, "mean = {:?}", film.mean());
}

#[test]
fn mesh_sphere_matches_the_analytic_sphere() {
    // Same shape two ways, under a sky-like gradient so shading varies.
    let background = Background::Sky { horizon: Color::splat(0.2), zenith: Color::new(0.6, 0.8, 1.0) };
    let albedo = Color::new(0.7, 0.5, 0.3);

    let mut analytic = Scene::builder();
    let id = analytic.material(Material::lambertian(albedo));
    analytic.add(Sphere::new(Point3::ZERO, 1.0, id));
    analytic.background(background);
    analytic.camera(camera());

    let mut meshed = Scene::builder();
    let id = meshed.material(Material::lambertian(albedo));
    meshed.add_mesh(&geodesic_sphere(4), id);
    meshed.background(background);
    meshed.camera(camera());

    let a = render_scene(&analytic.build(SplitMethod::Sah), 256).mean();
    let m = render_scene(&meshed.build(SplitMethod::Sah), 256).mean();
    for (name, a, m) in [("r", a.x, m.x), ("g", a.y, m.y), ("b", a.z, m.z)] {
        assert!((a - m).abs() / a < 0.01, "{name}: analytic {a:.5}, mesh {m:.5}");
    }
}

#[test]
fn split_method_does_not_change_a_mesh_render() {
    let mesh = geodesic_sphere(3);
    let material = Material::metal(Color::splat(0.8), 0.3);
    let sky = Color::new(0.5, 0.7, 1.0);
    let sah = render_scene(&mesh_scene(&mesh, material, sky, SplitMethod::Sah), 8);
    let midpoint = render_scene(&mesh_scene(&mesh, material, sky, SplitMethod::Midpoint), 8);
    assert_eq!(sah.pixels(), midpoint.pixels());
}

#[test]
fn glass_mesh_conserves_energy() {
    // Winding must be consistent for glass: a flipped face would refract
    // with the inverse index. A closed glass mesh in the furnace stays white.
    let film =
        render_scene(&mesh_scene(&geodesic_sphere(3), Material::dielectric(1.5), Color::ONE, SplitMethod::Sah), 64);
    assert!((film.mean().x - 1.0).abs() < 5e-3, "mean = {:?}", film.mean());
}

#[test]
fn an_emissive_mesh_lights_the_scene_the_same_with_and_without_light_sampling() {
    // A two-triangle lamp over a floor quad written as OBJ.
    let lamp = parse_obj("v -1 3 -1\nv 1 3 -1\nv 1 3 1\nv -1 3 1\nf 1 2 3 4\n").unwrap();
    let floor = parse_obj("v -6 0 -6\nv -6 0 6\nv 6 0 6\nv 6 0 -6\nf 1 2 3 4\n").unwrap();
    let mean = |direct_lighting: bool, samples_per_pixel: u32| {
        let mut b = Scene::builder();
        let light = b.material(Material::emissive(Color::splat(8.0)));
        let grey = b.material(Material::lambertian(Color::splat(0.6)));
        b.add_mesh(&lamp, light);
        b.add_mesh(&floor, grey);
        b.camera(CameraSpec {
            look_from: Point3::new(0.0, 2.0, 9.0),
            look_at: Point3::new(0.0, 0.5, 0.0),
            aspect_ratio: 1.0,
            focus_distance: 9.0,
            ..CameraSpec::default()
        });
        let scene = b.build(SplitMethod::Sah);
        assert_eq!(scene.lights().len(), 2);
        let settings = RenderSettings {
            width: 40,
            height: 40,
            samples_per_pixel,
            direct_lighting,
            seed: 4,
            ..RenderSettings::default()
        };
        render(&scene, &settings).0.mean().x
    };
    let (reference, sampled) = (mean(false, 2000), mean(true, 300));
    assert!(reference > 0.02, "scene should be lit: {reference}");
    assert!((reference - sampled).abs() / reference < 0.02, "reference {reference:.5}, direct {sampled:.5}");
}
