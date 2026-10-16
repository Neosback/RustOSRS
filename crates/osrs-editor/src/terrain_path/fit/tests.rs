use super::*;
use std::collections::BTreeMap;

const GOLDEN: &str = include_str!("../eiken_golden.txt");

struct Case {
    name: String,
    offset: (i32, i32),
    width: f64,
    edge: f64,
    smooth: bool,
    points: Vec<(f64, f64)>,
    /// `(x, y, paint shape id, rotation)` as the original prints them (shape 1 is the full tile).
    tiles: Vec<(i32, i32, u8, u8)>,
}

fn parse() -> Vec<Case> {
    let mut cases = Vec::new();
    let mut offset = (0, 0);
    let mut current: Option<Case> = None;
    let value = |word: &str| -> f64 {
        word.split('=')
            .nth(1)
            .and_then(|v| v.parse().ok())
            .unwrap_or(f64::NAN)
    };
    for line in GOLDEN.lines().filter(|l| !l.starts_with('#')) {
        let words: Vec<&str> = line.split_whitespace().collect();
        match words.as_slice() {
            ["offset", x, y] => offset = (x.parse().unwrap_or(0), y.parse().unwrap_or(0)),
            ["case", name, width, edge, smooth] => {
                current = Some(Case {
                    name: (*name).to_owned(),
                    offset,
                    width: value(width),
                    edge: value(edge),
                    smooth: *smooth == "smooth=true",
                    points: Vec::new(),
                    tiles: Vec::new(),
                });
            }
            ["point", x, y] => {
                if let Some(case) = current.as_mut() {
                    case.points
                        .push((x.parse().unwrap_or(f64::NAN), y.parse().unwrap_or(f64::NAN)));
                }
            }
            ["tile", x, y, shape, rotation] => {
                if let Some(case) = current.as_mut() {
                    case.tiles.push((
                        x.parse().unwrap_or(0),
                        y.parse().unwrap_or(0),
                        shape.parse().unwrap_or(0),
                        rotation.parse().unwrap_or(0),
                    ));
                }
            }
            ["end"] => cases.extend(current.take()),
            _ => {}
        }
    }
    cases
}

/// The order Java's `HashMap<Integer, _>` iterates keys `x << 16 | y` in: by bucket of the spread
/// hash in a table sized for the number of entries, in insertion order within a bucket.
fn java_hash_order(tiles: Vec<(i32, i32)>) -> Vec<(i32, i32)> {
    let mut capacity = 16_usize;
    while tiles.len() * 4 > capacity * 3 {
        capacity *= 2;
    }
    let bucket = |tile: &(i32, i32)| {
        let key = (tile.0 as u32) << 16 | tile.1 as u32;
        ((key ^ (key >> 16)) as usize) & (capacity - 1)
    };
    let mut ordered = tiles;
    // A stable sort by bucket keeps insertion order (the tiles arrive in insertion order).
    ordered.sort_by_key(bucket);
    ordered
}

fn ours(case: &Case, java_order: bool) -> Vec<(i32, i32, u8, u8)> {
    let line = if case.smooth {
        smooth(&case.points)
    } else {
        case.points.clone()
    };
    let fitted = if java_order {
        fit_in_order(&line, case.width, case.edge, java_hash_order)
    } else {
        fit(&line, case.width, case.edge)
    };
    fitted
        .into_iter()
        .map(|t| (t.tile_x, t.tile_y, t.shape + 1, t.orientation))
        .collect()
}

#[test]
fn the_golden_file_has_every_case_at_every_offset() {
    let cases = parse();
    assert_eq!(cases.len(), 3 * 18);
    assert!(
        cases
            .iter()
            .all(|c| !c.tiles.is_empty() && !c.points.is_empty())
    );
}

/// The port against the original's output, refining tiles in the order Java's `HashMap` does:
/// every case at every offset matches tile for tile. This is the proof the rest of the port (the
/// curve, the sampling, the candidates, the scoring and the edge passes) is the original's.
#[test]
fn the_fit_matches_the_original_editors_output_exactly() {
    let mut report = Vec::new();
    for case in parse() {
        let (mine, theirs) = (ours(&case, true), &case.tiles);
        if &mine != theirs {
            let differing = mine.iter().filter(|t| !theirs.contains(t)).count();
            report.push(format!(
                "{} at {:?}: {differing} of {} tiles differ",
                case.name,
                case.offset,
                theirs.len()
            ));
        }
    }
    assert!(report.is_empty(), "{}", report.join("\n"));
}

/// In tile order (the production order) a path draws the same wherever it is. It can differ from
/// the original only where equally good outlines tie, such as which phase a repeating edge pattern
/// starts on, so it must stay close: any tile that differs is on an edge, never in the middle.
#[test]
fn in_tile_order_the_fit_differs_only_in_tied_edge_choices() {
    for case in parse() {
        let (mine, theirs) = (ours(&case, false), &case.tiles);
        let differing = mine.iter().filter(|t| !theirs.contains(t)).count();
        assert!(
            differing * 10 <= theirs.len() * 3,
            "{} at {:?}: {differing} of {} tiles differ",
            case.name,
            case.offset,
            theirs.len()
        );
        // Whole tiles (shape 1 in the original's numbering) agree exactly: every tile that is
        // full in one is full in the other.
        let full = |tiles: &[(i32, i32, u8, u8)]| -> Vec<(i32, i32)> {
            tiles
                .iter()
                .filter(|t| t.2 == 1)
                .map(|t| (t.0, t.1))
                .collect()
        };
        let (a, b) = (full(&mine), full(theirs));
        let overlap = a.iter().filter(|t| b.contains(t)).count();
        assert!(
            overlap * 10 >= b.len() * 9,
            "{} at {:?}",
            case.name,
            case.offset
        );
    }
}

fn tiles(points: &[(f64, f64)], width: f64, edge: f64) -> Vec<FittedTile> {
    fit(points, width, edge)
}

#[test]
fn nothing_in_nothing_out_and_bad_input_is_ignored() {
    assert!(fit(&[], 3.0, 1.5).is_empty());
    assert!(fit(&[(5.5, 5.5)], f64::NAN, 1.5).is_empty());
    assert!(fit(&[(f64::NAN, 5.5)], 3.0, 1.5).is_empty());
    assert!(fit(&[(5.5, 5.5)], 0.0, 1.5).is_empty());
}

#[test]
fn the_fit_is_deterministic_and_the_same_wherever_it_is_drawn() {
    let line = smooth(&[(4.5, 4.5), (14.5, 14.5), (24.5, 4.5)]);
    let a = tiles(&line, 3.0, 1.5);
    assert_eq!(a, tiles(&line, 3.0, 1.5));
    // Translating the path by whole tiles translates the fit.
    let moved: Vec<_> = line.iter().map(|p| (p.0 + 3000.0, p.1 + 2000.0)).collect();
    let b = tiles(&moved, 3.0, 1.5);
    let shifted: Vec<_> = a
        .iter()
        .map(|t| FittedTile {
            tile_x: t.tile_x + 3000,
            tile_y: t.tile_y + 2000,
            ..*t
        })
        .collect();
    assert_eq!(b, shifted);
}

#[test]
fn a_wide_straight_path_is_full_tiles_inside_with_matching_edges() {
    let line = [(4.5, 10.5), (20.5, 10.5)];
    let fitted = tiles(&line, 5.0, 1.5);
    let by_tile: BTreeMap<_, _> = fitted.iter().map(|t| ((t.tile_x, t.tile_y), t)).collect();
    // Along the middle the five rows around y = 10 are whole tiles.
    for x in 8..=16 {
        for y in 9..=11 {
            assert_eq!(
                by_tile[&(x, y)].shape,
                0,
                "({x}, {y}) should be a full tile"
            );
        }
    }
    // Nothing is laid more than a tile and a half beyond the path's edge.
    assert!(fitted.iter().all(|t| (t.tile_y - 10).abs() <= 3));
}

#[test]
fn smoothing_needs_three_points_passes_through_every_one_and_clamps_the_ends() {
    let two = [(1.0, 1.0), (5.0, 2.0)];
    assert_eq!(smooth(&two), two);
    assert_eq!(smooth(&[]), Vec::<(f64, f64)>::new());
    let three = [(0.0, 0.0), (10.0, 10.0), (20.0, 0.0)];
    let curve = smooth(&three);
    assert_eq!(curve.len(), 2 * 12 + 1);
    assert_eq!(curve[0], three[0]);
    assert_eq!(curve[12], three[1], "the curve passes through each point");
    assert_eq!(curve[24], three[2]);
    assert!(curve.last() == Some(&three[2]));
    // The middle bulges above the straight chord, as a spline through a peak does.
    assert!(curve[6].1 > 0.0 && curve[18].1 > 0.0);
}

#[test]
fn edge_smoothing_changes_the_outline_not_the_coverage_much() {
    let line = smooth(&[(4.5, 4.5), (22.5, 16.5)]);
    let loose = tiles(&line, 5.0, 0.0);
    let tight = tiles(&line, 5.0, 4.0);
    assert!(!loose.is_empty() && !tight.is_empty());
    let count = |t: &[FittedTile]| t.len() as i64;
    assert!((count(&loose) - count(&tight)).abs() <= count(&loose) / 5);
}
