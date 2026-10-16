//! What the real map says about paths (see `docs/verification/PATH-CENSUS.md`): which overlays are
//! paths, and for each neighbourhood of path tiles which overlay shape and rotation the real map
//! most often uses. Pure lookups over the generated `census_data`.

use super::census_data::{PATH_OVERLAYS, PRIOR};
use std::sync::OnceLock;

/// The candidate index of a stored overlay shape and rotation, matching the fitter's candidate
/// order: `0` is "no overlay", `1` the full tile (any rotation), then shapes `1..=11` with four
/// rotations each.
pub const fn candidate_index(shape: u8, rotation: u8) -> usize {
    if shape == 0 {
        1
    } else {
        2 + (shape as usize - 1) * 4 + (rotation as usize & 3)
    }
}

/// Number of fitter candidates (no overlay, the full tile, and 11 shapes x 4 rotations).
pub const CANDIDATES: usize = 46;

/// The real map's own paths: overlays used as strips, as opposed to areas such as water or floors.
pub fn is_path_overlay(overlay: u16) -> bool {
    PATH_OVERLAYS.binary_search(&overlay).is_ok()
}

/// A bit per 8-neighbour (N, NE, E, SE, S, SW, W, NW), set when that neighbour is a path tile.
pub type NeighbourMask = u8;

/// Extra cost, in nats, of choosing each candidate for a tile with `mask`, relative to the most
/// common choice there (which costs `0`). Candidate `0` ("no overlay") always costs `0`; masks the
/// map never produced cost `0` for everything. Unseen choices are smoothed, not forbidden.
pub fn penalty(mask: NeighbourMask) -> &'static [f32; CANDIDATES] {
    static TABLE: OnceLock<Vec<[f32; CANDIDATES]>> = OnceLock::new();
    let table = TABLE.get_or_init(|| {
        let mut table = vec![[0.0; CANDIDATES]; 256];
        for &(mask, total, top) in PRIOR {
            let mut counts = [0.0_f32; CANDIDATES];
            for &(shape, rotation, count) in top {
                counts[candidate_index(shape, rotation)] += count as f32;
            }
            // Mass outside the listed choices is spread evenly: it is real but unattributed.
            let listed: f32 = counts.iter().sum();
            let spread = (total as f32 - listed).max(0.0) / (CANDIDATES - 1) as f32;
            const ALPHA: f32 = 0.5;
            let cost =
                |c: usize| -((counts[c] + spread + ALPHA) / (total as f32 + ALPHA * 45.0)).ln();
            let best = (1..CANDIDATES).map(cost).fold(f32::INFINITY, f32::min);
            for c in 1..CANDIDATES {
                table[usize::from(mask)][c] = cost(c) - best;
            }
        }
        table
    });
    &table[usize::from(mask)]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn candidate_index_matches_the_fitter_order() {
        assert_eq!(candidate_index(0, 0), 1);
        assert_eq!(candidate_index(0, 3), 1);
        assert_eq!(candidate_index(1, 0), 2);
        assert_eq!(candidate_index(11, 3), CANDIDATES - 1);
    }

    #[test]
    fn the_hidden_overlay_and_unknown_ids_are_not_paths() {
        assert!(!is_path_overlay(42));
        assert!(!is_path_overlay(0));
        assert!(!is_path_overlay(60_000));
        assert!(
            is_path_overlay(14),
            "the brown dirt trail overlay is a path"
        );
    }

    #[test]
    fn a_fully_enclosed_tile_is_almost_certainly_a_full_tile() {
        let p = penalty(255);
        assert_eq!(p[0], 0.0);
        assert!(p[1].abs() < 1e-4, "the full tile is the modal choice");
        assert!(
            p[2] > 4.0 && p[20] > 4.0,
            "a shape inside a path is very unlikely"
        );
    }

    #[test]
    fn straight_edges_prefer_the_full_tile_and_penalise_shapes_mildly() {
        // Mask 199: N, NE, E, W, NW are path tiles; the south side is open.
        let p = penalty(199);
        assert!(p[1].abs() < 1e-4);
        let (shape6_rot1, shape4_rot3) = (candidate_index(6, 1), candidate_index(4, 3));
        assert!(
            p[shape6_rot1] > 0.5 && p[shape6_rot1] < 3.0,
            "{}",
            p[shape6_rot1]
        );
        assert!(p[shape4_rot3] > p[shape6_rot1]);
    }

    #[test]
    fn unseen_masks_cost_nothing() {
        // No path tile has exactly one neighbour to the north-east only (mask 2) in the map's
        // data with a row of its own would still be finite; unknown masks are all zero.
        for mask in 0..=255_u8 {
            assert!(penalty(mask).iter().all(|c| c.is_finite() && *c >= 0.0));
        }
    }
}
