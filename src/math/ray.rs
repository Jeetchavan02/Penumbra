use super::{Point3, Vec3};

/// A half-line `origin + t * dir`.
///
/// Invariant: `dir` is unit length, so `t` is a distance in world units. Every
/// epsilon in the renderer relies on that, which is why the fields are private.
#[derive(Debug, Clone, Copy)]
pub struct Ray {
    origin: Point3,
    dir: Vec3,
    inv_dir: Vec3,
}

impl Ray {
    /// `dir` must already be normalised.
    pub fn new(origin: Point3, dir: Vec3) -> Self {
        debug_assert!((dir.length_squared() - 1.0).abs() < 1e-6, "ray direction must be unit length, got {dir:?}");
        Ray {
            origin,
            dir,
            // Zero components become infinities, which the slab test handles.
            inv_dir: Vec3::new(1.0 / dir.x, 1.0 / dir.y, 1.0 / dir.z),
        }
    }

    pub fn origin(&self) -> Point3 {
        self.origin
    }

    pub fn dir(&self) -> Vec3 {
        self.dir
    }

    /// Component-wise reciprocal of the direction, cached for box tests.
    pub fn inv_dir(&self) -> Vec3 {
        self.inv_dir
    }

    pub fn at(&self, t: f64) -> Point3 {
        self.origin + self.dir * t
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn at_walks_along_the_direction() {
        let r = Ray::new(Vec3::new(1.0, 2.0, 3.0), Vec3::new(0.0, 0.0, -1.0));
        assert_eq!(r.at(2.5), Vec3::new(1.0, 2.0, 0.5));
        assert_eq!(r.inv_dir().z, -1.0);
        assert!(r.inv_dir().x.is_infinite());
    }

    #[test]
    #[should_panic(expected = "unit length")]
    #[cfg(debug_assertions)]
    fn rejects_unnormalised_direction() {
        let _ = Ray::new(Vec3::ZERO, Vec3::new(0.0, 2.0, 0.0));
    }
}
