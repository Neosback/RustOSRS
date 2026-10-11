//! Measuring a tiling against its ideal outline, and nudging the path (a slide of up to half a tile,
//! a little width) to the position where the shapes draw it most cleanly.
//!
//! The shapes sit on a lattice (corners, edge midpoints, quarter points). How an edge falls on that
//! lattice matters more than any weight in the search: the same straight 26.6 degree road wobbles
//! 0.05 or 0.21 tiles depending on where it is laid, so sliding the whole path by a fraction of a
//! tile is the cheapest large gain (see `docs/plans/AUTO-TILER.md`).

use super::*;

/// How well a tiling follows the outline it was made for (tiles; lower is better).
#[derive(Debug, Clone, Copy, Default)]
pub(super) struct Measure {
    /// Wrongly covered or uncovered area as a share of the ideal band's.
    pub area: f32,
    /// RMS second difference of the made outline's offset from the ideal one, sampled every half
    /// tile along both sides: zero for an edge that is straight relative to the ideal, large for
    /// stairs, notches and ears. This is what "jagged" measures.
    pub wobble: f32,
    /// Furthest an error cell lies from the ideal outline.
    pub max: f32,
    /// Mean distance of the error cells from the ideal outline.
    pub mean: f32,
}

impl Measure {
    /// One number to rank by (lower is better).
    pub fn cost(&self) -> f32 {
        self.area * 2.0 + self.wobble * 8.0 + self.max * 1.5 + self.mean * 3.0
    }
}

pub(super) fn result_masks(result: &[((i32, i32), TileChoice)]) -> HashMap<(i32, i32), Mask> {
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

/// Measure `result` against the band of `points` and `params` (the path as tiled, after any nudging).
pub(super) fn measure(
    points: &[(f32, f32)],
    params: PathParams,
    result: &[((i32, i32), TileChoice)],
) -> Measure {
    let half = (params.width / 2.0).max(0.5);
    let line = path_polyline(points, params, 0.25);
    let index = SegmentIndex::new(&line, half + 0.1);
    let masks = result_masks(result);
    let mut tiles: std::collections::HashSet<(i32, i32)> = masks.keys().copied().collect();
    tiles.extend(index.cells.keys().copied());
    let (mut wrong, mut ideal) = (0_u32, 0_u32);
    let (mut max, mut sum) = (0.0_f32, 0.0_f32);
    for tile in &tiles {
        let target = band_mask(&index, half, *tile);
        let mine = masks.get(tile).copied().unwrap_or(EMPTY);
        ideal += popcount(&target);
        for y in 0..GRID {
            for x in 0..GRID {
                if get(&target, x, y) != get(&mine, x, y) {
                    wrong += 1;
                    let c = cell_centre(x, y);
                    let at = (tile.0 as f32 + c[0], tile.1 as f32 + c[1]);
                    let deviation = (index.distance(*tile, at) - half).abs();
                    max = max.max(deviation);
                    sum += deviation;
                }
            }
        }
    }
    Measure {
        area: wrong as f32 / ideal.max(256) as f32,
        wobble: wobble(&line, half, params.closed, &masks),
        max,
        mean: sum / wrong.max(1) as f32,
    }
}

/// How much the offset between the made outline and the ideal one changes along the path.
pub(super) fn wobble(
    line: &[(f32, f32)],
    half: f32,
    closed: bool,
    masks: &HashMap<(i32, i32), Mask>,
) -> f32 {
    let inside = |p: (f32, f32)| -> bool {
        let tile = (p.0.floor() as i32, p.1.floor() as i32);
        masks.get(&tile).is_some_and(|mask| {
            get(
                mask,
                (((p.0 - tile.0 as f32) * GRID as f32) as usize).min(GRID - 1),
                (((p.1 - tile.1 as f32) * GRID as f32) as usize).min(GRID - 1),
            )
        })
    };
    // Samples every half tile along the centre line, with its unit normal.
    let mut samples: Vec<((f32, f32), (f32, f32))> = Vec::new();
    let mut carried = 0.0_f32;
    for pair in line.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let length = dist(a, b);
        if length < 1e-4 {
            continue;
        }
        let (tx, ty) = ((b.0 - a.0) / length, (b.1 - a.1) / length);
        let mut at = carried;
        while at < length {
            samples.push(((a.0 + tx * at, a.1 + ty * at), (-ty, tx)));
            at += 0.5;
        }
        carried = at - length;
    }
    let mut total = 0.0_f32;
    let mut count = 0_u32;
    for side in [1.0_f32, -1.0] {
        // The made outline's offset from the ideal one at each sample, `None` where there is no
        // outline within reach (path ends, a band narrower than its own width).
        let offsets: Vec<Option<f32>> = samples
            .iter()
            .map(|(p, n)| {
                let at = |t: f32| (p.0 + n.0 * side * (half + t), p.1 + n.1 * side * (half + t));
                const REACH: f32 = 0.9;
                const STEPS: i32 = 36;
                let mut best: Option<f32> = None;
                let mut previous = inside(at(-REACH));
                for i in 1..=STEPS {
                    let t = -REACH + 2.0 * REACH * i as f32 / STEPS as f32;
                    let now = inside(at(t));
                    // The outer edge: inside just before, outside just after.
                    if previous && !now && best.is_none_or(|b| t.abs() < b.abs()) {
                        best = Some(t - REACH / STEPS as f32);
                    }
                    previous = now;
                }
                best
            })
            .collect();
        let n = offsets.len();
        for i in 0..n {
            let (prev, next) = if closed {
                ((i + n - 1) % n, (i + 1) % n)
            } else if i == 0 || i + 1 == n {
                continue;
            } else {
                (i - 1, i + 1)
            };
            if let (Some(a), Some(b), Some(c)) = (offsets[prev], offsets[i], offsets[next]) {
                let second = a - 2.0 * b + c;
                total += second * second;
                count += 1;
            }
        }
    }
    (total / count.max(1) as f32).sqrt()
}

/// The path slid and widened a little to where it tiles most cleanly: `(points, params)` to tile
/// with (the points moved together, so the shape of the path is untouched).
pub(super) fn refine(
    points: &[(f32, f32)],
    params: PathParams,
    existing: &dyn Fn((i32, i32)) -> Option<(u8, u8)>,
    tuning: &Tuning,
) -> (Vec<(f32, f32)>, PathParams) {
    if points.is_empty() {
        return (Vec::new(), params);
    }
    // Candidates are judged with a short search; the winner is tiled in full by the caller.
    let quick = Tuning {
        sweeps: tuning.sweeps.min(40),
        ..*tuning
    };
    let evaluate = |dx: f32, dy: f32, width: f32| -> f32 {
        let moved: Vec<(f32, f32)> = points.iter().map(|p| (p.0 + dx, p.1 + dy)).collect();
        let candidate = PathParams { width, ..params };
        let result = generate_with(&moved, candidate, existing, &quick);
        let quality = measure(&moved, candidate, &result);
        // Moving costs a little, so a path that is already clean stays where it was put.
        quality.cost() + 0.4 * (dx.hypot(dy) + 0.5 * (width - params.width).abs())
    };
    let mut best = (
        0.0_f32,
        0.0_f32,
        params.width,
        evaluate(0.0, 0.0, params.width),
    );
    let consider = |dx: f32, dy: f32, width: f32, best: &mut (f32, f32, f32, f32)| {
        let cost = evaluate(dx, dy, width);
        if cost < best.3 {
            *best = (dx, dy, width, cost);
        }
    };
    for dx in [-0.375, -0.125, 0.125, 0.375] {
        for dy in [-0.375, -0.125, 0.125, 0.375] {
            consider(dx, dy, params.width, &mut best);
        }
    }
    let centre = best;
    for ox in [-0.0625, 0.0, 0.0625] {
        for oy in [-0.0625, 0.0, 0.0625] {
            if ox != 0.0 || oy != 0.0 {
                consider(centre.0 + ox, centre.1 + oy, params.width, &mut best);
            }
        }
    }
    let at = best;
    for dw in [-0.25, 0.25] {
        let width = (params.width + dw).max(1.0);
        if (width - params.width).abs() > 1e-3 {
            consider(at.0, at.1, width, &mut best);
        }
    }
    let moved = points
        .iter()
        .map(|p| (p.0 + best.0, p.1 + best.1))
        .collect();
    (
        moved,
        PathParams {
            width: best.2,
            ..params
        },
    )
}

/// The points and parameters a refined path was tiled with, and its tiles.
pub type Refined = (Vec<(f32, f32)>, PathParams, Vec<((i32, i32), TileChoice)>);

/// [`generate_with`] on the path as [`refine`] positions it. Returns the points and parameters
/// that were tiled too, so a preview can draw the same curve.
pub fn generate_refined(
    points: &[(f32, f32)],
    params: PathParams,
    existing: &dyn Fn((i32, i32)) -> Option<(u8, u8)>,
    tuning: &Tuning,
) -> Refined {
    let (moved, adjusted) = refine(points, params, existing, tuning);
    let tiles = generate_with(&moved, adjusted, existing, tuning);
    (moved, adjusted, tiles)
}
