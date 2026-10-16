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
    let (mut boundary, mut partial) = (0.0, 0.0);
    let (mut straight, mut straight_full) = (0.0, 0.0);
    let (mut modal, mut total) = (0.0, 0.0);
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

#[test]
fn print_conformance_for_tuning() {
    for weight in [0.0, 1.0, 2.0, 3.0, 4.0, 6.0] {
        eprintln!("prior {weight}: {:?}", average(weight));
    }
}
