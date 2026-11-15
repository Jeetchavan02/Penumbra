use super::{Aabb, HitRecord, Hittable, MaterialId};
use crate::math::{Interval, Point3, Ray, Vec3};

/// A triangle with a normal at each corner.
///
/// Stored as one corner and two edges, which is the form the intersection test
/// wants. The corner normals are interpolated across the face so that a mesh
/// of flat triangles shades as a smooth surface.
#[derive(Debug, Clone, Copy)]
pub struct Triangle {
    pub(super) p0: Point3,
    pub(super) e1: Vec3,
    pub(super) e2: Vec3,
    normals: [Vec3; 3],
    pub material: MaterialId,
}

impl Triangle {
    /// Bounding boxes of axis-aligned triangles are padded to this thickness.
    const BOX_PADDING: f64 = 1e-6;

    /// `None` if the corners are collinear or not finite. Corner normals that
    /// cannot be normalised fall back to the face normal.
    pub fn new(corners: [Point3; 3], normals: [Vec3; 3], material: MaterialId) -> Option<Self> {
        let (e1, e2) = (corners[1] - corners[0], corners[2] - corners[0]);
        // Degeneracy is judged by the angle between the edges, not by the area,
        // so that a well-shaped triangle is accepted however small it is.
        let cross = e1.cross(e2);
        let scale = e1.length() * e2.length();
        if !(scale > 0.0 && scale.is_finite() && cross.length() > 1e-12 * scale) {
            return None;
        }
        let face = cross / cross.length();
        let normals = normals.map(|n| n.try_normalized().unwrap_or(face));
        Some(Triangle { p0: corners[0], e1, e2, normals, material })
    }

    /// A triangle shaded with its own face normal.
    pub fn flat(corners: [Point3; 3], material: MaterialId) -> Option<Self> {
        Triangle::new(corners, [Vec3::ZERO; 3], material)
    }

    pub fn corners(&self) -> [Point3; 3] {
        [self.p0, self.p0 + self.e1, self.p0 + self.e2]
    }

    /// Unit normal of the flat face; counter-clockwise corners face outward.
    pub fn face_normal(&self) -> Vec3 {
        self.e1.cross(self.e2).normalized()
    }

    pub fn area(&self) -> f64 {
        0.5 * self.e1.cross(self.e2).length()
    }
}

impl Hittable for Triangle {
    /// Moller-Trumbore intersection.
    fn hit(&self, ray: &Ray, t: Interval) -> Option<HitRecord> {
        let p = ray.dir().cross(self.e2);
        let det = self.e1.dot(p);
        // Zero for rays in the plane of the triangle. Compared with zero
        // exactly: a scale-dependent threshold would drop small triangles.
        if det == 0.0 {
            return None;
        }
        let inv_det = 1.0 / det;
        let s = ray.origin() - self.p0;
        let u = s.dot(p) * inv_det;
        if !(0.0..=1.0).contains(&u) {
            return None;
        }
        let q = s.cross(self.e1);
        let v = ray.dir().dot(q) * inv_det;
        if v < 0.0 || u + v > 1.0 {
            return None;
        }
        let root = self.e2.dot(q) * inv_det;
        if !t.surrounds(root) {
            return None;
        }

        // Rebuilt from the barycentrics so the point lies in the triangle's
        // plane regardless of how far the ray has travelled.
        let point = self.p0 + self.e1 * u + self.e2 * v;
        let face = self.face_normal();
        let [n0, n1, n2] = self.normals;
        let shading = (n0 * (1.0 - u - v) + n1 * u + n2 * v).try_normalized().unwrap_or(face);
        Some(HitRecord::with_shading_normal(ray, root, point, face, shading, self.material))
    }

    fn bounding_box(&self) -> Aabb {
        let [a, b, c] = self.corners();
        Aabb::from_points(a, b).grow(c).padded(Self::BOX_PADDING)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RANGE: Interval = Interval::new(1e-4, f64::INFINITY);
    const UP: Vec3 = Vec3::new(0.0, 1.0, 0.0);
    const DOWN: Vec3 = Vec3::new(0.0, -1.0, 0.0);

    /// Right triangle in the y = 0 plane, counter-clockwise seen from above.
    fn floor() -> Triangle {
        let corners = [Point3::ZERO, Point3::new(0.0, 0.0, 1.0), Point3::new(1.0, 0.0, 0.0)];
        Triangle::flat(corners, MaterialId(0)).unwrap()
    }

    #[test]
    fn winding_sets_the_face_normal() {
        assert!((floor().face_normal() - UP).length() < 1e-12);
        assert!((floor().area() - 0.5).abs() < 1e-12);
    }

    #[test]
    fn hits_inside_and_misses_outside() {
        let hit = floor().hit(&Ray::new(Point3::new(0.25, 3.0, 0.25), DOWN), RANGE).unwrap();
        assert!((hit.t - 3.0).abs() < 1e-12);
        assert!(hit.front_face);
        assert!((hit.point - Point3::new(0.25, 0.0, 0.25)).length() < 1e-12);
        // Inside the bounding square but beyond the hypotenuse.
        assert!(floor().hit(&Ray::new(Point3::new(0.75, 3.0, 0.75), DOWN), RANGE).is_none());
        assert!(floor().hit(&Ray::new(Point3::new(-0.1, 3.0, 0.2), DOWN), RANGE).is_none());
    }

    #[test]
    fn back_face_is_hit_and_flagged() {
        let hit = floor().hit(&Ray::new(Point3::new(0.25, -1.0, 0.25), UP), RANGE).unwrap();
        assert!(!hit.front_face);
        assert!((hit.normal - DOWN).length() < 1e-12);
    }

    #[test]
    fn in_plane_and_behind_rays_miss() {
        let along = Ray::new(Point3::new(-1.0, 0.0, 0.25), Vec3::new(1.0, 0.0, 0.0));
        assert!(floor().hit(&along, RANGE).is_none());
        assert!(floor().hit(&Ray::new(Point3::new(0.25, 3.0, 0.25), UP), RANGE).is_none());
    }

    #[test]
    fn corner_normals_are_interpolated() {
        let tilt_x = Vec3::new(1.0, 1.0, 0.0);
        let tilt_z = Vec3::new(0.0, 1.0, 1.0);
        let t = Triangle::new(floor().corners(), [UP, tilt_z, tilt_x], MaterialId(0)).unwrap();
        // At a corner the shading normal is that corner's normal.
        let near_origin = t.hit(&Ray::new(Point3::new(1e-9, 1.0, 1e-9), DOWN), RANGE).unwrap();
        assert!((near_origin.normal - UP).length() < 1e-6);
        // Along the x edge it leans toward +x, and stays unit length.
        let toward_x = t.hit(&Ray::new(Point3::new(0.9, 1.0, 0.01), DOWN), RANGE).unwrap();
        assert!(toward_x.normal.x > 0.5 && toward_x.normal.z.abs() < 0.05);
        assert!((toward_x.normal.length() - 1.0).abs() < 1e-12);
    }

    #[test]
    fn shading_normal_never_faces_away_from_the_viewer() {
        // A corner normal bent almost into the surface, viewed at a grazing angle.
        let bent = Vec3::new(-1.0, 0.05, 0.0);
        let t = Triangle::new(floor().corners(), [bent; 3], MaterialId(0)).unwrap();
        let ray = Ray::new(Point3::new(-2.0, 0.6, 0.25), Vec3::new(1.0, -0.25, 0.0).normalized());
        let hit = t.hit(&ray, RANGE).unwrap();
        assert!(hit.normal.dot(ray.dir()) < 0.0);
    }

    #[test]
    fn degenerate_triangles_are_rejected() {
        let line = [Point3::ZERO, Point3::new(1.0, 0.0, 0.0), Point3::new(2.0, 0.0, 0.0)];
        assert!(Triangle::flat(line, MaterialId(0)).is_none());
        let nan = [Point3::ZERO, Point3::new(f64::NAN, 0.0, 0.0), Point3::new(0.0, 1.0, 0.0)];
        assert!(Triangle::flat(nan, MaterialId(0)).is_none());
    }

    #[test]
    fn tiny_triangles_are_still_hit() {
        let s = 1e-7;
        let corners = [Point3::ZERO, Point3::new(0.0, 0.0, s), Point3::new(s, 0.0, 0.0)];
        let t = Triangle::flat(corners, MaterialId(0)).unwrap();
        assert!(t.hit(&Ray::new(Point3::new(s * 0.25, 1.0, s * 0.25), DOWN), RANGE).is_some());
    }

    #[test]
    fn flat_box_has_thickness() {
        assert!(floor().bounding_box().extent().y > 0.0);
    }
}
