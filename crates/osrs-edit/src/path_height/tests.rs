use super::*;
use crate::MapDocument;
use std::collections::BTreeMap;

type Heights = BTreeMap<(i32, i32), i32>;

fn field(size: i32, f: impl Fn(i32, i32) -> i32) -> Heights {
    let mut map = Heights::new();
    for x in -size..=size {
        for y in -size..=size {
            map.insert((x, y), f(x, y));
        }
    }
    map
}

fn spec(passes: u32, blend: f64) -> PathHeightSpec {
    PathHeightSpec {
        width: 3.0,
        passes,
        blend,
    }
}

fn plan(line: &[(f64, f64)], spec: PathHeightSpec, heights: &Heights) -> Heights {
    plan_path_height_smoothing(line, spec, &|v| heights.get(&v).copied())
        .into_iter()
        .collect()
}

const ACROSS: [(f64, f64); 2] = [(-10.0, 0.5), (10.0, 0.5)];

#[test]
fn the_reach_is_half_the_width_plus_one_and_a_half_plus_the_blend() {
    let vertices = smoothing_vertices(&[(0.5, 0.5)], spec(1, 2.0));
    // reach = 1.5 + 1.5 + 2 = 5 tiles around (0.5, 0.5).
    assert!(vertices.iter().all(|&(x, y)| {
        ((f64::from(x) - 0.5).powi(2) + (f64::from(y) - 0.5).powi(2)).sqrt() <= 5.0
    }));
    assert!(vertices.contains(&(0, 0)) && vertices.contains(&(5, 0)));
    assert!(!vertices.contains(&(6, 0)) && !vertices.contains(&(0, 6)));
    assert!(smoothing_vertices(&[], spec(1, 2.0)).is_empty());
}

#[test]
fn flat_ground_and_zero_passes_change_nothing() {
    let flat = field(30, |_, _| -64);
    assert!(plan(&ACROSS, spec(5, 4.0), &flat).is_empty());
    let bumpy = field(30, |x, _| if x % 4 == 0 { -96 } else { 0 });
    assert!(plan(&ACROSS, spec(0, 4.0), &bumpy).is_empty());
}

#[test]
fn smoothing_evens_out_bumps_only_within_the_reach() {
    let bumpy = field(30, |x, _| if x % 4 == 0 { -96 } else { 0 });
    let result = plan(&ACROSS, spec(5, 0.0), &bumpy);
    assert!(!result.is_empty());
    let range = |h: &Heights, xs: std::ops::RangeInclusive<i32>| {
        let values: Vec<i32> = (xs)
            .flat_map(|x| (0..=1).map(move |y| h[&(x, y)]))
            .collect();
        values.iter().max().copied().unwrap_or(0) - values.iter().min().copied().unwrap_or(0)
    };
    let mut after = bumpy.clone();
    after.extend(result.iter().map(|(k, v)| (*k, *v)));
    assert!(
        range(&after, -6..=6) < range(&bumpy, -6..=6),
        "bumps are flattened"
    );
    // Nothing beyond the reach (10 + 3 tiles along, 3 across) is touched.
    assert!(
        result.keys().all(|&(x, y)| x.abs() <= 13 && y.abs() <= 3),
        "{result:?}"
    );
    // The lowest plane never goes below sea level (positive heights).
    assert!(result.values().all(|&h| h <= 0));
}

#[test]
fn more_passes_smooth_more_and_a_wider_blend_reaches_further() {
    let bumpy = field(40, |x, _| if x % 4 == 0 { -96 } else { 0 });
    let size = |r: &Heights| r.values().map(|v| i64::from(v.abs())).sum::<i64>();
    let few = plan(&ACROSS, spec(1, 0.0), &bumpy);
    let many = plan(&ACROSS, spec(40, 0.0), &bumpy);
    assert!(many.len() >= few.len());
    // Vertices change in both, but 40 passes pull them further toward their neighbours.
    let distance =
        |r: &Heights| -> i64 { r.iter().map(|(v, h)| i64::from((h - bumpy[v]).abs())).sum() };
    assert!(
        distance(&many) > distance(&few),
        "{} vs {}",
        distance(&many),
        distance(&few)
    );
    assert!(size(&many) > 0);
    let narrow = plan(&ACROSS, spec(5, 0.0), &bumpy);
    let wide = plan(&ACROSS, spec(5, 8.0), &bumpy);
    assert!(
        wide.keys().any(|&(_, y)| y.abs() > 3),
        "the blend zone is reached"
    );
    assert!(narrow.keys().all(|&(_, y)| y.abs() <= 3));
}

#[test]
fn unknown_vertices_are_left_alone_and_not_counted_as_neighbours() {
    let mut bumpy = field(30, |x, _| if x % 4 == 0 { -96 } else { 0 });
    for x in -3..=3 {
        for y in -3..=3 {
            bumpy.remove(&(x, y));
        }
    }
    let result = plan(&ACROSS, spec(3, 0.0), &bumpy);
    assert!(result.keys().all(|v| bumpy.contains_key(v)));
    // The same line with nothing loaded plans nothing.
    assert!(plan(&ACROSS, spec(3, 0.0), &Heights::new()).is_empty());
}

#[test]
fn the_plan_is_deterministic_and_sorted() {
    let bumpy = field(30, |x, y| if (x + y) % 3 == 0 { -64 } else { 0 });
    let a = plan_path_height_smoothing(&ACROSS, spec(7, 2.0), &|v| bumpy.get(&v).copied());
    let b = plan_path_height_smoothing(&ACROSS, spec(7, 2.0), &|v| bumpy.get(&v).copied());
    assert_eq!(a, b);
    assert!(a.windows(2).all(|w| w[0].0 < w[1].0));
}

fn plane(value: u8) -> SourcePlane {
    SourcePlane::new(value).unwrap_or_else(|| unreachable!("valid plane"))
}

fn vertex(tile: (i32, i32), plane_index: u8) -> HeightVertexCoord {
    HeightVertexCoord::new(MapTile::new(tile.0, tile.1), plane(plane_index))
}

#[test]
fn the_patch_quantises_carries_upper_planes_and_undoes_exactly() {
    let tile = (3200, 3200);
    let mut document = MapDocument::from_parts(
        [],
        [
            (vertex(tile, 0), -40),
            (vertex(tile, 1), -240),
            (vertex(tile, 2), -400),
        ],
        [],
    );
    let original = document.clone();
    // Ask for -52: the map file stores multiples of eight.
    let patch = build_path_height_patch(&document, plane(0), &[(tile, -52)])
        .unwrap_or_else(|error| unreachable!("valid patch: {error:?}"));
    assert!(document.apply_patch(&patch).is_ok());
    let after = document.height_at(vertex(tile, 0));
    assert!(matches!(after, Some(-56 | -48)), "{after:?}");
    let shift = after.unwrap_or(0) + 40;
    // The planes above kept their offset from the plane below.
    assert_eq!(document.height_at(vertex(tile, 1)), Some(-240 + shift));
    assert_eq!(document.height_at(vertex(tile, 2)), Some(-400 + shift));
    assert!(document.apply_patch(&patch.inverse()).is_ok());
    assert_eq!(document, original);
}

#[test]
fn upper_planes_never_end_up_above_the_plane_below() {
    let tile = (3200, 3200);
    // Plane 1 is only 8 units above plane 0; lowering plane 0 by 40 would carry plane 1 above... no:
    // raising plane 0's height (toward zero is lower) pulls plane 1 down with it, clamped.
    let mut document =
        MapDocument::from_parts([], [(vertex(tile, 0), -80), (vertex(tile, 1), -88)], []);
    let patch = build_path_height_patch(&document, plane(0), &[(tile, -16)])
        .unwrap_or_else(|error| unreachable!("valid patch: {error:?}"));
    assert!(document.apply_patch(&patch).is_ok());
    let (lower, upper) = (
        document.height_at(vertex(tile, 0)),
        document.height_at(vertex(tile, 1)),
    );
    assert!(upper <= lower, "{upper:?} above {lower:?}");
}

#[test]
fn a_vertex_that_is_not_loaded_is_an_error_not_a_guess() {
    let document = MapDocument::new();
    assert_eq!(
        build_path_height_patch(&document, plane(0), &[((1, 1), -8)]),
        Err(PathCommitError::MissingHeight {
            tile: MapTile::new(1, 1)
        })
    );
}

#[test]
fn an_unchanged_height_makes_no_operation() {
    let tile = (3200, 3200);
    let document = MapDocument::from_parts([], [(vertex(tile, 0), -40)], []);
    let patch = build_path_height_patch(&document, plane(0), &[(tile, -40)])
        .unwrap_or_else(|error| unreachable!("valid patch: {error:?}"));
    assert!(patch.is_empty());
}
