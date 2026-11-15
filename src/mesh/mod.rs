//! Indexed triangle meshes and the Wavefront OBJ format.

mod obj;

pub use obj::{load_obj, parse_obj, to_obj, ObjError};

use crate::geometry::{Aabb, MaterialId, Triangle};
use crate::math::{Point3, Vec3};

/// One corner of a face: which position and which normal it uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Corner {
    pub position: u32,
    pub normal: u32,
}

/// A triangle mesh with shared vertices.
///
/// This is the editable form: it can be transformed and measured. Rendering
/// uses the flat list of [`Triangle`]s produced by [`Mesh::triangles`].
#[derive(Debug, Clone, Default)]
pub struct Mesh {
    positions: Vec<Point3>,
    normals: Vec<Vec3>,
    faces: Vec<[Corner; 3]>,
}

impl Mesh {
    /// Build a mesh from positions and counter-clockwise index triples,
    /// computing smooth normals. Panics if an index is out of range.
    pub fn from_triangles(positions: Vec<Point3>, triangles: &[[u32; 3]]) -> Self {
        for index in triangles.iter().flatten() {
            assert!((*index as usize) < positions.len(), "triangle index {index} is out of range");
        }
        let normals = smooth_normals(&positions, triangles);
        let faces = triangles.iter().map(|t| t.map(|i| Corner { position: i, normal: i })).collect();
        Mesh { positions, normals, faces }
    }

    pub fn positions(&self) -> &[Point3] {
        &self.positions
    }

    pub fn normals(&self) -> &[Vec3] {
        &self.normals
    }

    pub fn faces(&self) -> &[[Corner; 3]] {
        &self.faces
    }

    pub fn triangle_count(&self) -> usize {
        self.faces.len()
    }

    /// Bounding box of every position, used or not.
    pub fn bounds(&self) -> Aabb {
        self.positions.iter().fold(Aabb::EMPTY, |b, p| b.grow(*p))
    }

    /// Scale uniformly about the origin, then rotate about the y axis by
    /// `turn_degrees`, then translate.
    pub fn transform(&mut self, scale: f64, turn_degrees: f64, translate: Vec3) -> &mut Self {
        let (sin, cos) = turn_degrees.to_radians().sin_cos();
        let rotate = |v: Vec3| Vec3::new(cos * v.x + sin * v.z, v.y, -sin * v.x + cos * v.z);
        for p in &mut self.positions {
            *p = rotate(*p * scale) + translate;
        }
        // Uniform scale and rotation leave normals needing only the rotation.
        for n in &mut self.normals {
            *n = rotate(*n);
        }
        self
    }

    /// Scale and move the mesh so it is `height` tall, stands on the plane
    /// y = 0 and is centred on the y axis. A flat mesh is only moved.
    pub fn stand_on_floor(&mut self, height: f64) -> &mut Self {
        let bounds = self.bounds();
        let extent = bounds.extent();
        if !(extent.y.is_finite() && extent.max_component() >= 0.0) {
            return self;
        }
        let scale = if extent.y > 0.0 { height / extent.y } else { 1.0 };
        let centre = bounds.centroid();
        let anchor = Vec3::new(centre.x, bounds.min.y, centre.z);
        self.transform(scale, 0.0, -anchor * scale)
    }

    /// The renderable triangles. Degenerate faces are skipped.
    pub fn triangles(&self, material: MaterialId) -> impl Iterator<Item = Triangle> + '_ {
        self.faces.iter().filter_map(move |face| {
            let corners = face.map(|c| self.positions[c.position as usize]);
            let normals = face.map(|c| self.normals[c.normal as usize]);
            Triangle::new(corners, normals, material)
        })
    }
}

/// One normal per position: the average of the normals of the faces that
/// share it, weighted by face area so that slivers do not dominate.
fn smooth_normals(positions: &[Point3], triangles: &[[u32; 3]]) -> Vec<Vec3> {
    let mut sums = vec![Vec3::ZERO; positions.len()];
    for [a, b, c] in triangles.iter().map(|t| t.map(|i| i as usize)) {
        // The unnormalised cross product has length twice the face area.
        let weighted = (positions[b] - positions[a]).cross(positions[c] - positions[a]);
        if weighted.is_finite() {
            sums[a] += weighted;
            sums[b] += weighted;
            sums[c] += weighted;
        }
    }
    // Unused or fully degenerate vertices keep a zero normal; `Triangle::new`
    // replaces those with the face normal.
    sums.into_iter().map(|n| n.try_normalized().unwrap_or(Vec3::ZERO)).collect()
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A regular octahedron with outward-facing winding.
    pub(crate) fn octahedron() -> Mesh {
        let positions = vec![
            Point3::new(1.0, 0.0, 0.0),
            Point3::new(-1.0, 0.0, 0.0),
            Point3::new(0.0, 1.0, 0.0),
            Point3::new(0.0, -1.0, 0.0),
            Point3::new(0.0, 0.0, 1.0),
            Point3::new(0.0, 0.0, -1.0),
        ];
        let triangles = [[0, 2, 4], [2, 1, 4], [1, 3, 4], [3, 0, 4], [2, 0, 5], [1, 2, 5], [3, 1, 5], [0, 3, 5]];
        Mesh::from_triangles(positions, &triangles)
    }

    #[test]
    fn smooth_normals_point_outward_on_a_closed_convex_mesh() {
        let mesh = octahedron();
        for (p, n) in mesh.positions().iter().zip(mesh.normals()) {
            assert!((*n - p.normalized()).length() < 1e-12, "{p:?} has normal {n:?}");
        }
        for t in mesh.triangles(MaterialId(0)) {
            let centre = t.corners().into_iter().sum::<Vec3>() / 3.0;
            assert!(t.face_normal().dot(centre) > 0.0, "winding must face outward");
        }
        assert_eq!(mesh.triangles(MaterialId(0)).count(), 8);
    }

    #[test]
    fn stand_on_floor_sets_height_and_centres() {
        let mut mesh = octahedron();
        mesh.transform(3.0, 0.0, Vec3::new(10.0, -4.0, 2.0));
        mesh.stand_on_floor(5.0);
        let b = mesh.bounds();
        assert!(b.min.y.abs() < 1e-12 && (b.max.y - 5.0).abs() < 1e-12);
        assert!(b.centroid().x.abs() < 1e-12 && b.centroid().z.abs() < 1e-12);
    }

    #[test]
    fn rotation_turns_positions_and_normals_together() {
        let mut mesh = octahedron();
        mesh.transform(2.0, 90.0, Vec3::ZERO);
        // +x goes to -z under a 90 degree turn about y.
        assert!((mesh.positions()[0] - Point3::new(0.0, 0.0, -2.0)).length() < 1e-12);
        assert!((mesh.normals()[0] - Vec3::new(0.0, 0.0, -1.0)).length() < 1e-12);
        for n in mesh.normals() {
            assert!((n.length() - 1.0).abs() < 1e-12);
        }
    }

    #[test]
    fn degenerate_faces_are_skipped_not_fatal() {
        let positions =
            vec![Point3::ZERO, Point3::new(1.0, 0.0, 0.0), Point3::new(2.0, 0.0, 0.0), Point3::new(0.0, 1.0, 0.0)];
        let mesh = Mesh::from_triangles(positions, &[[0, 1, 2], [0, 1, 3]]);
        assert_eq!(mesh.triangle_count(), 2);
        assert_eq!(mesh.triangles(MaterialId(0)).count(), 1);
    }

    #[test]
    #[should_panic(expected = "out of range")]
    fn out_of_range_indices_are_rejected() {
        let _ = Mesh::from_triangles(vec![Point3::ZERO], &[[0, 0, 1]]);
    }
}
