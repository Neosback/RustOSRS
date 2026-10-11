//! The auto-tiler playground: many generated paths through the same code the editor runs, scored
//! against the ideal band, drawn as a contact sheet, and swept over the search weights.
//!
//! ```text
//! cargo test --release -p osrs-editor playground_sheet -- --ignored --nocapture
//! cargo test --release -p osrs-editor playground_sweep -- --ignored --nocapture
//! ```
//!
//! Sheets go to `/tmp/playground-*.png` (cell `n` is case `n`, row-major; the red line is the ideal
//! band's outline, the fill is what the tiler made). The score is measured on the same 16 x 16
//! sub-tile cells the tiler fits, so it cannot flatter the tiler by looking at something else:
//!
//! * `area`: wrongly covered or uncovered area, as a share of the ideal band's area;
//! * `wobble`: how much the made outline's offset from the ideal outline changes from sample to
//!   sample (RMS second difference, samples half a tile apart along both sides): zero for any edge
//!   that is straight relative to the ideal, large for stairs, notches and ears (this is what
//!   "jagged" measures);
//! * `blobs`: separate error blobs per tile of path (reported, not ranked: it barely moves);
//! * `max` / `mean`: how far (tiles) an error cell lies from the ideal outline.

use super::*;
use std::collections::{HashSet, VecDeque};

const AREA_TILES: i32 = 56;
const SHEET_PX: i32 = 10;

pub struct Case {
    pub points: Vec<(f32, f32)>,
    pub params: PathParams,
}

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 40) as f32 / (1u64 << 24) as f32
    }

    fn between(&mut self, low: f32, high: f32) -> f32 {
        low + (high - low) * self.next()
    }
}

/// `count` varied paths: widths from a pixel-thin track to a wide road, two to five control points
/// at random angles (straight runs, bends, S-curves), curves and polylines, a few closed loops.
pub fn cases(count: usize, seed: u64) -> Vec<Case> {
    let mut rng = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
    let widths = [1.0, 1.5, 2.0, 3.0, 3.0, 4.0, 5.0, 7.0];
    (0..count)
        .map(|n| {
            let width = widths[n % widths.len()];
            let closed = n % 11 == 5;
            let count = if closed {
                4 + (rng.next() * 2.0) as usize
            } else {
                2 + (rng.next() * 4.0) as usize
            };
            let mut points = Vec::new();
            if closed {
                // A rough ring.
                let radius = rng.between(7.0, 11.0);
                let centre = (AREA_TILES as f32 / 2.0, AREA_TILES as f32 / 2.0);
                for i in 0..count {
                    let angle = i as f32 / count as f32 * std::f32::consts::TAU + rng.next();
                    let r = radius * rng.between(0.75, 1.15);
                    points.push((centre.0 + r * angle.cos(), centre.1 + r * angle.sin()));
                }
            } else {
                let mut at = (rng.between(4.0, 10.0), rng.between(4.0, 32.0));
                let mut heading = rng.between(-1.2, 1.2);
                points.push(at);
                for _ in 1..count {
                    heading += rng.between(-1.1, 1.1);
                    let length = rng.between(7.0, 14.0);
                    at = (
                        (at.0 + length * heading.cos()).clamp(3.0, AREA_TILES as f32 - 3.0),
                        (at.1 + length * heading.sin()).clamp(3.0, AREA_TILES as f32 - 3.0),
                    );
                    points.push(at);
                }
            }
            Case {
                points,
                params: PathParams {
                    width,
                    style: EdgeStyle::Smooth,
                    curve: if n % 7 == 3 {
                        CurveKind::Lines
                    } else {
                        CurveKind::Curve
                    },
                    closed,
                    ..PathParams::default()
                },
            }
        })
        .collect()
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Score {
    pub area: f32,
    pub wobble: f32,
    pub max: f32,
    pub mean: f32,
    /// Furthest a control point was nudged (tiles).
    pub shift: f32,
    pub tiles: usize,
    pub millis: f32,
}

impl Score {
    /// One number to rank by (lower is better): area error, wobble and the worst miss, weighted so
    /// a visible spike (max) and a wobbling edge (blobs) both outweigh a little area.
    pub fn total(&self) -> f32 {
        self.area * 2.0 + self.wobble * 8.0 + self.max * 1.5 + self.mean * 3.0 + self.shift * 0.4
    }
}

fn result_masks(result: &[((i32, i32), TileChoice)]) -> HashMap<(i32, i32), Mask> {
    result
        .iter()
        .filter_map(|(tile, choice)| {
            let mask = match choice {
                TileChoice::Keep => return None,
                TileChoice::Full => FULL,
                TileChoice::Shape { shape, rotation } => shape_mask(*shape, *rotation),
            };
            Some((*tile, mask))
        })
        .collect()
}

pub fn evaluate(
    case: &Case,
    tuning: &Tuning,
    refined: bool,
) -> (Score, Vec<((i32, i32), TileChoice)>) {
    let none = |_: (i32, i32)| None;
    let started = std::time::Instant::now();
    let (points, params, result) = if refined {
        generate_refined(&case.points, case.params, &none, tuning)
    } else {
        (
            case.points.clone(),
            case.params,
            generate_with(&case.points, case.params, &none, tuning),
        )
    };
    let millis = started.elapsed().as_secs_f32() * 1000.0;
    // Quality is judged against the path that was actually tiled (after sliding and nudging); how
    // far the points moved is reported separately.
    let quality = measure(&points, params, &result);
    let shift = {
        let tiled = path_points(&points, params);
        let points_moved = tiled
            .iter()
            .zip(&case.points)
            .map(|(a, b)| dist(*a, *b))
            .fold(0.0_f32, f32::max);
        points_moved
    };
    (
        Score {
            area: quality.area,
            wobble: quality.wobble,
            max: quality.max,
            mean: quality.mean,
            shift,
            tiles: result.len(),
            millis,
        },
        result,
    )
}

/// The control points the tiler actually used (after clean-direction nudging).
fn path_points(points: &[(f32, f32)], params: PathParams) -> Vec<(f32, f32)> {
    if params.snap > 0.0 && !params.closed && points.len() >= 2 {
        snap_points(points, params.snap, params.curve)
    } else {
        points.to_vec()
    }
}

pub fn mean(scores: &[Score]) -> Score {
    let n = scores.len().max(1) as f32;
    let sum = |f: fn(&Score) -> f32| scores.iter().map(f).sum::<f32>() / n;
    Score {
        area: sum(|s| s.area),
        wobble: sum(|s| s.wobble),
        max: sum(|s| s.max),
        mean: sum(|s| s.mean),
        shift: sum(|s| s.shift),
        tiles: (scores.iter().map(|s| s.tiles).sum::<usize>() as f32 / n) as usize,
        millis: sum(|s| s.millis),
    }
}

/// Draw every case's result into a contact sheet.
pub fn sheet(
    cases: &[Case],
    results: &[Vec<((i32, i32), TileChoice)>],
    columns: usize,
) -> image::RgbaImage {
    let cell = AREA_TILES * SHEET_PX;
    let rows = cases.len().div_ceil(columns) as i32;
    let mut img = image::RgbaImage::from_pixel(
        (columns as i32 * cell) as u32,
        (rows * cell) as u32,
        image::Rgba([70, 110, 50, 255]),
    );
    for (n, (case, result)) in cases.iter().zip(results).enumerate() {
        let (ox, oy) = ((n % columns) as i32 * cell, (n / columns) as i32 * cell);
        let masks = result_masks(result);
        let half = (case.params.width / 2.0).max(0.5);
        let line = path_polyline(&case.points, case.params, 0.25);
        let index = SegmentIndex::new(&line, half + 0.1);
        for py in 0..cell {
            for px in 0..cell {
                let wx = (px as f32 + 0.5) / SHEET_PX as f32;
                let wy = AREA_TILES as f32 - (py as f32 + 0.5) / SHEET_PX as f32;
                let tile = (wx.floor() as i32, wy.floor() as i32);
                let (fx, fy) = (wx - tile.0 as f32, wy - tile.1 as f32);
                let inside = masks.get(&tile).is_some_and(|mask| {
                    get(
                        mask,
                        ((fx * GRID as f32) as usize).min(GRID - 1),
                        ((fy * GRID as f32) as usize).min(GRID - 1),
                    )
                });
                let colour = if inside {
                    image::Rgba([196, 166, 96, 255])
                } else {
                    image::Rgba([70, 110, 50, 255])
                };
                img.put_pixel((ox + px) as u32, (oy + py) as u32, colour);
                let d = index.distance(tile, (wx, wy));
                if (d - half).abs() < 0.6 / SHEET_PX as f32 {
                    img.put_pixel(
                        (ox + px) as u32,
                        (oy + py) as u32,
                        image::Rgba([225, 50, 50, 255]),
                    );
                }
                if px == 0 || py == 0 {
                    img.put_pixel(
                        (ox + px) as u32,
                        (oy + py) as u32,
                        image::Rgba([20, 20, 20, 255]),
                    );
                }
            }
        }
    }
    img
}

fn run_all(cases: &[Case], tuning: &Tuning) -> (Vec<Score>, Vec<Vec<((i32, i32), TileChoice)>>) {
    run_all_with(cases, tuning, false)
}

fn run_all_with(
    cases: &[Case],
    tuning: &Tuning,
    refined: bool,
) -> (Vec<Score>, Vec<Vec<((i32, i32), TileChoice)>>) {
    let mut scores = Vec::new();
    let mut results = Vec::new();
    for case in cases {
        let (score, result) = evaluate(case, tuning, refined);
        scores.push(score);
        results.push(result);
    }
    (scores, results)
}

fn print_row(label: &str, s: &Score) {
    println!(
        "{label:<46} total {:6.3}  area {:5.3}  wobble {:5.3}  max {:5.3}  mean {:5.3}  moved {:4.2}  {:5.0} tiles  {:6.1} ms",
        s.total(),
        s.area,
        s.wobble,
        s.max,
        s.mean,
        s.shift,
        s.tiles,
        s.millis
    );
}

#[test]
#[ignore = "writes a contact sheet to /tmp"]
fn playground_sheet() {
    let cases = cases(48, 7);
    let (scores, results) = run_all(&cases, &Tuning::default());
    print_row("default tuning, 48 cases", &mean(&scores));
    for (n, s) in scores.iter().enumerate() {
        print_row(&format!("case {n:2} width {}", cases[n].params.width), s);
    }
    let _ = sheet(&cases, &results, 8).save("/tmp/playground-default.png");
}

#[test]
#[ignore = "sweeps the search weights (a few minutes)"]
fn playground_sweep() {
    let cases = cases(40, 11);
    let mut configs = Vec::new();
    for geometry in [60.0, 120.0] {
        for tolerance in [0.15, 0.2, 0.25, 0.3] {
            for pull in [0.04, 0.12, 0.3] {
                for portal in [1.5, 3.0] {
                    for smooth in [0.0, 15.0] {
                        configs.push(Tuning {
                            geometry,
                            portal,
                            smooth,
                            tolerance,
                            pull,
                            sweeps: 300,
                            ..Tuning::default()
                        });
                    }
                }
            }
        }
    }
    let threads = std::thread::available_parallelism().map_or(4, usize::from);
    let chunk = configs.len().div_ceil(threads);
    let mut ranked: Vec<(Tuning, Score)> = std::thread::scope(|scope| {
        let handles: Vec<_> = configs
            .chunks(chunk)
            .map(|part| {
                let cases = &cases;
                scope.spawn(move || {
                    part.iter()
                        .map(|tuning| {
                            let (scores, _) = run_all(cases, tuning);
                            (*tuning, mean(&scores))
                        })
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        handles
            .into_iter()
            .flat_map(|h| h.join().unwrap_or_default())
            .collect()
    });
    ranked.sort_by(|a, b| a.1.total().total_cmp(&b.1.total()));
    let describe = |t: &Tuning| {
        format!(
            "G {:4.0} P {:3.1} S {:4.0} tol {:4.2} pull {:4.2}",
            t.geometry, t.portal, t.smooth, t.tolerance, t.pull
        )
    };
    println!("best ten of {}:", ranked.len());
    for (tuning, score) in ranked.iter().take(10) {
        print_row(&describe(tuning), score);
    }
    println!("worst three:");
    for (tuning, score) in ranked.iter().rev().take(3) {
        print_row(&describe(tuning), score);
    }
    let default = Tuning::default();
    let (scores, results) = run_all(&cases, &default);
    print_row(
        &format!("{} (current default)", describe(&default)),
        &mean(&scores),
    );
    let _ = sheet(&cases, &results, 8).save("/tmp/playground-sweep-default.png");
    let (_, results) = run_all(&cases, &ranked[0].0);
    let _ = sheet(&cases, &results, 8).save("/tmp/playground-sweep-best.png");
}

#[test]
#[ignore = "compares how far control points may be nudged"]
fn playground_snap() {
    let base = cases(48, 21);
    for snap in [0.0_f32, 0.4, 0.75, 1.0, 1.5, 2.5] {
        let with: Vec<Case> = base
            .iter()
            .map(|case| Case {
                points: case.points.clone(),
                params: PathParams {
                    snap,
                    ..case.params
                },
            })
            .collect();
        let (scores, results) = run_all(&with, &Tuning::default());
        print_row(&format!("snap {snap}"), &mean(&scores));
        let _ = sheet(&with, &results, 8).save(format!("/tmp/playground-snap-{snap}.png"));
    }
}

#[test]
#[ignore = "straight runs at every angle, with and without nudging"]
fn playground_straight() {
    let mut rng = Rng(0xABCDEF);
    let mut base = Vec::new();
    for n in 0..48 {
        let angle = n as f32 / 48.0 * std::f32::consts::FRAC_PI_2 * 1.0;
        let length = rng.between(14.0, 24.0);
        let start = (6.0 + rng.next(), 6.0 + rng.next());
        base.push(Case {
            points: vec![
                start,
                (
                    start.0 + length * angle.cos(),
                    start.1 + length * angle.sin(),
                ),
            ],
            params: PathParams {
                width: [1.5, 2.0, 3.0, 4.0, 5.0][n % 5],
                curve: CurveKind::Lines,
                ..PathParams::default()
            },
        });
    }
    for case in &base {
        let (score, _) = evaluate(case, &Tuning::default(), false);
        let d = (
            case.points[1].0 - case.points[0].0,
            case.points[1].1 - case.points[0].1,
        );
        println!(
            "angle {:5.1} width {}: wobble {:5.3} max {:5.3} area {:5.3}",
            d.1.atan2(d.0).to_degrees(),
            case.params.width,
            score.wobble,
            score.max,
            score.area
        );
    }
    for snap in [0.0_f32, 1.0, 2.0] {
        let with: Vec<Case> = base
            .iter()
            .map(|case| Case {
                points: case.points.clone(),
                params: PathParams {
                    snap,
                    ..case.params
                },
            })
            .collect();
        let (scores, results) = run_all(&with, &Tuning::default());
        print_row(&format!("straight, snap {snap}"), &mean(&scores));
        let _ = sheet(&with, &results, 8).save(format!("/tmp/playground-straight-{snap}.png"));
    }
}

#[test]
#[ignore = "how much the line's position against the tile lattice matters"]
fn playground_phase() {
    for (name, angle) in [
        ("0 deg", 0.0_f32),
        ("26.6 deg", 0.5_f32.atan()),
        ("45 deg", std::f32::consts::FRAC_PI_4),
        ("63.4 deg", 2.0_f32.atan()),
        ("90 deg", std::f32::consts::FRAC_PI_2),
        ("35 deg (not clean)", 35.0_f32.to_radians()),
    ] {
        for width in [2.0_f32, 3.0, 4.0] {
            let mut row = String::new();
            for step in 0..8 {
                let offset = step as f32 / 8.0;
                let (c, s) = (angle.cos(), angle.sin());
                // Start at (10, 10), shifted across the run by `offset` tiles in x.
                let start = (10.0 + offset, 10.0);
                let case = Case {
                    points: vec![start, (start.0 + 18.0 * c, start.1 + 18.0 * s)],
                    params: PathParams {
                        width,
                        curve: CurveKind::Lines,
                        ..PathParams::default()
                    },
                };
                let (score, _) = evaluate(&case, &Tuning::default(), false);
                row.push_str(&format!(" {:5.3}", score.wobble));
            }
            println!("{name:<20} width {width}: wobble by phase {row}");
        }
    }
}

#[test]
#[ignore = "refine (slide / widen the path to its cleanest position) against not"]
fn playground_refine() {
    for (name, cases) in [("mixed", cases(48, 31)), ("straight", straight_cases())] {
        for refined in [false, true] {
            let (scores, results) = run_all_with(&cases, &Tuning::default(), refined);
            print_row(&format!("{name}, refine {refined}"), &mean(&scores));
            let _ = sheet(&cases, &results, 8)
                .save(format!("/tmp/playground-refine-{name}-{refined}.png"));
        }
    }
}

fn straight_cases() -> Vec<Case> {
    let mut rng = Rng(0xABCDEF);
    (0..48)
        .map(|n| {
            let angle = n as f32 / 48.0 * std::f32::consts::FRAC_PI_2;
            let length = rng.between(14.0, 24.0);
            let start = (6.0 + rng.next(), 6.0 + rng.next());
            Case {
                points: vec![
                    start,
                    (
                        start.0 + length * angle.cos(),
                        start.1 + length * angle.sin(),
                    ),
                ],
                params: PathParams {
                    width: [1.5, 2.0, 3.0, 4.0, 5.0][n % 5],
                    curve: CurveKind::Lines,
                    ..PathParams::default()
                },
            }
        })
        .collect()
}

/// Paths pasted from the editor's "Copy" button (`PATH ...` lines in `data/user-paths.txt`).
fn user_cases() -> Vec<Case> {
    include_str!("../../data/user-paths.txt")
        .lines()
        .filter_map(|line| {
            let rest = line.trim().strip_prefix("PATH ")?;
            let mut params = PathParams::default();
            let mut points = Vec::new();
            for field in rest.split_whitespace() {
                let (key, value) = field.split_once('=')?;
                match key {
                    "width" => params.width = value.parse().ok()?,
                    "closed" => params.closed = value == "true",
                    "curve" => {
                        params.curve = if value == "Lines" {
                            CurveKind::Lines
                        } else {
                            CurveKind::Curve
                        }
                    }
                    "snap" => params.snap = value.parse().ok()?,
                    "soften" => params.soften = value.parse().ok()?,
                    "style" => {
                        params.style = match value {
                            "Chiselled" => EdgeStyle::Chiselled,
                            "Whole" => EdgeStyle::Whole,
                            _ => EdgeStyle::Smooth,
                        }
                    }
                    "points" => {
                        points = value
                            .split(';')
                            .filter_map(|pair| {
                                let (x, y) = pair.split_once(',')?;
                                Some((x.parse().ok()?, y.parse().ok()?))
                            })
                            .collect();
                    }
                    _ => {}
                }
            }
            // Move the area to the origin so the sheet shows it.
            let (min_x, min_y) = points
                .iter()
                .fold((f32::MAX, f32::MAX), |m, p| (m.0.min(p.0), m.1.min(p.1)));
            let points = points
                .into_iter()
                .map(|p| (p.0 - min_x.floor() + 6.0, p.1 - min_y.floor() + 6.0))
                .collect();
            Some(Case { points, params })
        })
        .collect()
}

#[test]
#[ignore = "runs the paths pasted into data/user-paths.txt, as drawn and refined"]
fn playground_user_paths() {
    let cases = user_cases();
    println!("{} pasted paths", cases.len());
    for refined in [false, true] {
        let (scores, results) = run_all_with(&cases, &Tuning::default(), refined);
        for (n, s) in scores.iter().enumerate() {
            print_row(&format!("pasted {n}, refine {refined}"), s);
        }
        let _ = sheet(&cases, &results, 4).save(format!("/tmp/playground-user-{refined}.png"));
    }
}

#[test]
#[ignore = "bake-off: which combination of the ideas scores best"]
fn playground_bakeoff() {
    let sets: Vec<(&str, Vec<Case>)> = vec![
        ("mixed", cases(48, 41)),
        ("straight", straight_cases()),
        ("pasted", user_cases()),
    ];
    let base = Tuning::default();
    // (label, snap, refine, tuning)
    let variants: Vec<(&str, f32, bool, Tuning)> = vec![
        ("plain", 0.0, false, base),
        ("refine", 0.0, true, base),
        ("snap 1.0", 1.0, false, base),
        ("refine + snap 0.45", 0.45, true, base),
        ("refine + snap 1.0", 1.0, true, base),
    ];
    for (name, cases) in &sets {
        println!("--- {name} ({} paths)", cases.len());
        for (label, snap, refined, tuning) in &variants {
            let with: Vec<Case> = cases
                .iter()
                .map(|case| Case {
                    points: case.points.clone(),
                    params: PathParams {
                        snap: *snap,
                        ..case.params
                    },
                })
                .collect();
            let (scores, results) = run_all_with(&with, tuning, *refined);
            print_row(label, &mean(&scores));
            if *name == "pasted" {
                let tag = label.replace([' ', '+', ',', '(', ')'], "_");
                let _ = sheet(&with, &results, 4).save(format!("/tmp/playground-pasted-{tag}.png"));
            }
        }
    }
}
