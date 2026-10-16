use super::*;
use crate::terrain_path::smooth::{ControlPoint, smooth_path};

const OPEN: &dyn Fn((i32, i32)) -> bool = &|_| false;

fn line(points: &[(f32, f32)], width: f32) -> Vec<PathSample> {
    let controls: Vec<_> = points
        .iter()
        .map(|&(x, y)| ControlPoint::new(x, y, width))
        .collect();
    smooth_path(&controls)
}

fn options(prior_weight: f32) -> FitOptions<'static> {
    FitOptions {
        edge_weight: 0.75,
        prior_weight,
        blocked: OPEN,
    }
}

/// How a fit compares with the real map: the share of boundary tiles that are partial shapes, the
/// share of straight-edge tiles that are full tiles, and how often a tile is its neighbourhood's
/// most common choice in the census.
#[derive(Debug, Clone, Copy)]
struct Conformance {
    boundary_partial: f32,
    straight_edge_full: f32,
    modal: f32,
}

/// Masks of tiles on a straight edge: three path tiles along one side and the opposite side open
/// (N/E/S/W edge), as in the census (199, 124, 31, 241).
const STRAIGHT_EDGE_MASKS: [u8; 4] = [199, 124, 31, 241];

fn conformance(tiles: &[FittedTile]) -> Conformance {
    let set: std::collections::BTreeSet<_> = tiles.iter().map(|t| (t.tile_x, t.tile_y)).collect();
    let (mut boundary, mut partial) = (0.0_f32, 0.0_f32);
    let (mut straight, mut straight_full) = (0.0_f32, 0.0_f32);
    let (mut modal, mut total) = (0.0_f32, 0.0_f32);
    for tile in tiles {
        let key = (tile.tile_x, tile.tile_y);
        let mut mask = 0u8;
        for (bit, (dx, dy)) in NEIGHBOURS.iter().enumerate() {
            if set.contains(&(key.0 + dx, key.1 + dy)) {
                mask |= 1 << bit;
            }
        }
        let on_boundary = [(0, 1), (1, 0), (0, -1), (-1, 0)]
            .iter()
            .any(|(dx, dy)| !set.contains(&(key.0 + dx, key.1 + dy)));
        if on_boundary {
            boundary += 1.0;
            partial += f32::from(u8::from(tile.shape != 0));
        }
        if STRAIGHT_EDGE_MASKS.contains(&mask) {
            straight += 1.0;
            straight_full += f32::from(u8::from(tile.shape == 0));
        }
        let index = census::candidate_index(tile.shape, tile.orientation);
        total += 1.0;
        modal += f32::from(u8::from(census::penalty(mask)[index] < 0.05));
    }
    Conformance {
        boundary_partial: partial / boundary.max(1.0),
        straight_edge_full: straight_full / straight.max(1.0),
        modal: modal / total.max(1.0),
    }
}

/// Paths like the ones people draw: straight runs, gentle curves, bends, narrow to wide.
fn battery() -> Vec<(Vec<(f32, f32)>, f32)> {
    let mut paths = Vec::new();
    for width in [1.0, 3.0, 5.0] {
        paths.push((vec![(2.5, 10.5), (28.5, 10.5)], width));
        paths.push((vec![(10.5, 2.5), (10.5, 30.5)], width));
        paths.push((vec![(2.5, 3.5), (30.5, 21.5)], width));
        paths.push((
            vec![(2.0, 4.0), (10.0, 12.0), (18.0, 10.0), (28.0, 20.0)],
            width,
        ));
        paths.push((
            vec![(4.0, 4.0), (24.0, 6.0), (26.0, 24.0), (6.0, 26.0)],
            width,
        ));
    }
    paths
}

fn average(prior_weight: f32) -> Conformance {
    let mut sum = Conformance {
        boundary_partial: 0.0,
        straight_edge_full: 0.0,
        modal: 0.0,
    };
    let paths = battery();
    for (points, width) in &paths {
        let fit = fit_path_overlay(&line(points, *width), &options(prior_weight));
        let c = conformance(&fit.tiles);
        sum.boundary_partial += c.boundary_partial;
        sum.straight_edge_full += c.straight_edge_full;
        sum.modal += c.modal;
    }
    let n = paths.len() as f32;
    Conformance {
        boundary_partial: sum.boundary_partial / n,
        straight_edge_full: sum.straight_edge_full / n,
        modal: sum.modal / n,
    }
}

#[test]
fn candidate_set_is_empty_full_plus_all_shaped_rotations() {
    assert_eq!(candidates().len(), census::CANDIDATES);
    assert!(candidates()[0].shape.is_none());
    assert_eq!(candidates()[1].shape, Some(0));
    assert!(candidates()[1].covered.iter().all(|covered| *covered));
    for (index, candidate) in candidates().iter().enumerate().skip(1) {
        let (shape, orientation) = (candidate.shape.unwrap_or(0), candidate.orientation);
        assert_eq!(census::candidate_index(shape, orientation), index);
    }
}

#[test]
fn fit_is_deterministic_and_uses_only_valid_stored_shapes() {
    let path = line(&[(0.25, 0.5), (4.0, 2.0), (8.75, 2.75)], 2.5);
    let first = fit_path_overlay(&path, &options(2.0));
    assert_eq!(first, fit_path_overlay(&path, &options(2.0)));
    assert!(!first.tiles.is_empty());
    assert!(
        first
            .tiles
            .iter()
            .all(|t| t.shape <= 11 && t.orientation <= 3)
    );
}

#[test]
fn negative_world_coordinates_are_supported() {
    let fit = fit_path_overlay(&line(&[(-4.5, -2.25), (-0.25, -2.25)], 1.5), &options(2.0));
    assert!(
        fit.tiles
            .iter()
            .any(|tile| tile.tile_x < 0 && tile.tile_y < 0)
    );
}

#[test]
fn a_path_that_widens_covers_more_tiles_at_the_wide_end() {
    let controls = [
        ControlPoint::new(2.5, 10.5, 1.0),
        ControlPoint::new(30.5, 10.5, 7.0),
    ];
    let fit = fit_path_overlay(&smooth_path(&controls), &options(2.0));
    let count = |lo: i32, hi: i32| {
        fit.tiles
            .iter()
            .filter(|t| (lo..hi).contains(&t.tile_x))
            .count()
    };
    assert!(
        count(22, 31) > 2 * count(2, 11),
        "{} vs {}",
        count(22, 31),
        count(2, 11)
    );
}

#[test]
fn blocked_tiles_are_left_alone_and_reported() {
    let path = line(&[(2.5, 10.5), (20.5, 10.5)], 3.0);
    let open = fit_path_overlay(&path, &options(2.0));
    let wall = |tile: (i32, i32)| (10..=12).contains(&tile.0);
    let fit = fit_path_overlay(
        &path,
        &FitOptions {
            edge_weight: 0.75,
            prior_weight: 2.0,
            blocked: &wall,
        },
    );
    assert!(fit.tiles.iter().all(|t| !(10..=12).contains(&t.tile_x)));
    assert!(fit.clipped.iter().all(|t| (10..=12).contains(&t.0)));
    assert!(fit.clipped.len() >= 6, "{:?}", fit.clipped);
    // Everything outside the wall is the same path.
    let outside = |tiles: &[FittedTile]| {
        tiles
            .iter()
            .filter(|t| !(9..=13).contains(&t.tile_x))
            .count()
    };
    assert_eq!(outside(&fit.tiles), outside(&open.tiles));
}

#[test]
fn an_empty_line_fits_nothing() {
    assert_eq!(fit_path_overlay(&[], &options(2.0)), Fit::default());
}

/// Share of the ideal band's samples the fit reproduces, over the tiles the band touches.
fn coverage(line: &[PathSample], tiles: &[FittedTile]) -> f32 {
    let by_tile: std::collections::BTreeMap<_, _> =
        tiles.iter().map(|t| ((t.tile_x, t.tile_y), *t)).collect();
    let (mut agreed, mut sampled) = (0_u32, 0_u32);
    for tile in candidate_tiles(line) {
        let ideal = ideal_mask(tile, line);
        if !ideal.iter().any(|c| *c) {
            continue;
        }
        let index = by_tile
            .get(&tile)
            .map_or(0, |t| census::candidate_index(t.shape, t.orientation));
        agreed += agreement(&ideal, &candidates()[index].covered);
        sampled += SAMPLE_COUNT as u32;
    }
    agreed as f32 / sampled.max(1) as f32
}

fn average_coverage(prior_weight: f32) -> f32 {
    let paths = battery();
    let sum: f32 = paths
        .iter()
        .map(|(points, width)| {
            let path = line(points, *width);
            coverage(
                &path,
                &fit_path_overlay(&path, &options(prior_weight)).tiles,
            )
        })
        .sum();
    sum / paths.len() as f32
}

/// The real map's figures (docs/verification/PATH-CENSUS.md): 44.7% of boundary tiles are partial
/// shapes and about 77% of straight-edge tiles are full tiles. Occupancy-first fitting is cleaner
/// than the real map, so it uses fewer partial shapes (about 30%).
#[test]
fn the_census_prior_moves_the_fit_toward_how_the_real_map_tiles_paths() {
    let without = average(0.0);
    let with = average(PRIOR_WEIGHT);
    // Straight edges: more full tiles, close to the real 77%.
    assert!(
        with.straight_edge_full > without.straight_edge_full + 0.15,
        "{with:?} vs {without:?}"
    );
    assert!(with.straight_edge_full > 0.65, "{with:?}");
    // Boundaries: partial shapes where the outline turns, not along straight edges.
    assert!((0.20..=0.52).contains(&with.boundary_partial), "{with:?}");
    // Tiles are overwhelmingly the most common choice for their neighbourhood.
    assert!(
        with.modal > without.modal + 0.10 && with.modal > 0.85,
        "{with:?}"
    );
}

#[test]
fn following_the_census_costs_little_fidelity_to_the_drawn_band() {
    let without = average_coverage(0.0);
    let with = average_coverage(PRIOR_WEIGHT);
    assert!(without > 0.93, "{without}");
    assert!(with > 0.88 && without - with < 0.06, "{with} vs {without}");
}

#[test]
fn wide_paths_have_no_spurs_and_no_pits() {
    for width in [3.0_f32, 5.0] {
        for (points, w) in battery().iter().filter(|(_, w)| *w == width) {
            let fit = fit_path_overlay(&line(points, *w), &options(PRIOR_WEIGHT));
            assert_eq!(ragged(&fit.tiles), (0, 0), "{points:?} at width {w}");
        }
    }
}

/// The tile map of a five-wide road at an arbitrary angle: a regular staircase, five tiles across
/// (six on the rows where a small corner piece `b` steps out on each side), nothing else. `#` is a
/// full tile.
#[test]
fn a_wide_diagonal_road_is_a_regular_staircase() {
    let path = line(&[(10.5, 2.5), (22.5, 34.5)], 5.0);
    let map = ascii(&fit_path_overlay(&path, &options(PRIOR_WEIGHT)).tiles);
    let widths: Vec<usize> = map
        .lines()
        .map(|row| row.chars().filter(|c| *c != '.').count())
        .collect();
    // Interior rows are five tiles wide, or six where corner pieces step out (the ends taper).
    assert!(
        widths[3..widths.len() - 3]
            .iter()
            .all(|w| (5..=6).contains(w)),
        "{widths:?}\n{map}"
    );
    // Every non-full tile is the small corner piece.
    assert!(
        map.chars().all(|c| matches!(c, '.' | '#' | 'b' | '\n')),
        "{map}"
    );
}

#[test]
fn narrow_trails_keep_their_length_instead_of_dissolving() {
    // A one-tile trail is the most common real path; the prior must not eat it.
    let path = line(&[(2.5, 10.5), (28.5, 10.5)], 1.0);
    let fit = fit_path_overlay(&path, &options(PRIOR_WEIGHT));
    let columns: std::collections::BTreeSet<_> = fit.tiles.iter().map(|t| t.tile_x).collect();
    assert!((2..=28).all(|x| columns.contains(&x)), "{columns:?}");
}

/// Ragged-edge measures over a fit: path tiles that are lonely spurs (at most one orthogonal path
/// neighbour) and empty tiles that are pits (at least three). A clean edge has neither.
fn ragged(tiles: &[FittedTile]) -> (usize, usize) {
    let set: std::collections::BTreeSet<_> = tiles.iter().map(|t| (t.tile_x, t.tile_y)).collect();
    let around = |(x, y): (i32, i32)| {
        [(0, 1), (1, 0), (0, -1), (-1, 0)]
            .iter()
            .filter(|(dx, dy)| set.contains(&(x + dx, y + dy)))
            .count()
    };
    let spurs = set.iter().filter(|t| around(**t) <= 1).count();
    let (min_x, max_x) = (
        set.iter().map(|t| t.0).min().unwrap_or(0),
        set.iter().map(|t| t.0).max().unwrap_or(0),
    );
    let (min_y, max_y) = (
        set.iter().map(|t| t.1).min().unwrap_or(0),
        set.iter().map(|t| t.1).max().unwrap_or(0),
    );
    let mut pits = 0;
    for x in min_x..=max_x {
        for y in min_y..=max_y {
            if !set.contains(&(x, y)) && around((x, y)) >= 3 {
                pits += 1;
            }
        }
    }
    (spurs, pits)
}

fn ascii(tiles: &[FittedTile]) -> String {
    let map: std::collections::BTreeMap<_, _> =
        tiles.iter().map(|t| ((t.tile_x, t.tile_y), *t)).collect();
    let (min_x, max_x) = (
        map.keys().map(|t| t.0).min().unwrap_or(0),
        map.keys().map(|t| t.0).max().unwrap_or(0),
    );
    let (min_y, max_y) = (
        map.keys().map(|t| t.1).min().unwrap_or(0),
        map.keys().map(|t| t.1).max().unwrap_or(0),
    );
    let mut out = String::new();
    for y in (min_y..=max_y).rev() {
        for x in min_x..=max_x {
            out.push(match map.get(&(x, y)) {
                None => '.',
                Some(t) if t.shape == 0 => '#',
                Some(t) => char::from(b'a' + t.shape),
            });
        }
        out.push('\n');
    }
    out
}
