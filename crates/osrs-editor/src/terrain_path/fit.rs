//! The path tool's fitter: turns a drawn polyline and a width into overlay tiles, choosing for
//! every tile the overlay shape and rotation whose covered area best matches the ideal path area,
//! while keeping the edges of neighbouring tiles consistent.
//!
//! This is a port of the EikenB00m map editor's `PathOverlayFitter` and follows it step for step:
//! the ideal coverage of a tile is an 8x8 grid of samples closer to the line than half the width;
//! every candidate (no overlay, the full tile, and each of the eleven shapes in four rotations) is
//! scored by how many samples agree; then up to six passes re-score every tile against the edges
//! of its neighbours (a missing neighbour counts as an empty tile). The shapes come from the real
//! terrain tables ([`super::overlay_triangles`]), so any shape the overlay painter can produce is
//! a candidate. It is compared with the original's output in `eiken_golden.txt`.
//!
//! The one deliberate difference: the original refines tiles in Java `HashMap` order, which
//! depends on the map coordinates by accident. Tiles are refined in `(x, y)` order here, so a path
//! gives the same fit wherever it is drawn.

use super::overlay_triangles;
use std::collections::BTreeMap;
use std::sync::OnceLock;

/// Sub-samples per tile axis used to compare the ideal path area with a candidate shape.
const SAMPLES: usize = 8;
const REFINE_PASSES: usize = 6;
const WEST: usize = 0;
const EAST: usize = 1;
const SOUTH: usize = 2;
const NORTH: usize = 3;
/// Catmull-Rom samples per segment.
const CURVE_STEPS: usize = 12;

/// One fitted overlay tile. `shape` is the stored map value, 0 (the whole tile) to 11.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FittedTile {
    pub tile_x: i32,
    pub tile_y: i32,
    pub shape: u8,
    pub orientation: u8,
}

#[derive(Debug, Clone)]
struct Candidate {
    /// `None` for "no overlay".
    shape: Option<u8>,
    rotation: u8,
    covered: [bool; SAMPLES * SAMPLES],
    /// Coverage just inside each tile side, indexed `[side][position along the side]`.
    edges: [[bool; SAMPLES]; 4],
}

/// Turn waypoints into a smooth curve through every waypoint (Catmull-Rom spline, the ends
/// repeated), as a dense polyline. Fewer than three points are returned as they are.
pub fn smooth(points: &[(f64, f64)]) -> Vec<(f64, f64)> {
    if points.len() < 3 {
        return points.to_vec();
    }
    let n = points.len();
    let mut out = Vec::with_capacity((n - 1) * CURVE_STEPS + 1);
    for i in 0..n - 1 {
        let p0 = points[i.saturating_sub(1)];
        let (p1, p2) = (points[i], points[i + 1]);
        let p3 = points[(i + 2).min(n - 1)];
        for step in 0..CURVE_STEPS {
            let t = step as f64 / CURVE_STEPS as f64;
            let (t2, t3) = (t * t, t * t * t);
            out.push((
                catmull_rom(p0.0, p1.0, p2.0, p3.0, t, t2, t3),
                catmull_rom(p0.1, p1.1, p2.1, p3.1, t, t2, t3),
            ));
        }
    }
    out.push(points[n - 1]);
    out
}

fn catmull_rom(p0: f64, p1: f64, p2: f64, p3: f64, t: f64, t2: f64, t3: f64) -> f64 {
    0.5 * (2.0 * p1
        + (p2 - p0) * t
        + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * t2
        + (3.0 * p1 - p0 - 3.0 * p2 + p3) * t3)
}

/// The overlay tiles for a polyline of tile coordinates (a tile `(x, y)` spans `[x, x + 1]` on
/// both axes) drawn `width` tiles wide, sorted by tile. `edge_weight` is how strongly neighbouring
/// tiles are made to line up; zero fits every tile on its own.
pub fn fit(line: &[(f64, f64)], width: f64, edge_weight: f64) -> Vec<FittedTile> {
    fit_in_order(line, width, edge_weight, |tiles| tiles)
}

/// [`fit`] with the order tiles are refined in chosen by `order`, which is given the touched tiles
/// in `(x, y)` order. Refining in place makes ties between equally good outlines (a path edge that
/// could start on either phase of a repeating pattern) depend on the order.
fn fit_in_order(
    line: &[(f64, f64)],
    width: f64,
    edge_weight: f64,
    order: impl FnOnce(Vec<(i32, i32)>) -> Vec<(i32, i32)>,
) -> Vec<FittedTile> {
    if line.is_empty()
        || !width.is_finite()
        || line.iter().any(|p| !p.0.is_finite() || !p.1.is_finite())
    {
        return Vec::new();
    }
    let half = width / 2.0;
    let (mut min, mut max) = ((f64::MAX, f64::MAX), (f64::MIN, f64::MIN));
    for p in line {
        min = (min.0.min(p.0), min.1.min(p.1));
        max = (max.0.max(p.0), max.1.max(p.1));
    }
    let start = (
        ((min.0 - half).floor() as i32).max(0),
        ((min.1 - half).floor() as i32).max(0),
    );
    let end = ((max.0 + half).ceil() as i32, (max.1 + half).ceil() as i32);
    let candidates = candidates();

    // 1. The ideal coverage of every tile the path can touch, and a first guess for each.
    let mut ideals = BTreeMap::<(i32, i32), [bool; SAMPLES * SAMPLES]>::new();
    let mut chosen = BTreeMap::<(i32, i32), usize>::new();
    for x in start.0..=end.0 {
        for y in start.1..=end.1 {
            if distance_to_path(line, f64::from(x) + 0.5, f64::from(y) + 0.5) > half + 0.75 {
                continue; // the tile cannot touch the path
            }
            let mut ideal = [false; SAMPLES * SAMPLES];
            let mut hits = 0;
            for sx in 0..SAMPLES {
                for sy in 0..SAMPLES {
                    let px = f64::from(x) + (sx as f64 + 0.5) / SAMPLES as f64;
                    let py = f64::from(y) + (sy as f64 + 0.5) / SAMPLES as f64;
                    if distance_to_path(line, px, py) <= half {
                        ideal[sx * SAMPLES + sy] = true;
                        hits += 1;
                    }
                }
            }
            if hits == 0 {
                continue;
            }
            chosen.insert((x, y), best(&ideal, None, 0.0, (x, y), candidates));
            ideals.insert((x, y), ideal);
        }
    }

    // 2. Let neighbouring tiles agree on their shared edges.
    if edge_weight > 0.0 {
        let order = order(ideals.keys().copied().collect());
        for _ in 0..REFINE_PASSES {
            let mut changed = false;
            for tile in &order {
                let (tile, ideal) = (*tile, &ideals[tile]);
                let better = best(ideal, Some(&chosen), edge_weight, tile, candidates);
                if chosen.insert(tile, better) != Some(better) {
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
    }

    chosen
        .into_iter()
        .filter_map(|((tile_x, tile_y), index)| {
            let candidate = &candidates[index];
            candidate.shape.map(|shape| FittedTile {
                tile_x,
                tile_y,
                shape,
                orientation: candidate.rotation,
            })
        })
        .collect()
}

/// The candidate with the best area match, minus a penalty for edges that do not line up with the
/// neighbours' (when `chosen` is given). The first of equal scores wins: "no overlay", then the
/// full tile, then shapes in order.
fn best(
    ideal: &[bool; SAMPLES * SAMPLES],
    chosen: Option<&BTreeMap<(i32, i32), usize>>,
    edge_weight: f64,
    tile: (i32, i32),
    candidates: &[Candidate],
) -> usize {
    let neighbour = |dx: i32, dy: i32| -> &Candidate {
        let at = (tile.0 + dx, tile.1 + dy);
        let index = if at.0 < 0 || at.1 < 0 {
            0
        } else {
            chosen.and_then(|c| c.get(&at)).copied().unwrap_or(0)
        };
        &candidates[index]
    };
    let neighbours = chosen.map(|_| {
        (
            neighbour(-1, 0),
            neighbour(1, 0),
            neighbour(0, -1),
            neighbour(0, 1),
        )
    });
    let mut best = 0;
    let mut best_score = f64::NEG_INFINITY;
    for (index, candidate) in candidates.iter().enumerate() {
        let matching = ideal
            .iter()
            .zip(&candidate.covered)
            .filter(|(a, b)| a == b)
            .count();
        let mut score = matching as f64;
        if let Some((west, east, south, north)) = neighbours {
            score -= edge_weight
                * (mismatch(&candidate.edges[WEST], &west.edges[EAST])
                    + mismatch(&candidate.edges[EAST], &east.edges[WEST])
                    + mismatch(&candidate.edges[SOUTH], &south.edges[NORTH])
                    + mismatch(&candidate.edges[NORTH], &north.edges[SOUTH]))
                    as f64;
        }
        if score > best_score {
            best_score = score;
            best = index;
        }
    }
    best
}

fn mismatch(a: &[bool; SAMPLES], b: &[bool; SAMPLES]) -> usize {
    a.iter().zip(b).filter(|(a, b)| a != b).count()
}

/// Distance from a point to a polyline (to the point itself for a one-point line).
fn distance_to_path(line: &[(f64, f64)], x: f64, y: f64) -> f64 {
    osrs_edit::distance_to_line(line, x, y)
}

/// "No overlay", the full tile, then every shape in four rotations.
fn candidates() -> &'static [Candidate] {
    static CANDIDATES: OnceLock<Vec<Candidate>> = OnceLock::new();
    CANDIDATES.get_or_init(|| {
        let mut out = vec![Candidate {
            shape: None,
            rotation: 0,
            covered: [false; SAMPLES * SAMPLES],
            edges: [[false; SAMPLES]; 4],
        }];
        out.push(Candidate {
            shape: Some(0),
            rotation: 0,
            covered: [true; SAMPLES * SAMPLES],
            edges: [[true; SAMPLES]; 4],
        });
        for shape in 1..=11 {
            for rotation in 0..=3 {
                out.push(sampled(shape, rotation));
            }
        }
        out
    })
}

fn sampled(shape: u8, rotation: u8) -> Candidate {
    let triangles = overlay_triangles(shape, rotation);
    let covered = std::array::from_fn(|index| {
        let (sx, sy) = (index / SAMPLES, index % SAMPLES);
        covered_by(
            (
                (sx as f64 + 0.5) / SAMPLES as f64,
                (sy as f64 + 0.5) / SAMPLES as f64,
            ),
            triangles,
        )
    });
    let edges = std::array::from_fn(|side| {
        std::array::from_fn(|k| {
            let along = (k as f64 + 0.5) / SAMPLES as f64;
            let point = match side {
                WEST => (1.0 / 128.0, along),
                EAST => (127.0 / 128.0, along),
                SOUTH => (along, 1.0 / 128.0),
                _ => (along, 127.0 / 128.0),
            };
            covered_by(point, triangles)
        })
    });
    Candidate {
        shape: Some(shape),
        rotation,
        covered,
        edges,
    }
}

fn covered_by(point: (f64, f64), triangles: &[[[f32; 2]; 3]]) -> bool {
    triangles.iter().any(|t| in_triangle(point, *t))
}

/// Inside or on the border of the triangle. The corners are multiples of 1/128 and the sample
/// points of 1/128 too, so every product here is exact and "on the border" is a real zero.
fn in_triangle(point: (f64, f64), t: [[f32; 2]; 3]) -> bool {
    let corner = |i: usize| (f64::from(t[i][0]), f64::from(t[i][1]));
    let sign = |a: (f64, f64), b: (f64, f64)| {
        (point.0 - b.0) * (a.1 - b.1) - (a.0 - b.0) * (point.1 - b.1)
    };
    let (a, b, c) = (corner(0), corner(1), corner(2));
    let (d1, d2, d3) = (sign(a, b), sign(b, c), sign(c, a));
    let negative = d1 < 0.0 || d2 < 0.0 || d3 < 0.0;
    let positive = d1 > 0.0 || d2 > 0.0 || d3 > 0.0;
    !(negative && positive)
}

#[cfg(test)]
mod tests;
