//! The immutable world a render reads from.

pub mod library;

use crate::accel::{Bvh, SplitMethod};
use crate::camera::CameraSpec;
use crate::geometry::{HitRecord, Hittable, MaterialId, Primitive};
use crate::material::Material;
use crate::math::{Color, Interval, Ray, Vec3};
use crate::mesh::Mesh;

/// Radiance arriving from rays that leave the scene.
#[derive(Debug, Clone, Copy)]
pub enum Background {
    Solid(Color),
    /// Vertical gradient from `horizon` (looking down) to `zenith` (looking up).
    Sky {
        horizon: Color,
        zenith: Color,
    },
}

impl Background {
    /// `dir` is a unit vector.
    pub fn radiance(&self, dir: Vec3) -> Color {
        match *self {
            Background::Solid(color) => color,
            Background::Sky { horizon, zenith } => Color::lerp(horizon, zenith, 0.5 * (dir.y + 1.0)),
        }
    }
}

/// Geometry, materials, environment and camera.
///
/// Built once by [`SceneBuilder`] and never mutated afterwards. It contains only
/// plain owned data, so render threads share it through a plain `&Scene`.
pub struct Scene {
    bvh: Bvh,
    materials: Vec<Material>,
    /// Copies of the emissive primitives, for direct light sampling.
    lights: Vec<Primitive>,
    pub background: Background,
    pub camera: CameraSpec,
}

// Rayon needs `&Scene: Send`, i.e. `Scene: Sync`. If a future field breaks that
// (an `Rc`, a `RefCell`, a `Cell` counter), fail here with a clear cause rather
// than deep inside a `par_iter` type error.
const _: fn() = || {
    fn assert_shareable<T: Send + Sync>() {}
    assert_shareable::<Scene>();
};

impl Scene {
    pub fn builder() -> SceneBuilder {
        SceneBuilder::default()
    }

    /// Nearest intersection along `ray` within `t`.
    pub fn hit(&self, ray: &Ray, t: Interval) -> Option<HitRecord> {
        self.bvh.hit(ray, t)
    }

    pub fn material(&self, id: MaterialId) -> &Material {
        &self.materials[id.index()]
    }

    /// The primitive a hit refers to through [`HitRecord::primitive`].
    pub fn primitive(&self, index: u32) -> &Primitive {
        &self.bvh.primitives()[index as usize]
    }

    /// Every primitive whose material emits light.
    pub fn lights(&self) -> &[Primitive] {
        &self.lights
    }

    pub fn bvh(&self) -> &Bvh {
        &self.bvh
    }
}

/// Collects primitives and materials, then freezes them into a [`Scene`].
pub struct SceneBuilder {
    primitives: Vec<Primitive>,
    materials: Vec<Material>,
    background: Background,
    camera: CameraSpec,
}

impl Default for SceneBuilder {
    fn default() -> Self {
        SceneBuilder {
            primitives: Vec::new(),
            materials: Vec::new(),
            background: Background::Solid(Color::ZERO),
            camera: CameraSpec::default(),
        }
    }
}

impl SceneBuilder {
    /// Register a material and get the handle primitives refer to it by.
    pub fn material(&mut self, material: Material) -> MaterialId {
        let id = MaterialId(u32::try_from(self.materials.len()).expect("too many materials"));
        self.materials.push(material);
        id
    }

    pub fn add(&mut self, primitive: impl Into<Primitive>) -> &mut Self {
        self.primitives.push(primitive.into());
        self
    }

    /// Add every triangle of `mesh` with one material.
    pub fn add_mesh(&mut self, mesh: &Mesh, material: MaterialId) -> &mut Self {
        self.primitives.extend(mesh.triangles(material).map(Primitive::from));
        self
    }

    pub fn background(&mut self, background: Background) -> &mut Self {
        self.background = background;
        self
    }

    pub fn camera(&mut self, camera: CameraSpec) -> &mut Self {
        self.camera = camera;
        self
    }

    /// Panics if a primitive refers to a material from a different builder.
    pub fn build(self, split: SplitMethod) -> Scene {
        for primitive in &self.primitives {
            assert!(
                primitive.material().index() < self.materials.len(),
                "primitive refers to a material that was not registered with this builder"
            );
        }
        let lights = self
            .primitives
            .iter()
            .filter(|primitive| self.materials[primitive.material().index()].is_emissive())
            .copied()
            .collect();
        Scene {
            bvh: Bvh::build(self.primitives, split),
            materials: self.materials,
            lights,
            background: self.background,
            camera: self.camera,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Sphere;

    #[test]
    fn sky_interpolates_by_height() {
        let sky = Background::Sky { horizon: Color::ONE, zenith: Color::ZERO };
        assert_eq!(sky.radiance(Vec3::new(0.0, 1.0, 0.0)), Color::ZERO);
        assert_eq!(sky.radiance(Vec3::new(0.0, -1.0, 0.0)), Color::ONE);
        assert_eq!(sky.radiance(Vec3::new(1.0, 0.0, 0.0)), Color::splat(0.5));
    }

    #[test]
    fn builder_hands_out_distinct_material_ids() {
        let mut b = Scene::builder();
        let red = b.material(Material::lambertian(Color::new(1.0, 0.0, 0.0)));
        let light = b.material(Material::emissive(Color::ONE));
        assert_ne!(red, light);
        b.add(Sphere::new(Vec3::ZERO, 1.0, light));
        let scene = b.build(SplitMethod::Sah);
        assert!(scene.material(light).is_emissive());
        assert_eq!(scene.lights().len(), 1);
        assert!(!scene.material(red).is_emissive());
        let ray = Ray::new(Vec3::new(0.0, 0.0, 5.0), Vec3::new(0.0, 0.0, -1.0));
        assert_eq!(scene.hit(&ray, Interval::new(1e-4, f64::INFINITY)).unwrap().material, light);
    }

    #[test]
    #[should_panic(expected = "not registered with this builder")]
    fn foreign_material_ids_are_rejected() {
        let mut other = Scene::builder();
        other.material(Material::lambertian(Color::ONE));
        let foreign = other.material(Material::lambertian(Color::ONE));
        let mut b = Scene::builder();
        b.add(Sphere::new(Vec3::ZERO, 1.0, foreign));
        let _ = b.build(SplitMethod::Sah);
    }
}
