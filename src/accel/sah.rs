//! Binned surface area heuristic.

use super::bvh::{median_split, partition, Item, MAX_LEAF_SIZE};
use crate::geometry::Aabb;

const BINS: usize = 12;
/// Cost of splitting a node, in units of one primitive intersection.
///
/// Splitting replaces one leaf with two children, and traversal tests the box
/// of each child it visits. A slab test costs about as much as a sphere test,
/// so a split has to save roughly two primitive tests to pay for itself.
const TRAVERSAL_COST: f64 = 2.0;

fn bin_of(coord: f64, lo: f64, extent: f64) -> usize {
    (((coord - lo) / extent * BINS as f64) as usize).min(BINS - 1)
}

/// Choose the cheapest of `3 * (BINS - 1)` candidate planes under the SAH,
/// partition `items` around it, and return `(axis, split index)`. Returns
/// `None` when leaving the primitives in one leaf is estimated to be cheaper.
pub(super) fn split(items: &mut [Item], bbox: Aabb, centroid_bounds: Aabb) -> Option<(usize, usize)> {
    let n = items.len();
    let parent_area = bbox.surface_area();
    // (cost, axis, last bin on the left side)
    let mut best: Option<(f64, usize, usize)> = None;

    for axis in 0..3 {
        let lo = centroid_bounds.min[axis];
        let extent = centroid_bounds.max[axis] - lo;
        if extent <= 0.0 {
            continue;
        }

        let mut bins = [(Aabb::EMPTY, 0usize); BINS];
        for item in items.iter() {
            let bin = &mut bins[bin_of(item.centroid[axis], lo, extent)];
            bin.0 = bin.0.union(item.bbox);
            bin.1 += 1;
        }

        // Sweep from the right to get area and count of everything after each plane.
        let mut right = [(0.0f64, 0usize); BINS];
        let (mut acc, mut count) = (Aabb::EMPTY, 0);
        for i in (1..BINS).rev() {
            acc = acc.union(bins[i].0);
            count += bins[i].1;
            right[i] = (acc.surface_area(), count);
        }

        // Sweep from the left, scoring the plane after bin `i`.
        let (mut acc, mut count) = (Aabb::EMPTY, 0);
        for i in 0..BINS - 1 {
            acc = acc.union(bins[i].0);
            count += bins[i].1;
            let (right_area, right_count) = right[i + 1];
            if count == 0 || right_count == 0 {
                continue;
            }
            let cost =
                TRAVERSAL_COST + (acc.surface_area() * count as f64 + right_area * right_count as f64) / parent_area;
            // A NaN cost (zero-area parent) compares false and is skipped.
            if best.map_or(cost.is_finite(), |(best_cost, ..)| cost < best_cost) {
                best = Some((cost, axis, i));
            }
        }
    }

    let leaf_cost = n as f64;
    match best {
        Some((cost, axis, bin)) if cost < leaf_cost || n > MAX_LEAF_SIZE => {
            let lo = centroid_bounds.min[axis];
            let extent = centroid_bounds.max[axis] - lo;
            let mid = partition(items, |item| bin_of(item.centroid[axis], lo, extent) <= bin);
            debug_assert!(mid > 0 && mid < n);
            Some((axis, mid))
        }
        None if n > MAX_LEAF_SIZE => {
            let axis = centroid_bounds.longest_axis();
            Some((axis, median_split(items, axis)))
        }
        _ => None,
    }
}
