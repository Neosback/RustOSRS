//! Outline-first tiling: fit the *outline* of the path with the fewest straight runs the shapes can draw,
//! then cut the tiles from that outline.
//!
//! Choosing every tile's piece on its own (even with neighbours' edges considered) leaves zigzags: a
//! slanted edge becomes `1:1, 2:1, 1:1, 2:1` because each tile finds the piece that fits it best. Here
//! the outline is one object. Its vertices lie on the lattice the shapes use (tile corners and edge
//! midpoints), its runs take one of the exact directions (flat, upright, 45 degrees, 2:1), and a
//! shortest-path search over (lattice point, direction) pays for how far a run strays from the ideal
//! outline and for every change of direction. The result has long straight runs and few kinks by
//! construction; each tile's piece is then whatever part of the outlined area falls in it.

use super::*;
use std::collections::{BTreeSet, HashSet};

/// Spacing of the sampled ideal outline (tiles).
const SPACING: f32 = 0.125;
/// Lattice points further than this from the ideal outline are not considered (tiles).
const REACH: f32 = 0.75;

/// The search's weights (the playground sweeps them).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OutlineTuning {
    /// Cost of a run straying from the ideal outline: weight times the integral of distance squared.
    pub deviation: f32,
    /// Cost of any change of direction.
    pub kink: f32,
    /// Extra cost per radian of the change.
    pub kink_angle: f32,
    /// No run may stray further than this from the ideal outline (tiles).
    pub max_miss: f32,
}

impl Default for OutlineTuning {
    fn default() -> Self {
        Self {
            deviation: 8.0,
            kink: 0.8,
            kink_angle: 1.5,
            max_miss: 0.45,
        }
    }
}

type Point = (f32, f32);
/// A lattice point in half-tile units.
type Node = (i32, i32);
/// A lattice point and the heading class the outline arrived with (16 = the start).
type State = (Node, usize);

/// Closed loops of cell-corner coordinates around the cells of `inside` (interior on the left).
fn trace_loops(inside: &HashSet<(i32, i32)>) -> Vec<Vec<(i32, i32)>> {
    let mut edges: HashMap<(i32, i32), Vec<(i32, i32)>> = HashMap::new();
    for &(x, y) in inside {
        if !inside.contains(&(x, y - 1)) {
            edges.entry((x, y)).or_default().push((x + 1, y));
        }
        if !inside.contains(&(x + 1, y)) {
            edges.entry((x + 1, y)).or_default().push((x + 1, y + 1));
        }
        if !inside.contains(&(x, y + 1)) {
            edges.entry((x + 1, y + 1)).or_default().push((x, y + 1));
        }
        if !inside.contains(&(x - 1, y)) {
            edges.entry((x, y + 1)).or_default().push((x, y));
        }
    }
    let mut starts: Vec<(i32, i32)> = edges.keys().copied().collect();
    starts.sort();
    let mut loops = Vec::new();
    for start in starts {
        while edges.get(&start).is_some_and(|out| !out.is_empty()) {
            let mut loop_points = vec![start];
            let mut at = start;
            let mut heading = (0, 0);
            while let Some(out) = edges.get_mut(&at).filter(|out| !out.is_empty()) {
                // At a vertex with two ways on, keep turning left (cells touching at a corner stay
                // one shape).
                let pick = (0..out.len())
                    .max_by_key(|i| {
                        let d = (out[*i].0 - at.0, out[*i].1 - at.1);
                        heading.0 * d.1 - heading.1 * d.0
                    })
                    .unwrap_or(0);
                let next = out.swap_remove(pick);
                heading = (next.0 - at.0, next.1 - at.1);
                at = next;
                if at == start {
                    break;
                }
                loop_points.push(at);
            }
            if loop_points.len() >= 4 {
                loops.push(loop_points);
            }
        }
    }
    loops
}

/// A loop smoothed (the cell stairs are 1/16 tile) and resampled at [`SPACING`].
fn resample(loop_points: &[(i32, i32)]) -> Vec<Point> {
    let mut points: Vec<Point> = loop_points
        .iter()
        .map(|p| (p.0 as f32 / GRID as f32, p.1 as f32 / GRID as f32))
        .collect();
    let n = points.len();
    for _ in 0..3 {
        let before = points.clone();
        for i in 0..n {
            let mut sum = (0.0, 0.0);
            for k in -3_i32..=3 {
                let p = before[(i as i32 + k).rem_euclid(n as i32) as usize];
                sum = (sum.0 + p.0, sum.1 + p.1);
            }
            points[i] = (sum.0 / 7.0, sum.1 / 7.0);
        }
    }
    let mut out = Vec::new();
    let mut carried = 0.0_f32;
    for i in 0..n {
        let (a, b) = (points[i], points[(i + 1) % n]);
        let length = dist(a, b);
        if length < 1e-6 {
            continue;
        }
        let mut at = carried;
        while at < length {
            let t = at / length;
            out.push((a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t));
            at += SPACING;
        }
        carried = at - length;
    }
    out
}

/// The eight clean headings (radians, counter-clockwise from east) the shapes draw exactly straight,
/// times two for each quadrant: 16 classes.
fn heading_classes() -> &'static [f32; 16] {
    static CLASSES: OnceLock<[f32; 16]> = OnceLock::new();
    CLASSES.get_or_init(|| {
        let mut list = [0.0_f32; 16];
        let base = [
            0.0,
            0.5_f32.atan(),
            std::f32::consts::FRAC_PI_4,
            2.0_f32.atan(),
        ];
        for q in 0..4 {
            for (i, a) in base.iter().enumerate() {
                list[q * 4 + i] = q as f32 * std::f32::consts::FRAC_PI_2 + a;
            }
        }
        list
    })
}

fn class_of(step: (f32, f32)) -> usize {
    let angle = step.1.atan2(step.0).rem_euclid(std::f32::consts::TAU);
    (0..16)
        .min_by(|a, b| {
            let d = |k: usize| {
                let diff = (heading_classes()[k] - angle).rem_euclid(std::f32::consts::TAU);
                diff.min(std::f32::consts::TAU - diff)
            };
            d(*a).total_cmp(&d(*b))
        })
        .unwrap_or(0)
}

fn kink_cost(a: usize, b: usize, tuning: &OutlineTuning) -> f32 {
    if a == b {
        return 0.0;
    }
    let diff = (heading_classes()[a] - heading_classes()[b]).rem_euclid(std::f32::consts::TAU);
    let turn = diff.min(std::f32::consts::TAU - diff);
    tuning.kink + tuning.kink_angle * turn
}

/// The steps between lattice points: every way across a tile between two points of its border.
fn steps() -> &'static Vec<((f32, f32), usize)> {
    static STEPS: OnceLock<Vec<((f32, f32), usize)>> = OnceLock::new();
    STEPS.get_or_init(|| {
        let base = [
            (0.5, 0.0),
            (1.0, 0.0),
            (0.5, 0.5),
            (1.0, 1.0),
            (1.0, 0.5),
            (0.5, 1.0),
            (0.0, 0.5),
            (0.0, 1.0),
        ];
        let mut list: Vec<(f32, f32)> = Vec::new();
        for (x, y) in base {
            for sx in [1.0_f32, -1.0] {
                for sy in [1.0_f32, -1.0] {
                    let v = (sx * x, sy * y);
                    if !list.contains(&v) {
                        list.push(v);
                    }
                }
            }
        }
        list.into_iter().map(|v| (v, class_of(v))).collect()
    })
}

/// Lattice points are the corners and edge midpoints of tiles, in half-tile units.
fn is_node(x2: i32, y2: i32) -> bool {
    x2 % 2 == 0 || y2 % 2 == 0
}

/// Whether two nodes (half-tile units) both lie in one closed tile, so the run between them is a
/// chord of that tile (or runs along its border).
fn same_tile(a: (i32, i32), b: (i32, i32)) -> bool {
    let mx = (a.0 + b.0).div_euclid(4);
    let my = (a.1 + b.1).div_euclid(4);
    let inside =
        |p: (i32, i32)| p.0 >= mx * 2 && p.0 <= mx * 2 + 2 && p.1 >= my * 2 && p.1 <= my * 2 + 2;
    inside(a) && inside(b)
}

/// The lattice polygon that follows one ideal outline loop, or `None` if no run of lattice chords
/// does.
fn fit_loop(
    contour: &[Point],
    sd: &dyn Fn(Point) -> f32,
    tuning: &OutlineTuning,
) -> Option<Vec<Point>> {
    let n = contour.len();
    if n < 16 {
        return None;
    }
    // Candidate lattice points: within REACH of the outline, with the nearest outline sample.
    let mut nearest: HashMap<(i32, i32), (f32, usize)> = HashMap::new();
    let range = (REACH * 2.0).ceil() as i32;
    for (i, q) in contour.iter().enumerate() {
        let (cx, cy) = ((q.0 * 2.0).round() as i32, (q.1 * 2.0).round() as i32);
        for dx in -range..=range {
            for dy in -range..=range {
                let node = (cx + dx, cy + dy);
                if !is_node(node.0, node.1) {
                    continue;
                }
                let p = (node.0 as f32 / 2.0, node.1 as f32 / 2.0);
                let d = dist(p, *q);
                if d <= REACH && nearest.get(&node).is_none_or(|(best, _)| d < *best) {
                    nearest.insert(node, (d, i));
                }
            }
        }
    }
    if nearest.is_empty() {
        return None;
    }
    // Starting points: the lattice points nearest the first outline sample.
    let mut starts: Vec<(i32, i32)> = nearest.keys().copied().collect();
    let first = contour[0];
    starts.sort_by(|a, b| {
        let da = dist((a.0 as f32 / 2.0, a.1 as f32 / 2.0), first);
        let db = dist((b.0 as f32 / 2.0, b.1 as f32 / 2.0), first);
        da.total_cmp(&db).then(a.cmp(b))
    });
    starts.truncate(4);

    let max_jump = (1.5 / SPACING) as usize + 2;
    let mut best: Option<(f32, Vec<Point>)> = None;
    // Costs of the runs (memoised: they do not depend on where the search came from).
    let mut run_cost: HashMap<(Node, Node), f32> = HashMap::new();
    for &start in &starts {
        let origin = nearest[&start].1;
        let relative = |node: (i32, i32)| (nearest[&node].1 + n - origin) % n;
        let mut order: Vec<(i32, i32)> = nearest.keys().copied().filter(|k| *k != start).collect();
        order.sort_by_key(|k| (relative(*k), *k));
        // cost[(node, heading)] and where it came from.
        let mut cost: HashMap<State, f32> = HashMap::new();
        let mut came: HashMap<State, State> = HashMap::new();
        let source_class = 16_usize;
        cost.insert((start, source_class), 0.0);
        let mut queue: Vec<(i32, i32)> = vec![start];
        queue.extend(order);
        let mut sink: Option<(f32, usize, (i32, i32), usize)> = None;
        for u in queue {
            let from_p = if u == start { 0 } else { relative(u) };
            for heading in 0..=16_usize {
                let Some(&base) = cost.get(&(u, heading)) else {
                    continue;
                };
                for &(step, class) in steps() {
                    let v = (u.0 + (step.0 * 2.0) as i32, u.1 + (step.1 * 2.0) as i32);
                    if !same_tile(u, v) {
                        continue;
                    }
                    let closing = v == start;
                    if !closing && !nearest.contains_key(&v) {
                        continue;
                    }
                    let to_p = if closing { n } else { relative(v) };
                    if to_p <= from_p || to_p - from_p > max_jump {
                        continue;
                    }
                    let run = *run_cost.entry((u, v)).or_insert_with(|| {
                        let a = (u.0 as f32 / 2.0, u.1 as f32 / 2.0);
                        let b = (v.0 as f32 / 2.0, v.1 as f32 / 2.0);
                        let length = dist(a, b);
                        let samples = (length / SPACING).ceil() as usize;
                        let mut sum = 0.0;
                        for k in 0..=samples {
                            let t = k as f32 / samples as f32;
                            let d = sd((a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t));
                            if d.abs() > tuning.max_miss {
                                return f32::INFINITY;
                            }
                            if k > 0 && k < samples {
                                sum += d.powi(2) * (length / samples as f32);
                            }
                        }
                        tuning.deviation * sum
                    });
                    let kink = if heading == source_class {
                        0.0
                    } else {
                        kink_cost(heading, class, tuning)
                    };
                    if !run.is_finite() {
                        continue;
                    }
                    let total = base + run + kink;
                    if closing {
                        if sink.is_none_or(|(c, ..)| total < c) {
                            sink = Some((total, class, u, heading));
                        }
                    } else if cost.get(&(v, class)).is_none_or(|c| total < *c) {
                        cost.insert((v, class), total);
                        came.insert((v, class), (u, heading));
                    }
                }
            }
        }
        let Some((total, _, mut node, heading)) = sink else {
            continue;
        };
        if best.as_ref().is_some_and(|(c, _)| *c <= total) {
            continue;
        }
        // Walk back to the start.
        let mut path = vec![(start.0 as f32 / 2.0, start.1 as f32 / 2.0)];
        let mut reversed = vec![node];
        let mut class = heading;
        while node != start || class != source_class {
            let Some(&(prev, prev_class)) = came.get(&(node, class)) else {
                break;
            };
            node = prev;
            class = prev_class;
            if node == start && class == source_class {
                break;
            }
            reversed.push(node);
        }
        reversed.reverse();
        path.extend(
            reversed
                .iter()
                .map(|k| (k.0 as f32 / 2.0, k.1 as f32 / 2.0)),
        );
        best = Some((total, path));
    }
    best.map(|(_, path)| path)
}

/// Whether the cells of `loops` (even-odd) cover each cell centre, as per-tile masks.
fn polygon_masks(loops: &[Vec<Point>]) -> HashMap<(i32, i32), Mask> {
    let mut rows: HashMap<i32, Vec<f32>> = HashMap::new();
    for polygon in loops {
        let n = polygon.len();
        for i in 0..n {
            let (a, b) = (polygon[i], polygon[(i + 1) % n]);
            if (a.1 - b.1).abs() < 1e-9 {
                continue;
            }
            let (lo, hi) = if a.1 < b.1 { (a, b) } else { (b, a) };
            // Rows whose centre lies in [lo.y, hi.y).
            let first = (lo.1 * GRID as f32 - 0.5).ceil() as i32;
            let last = (hi.1 * GRID as f32 - 0.5).ceil() as i32 - 1;
            for row in first..=last {
                let y = (row as f32 + 0.5) / GRID as f32;
                let t = (y - lo.1) / (hi.1 - lo.1);
                rows.entry(row).or_default().push(lo.0 + (hi.0 - lo.0) * t);
            }
        }
    }
    let mut masks: HashMap<(i32, i32), Mask> = HashMap::new();
    for (row, mut crossings) in rows {
        crossings.sort_by(f32::total_cmp);
        for pair in crossings.as_chunks::<2>().0 {
            let first = (pair[0] * GRID as f32 - 0.5).ceil() as i32;
            let last = (pair[1] * GRID as f32 - 0.5).ceil() as i32 - 1;
            for column in first..=last {
                let tile = (column.div_euclid(GRID as i32), row.div_euclid(GRID as i32));
                let mask = masks.entry(tile).or_insert(EMPTY);
                set(
                    mask,
                    column.rem_euclid(GRID as i32) as usize,
                    row.rem_euclid(GRID as i32) as usize,
                );
            }
        }
    }
    masks
}

/// The tiles of the path made by fitting its outline; `None` when the outline cannot be fitted (the
/// caller falls back to choosing every tile's piece on its own).
pub(super) fn generate_outline(
    line: &[Point],
    half: f32,
    style: EdgeStyle,
    tuning: &OutlineTuning,
) -> Option<Vec<((i32, i32), TileChoice)>> {
    let index = SegmentIndex::new(line, half + REACH + 0.6);
    let sd = |p: Point| -> f32 {
        let tile = (p.0.floor() as i32, p.1.floor() as i32);
        let d = index.distance(tile, p);
        if d == f32::MAX { 1.0 } else { d - half }
    };
    // The ideal band as cells.
    let mut inside: HashSet<(i32, i32)> = HashSet::new();
    let tiles: Vec<(i32, i32)> = index.cells.keys().copied().collect();
    for tile in tiles {
        let mask = band_mask(&index, half, tile);
        for y in 0..GRID {
            for x in 0..GRID {
                if get(&mask, x, y) {
                    inside.insert((
                        tile.0 * GRID as i32 + x as i32,
                        tile.1 * GRID as i32 + y as i32,
                    ));
                }
            }
        }
    }
    let mut polygons = Vec::new();
    for traced in trace_loops(&inside) {
        let contour = resample(&traced);
        // Tiny loops (a pinhole between two bends) are left to the cell mask below.
        if contour.len() < 24 {
            continue;
        }
        polygons.push(fit_loop(&contour, &sd, tuning)?);
    }
    if polygons.is_empty() {
        return None;
    }
    let masks = polygon_masks(&polygons);
    let list = candidates_for(style);
    let prior = prior();
    // Each tile's piece: the candidate whose mask is nearest (they are equal where the outline is a
    // chord of the tile), the census choosing among spellings of the same mask.
    let occupied: BTreeSet<(i32, i32)> = masks
        .iter()
        .filter(|(_, mask)| popcount(mask) >= 8)
        .map(|(tile, _)| *tile)
        .collect();
    let mut out = Vec::new();
    for tile in &occupied {
        let mask = masks[tile];
        let mut pattern = 0_usize;
        for (bit, (dx, dy)) in [
            (0, 1),
            (1, 1),
            (1, 0),
            (1, -1),
            (0, -1),
            (-1, -1),
            (-1, 0),
            (-1, 1),
        ]
        .iter()
        .enumerate()
        {
            if occupied.contains(&(tile.0 + dx, tile.1 + dy)) {
                pattern |= 1 << bit;
            }
        }
        let pick = list.iter().filter(|c| c.slot != KEEP_SLOT).min_by(|a, b| {
            let score = |c: &&Candidate| {
                cost(&mask, &c.mask) as f32
                    + 0.01
                        * if pattern == 255 {
                            if c.slot == 0 { 0.0 } else { 12.0 }
                        } else {
                            prior.cost[pattern][c.slot]
                        }
            };
            score(a).total_cmp(&score(b))
        })?;
        out.push((*tile, pick.choice));
    }
    Some(out)
}

/// Douglas-Peucker on an open polyline: the indices kept.
#[cfg(test)]
fn simplify(points: &[Point], epsilon: f32, keep: &mut Vec<usize>, from: usize, to: usize) {
    if to <= from + 1 {
        return;
    }
    let (a, b) = (points[from], points[to]);
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let length = (dx * dx + dy * dy).sqrt().max(1e-9);
    let mut worst = (0.0_f32, from);
    for (i, p) in points.iter().enumerate().take(to).skip(from + 1) {
        let d = ((p.0 - a.0) * dy - (p.1 - a.1) * dx).abs() / length;
        if d > worst.0 {
            worst = (d, i);
        }
    }
    if worst.0 > epsilon {
        simplify(points, epsilon, keep, from, worst.1);
        keep.push(worst.1);
        simplify(points, epsilon, keep, worst.1, to);
    }
}

/// How the made outline reads: `(kinks, zigzags)` per ten tiles of outline. The outline is traced
/// from the made tiles, straightened within 0.07 tiles (so cell stairs are not counted), and every
/// turn of 10 degrees or more is a kink; a zigzag is a turn one way followed within two tiles by a
/// turn back (the `1:1, 2:1, 1:1` pattern that reads as a saw).
#[cfg(test)]
pub(super) fn outline_stats(masks: &HashMap<(i32, i32), Mask>) -> (f32, f32) {
    let mut cells: HashSet<(i32, i32)> = HashSet::new();
    for (tile, mask) in masks {
        for y in 0..GRID {
            for x in 0..GRID {
                if get(mask, x, y) {
                    cells.insert((
                        tile.0 * GRID as i32 + x as i32,
                        tile.1 * GRID as i32 + y as i32,
                    ));
                }
            }
        }
    }
    let (mut kinks, mut zigzags, mut length) = (0_u32, 0_u32, 0.0_f32);
    for traced in trace_loops(&cells) {
        let points: Vec<Point> = traced
            .iter()
            .map(|p| (p.0 as f32 / GRID as f32, p.1 as f32 / GRID as f32))
            .collect();
        let n = points.len();
        if n < 8 {
            continue;
        }
        // Split the loop at its first point and the point furthest from it.
        let far = (0..n)
            .max_by(|a, b| dist(points[*a], points[0]).total_cmp(&dist(points[*b], points[0])))
            .unwrap_or(n / 2);
        let mut closed = points.clone();
        closed.push(points[0]);
        let mut keep = vec![0, far, n];
        simplify(&closed, 0.07, &mut keep, 0, far);
        simplify(&closed, 0.07, &mut keep, far, n);
        keep.sort_unstable();
        keep.dedup();
        let corners: Vec<Point> = keep[..keep.len() - 1].iter().map(|i| closed[*i]).collect();
        let m = corners.len();
        if m < 3 {
            continue;
        }
        // Signed turn at each corner, and where it is along the outline.
        let mut turns: Vec<(f32, f32)> = Vec::new();
        let mut along = 0.0_f32;
        for i in 0..m {
            let (a, b, c) = (corners[(i + m - 1) % m], corners[i], corners[(i + 1) % m]);
            let h1 = (b.1 - a.1).atan2(b.0 - a.0);
            let h2 = (c.1 - b.1).atan2(c.0 - b.0);
            let mut turn = h2 - h1;
            turn = turn.sin().atan2(turn.cos());
            turns.push((turn, along));
            along += dist(b, c);
        }
        length += along;
        let significant: Vec<(f32, f32)> = turns
            .into_iter()
            .filter(|(t, _)| t.abs() >= 10.0_f32.to_radians())
            .collect();
        kinks += significant.len() as u32;
        for pair in significant.windows(2) {
            if pair[0].0.signum() != pair[1].0.signum() && pair[1].1 - pair[0].1 <= 2.0 {
                zigzags += 1;
            }
        }
    }
    let per = 10.0 / length.max(1.0);
    (kinks as f32 * per, zigzags as f32 * per)
}
