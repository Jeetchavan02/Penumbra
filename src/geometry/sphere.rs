use super::{Aabb, HitRecord, Hittable, MaterialId};
use crate::math::{Interval, Point3, Ray, Vec3};

#[derive(Debug, Clone, Copy)]
pub struct Sphere {
    pub center: Point3,
    pub radius: f64,
    pub material: MaterialId,
}

impl Sphere {
    /// Panics unless `radius` is positive and finite. Hollow shells are built
    /// from an inner sphere with an inverted refractive index, not a negative
    /// radius, so bounding boxes are always well formed.
    pub fn new(center: Point3, radius: f64, material: MaterialId) -> Self {
        assert!(radius > 0.0 && radius.is_finite(), "sphere radius must be positive, got {radius}");
        Sphere { center, radius, material }
    }
}

impl Hittable for Sphere {
    fn hit(&self, ray: &Ray, t: Interval) -> Option<HitRecord> {
        // Solve |origin + t*dir - center|^2 = r^2 with the half-b form.
        let oc = self.center - ray.origin();
        let a = ray.dir().length_squared();
        let h = ray.dir().dot(oc);
        let c = oc.length_squared() - self.radius * self.radius;
        let discriminant = h * h - a * c;
        if discriminant < 0.0 {
            return None;
        }
        let sqrt_d = discriminant.sqrt();

        let mut root = (h - sqrt_d) / a;
        if !t.surrounds(root) {
            root = (h + sqrt_d) / a;
            if !t.surrounds(root) {
                return None;
            }
        }

        let outward_normal = (ray.at(root) - self.center) / self.radius;
        // Re-project onto the sphere: `ray.at(root)` drifts off the surface as
        // `root` grows, and that drift is what causes acne on large spheres.
        let point = self.center + outward_normal * self.radius;
        Some(HitRecord::new(ray, root, point, outward_normal, self.material))
    }

    fn bounding_box(&self) -> Aabb {
        let r = Vec3::splat(self.radius);
        Aabb { min: self.center - r, max: self.center + r }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FOREVER: Interval = Interval::new(1e-4, f64::INFINITY);

    fn unit() -> Sphere {
        Sphere::new(Vec3::ZERO, 1.0, MaterialId(0))
    }

    #[test]
    fn head_on_hit() {
        let ray = Ray::new(Vec3::new(0.0, 0.0, 5.0), Vec3::new(0.0, 0.0, -1.0));
        let hit = unit().hit(&ray, FOREVER).unwrap();
        assert!((hit.t - 4.0).abs() < 1e-12);
        assert!(hit.front_face);
        assert!((hit.normal - Vec3::new(0.0, 0.0, 1.0)).length() < 1e-12);
    }

    #[test]
    fn from_inside_hits_the_back_face() {
        let ray = Ray::new(Vec3::ZERO, Vec3::new(1.0, 0.0, 0.0));
        let hit = unit().hit(&ray, FOREVER).unwrap();
        assert!((hit.t - 1.0).abs() < 1e-12);
        assert!(!hit.front_face);
        assert!((hit.normal - Vec3::new(-1.0, 0.0, 0.0)).length() < 1e-12);
    }

    #[test]
    fn misses() {
        let beside = Ray::new(Vec3::new(2.0, 0.0, 5.0), Vec3::new(0.0, 0.0, -1.0));
        let behind = Ray::new(Vec3::new(0.0, 0.0, 5.0), Vec3::new(0.0, 0.0, 1.0));
        assert!(unit().hit(&beside, FOREVER).is_none());
        assert!(unit().hit(&behind, FOREVER).is_none());
    }

    #[test]
    fn a_ray_leaving_the_surface_does_not_hit_it_again() {
        let s = Sphere::new(Vec3::new(0.0, -1000.0, 0.0), 1000.0, MaterialId(0));
        let first = s
            .hit(&Ray::new(Vec3::new(300.0, 500.0, 300.0), Vec3::new(-0.3, -1.0, -0.2).normalized()), FOREVER)
            .unwrap();
        let bounce = Ray::new(first.point, first.normal);
        assert!(s.hit(&bounce, FOREVER).is_none());
        // Grazing departure is the hard case for self-intersection.
        let (tangent, _) = first.normal.orthonormal_basis();
        let graze = Ray::new(first.point, (tangent + first.normal * 1e-3).normalized());
        assert!(s.hit(&graze, FOREVER).is_none());
    }

    #[test]
    fn hit_point_lies_on_the_surface() {
        let s = Sphere::new(Vec3::new(10.0, -1000.0, 3.0), 1000.0, MaterialId(0));
        let ray = Ray::new(Vec3::new(400.0, 50.0, -200.0), Vec3::new(-1.0, -0.2, 0.5).normalized());
        let hit = s.hit(&ray, FOREVER).unwrap();
        assert!(((hit.point - s.center).length() - s.radius).abs() < 1e-9);
    }

    #[test]
    #[should_panic(expected = "radius must be positive")]
    fn rejects_negative_radius() {
        let _ = Sphere::new(Vec3::ZERO, -1.0, MaterialId(0));
    }
}
