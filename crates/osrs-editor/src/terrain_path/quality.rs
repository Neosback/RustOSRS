//! Black-box quality fixtures for the simplified terrain-path fitter.
//!
//! These tests intentionally do not call the fitter's private scoring helpers. They independently
//! rasterize the requested band and independently sample the renderer-derived overlay triangles,
//! so they can catch drift between requested coverage and the shape/rotation choices the fitter
//! emits.

use super::fitter::{EDGE_WEIGHT, PRIOR_WEIGHT};
use super::{
    ControlPoint, FitOptions, FittedTile, PathSample, fit_path_overlay, overlay_triangles,
    smooth_path,
};
use std::collections::{BTreeMap, BTreeSet};

const GRID: usize = 8;
const CELLS: usize = GRID * GRID;
const EDGE_SAMPLE_OFFSET: f32 = 1.0 / 128.0;
const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

#[derive(Debug, Clone, Copy)]
struct Fixture {
    name: &'static str,
    control_points: &'static [(f32, f32)],
    width: f32,
    min_coverage_agreement: f32,
    max_edge_mismatch_rate: f32,
}

#[derive(Debug, Clone, Copy)]
struct QualityMetrics {
    ideal_tiles: usize,
    fitted_tiles: usize,
    coverage_agreement: f32,
    edge_mismatches: u32,
    shared_edge_samples: u32,
    hash: u64,
}

/// The dense centre line of a fixture's control points, all at one width.
fn dense(control_points: &[(f32, f32)], width: f32) -> Vec<(f32, f32)> {
    let controls: Vec<_> = control_points
        .iter()
        .map(|&(x, y)| ControlPoint::new(x, y, width))
        .collect();
    smooth_path(&controls).iter().map(|s| (s.x, s.y)).collect()
}

fn fixtures() -> [Fixture; 10] {
    [
        Fixture {
            name: "horizontal",
            control_points: &[(0.25, 2.5), (8.75, 2.5)],
            width: 1.5,
            min_coverage_agreement: 0.79,
            max_edge_mismatch_rate: 0.47,
        },
        Fixture {
            name: "vertical",
            control_points: &[(2.5, 0.25), (2.5, 8.75)],
            width: 1.5,
            min_coverage_agreement: 0.79,
            max_edge_mismatch_rate: 0.47,
        },
        Fixture {
            name: "diagonal_45",
            control_points: &[(0.25, 0.25), (8.75, 8.75)],
            width: 1.5,
            min_coverage_agreement: 0.89,
            max_edge_mismatch_rate: 0.12,
        },
        Fixture {
            name: "shallow_diagonal",
            control_points: &[(0.25, 1.0), (8.75, 4.0)],
            width: 1.5,
            min_coverage_agreement: 0.82,
            max_edge_mismatch_rate: 0.35,
        },
        Fixture {
            name: "gentle_curve",
            control_points: &[(0.25, 1.0), (4.0, 3.0), (8.75, 2.0)],
            width: 2.0,
            min_coverage_agreement: 0.80,
            max_edge_mismatch_rate: 0.22,
        },
        Fixture {
            name: "tight_curve",
            control_points: &[(0.25, 0.5), (3.5, 4.5), (6.0, 0.75), (8.75, 4.25)],
            width: 2.0,
            min_coverage_agreement: 0.78,
            max_edge_mismatch_rate: 0.30,
        },
        Fixture {
            name: "narrow",
            control_points: &[(0.25, 1.25), (8.75, 3.75)],
            width: 1.0,
            // Occupancy-first fitting rounds a band to whole tiles (a 1.5-wide band becomes one or
            // three tiles across), so floors are set from the measured fidelity of each fixture.
            min_coverage_agreement: 0.82,
            max_edge_mismatch_rate: 0.38,
        },
        Fixture {
            name: "wide",
            control_points: &[(0.25, 2.5), (8.75, 2.5)],
            width: 5.0,
            min_coverage_agreement: 0.93,
            max_edge_mismatch_rate: 0.16,
        },
        Fixture {
            name: "corner_crossing",
            control_points: &[(0.0, 0.0), (8.0, 8.0)],
            width: 1.25,
            min_coverage_agreement: 0.87,
            max_edge_mismatch_rate: 0.04,
        },
        Fixture {
            name: "negative_coordinates",
            control_points: &[(-8.75, -3.5), (-4.0, -1.0), (-0.25, -2.75)],
            width: 1.5,
            min_coverage_agreement: 0.80,
            max_edge_mismatch_rate: 0.32,
        },
    ]
}

#[test]
fn representative_fixture_matrix_meets_quality_floors() {
    for fixture in fixtures() {
        let dense = dense(fixture.control_points, fixture.width);
        let metrics = measure(&dense, fixture.width, EDGE_WEIGHT, PRIOR_WEIGHT);
        assert!(
            metrics.ideal_tiles > 0,
            "{} produced no ideal tiles",
            fixture.name
        );
        assert!(
            metrics.fitted_tiles > 0,
            "{} produced no fitted tiles",
            fixture.name
        );
        assert!(
            metrics.coverage_agreement >= fixture.min_coverage_agreement,
            "{} coverage agreement {:.4} fell below {:.4}: {metrics:?}",
            fixture.name,
            metrics.coverage_agreement,
            fixture.min_coverage_agreement,
        );
        let edge_mismatch_rate = if metrics.shared_edge_samples == 0 {
            0.0
        } else {
            metrics.edge_mismatches as f32 / metrics.shared_edge_samples as f32
        };
        assert!(
            edge_mismatch_rate <= fixture.max_edge_mismatch_rate,
            "{} edge mismatch rate {:.4} exceeded {:.4}: {metrics:?}",
            fixture.name,
            edge_mismatch_rate,
            fixture.max_edge_mismatch_rate,
        );
    }
}

#[test]
fn default_edge_relaxation_does_not_worsen_basic_shared_edges() {
    for fixture in fixtures().into_iter().take(4) {
        let dense = dense(fixture.control_points, fixture.width);
        // The prior is left out so only the edge relaxation is being tested.
        let independent = measure(&dense, fixture.width, 0.0, 0.0);
        let relaxed = measure(&dense, fixture.width, EDGE_WEIGHT, 0.0);
        assert!(
            relaxed.edge_mismatches <= independent.edge_mismatches,
            "{} edge relaxation worsened shared edges: independent={independent:?}, relaxed={relaxed:?}",
            fixture.name,
        );
    }
}

#[test]
fn fixture_outputs_are_repeatable() {
    for fixture in fixtures() {
        let dense = dense(fixture.control_points, fixture.width);
        let first = measure(&dense, fixture.width, EDGE_WEIGHT, PRIOR_WEIGHT);
        let second = measure(&dense, fixture.width, EDGE_WEIGHT, PRIOR_WEIGHT);
        assert_eq!(first.hash, second.hash, "{} changed hash", fixture.name);
        assert_eq!(
            first.fitted_tiles, second.fitted_tiles,
            "{} changed fitted tile count",
            fixture.name,
        );
    }
}

#[test]
fn fixture_hashes_are_locked() {
    let actual = fixtures().map(|fixture| {
        let dense = dense(fixture.control_points, fixture.width);
        measure(&dense, fixture.width, EDGE_WEIGHT, PRIOR_WEIGHT).hash
    });

    // Re-seeded when the fitter became occupancy-first (tiles chosen by rasterising the band and
    // cleaning its edges, shapes by neighbourhood bitmask against the real map's census, weight
    // `PRIOR_WEIGHT`) and the narrowest fixture became one tile wide. The visual/metric delta was
    // reviewed through the conformance and staircase tests in `fitter/tests.rs`: wide paths have no
    // spurs or pits, straight edges are full tiles ~73% of the time (real map ~77%), and fidelity to
    // the drawn band is ~0.91 (the floors below are set from each fixture's measured value). Any
    // further fitter change must review the delta before updating these values.
    let expected = [
        14_205_456_016_154_599_170,
        9_431_564_241_281_017_202,
        11_269_751_821_849_482_662,
        13_220_052_033_315_027_325,
        6_880_355_575_656_137_835,
        6_458_939_515_223_775_709,
        1_089_113_308_047_036_665,
        13_970_200_030_765_954_083,
        6_070_785_111_042_247_537,
        5_511_176_239_604_900_442,
    ];
    assert_eq!(actual, expected);
}

fn measure(
    points: &[(f32, f32)],
    width: f32,
    edge_weight: f32,
    prior_weight: f32,
) -> QualityMetrics {
    let line: Vec<PathSample> = points
        .iter()
        .map(|&(x, y)| PathSample {
            x,
            y,
            half: width * 0.5,
        })
        .collect();
    let fitted = fit_path_overlay(
        &line,
        &FitOptions {
            edge_weight,
            prior_weight,
            blocked: &|_| false,
        },
    )
    .tiles;
    let fitted_by_tile: BTreeMap<_, _> = fitted
        .iter()
        .map(|tile| ((tile.tile_x, tile.tile_y), *tile))
        .collect();
    let ideal_tiles = collect_ideal_tiles(points, width);

    let mut agreed = 0_u32;
    let mut sampled = 0_u32;
    for tile in &ideal_tiles {
        let ideal = ideal_mask(*tile, points, width * 0.5);
        let selected = fitted_by_tile.get(tile).copied();
        for (index, ideal_covered) in ideal.iter().copied().enumerate() {
            let sx = index % GRID;
            let sy = index / GRID;
            let point = (
                (sx as f32 + 0.5) / GRID as f32,
                (sy as f32 + 0.5) / GRID as f32,
            );
            if ideal_covered == selected_covers(selected, point) {
                agreed += 1;
            }
            sampled += 1;
        }
    }

    let mut edge_mismatches = 0_u32;
    let mut shared_edge_samples = 0_u32;
    for &(x, y) in &ideal_tiles {
        for (neighbor, side, opposite) in [
            ((x + 1, y), Side::East, Side::West),
            ((x, y + 1), Side::North, Side::South),
        ] {
            if !ideal_tiles.contains(&neighbor) {
                continue;
            }
            let left = fitted_by_tile.get(&(x, y)).copied();
            let right = fitted_by_tile.get(&neighbor).copied();
            for sample in 0..GRID {
                let along = (sample as f32 + 0.5) / GRID as f32;
                if selected_covers(left, edge_point(side, along))
                    != selected_covers(right, edge_point(opposite, along))
                {
                    edge_mismatches += 1;
                }
                shared_edge_samples += 1;
            }
        }
    }

    QualityMetrics {
        ideal_tiles: ideal_tiles.len(),
        fitted_tiles: fitted.len(),
        coverage_agreement: if sampled == 0 {
            1.0
        } else {
            agreed as f32 / sampled as f32
        },
        edge_mismatches,
        shared_edge_samples,
        hash: stable_hash(&fitted),
    }
}

#[derive(Debug, Clone, Copy)]
enum Side {
    West,
    East,
    South,
    North,
}

fn edge_point(side: Side, along: f32) -> (f32, f32) {
    match side {
        Side::West => (EDGE_SAMPLE_OFFSET, along),
        Side::East => (1.0 - EDGE_SAMPLE_OFFSET, along),
        Side::South => (along, EDGE_SAMPLE_OFFSET),
        Side::North => (along, 1.0 - EDGE_SAMPLE_OFFSET),
    }
}

fn collect_ideal_tiles(points: &[(f32, f32)], width: f32) -> BTreeSet<(i32, i32)> {
    let half = width * 0.5;
    let pad = half + 1.0;
    let mut min_x = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    let mut min_y = f32::INFINITY;
    let mut max_y = f32::NEG_INFINITY;
    for &(x, y) in points {
        min_x = min_x.min(x);
        max_x = max_x.max(x);
        min_y = min_y.min(y);
        max_y = max_y.max(y);
    }

    let x0 = (min_x - pad).floor() as i32;
    let x1 = (max_x + pad).ceil() as i32;
    let y0 = (min_y - pad).floor() as i32;
    let y1 = (max_y + pad).ceil() as i32;
    let mut tiles = BTreeSet::new();
    for x in x0..=x1 {
        for y in y0..=y1 {
            let mask = ideal_mask((x, y), points, half);
            if mask.iter().any(|covered| *covered) {
                tiles.insert((x, y));
            }
        }
    }
    tiles
}

fn ideal_mask(tile: (i32, i32), points: &[(f32, f32)], half: f32) -> [bool; CELLS] {
    std::array::from_fn(|index| {
        let sx = index % GRID;
        let sy = index / GRID;
        let point = (
            tile.0 as f32 + (sx as f32 + 0.5) / GRID as f32,
            tile.1 as f32 + (sy as f32 + 0.5) / GRID as f32,
        );
        distance_to_path(point, points) <= half
    })
}

fn selected_covers(tile: Option<FittedTile>, point: (f32, f32)) -> bool {
    let Some(tile) = tile else {
        return false;
    };
    overlay_triangles(tile.shape, tile.orientation)
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

fn distance_to_path(point: (f32, f32), points: &[(f32, f32)]) -> f32 {
    if points.len() == 1 {
        return distance(point, points[0]);
    }
    points
        .windows(2)
        .map(|pair| segment_distance(point, pair[0], pair[1]))
        .fold(f32::INFINITY, f32::min)
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

fn distance(a: (f32, f32), b: (f32, f32)) -> f32 {
    ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt()
}

fn stable_hash(fitted: &[FittedTile]) -> u64 {
    let mut hash = FNV_OFFSET;
    for tile in fitted {
        hash = hash_bytes(hash, &tile.tile_x.to_le_bytes());
        hash = hash_bytes(hash, &tile.tile_y.to_le_bytes());
        hash = hash_bytes(hash, &[tile.shape, tile.orientation]);
    }
    hash
}

fn hash_bytes(mut hash: u64, bytes: &[u8]) -> u64 {
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}
