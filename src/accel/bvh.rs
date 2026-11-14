use crate::geometry::{Aabb, HitRecord, Hittable, Primitive};
use crate::math::{Interval, Point3, Ray};

/// How the builder chooses where to cut a node in two.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SplitMethod {
    /// Cut the longest axis of the centroid bounds at its spatial midpoint.
    #[default]
    Midpoint,
}

/// Largest number of primitives a leaf may hold.
pub(super) const MAX_LEAF_SIZE: usize = 4;
/// Below this depth the builder stops trusting spatial splits and halves the
/// primitive count instead, which bounds the total depth at 64.
const MEDIAN_DEPTH: usize = 32;
const STACK_SIZE: usize = 64;

/// One node of the flattened tree.
///
/// Nodes are stored depth-first, so an interior node's left child is always the
/// next node and only the right child's index needs storing.
#[derive(Debug, Clone, Copy)]
struct Node {
    bbox: Aabb,
    /// Leaf: index of its first primitive. Interior: index of its right child.
    offset: u32,
    /// Number of primitives; zero marks an interior node.
    count: u32,
    /// Interior only: the axis the children were separated along.
    axis: u8,
}

/// A primitive together with the data the builder sorts by.
pub(super) struct Item {
    pub(super) prim: Primitive,
    pub(super) bbox: Aabb,
    pub(super) centroid: Point3,
}

/// Bounding volume hierarchy over a flat array of primitives.
///
/// The tree owns its primitives and reorders them during the build so that
/// every leaf is a contiguous slice. There are no pointers and no allocation
/// during traversal.
pub struct Bvh {
    nodes: Vec<Node>,
    primitives: Vec<Primitive>,
}

impl Bvh {
    pub fn build(primitives: Vec<Primitive>, method: SplitMethod) -> Self {
        assert!(primitives.len() < u32::MAX as usize, "too many primitives");
        let mut items: Vec<Item> = primitives
            .into_iter()
            .map(|prim| {
                let bbox = prim.bounding_box();
                Item { prim, bbox, centroid: bbox.centroid() }
            })
            .collect();

        let mut nodes = Vec::with_capacity(items.len().max(1) * 2);
        if !items.is_empty() {
            build_node(&mut items, 0, &mut nodes, method, 0);
        }
        Bvh { nodes, primitives: items.into_iter().map(|item| item.prim).collect() }
    }

    /// The primitives in leaf order.
    pub fn primitives(&self) -> &[Primitive] {
        &self.primitives
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Depth of the deepest leaf; a single-leaf tree has depth 1.
    pub fn depth(&self) -> usize {
        fn walk(nodes: &[Node], index: usize) -> usize {
            let node = &nodes[index];
            if node.count > 0 {
                1
            } else {
                1 + walk(nodes, index + 1).max(walk(nodes, node.offset as usize))
            }
        }
        if self.nodes.is_empty() {
            0
        } else {
            walk(&self.nodes, 0)
        }
    }
}

impl Hittable for Bvh {
    fn hit(&self, ray: &Ray, t: Interval) -> Option<HitRecord> {
        if self.nodes.is_empty() {
            return None;
        }
        let dir = ray.dir();
        let mut stack = [0u32; STACK_SIZE];
        let mut stack_len = 0;
        let mut current = 0usize;
        let mut closest = t.max;
        let mut best = None;

        loop {
            let node = &self.nodes[current];
            // `closest` shrinks as hits are found, culling everything behind them.
            if node.bbox.hit(ray, Interval::new(t.min, closest)) {
                if node.count > 0 {
                    let first = node.offset as usize;
                    for prim in &self.primitives[first..first + node.count as usize] {
                        if let Some(hit) = prim.hit(ray, Interval::new(t.min, closest)) {
                            closest = hit.t;
                            best = Some(hit);
                        }
                    }
                } else {
                    // Visit the child the ray reaches first; defer the other.
                    let (left, right) = (current + 1, node.offset as usize);
                    let (near, far) = if dir[node.axis as usize] < 0.0 { (right, left) } else { (left, right) };
                    stack[stack_len] = far as u32;
                    stack_len += 1;
                    current = near;
                    continue;
                }
            }
            if stack_len == 0 {
                break;
            }
            stack_len -= 1;
            current = stack[stack_len] as usize;
        }
        best
    }

    fn bounding_box(&self) -> Aabb {
        self.nodes.first().map_or(Aabb::EMPTY, |root| root.bbox)
    }
}

/// Build the subtree for `items`, which occupy `first..first + items.len()` of
/// the final primitive array. Returns the index of the node created.
fn build_node(items: &mut [Item], first: usize, nodes: &mut Vec<Node>, method: SplitMethod, depth: usize) -> usize {
    let bbox = items.iter().fold(Aabb::EMPTY, |b, item| b.union(item.bbox));
    let index = nodes.len();
    nodes.push(Node { bbox, offset: first as u32, count: items.len() as u32, axis: 0 });

    let Some((axis, mid)) = choose_split(items, bbox, method, depth) else {
        return index;
    };
    let (left, right) = items.split_at_mut(mid);
    build_node(left, first, nodes, method, depth + 1);
    let right_index = build_node(right, first + mid, nodes, method, depth + 1);
    nodes[index] = Node { bbox, offset: right_index as u32, count: 0, axis: axis as u8 };
    index
}

/// Partition `items` in place and return `(axis, split index)`, or `None` to
/// make a leaf. The split index is always strictly inside the slice.
fn choose_split(items: &mut [Item], bbox: Aabb, method: SplitMethod, depth: usize) -> Option<(usize, usize)> {
    let _ = bbox;
    let n = items.len();
    if n <= 1 {
        return None;
    }
    let centroid_bounds = items.iter().fold(Aabb::EMPTY, |b, item| b.grow(item.centroid));
    let axis = centroid_bounds.longest_axis();

    // Coincident centroids cannot be separated spatially, and very deep trees
    // mean spatial splits are not making progress. Halve the count instead.
    let degenerate = centroid_bounds.extent()[axis] <= 0.0;
    if degenerate || depth >= MEDIAN_DEPTH {
        return (n > MAX_LEAF_SIZE).then(|| (axis, median_split(items, axis)));
    }

    match method {
        SplitMethod::Midpoint => {
            if n <= MAX_LEAF_SIZE {
                return None;
            }
            let pivot = centroid_bounds.centroid()[axis];
            let mid = partition(items, |item| item.centroid[axis] < pivot);
            // Rounding can put everything on one side of the pivot.
            Some((axis, if mid == 0 || mid == n { median_split(items, axis) } else { mid }))
        }
    }
}

/// Split into equal halves by centroid order along `axis`.
pub(super) fn median_split(items: &mut [Item], axis: usize) -> usize {
    let mid = items.len() / 2;
    items.select_nth_unstable_by(mid, |a, b| a.centroid[axis].total_cmp(&b.centroid[axis]));
    mid
}

/// Move the elements satisfying `pred` to the front; return how many there are.
pub(super) fn partition<T>(slice: &mut [T], mut pred: impl FnMut(&T) -> bool) -> usize {
    let mut boundary = 0;
    for i in 0..slice.len() {
        if pred(&slice[i]) {
            slice.swap(boundary, i);
            boundary += 1;
        }
    }
    boundary
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::{MaterialId, Quad, Sphere};
    use crate::math::{Sampler, Vec3};

    const RANGE: Interval = Interval::new(1e-4, f64::INFINITY);

    fn random_point(s: &mut Sampler, half: f64) -> Vec3 {
        Vec3::new(s.range(-half, half), s.range(-half, half), s.range(-half, half))
    }

    /// A deliberately awkward mix: clustered small spheres, a few huge ones and
    /// flat quads.
    pub(crate) fn random_primitives(count: usize, seed: u64) -> Vec<Primitive> {
        let mut s = Sampler::new(seed);
        let m = MaterialId(0);
        (0..count)
            .map(|i| match i % 10 {
                0 => Sphere::new(random_point(&mut s, 50.0), s.range(5.0, 20.0), m).into(),
                1 => Quad::new(random_point(&mut s, 20.0), s.unit_vector() * 3.0, s.unit_vector() * 2.0 + Vec3::new(0.0, 0.1, 0.0), m)
                    .into(),
                _ => Sphere::new(random_point(&mut s, 5.0), s.range(0.05, 0.5), m).into(),
            })
            .collect()
    }

    fn brute_force(prims: &[Primitive], ray: &Ray) -> Option<HitRecord> {
        let mut best: Option<HitRecord> = None;
        for p in prims {
            let range = Interval::new(RANGE.min, best.map_or(RANGE.max, |h| h.t));
            if let Some(h) = p.hit(ray, range) {
                best = Some(h);
            }
        }
        best
    }

    pub(crate) fn assert_matches_brute_force(method: SplitMethod, count: usize) {
        let prims = random_primitives(count, 42);
        let bvh = Bvh::build(prims.clone(), method);
        assert_eq!(bvh.primitives().len(), count);
        assert!(bvh.depth() <= STACK_SIZE);

        let mut s = Sampler::new(7);
        let mut hits = 0;
        for _ in 0..5_000 {
            let ray = Ray::new(random_point(&mut s, 30.0), s.unit_vector());
            match (bvh.hit(&ray, RANGE), brute_force(&prims, &ray)) {
                (Some(a), Some(b)) => {
                    assert!((a.t - b.t).abs() < 1e-9, "t mismatch: {} vs {}", a.t, b.t);
                    hits += 1;
                }
                (None, None) => {}
                (a, b) => panic!("bvh {:?} disagrees with brute force {:?}", a.map(|h| h.t), b.map(|h| h.t)),
            }
        }
        // Guard against a vacuous pass where every ray misses.
        if count >= 30 {
            assert!(hits > 500, "test rays should hit often, got {hits}");
        }
    }

    #[test]
    fn midpoint_matches_brute_force() {
        for count in [1, 2, 5, 37, 400] {
            assert_matches_brute_force(SplitMethod::Midpoint, count);
        }
    }

    #[test]
    fn empty_tree_hits_nothing() {
        let bvh = Bvh::build(Vec::new(), SplitMethod::Midpoint);
        let ray = Ray::new(Vec3::ZERO, Vec3::new(0.0, 0.0, 1.0));
        assert!(bvh.hit(&ray, RANGE).is_none());
        assert_eq!(bvh.depth(), 0);
    }

    #[test]
    fn coincident_centroids_still_build() {
        // Concentric spheres cannot be separated by any spatial split.
        let prims: Vec<Primitive> =
            (1..=50).map(|i| Sphere::new(Vec3::ZERO, i as f64, MaterialId(0)).into()).collect();
        let bvh = Bvh::build(prims, SplitMethod::Midpoint);
        let hit = bvh.hit(&Ray::new(Vec3::new(0.0, 0.0, 100.0), Vec3::new(0.0, 0.0, -1.0)), RANGE).unwrap();
        assert!((hit.t - 50.0).abs() < 1e-9);
    }

    #[test]
    fn root_box_contains_everything() {
        let prims = random_primitives(100, 3);
        let bvh = Bvh::build(prims.clone(), SplitMethod::Midpoint);
        let root = bvh.bounding_box();
        for p in &prims {
            assert_eq!(root.union(p.bounding_box()), root);
        }
    }
}
