//! Built-in scenes, addressable by name from the command line.

mod penumbra;
mod showcase;

use super::{Scene, SceneBuilder};
use crate::accel::SplitMethod;
use crate::math::Color;

pub struct SceneEntry {
    pub name: &'static str,
    pub description: &'static str,
    builder: fn() -> SceneBuilder,
}

pub const SCENES: &[SceneEntry] = &[
    SceneEntry { name: "penumbra", description: "soft shadows from one spherical lamp in a dark room", builder: penumbra::build },
    SceneEntry { name: "showcase", description: "twenty spheres under a sky, one of each material", builder: showcase::build },
];

/// Build a library scene by name.
pub fn build(name: &str, split: SplitMethod) -> Option<Scene> {
    SCENES.iter().find(|entry| entry.name == name).map(|entry| (entry.builder)().build(split))
}

/// A fully saturated colour for a position on the colour wheel; `turn` wraps.
fn hue(turn: f64) -> Color {
    let h = turn.rem_euclid(1.0) * 6.0;
    let channel = |offset: f64| {
        let k = (offset + h).rem_euclid(6.0);
        1.0 - 0.85 * k.min(4.0 - k).clamp(0.0, 1.0)
    };
    Color::new(channel(5.0), channel(3.0), channel(1.0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::camera::Camera;

    #[test]
    fn every_scene_builds_with_both_split_methods() {
        for entry in SCENES {
            for split in [SplitMethod::Midpoint, SplitMethod::Sah] {
                let scene = build(entry.name, split).unwrap();
                assert!(!scene.bvh().primitives().is_empty(), "{} is empty", entry.name);
                // Panics if the scene's camera is degenerate.
                let _ = Camera::new(&scene.camera, 64, scene.camera.height_for(64));
            }
        }
    }

    #[test]
    fn names_are_unique_and_unknown_names_fail() {
        for (i, a) in SCENES.iter().enumerate() {
            assert!(SCENES[i + 1..].iter().all(|b| b.name != a.name));
        }
        assert!(build("no-such-scene", SplitMethod::Sah).is_none());
    }

    #[test]
    fn hue_is_a_valid_reflectance() {
        for i in 0..60 {
            let c = hue(i as f64 / 60.0);
            assert!(c.min_component() > 0.0 && c.max_component() <= 1.0);
        }
        assert!(hue(0.0).x > hue(0.0).z);
    }
}
