//! Deterministic overlay-shape fitting for the Spline tool.
//!
//! The fitter has no obstacle routing or map mutation. It takes a dense polyline, samples the ideal
//! path band on an 8x8 grid per touched tile, and picks only shapes that the audited OSRS terrain
//! geometry can actually render. A short deterministic relaxation then improves shared edges.

use super::overlay_triangles;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

const SAMPLE_GRID: usize = 8;
const SAMPLE_COUNT: usize = SAMPLE_GRID * SAMPLE_GRID;
const REFINE_PASSES: usize = 6;
const CENTER_REJECT_MARGIN: f32 = 0.75;
const WEST: usize = 0;
const EAST: usize = 1;
const SOUTH: usize = 2;
const NORTH: usize = 3;

/// One fitted map-file overlay shape. `shape` is the stored 0..=11 value, not the renderer's
/// internal shape-table row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FittedTile {
    pub tile_x: i32,
    pub tile_y: i32,
    pub shape: u8,
    pub orientation: u8,
}

#[derive(Debug, Clone)]
struct Candidate {
    shape: Option<u8>,
    orientation: u8,
    covered: [bool; SAMPLE_COUNT],
    edges: [[bool; SAMPLE_GRID]; 4],
}

#[derive(Debug, Clone)]
struct TileState {
    ideal: [bool; SAMPLE_COUNT],
    candidate: usize,
}

/// Uniform Catmull-Rom smoothing with clamped endpoints and twelve samples per control segment.
/// Two points remain a straight segment exactly.
pub fn smooth_catmull_rom(points: &[(f32, f32)]) -> Vec<(f32, f32)> {
    if points.len() < 3 {
        return points.to_vec();
    }
    let mut out = Vec::with_capacity((points.len() - 1) * 12 + 1);
    for i in 0..points.len() - 1 {
        let p0 = points[i.saturating_sub(1)];
        let p1 = points[i];
        let p2 = points[i + 1];
        let p3 = points[(i + 2).min(points.len() - 1)];
        for step in 0..12 {
            let t = step as f32 / 12.0;
            let t2 = t * t;
            let t3 = t2 * t;
            out.push((
                catmull_component(p0.0, p1.0, p2.0, p3.0, t, t2, t3),
                catmull_component(p0.1, p1.1, p2.1, p3.1, t, t2, t3),
            ));
        }
    }
    out.push(points[points.len() - 1]);
    out
}

fn catmull_component(p0: f32, p1: f32, p2: f32, p3: f32, t: f32, t2: f32, t3: f32) -> f32 {
    0.5 * (2.0 * p1
        + (p2 - p0) * t
        + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * t2
        + (3.0 * p1 - p0 - 3.0 * p2 + p3) * t3)
}

/// Fit OSRS overlay shapes to a dense path polyline.
///
/// Unlike the old planner this never reroutes. The output follows the supplied line exactly.
/// Candidate tiles come from segment-local expanded boxes rather than one giant global rectangle,
/// which preserves coverage while avoiding pathological work for long diagonal paths.
pub fn fit_path_overlay(points: &[(f32, f32)], width: f32, edge_weight: f32) -> Vec<FittedTile> {
    if points.is_empty() {
        return Vec::new();
    }
    let width = if width.is_finite() {
        width.max(0.05)
    } else {
        1.0
    };
    let half = width * 0.5;
    let edge_weight = if edge_weight.is_finite() {
        edge_weight.max(0.0)
    } else {
        0.0
    };
    let candidates = candidates();
    let mut states = BTreeMap::<(i32, i32), TileState>::new();

    for tile in candidate_tiles(points, half) {
        let center = (tile.0 as f32 + 0.5, tile.1 as f32 + 0.5);
        if distance_to_path(center, points) > half + CENTER_REJECT_MARGIN {
            continue;
        }
        let ideal = ideal_mask(tile, points, half);
        if !ideal.iter().any(|covered| *covered) {
            continue;
        }
        let candidate = best_independent(&ideal, candidates);
        states.insert(tile, TileState { ideal, candidate });
    }

    if edge_weight > 0.0 {
        let keys: Vec<_> = states.keys().copied().collect();
        for _ in 0..REFINE_PASSES {
            let mut changed = false;
            for key in &keys {
                let next = best_with_neighbors(*key, &states, candidates, edge_weight);
                if let Some(state) = states.get_mut(key)
                    && state.candidate != next
                {
                    state.candidate = next;
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
    }

    states
        .into_iter()
        .filter_map(|((tile_x, tile_y), state)| {
            let candidate = &candidates[state.candidate];
            candidate.shape.map(|shape| FittedTile {
                tile_x,
                tile_y,
                shape,
                orientation: candidate.orientation,
            })
        })
        .collect()
}

fn candidate_tiles(points: &[(f32, f32)], half: f32) -> BTreeSet<(i32, i32)> {
    let mut tiles = BTreeSet::new();
    let pad = half + 1.0;
    if points.len() == 1 {
        add_box(&mut tiles, points[0], points[0], pad);
        return tiles;
    }
    for pair in points.windows(2) {
        add_box(&mut tiles, pair[0], pair[1], pad);
    }
    tiles
}

fn add_box(tiles: &mut BTreeSet<(i32, i32)>, a: (f32, f32), b: (f32, f32), pad: f32) {
    let x0 = (a.0.min(b.0) - pad).floor() as i32;
    let x1 = (a.0.max(b.0) + pad).ceil() as i32;
    let y0 = (a.1.min(b.1) - pad).floor() as i32;
    let y1 = (a.1.max(b.1) + pad).ceil() as i32;
    for x in x0..=x1 {
        for y in y0..=y1 {
            tiles.insert((x, y));
        }
    }
}

fn ideal_mask(tile: (i32, i32), points: &[(f32, f32)], half: f32) -> [bool; SAMPLE_COUNT] {
    let mut ideal = [false; SAMPLE_COUNT];
    for sy in 0..SAMPLE_GRID {
        for sx in 0..SAMPLE_GRID {
            let point = (
                tile.0 as f32 + (sx as f32 + 0.5) / SAMPLE_GRID as f32,
                tile.1 as f32 + (sy as f32 + 0.5) / SAMPLE_GRID as f32,
            );
            ideal[sy * SAMPLE_GRID + sx] = distance_to_path(point, points) <= half;
        }
    }
    ideal
}

fn distance_to_path(point: (f32, f32), points: &[(f32, f32)]) -> f32 {
    if points.len() == 1 {
        return distance(point, points[0]);
    }
    points
        .windows(2)
        .map(|pair| segment_distance(point, pair[0], pair[1]))
        .fold(f32::INFINITY, f32::min)
}

fn distance(a: (f32, f32), b: (f32, f32)) -> f32 {
    ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt()
}

fn segment_distance(point: (f32, f32), a: (f32, f32), b: (f32, f32)) -> f32 {
    let dx = b.0 - a.0;
    let dy = b.1 - a.1;
    let length_squared = dx * dx + dy * dy;
    if length_squared <= f32::EPSILON {
        return distance(point, a);
    }
    let t = (((point.0 - a.0) * dx + (point.1 - a.1) * dy) / length_squared).clamp(0.0, 1.0);
    distance(point, (a.0 + dx * t, a.1 + dy * t))
}

fn candidates() -> &'static [Candidate] {
    static CANDIDATES: OnceLock<Vec<Candidate>> = OnceLock::new();
    CANDIDATES.get_or_init(|| {
        let mut out = Vec::with_capacity(46);
        out.push(Candidate {
            shape: None,
            orientation: 0,
            covered: [false; SAMPLE_COUNT],
            edges: [[false; SAMPLE_GRID]; 4],
        });
        if let Some(full) = sampled_candidate(0, 0) {
            out.push(full);
        }
        for shape in 1..=11 {
            for orientation in 0..=3 {
                if let Some(candidate) = sampled_candidate(shape, orientation) {
                    out.push(candidate);
                }
            }
        }
        out
    })
}

fn sampled_candidate(shape: u8, orientation: u8) -> Option<Candidate> {
    let triangles = overlay_triangles(shape, orientation);
    if triangles.is_empty() {
        return None;
    }
    let covered = std::array::from_fn(|index| {
        let sx = index % SAMPLE_GRID;
        let sy = index / SAMPLE_GRID;
        let point = (
            (sx as f32 + 0.5) / SAMPLE_GRID as f32,
            (sy as f32 + 0.5) / SAMPLE_GRID as f32,
        );
        covered_by(point, triangles)
    });
    let edges = std::array::from_fn(|side| {
        std::array::from_fn(|sample| {
            let along = (sample as f32 + 0.5) / SAMPLE_GRID as f32;
            let point = match side {
                WEST => (1.0 / 128.0, along),
                EAST => (127.0 / 128.0, along),
                SOUTH => (along, 1.0 / 128.0),
                NORTH => (along, 127.0 / 128.0),
                _ => unreachable!(),
            };
            covered_by(point, triangles)
        })
    });
    Some(Candidate {
        shape: Some(shape),
        orientation,
        covered,
        edges,
    })
}

fn covered_by(point: (f32, f32), triangles: &[[[f32; 2]; 3]]) -> bool {
    triangles
        .iter()
        .any(|triangle| point_in_triangle(point, *triangle))
}

fn point_in_triangle(point: (f32, f32), triangle: [[f32; 2]; 3]) -> bool {
    let sign = |p: (f32, f32), a: [f32; 2], b: [f32; 2]| {
        (p.0 - b[0]) * (a[1] - b[1]) - (a[0] - b[0]) * (p.1 - b[1])
    };
    let d0 = sign(point, triangle[0], triangle[1]);
    let d1 = sign(point, triangle[1], triangle[2]);
    let d2 = sign(point, triangle[2], triangle[0]);
    let has_negative = d0 < -f32::EPSILON || d1 < -f32::EPSILON || d2 < -f32::EPSILON;
    let has_positive = d0 > f32::EPSILON || d1 > f32::EPSILON || d2 > f32::EPSILON;
    !(has_negative && has_positive)
}

fn best_independent(ideal: &[bool; SAMPLE_COUNT], candidates: &[Candidate]) -> usize {
    let mut best = 0;
    let mut best_score = agreement(ideal, &candidates[0].covered);
    for (index, candidate) in candidates.iter().enumerate().skip(1) {
        let score = agreement(ideal, &candidate.covered);
        if score > best_score {
            best = index;
            best_score = score;
        }
    }
    best
}

fn best_with_neighbors(
    key: (i32, i32),
    states: &BTreeMap<(i32, i32), TileState>,
    candidates: &[Candidate],
    edge_weight: f32,
) -> usize {
    let Some(state) = states.get(&key) else {
        return 0;
    };
    let mut best = 0;
    let mut best_score = score_candidate(key, state, 0, states, candidates, edge_weight);
    for index in 1..candidates.len() {
        let score = score_candidate(key, state, index, states, candidates, edge_weight);
        if score > best_score {
            best = index;
            best_score = score;
        }
    }
    best
}

fn score_candidate(
    key: (i32, i32),
    state: &TileState,
    candidate_index: usize,
    states: &BTreeMap<(i32, i32), TileState>,
    candidates: &[Candidate],
    edge_weight: f32,
) -> f32 {
    let candidate = &candidates[candidate_index];
    let mut mismatch = 0_u32;
    for (neighbor_key, side, opposite) in [
        ((key.0 - 1, key.1), WEST, EAST),
        ((key.0 + 1, key.1), EAST, WEST),
        ((key.0, key.1 - 1), SOUTH, NORTH),
        ((key.0, key.1 + 1), NORTH, SOUTH),
    ] {
        if let Some(neighbor) = states.get(&neighbor_key) {
            let neighbor_candidate = &candidates[neighbor.candidate];
            mismatch += edge_mismatch(&candidate.edges[side], &neighbor_candidate.edges[opposite]);
        }
    }
    agreement(&state.ideal, &candidate.covered) as f32 - edge_weight * mismatch as f32
}

fn agreement(left: &[bool; SAMPLE_COUNT], right: &[bool; SAMPLE_COUNT]) -> u32 {
    left.iter()
        .zip(right)
        .filter(|(left, right)| left == right)
        .count() as u32
}

fn edge_mismatch(left: &[bool; SAMPLE_GRID], right: &[bool; SAMPLE_GRID]) -> u32 {
    left.iter()
        .zip(right)
        .filter(|(left, right)| left != right)
        .count() as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn candidate_set_is_empty_full_plus_all_shaped_rotations() {
        assert_eq!(candidates().len(), 46);
        assert!(candidates()[0].shape.is_none());
        assert_eq!(candidates()[1].shape, Some(0));
        assert!(candidates()[1].covered.iter().all(|covered| *covered));
    }

    #[test]
    fn two_points_remain_a_straight_segment() {
        let points = [(1.25, 2.5), (8.75, 4.0)];
        assert_eq!(smooth_catmull_rom(&points), points);
    }

    #[test]
    fn catmull_rom_keeps_exact_endpoints() {
        let points = [(0.0, 0.0), (2.0, 3.0), (6.0, 4.0)];
        let smooth = smooth_catmull_rom(&points);
        assert_eq!(smooth.first().copied(), Some(points[0]));
        assert_eq!(smooth.last().copied(), Some(points[2]));
        assert_eq!(smooth.len(), 25);
    }

    #[test]
    fn fit_is_deterministic_and_uses_only_valid_stored_shapes() {
        let line = smooth_catmull_rom(&[(0.25, 0.5), (4.0, 2.0), (8.75, 2.75)]);
        let first = fit_path_overlay(&line, 2.5, 0.75);
        let second = fit_path_overlay(&line, 2.5, 0.75);
        assert_eq!(first, second);
        assert!(!first.is_empty());
        assert!(
            first
                .iter()
                .all(|tile| tile.shape <= 11 && tile.orientation <= 3)
        );
    }

    #[test]
    fn negative_world_coordinates_are_supported() {
        let fitted = fit_path_overlay(&[(-4.5, -2.25), (-0.25, -2.25)], 1.5, 0.5);
        assert!(!fitted.is_empty());
        assert!(fitted.iter().any(|tile| tile.tile_x < 0 && tile.tile_y < 0));
    }
}
