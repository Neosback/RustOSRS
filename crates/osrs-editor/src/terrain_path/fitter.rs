//! Deterministic overlay-shape fitting for the Spline tool.
//!
//! The fitter has no obstacle routing or map mutation. It takes a dense centre line that carries
//! the local half-width, samples the ideal path band on an 8x8 grid per touched tile, and picks
//! only shapes the audited OSRS terrain geometry can render. A short deterministic relaxation then
//! lines up shared edges and prefers the choices the real map makes for each neighbourhood (see
//! [`super::census`]): full tiles along straight edges, square ends, partial shapes where the
//! outline turns.

use super::census::{self, NeighbourMask};
use super::smooth::PathSample;
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
/// 8-neighbour offsets, bit i of a [`NeighbourMask`]: N, NE, E, SE, S, SW, W, NW (y grows north).
const NEIGHBOURS: [(i32, i32); 8] = [
    (0, 1),
    (1, 1),
    (1, 0),
    (1, -1),
    (0, -1),
    (-1, -1),
    (-1, 0),
    (-1, 1),
];

/// One fitted map-file overlay shape. `shape` is the stored 0..=11 value, not the renderer's
/// internal shape-table row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FittedTile {
    pub tile_x: i32,
    pub tile_y: i32,
    pub shape: u8,
    pub orientation: u8,
}

/// How the fit is steered.
pub struct FitOptions<'a> {
    /// Shared-edge agreement weight. Zero fits each tile independently.
    pub edge_weight: f32,
    /// How strongly the real map's usual choice for a neighbourhood is preferred, in samples per
    /// nat of surprise. Zero ignores the census.
    pub prior_weight: f32,
    /// Tiles the path must not cover (overlays it should not paint over).
    pub blocked: &'a dyn Fn((i32, i32)) -> bool,
}

/// The result of a fit.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Fit {
    pub tiles: Vec<FittedTile>,
    /// Tiles the ideal path covers but that are blocked, so were left alone.
    pub clipped: Vec<(i32, i32)>,
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

/// Fit OSRS overlay shapes to a dense path centre line.
///
/// This never reroutes: the output follows the line exactly. Candidate tiles come from
/// segment-local expanded boxes rather than one giant rectangle, which preserves coverage and avoids
/// pathological work for long diagonal paths.
pub fn fit_path_overlay(line: &[PathSample], options: &FitOptions<'_>) -> Fit {
    if line.is_empty() {
        return Fit::default();
    }
    let edge_weight = if options.edge_weight.is_finite() {
        options.edge_weight.max(0.0)
    } else {
        0.0
    };
    let prior_weight = if options.prior_weight.is_finite() {
        options.prior_weight.max(0.0)
    } else {
        0.0
    };
    let candidates = candidates();
    let mut states = BTreeMap::<(i32, i32), TileState>::new();
    let mut clipped = Vec::new();

    for tile in candidate_tiles(line) {
        let center = (tile.0 as f32 + 0.5, tile.1 as f32 + 0.5);
        if path_margin(center, line) > CENTER_REJECT_MARGIN {
            continue;
        }
        let ideal = ideal_mask(tile, line);
        if !ideal.iter().any(|covered| *covered) {
            continue;
        }
        if (options.blocked)(tile) {
            clipped.push(tile);
            continue;
        }
        let candidate = best_independent(&ideal, candidates);
        states.insert(tile, TileState { ideal, candidate });
    }

    if edge_weight > 0.0 || prior_weight > 0.0 {
        let keys: Vec<_> = states.keys().copied().collect();
        for _ in 0..REFINE_PASSES {
            let mut changed = false;
            for key in &keys {
                let next =
                    best_with_neighbors(*key, &states, candidates, edge_weight, prior_weight);
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

    let tiles = states
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
        .collect();
    Fit { tiles, clipped }
}

fn candidate_tiles(line: &[PathSample]) -> BTreeSet<(i32, i32)> {
    let mut tiles = BTreeSet::new();
    if line.len() == 1 {
        add_box(
            &mut tiles,
            (line[0].x, line[0].y),
            (line[0].x, line[0].y),
            line[0].half + 1.0,
        );
        return tiles;
    }
    for pair in line.windows(2) {
        let pad = pair[0].half.max(pair[1].half) + 1.0;
        add_box(
            &mut tiles,
            (pair[0].x, pair[0].y),
            (pair[1].x, pair[1].y),
            pad,
        );
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

fn ideal_mask(tile: (i32, i32), line: &[PathSample]) -> [bool; SAMPLE_COUNT] {
    let mut ideal = [false; SAMPLE_COUNT];
    for sy in 0..SAMPLE_GRID {
        for sx in 0..SAMPLE_GRID {
            let point = (
                tile.0 as f32 + (sx as f32 + 0.5) / SAMPLE_GRID as f32,
                tile.1 as f32 + (sy as f32 + 0.5) / SAMPLE_GRID as f32,
            );
            ideal[sy * SAMPLE_GRID + sx] = path_margin(point, line) <= 0.0;
        }
    }
    ideal
}

/// Distance from `point` to the path edge: negative inside. The half-width is that of the nearest
/// stretch, interpolated along it.
fn path_margin(point: (f32, f32), line: &[PathSample]) -> f32 {
    if line.len() == 1 {
        return distance(point, (line[0].x, line[0].y)) - line[0].half;
    }
    line.windows(2)
        .map(|pair| segment_margin(point, pair[0], pair[1]))
        .fold(f32::INFINITY, f32::min)
}

fn distance(a: (f32, f32), b: (f32, f32)) -> f32 {
    ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt()
}

fn segment_margin(point: (f32, f32), a: PathSample, b: PathSample) -> f32 {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let length_squared = dx * dx + dy * dy;
    let t = if length_squared <= f32::EPSILON {
        0.0
    } else {
        (((point.0 - a.x) * dx + (point.1 - a.y) * dy) / length_squared).clamp(0.0, 1.0)
    };
    distance(point, (a.x + dx * t, a.y + dy * t)) - (a.half + (b.half - a.half) * t)
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
    prior_weight: f32,
) -> usize {
    let Some(state) = states.get(&key) else {
        return 0;
    };
    // The neighbourhood is read once: which neighbours currently carry the overlay.
    let mask = neighbour_mask(key, states);
    let penalty = census::penalty(mask);
    let mut best = 0;
    let mut best_score = f32::NEG_INFINITY;
    for index in 0..candidates.len() {
        let mut score = score_candidate(key, state, index, states, candidates, edge_weight);
        if index > 0 {
            score -= prior_weight * penalty[index];
        }
        if score > best_score {
            best = index;
            best_score = score;
        }
    }
    best
}

/// Which of the eight neighbours currently carry the overlay.
fn neighbour_mask(key: (i32, i32), states: &BTreeMap<(i32, i32), TileState>) -> NeighbourMask {
    let mut mask = 0;
    for (bit, (dx, dy)) in NEIGHBOURS.iter().enumerate() {
        if states
            .get(&(key.0 + dx, key.1 + dy))
            .is_some_and(|neighbor| neighbor.candidate != 0)
        {
            mask |= 1 << bit;
        }
    }
    mask
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
mod tests;
