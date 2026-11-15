//! A reader and writer for the geometry subset of Wavefront OBJ.
//!
//! Understood: `v`, `vn` and `f` records, the four face-corner forms (`7`,
//! `7/1`, `7//3`, `7/1/3`), negative (relative) indices, and polygons with more
//! than three corners. Everything else (texture coordinates, groups,
//! materials, lines, curves) is skipped.

use std::fmt::{self, Write as _};
use std::path::Path;

use super::{Corner, Mesh};
use crate::math::Vec3;

/// A problem in an OBJ file, with the line it was found on.
#[derive(Debug, Clone, PartialEq)]
pub struct ObjError {
    /// One-based line number; zero when the file could not be read at all.
    pub line: usize,
    pub message: String,
}

impl fmt::Display for ObjError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.line {
            0 => write!(f, "{}", self.message),
            line => write!(f, "line {line}: {}", self.message),
        }
    }
}

impl std::error::Error for ObjError {}

pub fn load_obj(path: impl AsRef<Path>) -> Result<Mesh, ObjError> {
    let path = path.as_ref();
    let source = std::fs::read_to_string(path)
        .map_err(|e| ObjError { line: 0, message: format!("could not read {}: {e}", path.display()) })?;
    parse_obj(&source)
}

/// Files without normals are smoothed, except across edges sharper than this.
const CREASE_DEGREES: f64 = 40.0;

/// A face corner as written in the file, before normals are settled.
#[derive(Clone, Copy)]
struct RawCorner {
    position: u32,
    normal: Option<u32>,
}

pub fn parse_obj(source: &str) -> Result<Mesh, ObjError> {
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut raw_faces: Vec<[RawCorner; 3]> = Vec::new();

    for (index, raw_line) in source.lines().enumerate() {
        let fail = |message: String| ObjError { line: index + 1, message };
        let line = raw_line.split('#').next().unwrap_or("");
        let mut fields = line.split_whitespace();
        match fields.next() {
            Some("v") => positions.push(parse_vector(&mut fields).map_err(fail)?),
            Some("vn") => normals.push(parse_vector(&mut fields).map_err(fail)?),
            Some("f") => {
                let corners = fields
                    .map(|field| parse_corner(field, positions.len(), normals.len()))
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(fail)?;
                if corners.len() < 3 {
                    return Err(fail(format!("a face needs at least 3 corners, found {}", corners.len())));
                }
                // Fan triangulation: exact for convex polygons, which is what
                // OBJ exporters write.
                for i in 1..corners.len() - 1 {
                    raw_faces.push([corners[0], corners[i], corners[i + 1]]);
                }
            }
            _ => {}
        }
    }

    if positions.len() > u32::MAX as usize || normals.len() > u32::MAX as usize {
        return Err(ObjError { line: 0, message: "mesh has too many vertices".to_string() });
    }

    // Use the file's normals only if every corner has one. A mesh that
    // supplies them for some faces and not others gets computed normals
    // throughout, rather than a visible seam between the two kinds.
    let has_normals = !raw_faces.is_empty() && raw_faces.iter().flatten().all(|c| c.normal.is_some());
    if has_normals {
        let faces = raw_faces
            .iter()
            .map(|face| face.map(|c| Corner { position: c.position, normal: c.normal.unwrap_or(0) }))
            .collect();
        Ok(Mesh { positions, normals, faces })
    } else {
        let triangles: Vec<[u32; 3]> = raw_faces.iter().map(|face| face.map(|c| c.position)).collect();
        Ok(Mesh::with_creases(positions, &triangles, CREASE_DEGREES))
    }
}

fn parse_vector<'a>(fields: &mut impl Iterator<Item = &'a str>) -> Result<Vec3, String> {
    let mut component = || -> Result<f64, String> {
        let text = fields.next().ok_or("expected three numbers")?;
        let value: f64 = text.parse().map_err(|_| format!("not a number: {text}"))?;
        if value.is_finite() {
            Ok(value)
        } else {
            Err(format!("not a finite number: {text}"))
        }
    };
    // Any further fields (a `w` coordinate, vertex colours) are ignored.
    Ok(Vec3::new(component()?, component()?, component()?))
}

fn parse_corner(field: &str, position_count: usize, normal_count: usize) -> Result<RawCorner, String> {
    let mut parts = field.split('/');
    let position = resolve_index(parts.next().unwrap_or(""), position_count, "vertex")?;
    let _texture = parts.next();
    let normal = match parts.next() {
        Some(text) if !text.is_empty() => Some(resolve_index(text, normal_count, "normal")?),
        _ => None,
    };
    if parts.next().is_some() {
        return Err(format!("malformed face corner: {field}"));
    }
    Ok(RawCorner { position, normal })
}

/// OBJ indices are one-based, and negative ones count back from the most
/// recently defined element.
fn resolve_index(text: &str, count: usize, kind: &str) -> Result<u32, String> {
    let index: i64 = text.parse().map_err(|_| format!("invalid {kind} index: {text:?}"))?;
    let resolved = if index > 0 { index - 1 } else { count as i64 + index };
    if index == 0 || resolved < 0 || resolved >= count as i64 {
        return Err(format!("{kind} index {index} is out of range (have {count})"));
    }
    Ok(resolved as u32)
}

/// Serialise a mesh as OBJ text, with full `f64` round-trip precision.
pub fn to_obj(mesh: &Mesh) -> String {
    let mut out = String::new();
    for p in mesh.positions() {
        let _ = writeln!(out, "v {} {} {}", p.x, p.y, p.z);
    }
    for n in mesh.normals() {
        let _ = writeln!(out, "vn {} {} {}", n.x, n.y, n.z);
    }
    for [a, b, c] in mesh.faces() {
        let corner = |c: &Corner| format!("{}//{}", c.position + 1, c.normal + 1);
        let _ = writeln!(out, "f {} {} {}", corner(a), corner(b), corner(c));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::super::tests::octahedron;
    use super::*;
    use crate::geometry::MaterialId;
    use crate::math::Point3;

    #[test]
    fn parses_positions_faces_and_comments() {
        let mesh = parse_obj(
            "# a single triangle\n\
             o thing\n\
             v 0 0 0\n\
             v 1 0 0   # trailing comment\n\
             v 0 1 0 1.0\n\
             vt 0.5 0.5\n\
             usemtl whatever\n\
             f 1 2 3\n",
        )
        .unwrap();
        assert_eq!(mesh.positions().len(), 3);
        assert_eq!(mesh.triangle_count(), 1);
        // No normals in the file, so they were computed: +z for this winding.
        assert!((mesh.normals()[0] - Vec3::new(0.0, 0.0, 1.0)).length() < 1e-12);
    }

    #[test]
    fn all_four_corner_forms_are_accepted() {
        let header = "v 0 0 0\nv 1 0 0\nv 0 1 0\nvt 0 0\nvn 0 0 1\n";
        for face in ["f 1 2 3", "f 1/1 2/1 3/1", "f 1//1 2//1 3//1", "f 1/1/1 2/1/1 3/1/1"] {
            let mesh = parse_obj(&format!("{header}{face}\n")).unwrap();
            assert_eq!(mesh.triangle_count(), 1, "{face}");
            let t = mesh.triangles(MaterialId(0)).next().unwrap();
            assert_eq!(t.corners()[1], Point3::new(1.0, 0.0, 0.0), "{face}");
        }
    }

    #[test]
    fn negative_indices_count_back_from_the_latest_vertex() {
        let mesh = parse_obj("v 9 9 9\nv 0 0 0\nv 1 0 0\nv 0 1 0\nf -3 -2 -1\n").unwrap();
        let t = mesh.triangles(MaterialId(0)).next().unwrap();
        assert_eq!(t.corners(), [Point3::ZERO, Point3::new(1.0, 0.0, 0.0), Point3::new(0.0, 1.0, 0.0)]);
    }

    #[test]
    fn polygons_are_fan_triangulated() {
        let mesh = parse_obj("v 0 0 0\nv 1 0 0\nv 1 1 0\nv 0 1 0\nf 1 2 3 4\n").unwrap();
        assert_eq!(mesh.triangle_count(), 2);
        let area: f64 = mesh.triangles(MaterialId(0)).map(|t| t.area()).sum();
        assert!((area - 1.0).abs() < 1e-12);
    }

    #[test]
    fn file_normals_are_used_when_every_corner_has_one() {
        let mesh = parse_obj("v 0 0 0\nv 1 0 0\nv 0 1 0\nvn 0 0 5\nvn 1 0 0\nf 1//2 2//2 3//1\n").unwrap();
        assert_eq!(mesh.normals().len(), 2);
        assert_eq!(mesh.faces()[0][0].normal, 1);
    }

    #[test]
    fn partial_normals_fall_back_to_smooth_normals() {
        let mesh = parse_obj("v 0 0 0\nv 1 0 0\nv 0 1 0\nvn 1 0 0\nf 1//1 2 3\n").unwrap();
        assert_eq!(mesh.normals().len(), 3);
        assert!((mesh.normals()[0] - Vec3::new(0.0, 0.0, 1.0)).length() < 1e-12);
    }

    #[test]
    fn hard_edges_stay_hard_without_file_normals() {
        use crate::geometry::Hittable;
        use crate::math::{Interval, Ray};

        // Two unit squares meeting at a right angle along the x axis.
        let mesh = parse_obj("v 0 0 0\nv 1 0 0\nv 1 0 1\nv 0 0 1\nv 1 1 0\nv 0 1 0\nf 1 4 3 2\nf 1 2 5 6\n").unwrap();
        assert_eq!(mesh.triangle_count(), 4);
        for t in mesh.triangles(MaterialId(0)) {
            // Seen from its front, each triangle shades with its own face normal.
            let centre = t.corners().into_iter().sum::<Vec3>() / 3.0;
            let ray = Ray::new(centre + t.face_normal(), -t.face_normal());
            let hit = t.hit(&ray, Interval::new(1e-6, f64::INFINITY)).unwrap();
            assert!((hit.normal - t.face_normal()).length() < 1e-9);
        }
    }

    #[test]
    fn errors_name_the_line() {
        let cases = [
            ("v 0 0\n", 1, "expected three numbers"),
            ("v 0 0 0\nv 1 zero 0\n", 2, "not a number"),
            ("v 0 0 nan\n", 1, "not a finite number"),
            ("v 0 0 0\nv 1 0 0\nv 0 1 0\nf 1 2 4\n", 4, "out of range"),
            ("v 0 0 0\nv 1 0 0\nv 0 1 0\nf 0 1 2\n", 4, "out of range"),
            ("v 0 0 0\nv 1 0 0\nf 1 2\n", 3, "at least 3 corners"),
            ("v 0 0 0\nv 1 0 0\nv 0 1 0\nf 1//1 2//1 3//1\n", 4, "normal index"),
            ("v 0 0 0\nv 1 0 0\nv 0 1 0\nvn 0 0 1\nf 1/1/1/1 2 3\n", 5, "malformed"),
            ("v 0 0 0\nv 1 0 0\nv 0 1 0\nf -4 1 2\n", 4, "out of range"),
        ];
        for (source, line, fragment) in cases {
            let error = parse_obj(source).unwrap_err();
            assert_eq!(error.line, line, "{source:?} gave {error}");
            assert!(error.message.contains(fragment), "{source:?} gave {error}");
        }
    }

    #[test]
    fn empty_input_is_an_empty_mesh() {
        let mesh = parse_obj("# nothing here\n\n").unwrap();
        assert_eq!(mesh.triangle_count(), 0);
        assert_eq!(mesh.triangles(MaterialId(0)).count(), 0);
    }

    #[test]
    fn written_meshes_read_back_identically() {
        let mut mesh = octahedron();
        mesh.transform(1.7, 33.0, Vec3::new(0.1, -2.0, 1e-9));
        let back = parse_obj(&to_obj(&mesh)).unwrap();
        assert_eq!(back.positions(), mesh.positions());
        assert_eq!(back.normals(), mesh.normals());
        assert_eq!(back.faces(), mesh.faces());
    }

    #[test]
    fn missing_files_report_the_path() {
        let error = load_obj("/definitely/not/here.obj").unwrap_err();
        assert_eq!(error.line, 0);
        assert!(error.to_string().contains("not/here.obj"));
    }
}
