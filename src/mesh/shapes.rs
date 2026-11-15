//! Meshes generated from formulas, so the repository needs no model files.

use std::f64::consts::TAU;

use super::Mesh;
use crate::math::Point3;

/// A tube of radius `tube_radius` swept along a (p, q) torus knot: a closed
/// curve that winds `p` times around the axis of a torus and `q` times through
/// its hole. (2, 3) is the trefoil.
///
/// The knot lies in the xy plane, about 6 units across, facing +z. It has
/// `2 * segments * sides` triangles, wound to face outward.
pub fn torus_knot(p: u32, q: u32, segments: u32, sides: u32, tube_radius: f64) -> Mesh {
    assert!(p > 0 && q > 0, "knot winding numbers must be positive");
    assert!(segments >= 3 && sides >= 3, "a tube needs at least 3 segments and 3 sides");
    const MAJOR: f64 = 2.0;
    const MINOR: f64 = 0.9;

    let (pf, qf) = (p as f64, q as f64);
    let curve = |t: f64| {
        let r = MAJOR + MINOR * (qf * t).cos();
        Point3::new(r * (pf * t).cos(), r * (pf * t).sin(), -MINOR * (qf * t).sin())
    };

    let mut positions = Vec::with_capacity((segments * sides) as usize);
    for i in 0..segments {
        let t = TAU * i as f64 / segments as f64;
        let centre = curve(t);
        // Central difference; the curve is smooth so a small step is accurate.
        let h = 1e-5;
        let tangent = (curve(t + h) - curve(t - h)).normalized();
        // The curve lies on a torus, whose surface normal is perpendicular to
        // the tangent everywhere. That gives a frame with no twist and no
        // degenerate points, which a Frenet frame cannot promise.
        let ring = Point3::new(MAJOR * (pf * t).cos(), MAJOR * (pf * t).sin(), 0.0);
        let outward = (centre - ring).normalized();
        let side = tangent.cross(outward);
        for j in 0..sides {
            let (sin, cos) = (TAU * j as f64 / sides as f64).sin_cos();
            positions.push(centre + (outward * cos + side * sin) * tube_radius);
        }
    }

    let vertex = |i: u32, j: u32| (i % segments) * sides + (j % sides);
    let mut triangles = Vec::with_capacity((2 * segments * sides) as usize);
    for i in 0..segments {
        for j in 0..sides {
            let (a, b, c, d) = (vertex(i, j), vertex(i + 1, j), vertex(i + 1, j + 1), vertex(i, j + 1));
            triangles.push([a, c, b]);
            triangles.push([a, d, c]);
        }
    }
    Mesh::from_triangles(positions, &triangles)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::MaterialId;
    use crate::math::Vec3;

    #[test]
    fn knot_has_the_promised_triangle_count_and_none_are_degenerate() {
        let knot = torus_knot(2, 3, 200, 16, 0.3);
        assert_eq!(knot.triangle_count(), 2 * 200 * 16);
        assert_eq!(knot.triangles(MaterialId(0)).count(), knot.triangle_count());
    }

    #[test]
    fn knot_is_closed() {
        // In a closed surface every edge is shared by exactly two triangles,
        // once in each direction.
        let knot = torus_knot(3, 4, 60, 8, 0.25);
        let mut edges = std::collections::HashMap::new();
        for face in knot.faces() {
            for k in 0..3 {
                let (a, b) = (face[k].position, face[(k + 1) % 3].position);
                *edges.entry((a, b)).or_insert(0) += 1;
            }
        }
        for (&(a, b), &count) in &edges {
            assert_eq!(count, 1, "edge {a}->{b} is used {count} times in the same direction");
            assert_eq!(edges.get(&(b, a)), Some(&1), "edge {a}->{b} has no opposite");
        }
    }

    #[test]
    fn knot_faces_point_away_from_the_tube_centre() {
        let (segments, sides) = (120, 12);
        let knot = torus_knot(2, 3, segments, sides, 0.3);
        // The centre of each ring of vertices is the curve point it surrounds.
        let ring_centre = |ring: usize| {
            knot.positions()[ring * sides as usize..(ring + 1) * sides as usize].iter().copied().sum::<Vec3>()
                / sides as f64
        };
        for (index, t) in knot.triangles(MaterialId(0)).enumerate() {
            let ring = index / (2 * sides as usize);
            let centroid = t.corners().into_iter().sum::<Vec3>() / 3.0;
            let centre = (ring_centre(ring) + ring_centre((ring + 1) % segments as usize)) * 0.5;
            assert!(t.face_normal().dot(centroid - centre) > 0.0, "triangle {index} faces inward");
        }
    }

    #[test]
    fn smooth_normals_agree_with_the_faces() {
        let knot = torus_knot(2, 3, 300, 24, 0.3);
        for face in knot.faces() {
            let [a, b, c] = face.map(|corner| knot.positions()[corner.position as usize]);
            let flat = (b - a).cross(c - a).normalized();
            for corner in face {
                assert!(knot.normals()[corner.normal as usize].dot(flat) > 0.9);
            }
        }
    }
}
