//! Which tiles a path occupies: the band rasterised onto the tile grid and cleaned into regular
//! edges. The fitter then picks each occupied tile's shape from its neighbourhood bitmask.

use super::fitter::{NEIGHBOURS, SAMPLE_COUNT, candidate_tiles, distance, ideal_mask};
use super::smooth::PathSample;
use std::collections::BTreeSet;

/// Share of a tile's area the band must cover for the tile to carry the overlay: a bit under half
/// for wide paths (whose edges should land on whole tiles) and less for narrow ones (so a thin
/// diagonal stays connected).
const OCCUPY_COVERAGE_WIDE: f32 = 0.45;
const OCCUPY_COVERAGE_NARROW: f32 = 0.30;
/// Half-width below which a path counts as narrow, in tiles.
const NARROW_HALF: f32 = 1.0;
/// Tiles carrying the overlay, and the tiles the band reaches that are blocked.
pub(super) struct Occupancy {
    pub(super) tiles: BTreeSet<(i32, i32)>,
    pub(super) clipped: Vec<(i32, i32)>,
}

/// Rasterise the band onto the tile grid: a tile carries the overlay when the band covers enough
/// of it, or when the centre line passes through it (so the path is never broken). Blocked tiles
/// are never occupied and are reported. Wide paths are then cleaned: lonely spurs removed and pits
/// filled, so edges are regular rather than speckled.
pub(super) fn occupy(line: &[PathSample], blocked: &dyn Fn((i32, i32)) -> bool) -> Occupancy {
    let mut tiles = BTreeSet::new();
    let mut clipped = BTreeSet::new();
    let mut centre = BTreeSet::new();
    for sample in line {
        centre.insert((sample.x.floor() as i32, sample.y.floor() as i32));
    }
    for tile in candidate_tiles(line) {
        let covered = ideal_mask(tile, line).iter().filter(|c| **c).count();
        if covered == 0 {
            continue;
        }
        let middle = (tile.0 as f32 + 0.5, tile.1 as f32 + 0.5);
        let needed = if half_width_near(middle, line) < NARROW_HALF {
            OCCUPY_COVERAGE_NARROW
        } else {
            OCCUPY_COVERAGE_WIDE
        };
        let wanted = covered as f32 / SAMPLE_COUNT as f32 >= needed || centre.contains(&tile);
        if blocked(tile) {
            clipped.insert(tile);
        } else if wanted {
            tiles.insert(tile);
        }
    }
    for tile in &centre {
        if blocked(*tile) {
            clipped.insert(*tile);
        } else {
            tiles.insert(*tile);
        }
    }
    if line.iter().all(|s| s.half >= 1.0) {
        for _ in 0..2 {
            clean(&mut tiles, &centre, blocked);
        }
    }
    Occupancy {
        tiles,
        clipped: clipped.into_iter().collect(),
    }
}

/// Orthogonal occupied neighbours of a tile.
fn orthogonal(tile: (i32, i32), tiles: &BTreeSet<(i32, i32)>) -> usize {
    [(0, 1), (1, 0), (0, -1), (-1, 0)]
        .iter()
        .filter(|(dx, dy)| tiles.contains(&(tile.0 + dx, tile.1 + dy)))
        .count()
}

/// One cleaning round: fill empty tiles with three or more occupied sides, then drop occupied tiles
/// (off the centre line) with at most one.
fn clean(
    tiles: &mut BTreeSet<(i32, i32)>,
    centre: &BTreeSet<(i32, i32)>,
    blocked: &dyn Fn((i32, i32)) -> bool,
) {
    let around: BTreeSet<(i32, i32)> = tiles
        .iter()
        .flat_map(|t| NEIGHBOURS.iter().map(move |(dx, dy)| (t.0 + dx, t.1 + dy)))
        .collect();
    let pits: Vec<_> = around
        .iter()
        .copied()
        .filter(|t| !tiles.contains(t) && !blocked(*t) && orthogonal(*t, tiles) >= 3)
        .collect();
    tiles.extend(pits);
    let spurs: Vec<_> = tiles
        .iter()
        .copied()
        .filter(|t| !centre.contains(t) && orthogonal(*t, tiles) <= 1)
        .collect();
    for spur in spurs {
        tiles.remove(&spur);
    }
}

/// The half-width of the stretch of the path nearest `point`.
fn half_width_near(point: (f32, f32), line: &[PathSample]) -> f32 {
    line.windows(2)
        .map(|pair| {
            let (a, b) = (pair[0], pair[1]);
            let (dx, dy) = (b.x - a.x, b.y - a.y);
            let length_squared = dx * dx + dy * dy;
            let t = if length_squared <= f32::EPSILON {
                0.0
            } else {
                (((point.0 - a.x) * dx + (point.1 - a.y) * dy) / length_squared).clamp(0.0, 1.0)
            };
            let nearest = (a.x + dx * t, a.y + dy * t);
            (distance(point, nearest), a.half + (b.half - a.half) * t)
        })
        .min_by(|l, r| l.0.total_cmp(&r.0))
        .map_or_else(|| line.first().map_or(1.0, |s| s.half), |(_, half)| half)
}
