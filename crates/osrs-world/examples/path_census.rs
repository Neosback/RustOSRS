//! Census of the paths (roads, trails, rivers' banks ...) the real map contains, to learn how they
//! are built instead of guessing: which overlays form path-like strips, how wide and how regular
//! they are, which shapes tile their edges (and how consistently), what they sit on and beside,
//! how they meet other overlays, and how the ground under them behaves.
//!
//! usage: path_census CACHE_DIR [OUT.json]
//!
//! A *path-like* component is a 4-connected set of tiles with one overlay id that is elongated:
//! at least 10 tiles, at least 6 medial-axis ("ridge") tiles, mean width <= 10 tiles and
//! area / width^2 >= 3. Components touching a map-square edge are skipped (they are cut). Plane 0
//! only. Heights are the final vertex heights the terrain loader produces.
use osrs_core::coords::RegionCoord;
use osrs_hd::serde_json::{Value, json};
use osrs_scene::terrain_load::{RegionTerrain, TerrainLoadGrid};
use osrs_world::{SourceSquares, WorldDefinitions, region_tiles_from_decoded};
use std::collections::{BTreeMap, VecDeque};

const N: usize = 64;
/// 8-neighbour offsets, bit i of a mask: N, NE, E, SE, S, SW, W, NW (y grows north).
const NEIGHBOURS: [(i32, i32); 8] = [
    (0, 1),
    (1, 1),
    (1, 0),
    (1, -1),
    (0, -1),
    (-1, -1),
    (-1, 0),
    (-1, 1),
];

struct Region {
    overlay: Vec<u16>,
    shape: Vec<u8>,
    rotation: Vec<u8>,
    underlay: Vec<u16>,
    /// Vertex heights `x * 64 + y`, valid for `x, y < 64`.
    height: Vec<i32>,
}

fn at(x: i32, y: i32) -> Option<usize> {
    ((0..N as i32).contains(&x) && (0..N as i32).contains(&y)).then(|| x as usize * N + y as usize)
}

#[derive(Default)]
struct Percentiles(Vec<f32>);

impl Percentiles {
    fn add(&mut self, value: f32) {
        self.0.push(value);
    }
    fn summary(&mut self) -> Value {
        if self.0.is_empty() {
            return Value::Null;
        }
        self.0.sort_by(f32::total_cmp);
        let p = |f: f32| self.0[((self.0.len() - 1) as f32 * f).round() as usize];
        json!({"n": self.0.len(), "p10": p(0.1), "p25": p(0.25), "p50": p(0.5),
               "p75": p(0.75), "p90": p(0.9), "p99": p(0.99), "max": self.0[self.0.len() - 1]})
    }
}

#[derive(Default)]
struct OverlayStats {
    tiles: u64,
    components: u64,
    path_like_components: u64,
    path_like_tiles: u64,
    shapes: [u64; 12],
}

#[derive(Default)]
struct Totals {
    overlays: BTreeMap<u16, OverlayStats>,
    widths: BTreeMap<u32, u64>,
    width_cov: Percentiles,
    comp_area: Percentiles,
    comp_length: Percentiles,
    boundary: [u64; 2],
    interior: [u64; 2],
    /// mask -> (shape, rotation) -> count
    prior: BTreeMap<u8, BTreeMap<(u8, u8), u64>>,
    flank_underlay: BTreeMap<(u16, u16), u64>,
    flank_overlay: BTreeMap<(u16, u16), u64>,
    flank_overlay_soft: BTreeMap<(u16, u16), [u64; 2]>,
    under_path: BTreeMap<(u16, u16), u64>,
    comps_underlay_zero: u64,
    comps_underlay_same: u64,
    comps_underlay_other: u64,
    endpoints: u64,
    junctions: u64,
    endpoint_shapes: BTreeMap<(u8, u8), u64>,
    range_path: Percentiles,
    range_ratio: Percentiles,
    bed_vs_surround: Percentiles,
    slope_path: Percentiles,
    slope_all: Percentiles,
    /// Second difference of the height along straight runs of the axis, path against anywhere.
    curve_path: Percentiles,
    curve_all: Percentiles,
    /// Height difference across the path (one tile either side of the axis), against the same
    /// measure on arbitrary ground, for paths that are at least 3 tiles wide.
    cross_path: Percentiles,
    cross_all: Percentiles,
    /// Median ridge width of each component, whole tiles.
    comp_width: BTreeMap<u32, u64>,
    /// overlay -> underlay beneath it -> tiles, for path-like components.
    under_by_overlay: BTreeMap<u16, BTreeMap<u16, u64>>,
    /// overlay -> median-width histogram, for path-like components.
    width_by_overlay: BTreeMap<u16, BTreeMap<u32, u64>>,
}

fn chamfer(inside: &[bool]) -> Vec<f32> {
    const D1: f32 = 1.0;
    const D2: f32 = std::f32::consts::SQRT_2;
    let mut d: Vec<f32> = inside.iter().map(|&i| if i { 1e9 } else { 0.0 }).collect();
    let step = |d: &mut Vec<f32>, x: i32, y: i32, deltas: &[(i32, i32, f32)]| {
        let i = x as usize * N + y as usize;
        for &(dx, dy, w) in deltas {
            let value = match at(x + dx, y + dy) {
                Some(j) => d[j] + w,
                None => w, // beyond the square counts as outside
            };
            if value < d[i] {
                d[i] = value;
            }
        }
    };
    let forward = [(-1, -1, D2), (-1, 0, D1), (-1, 1, D2), (0, -1, D1)];
    let backward = [(1, 1, D2), (1, 0, D1), (1, -1, D2), (0, 1, D1)];
    for x in 0..N as i32 {
        for y in 0..N as i32 {
            if inside[x as usize * N + y as usize] {
                step(&mut d, x, y, &forward);
            }
        }
    }
    for x in (0..N as i32).rev() {
        for y in (0..N as i32).rev() {
            if inside[x as usize * N + y as usize] {
                step(&mut d, x, y, &backward);
            }
        }
    }
    d
}

fn load_region(squares: &SourceSquares, region: RegionCoord) -> Option<Region> {
    let square = squares.read(region).ok()??;
    let terrain = RegionTerrain::new(region_tiles_from_decoded(&square.terrain)).ok()?;
    let mut grid = TerrainLoadGrid::new(N, N).ok()?;
    grid.load_region(&terrain, (0, 0), (region.x * 64, region.y * 64), 0);
    let mut out = Region {
        overlay: vec![0; N * N],
        shape: vec![0; N * N],
        rotation: vec![0; N * N],
        underlay: vec![0; N * N],
        height: vec![0; N * N],
    };
    for x in 0..N {
        for y in 0..N {
            let i = x * N + y;
            out.overlay[i] = grid.overlay_raw(0, x, y) & 0x7fff;
            out.shape[i] = grid.shape(0, x, y);
            out.rotation[i] = grid.rotation(0, x, y);
            out.underlay[i] = grid.underlay(0, x, y) & 0x7fff;
            out.height[i] = grid.height(0, x, y);
        }
    }
    Some(out)
}

fn analyse(region: &Region, totals: &mut Totals) {
    let mut seen = vec![false; N * N];
    for sx in 0..N as i32 {
        for sy in 0..N as i32 {
            let start = sx as usize * N + sy as usize;
            let id = region.overlay[start];
            if id == 0 || seen[start] {
                continue;
            }
            // 4-connected component of tiles with this overlay id.
            let mut tiles = Vec::new();
            let mut queue = VecDeque::from([(sx, sy)]);
            seen[start] = true;
            let mut touches_edge = false;
            while let Some((x, y)) = queue.pop_front() {
                tiles.push((x, y));
                touches_edge |= x == 0 || y == 0 || x == N as i32 - 1 || y == N as i32 - 1;
                for (dx, dy) in [(0, 1), (1, 0), (0, -1), (-1, 0)] {
                    if let Some(j) = at(x + dx, y + dy)
                        && !seen[j]
                        && region.overlay[j] == id
                    {
                        seen[j] = true;
                        queue.push_back((x + dx, y + dy));
                    }
                }
            }
            let stats = totals.overlays.entry(id).or_default();
            stats.components += 1;
            for &(x, y) in &tiles {
                stats.tiles += 1;
                let shape = region.shape[x as usize * N + y as usize] as usize;
                stats.shapes[shape.min(11)] += 1;
            }
            if touches_edge || tiles.len() < 10 {
                continue;
            }
            analyse_component(region, id, &tiles, totals);
        }
    }
}

fn analyse_component(region: &Region, id: u16, tiles: &[(i32, i32)], totals: &mut Totals) {
    let mut inside = vec![false; N * N];
    for &(x, y) in tiles {
        inside[x as usize * N + y as usize] = true;
    }
    let dist = chamfer(&inside);
    // Ridge: local maxima of the distance transform (the medial axis).
    let mut ridge = Vec::new();
    for &(x, y) in tiles {
        let d = dist[x as usize * N + y as usize];
        let is_max = NEIGHBOURS
            .iter()
            .all(|&(dx, dy)| at(x + dx, y + dy).is_none_or(|j| !inside[j] || dist[j] <= d + 1e-4));
        if is_max {
            ridge.push((x, y, (2.0 * d - 1.0).max(1.0)));
        }
    }
    if ridge.len() < 6 {
        return;
    }
    let mean_width = ridge.iter().map(|r| r.2).sum::<f32>() / ridge.len() as f32;
    let elongation = tiles.len() as f32 / (mean_width * mean_width);
    if mean_width > 10.0 || elongation < 3.0 {
        return;
    }
    let stats = totals.overlays.entry(id).or_default();
    stats.path_like_components += 1;
    stats.path_like_tiles += tiles.len() as u64;
    totals.comp_area.add(tiles.len() as f32);
    totals.comp_length.add(ridge.len() as f32);
    let variance = ridge
        .iter()
        .map(|r| (r.2 - mean_width).powi(2))
        .sum::<f32>()
        / ridge.len() as f32;
    totals.width_cov.add(variance.sqrt() / mean_width);
    for r in &ridge {
        *totals.widths.entry(r.2.round() as u32).or_default() += 1;
    }
    let mut sorted: Vec<f32> = ridge.iter().map(|r| r.2).collect();
    sorted.sort_by(f32::total_cmp);
    let median_width = sorted[sorted.len() / 2].round() as u32;
    *totals.comp_width.entry(median_width).or_default() += 1;
    *totals
        .width_by_overlay
        .entry(id)
        .or_default()
        .entry(median_width)
        .or_default() += 1;

    // Edge tiling: which shapes tile a path, and how consistently by neighbourhood.
    let mut flank_underlays: BTreeMap<u16, u64> = BTreeMap::new();
    for &(x, y) in tiles {
        let i = x as usize * N + y as usize;
        let (shape, rotation) = (region.shape[i], region.rotation[i]);
        let mut mask = 0u8;
        for (bit, &(dx, dy)) in NEIGHBOURS.iter().enumerate() {
            if at(x + dx, y + dy).is_some_and(|j| inside[j]) {
                mask |= 1 << bit;
            }
        }
        let boundary = [(0, 1), (1, 0), (0, -1), (-1, 0)]
            .iter()
            .any(|&(dx, dy)| at(x + dx, y + dy).is_none_or(|j| !inside[j]));
        let bucket = if boundary {
            &mut totals.boundary
        } else {
            &mut totals.interior
        };
        bucket[usize::from(shape != 0)] += 1;
        *totals
            .prior
            .entry(mask)
            .or_default()
            .entry((shape, rotation))
            .or_default() += 1;
        *totals
            .under_path
            .entry((id, region.underlay[i]))
            .or_default() += 1;
        *totals
            .under_by_overlay
            .entry(id)
            .or_default()
            .entry(region.underlay[i])
            .or_default() += 1;

        // What lies beside it.
        for &(dx, dy) in &NEIGHBOURS {
            let Some(j) = at(x + dx, y + dy) else {
                continue;
            };
            if inside[j] {
                continue;
            }
            let other = region.overlay[j];
            if other == 0 {
                *totals
                    .flank_underlay
                    .entry((id, region.underlay[j]))
                    .or_default() += 1;
                *flank_underlays.entry(region.underlay[j]).or_default() += 1;
            } else {
                *totals.flank_overlay.entry((id, other)).or_default() += 1;
                // Orthogonal contacts only: is either side a partial shape (a soft boundary)?
                if dx == 0 || dy == 0 {
                    let soft = shape != 0 || region.shape[j] != 0;
                    totals.flank_overlay_soft.entry((id, other)).or_default()[usize::from(soft)] +=
                        1;
                }
            }
        }
    }
    // Does the surrounding underlay continue beneath the path?
    let modal = flank_underlays
        .iter()
        .max_by_key(|(_, count)| **count)
        .map(|(under, _)| *under);
    for &(x, y) in tiles {
        let under = region.underlay[x as usize * N + y as usize];
        if under == 0 {
            totals.comps_underlay_zero += 1;
        } else if Some(under) == modal {
            totals.comps_underlay_same += 1;
        } else {
            totals.comps_underlay_other += 1;
        }
    }

    // Ends and junctions of the medial axis.
    let ridge_set: std::collections::HashSet<(i32, i32)> =
        ridge.iter().map(|r| (r.0, r.1)).collect();
    for &(x, y, _) in &ridge {
        let degree = NEIGHBOURS
            .iter()
            .filter(|&&(dx, dy)| ridge_set.contains(&(x + dx, y + dy)))
            .count();
        if degree <= 1 {
            totals.endpoints += 1;
            let i = x as usize * N + y as usize;
            *totals
                .endpoint_shapes
                .entry((region.shape[i], region.rotation[i]))
                .or_default() += 1;
        } else if degree >= 4 {
            totals.junctions += 1;
        }
    }

    // Ground under the path (interior ridge tiles only, so the whole window is in the square).
    let vertex = |x: i32, y: i32| at(x, y).map(|i| region.height[i]);
    for &(x, y, width) in &ridge {
        if x < 7 || y < 7 || x > N as i32 - 8 || y > N as i32 - 8 {
            continue;
        }
        // Flatness: the corner heights of the path's own tiles within 2 tiles of this point,
        // against all corners of the 5 x 5 window around it.
        let mut own = Vec::new();
        let mut window = Vec::new();
        for dx in -2..=2 {
            for dy in -2..=2 {
                let (tx, ty) = (x + dx, y + dy);
                let in_path = at(tx, ty).is_some_and(|j| inside[j]);
                for (cx, cy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                    if let Some(h) = vertex(tx + cx, ty + cy) {
                        window.push(h);
                        if in_path {
                            own.push(h);
                        }
                    }
                }
            }
        }
        let span = |v: &[i32]| {
            (v.iter().max().copied().unwrap_or(0) - v.iter().min().copied().unwrap_or(0)) as f32
        };
        if own.len() >= 8 && span(&window) > 0.0 {
            totals.range_path.add(span(&own));
            totals.range_ratio.add(span(&own) / span(&window));
        }
        // Bed against the ground 4-6 tiles out that is not itself path.
        let reach = (width / 2.0).ceil() as i32 + 3;
        let mut ring = Vec::new();
        for dx in -reach - 2..=reach + 2 {
            for dy in -reach - 2..=reach + 2 {
                let ring_distance = dx.abs().max(dy.abs());
                if ring_distance < reach || ring_distance > reach + 2 {
                    continue;
                }
                if at(x + dx, y + dy).is_some_and(|j| inside[j]) {
                    continue;
                }
                if let Some(h) = vertex(x + dx, y + dy) {
                    ring.push(h as f32);
                }
            }
        }
        if ring.len() >= 6
            && let Some(bed) = vertex(x, y)
        {
            totals
                .bed_vs_surround
                .add(bed as f32 - ring.iter().sum::<f32>() / ring.len() as f32);
        }
        // Level across the width: heights one tile either side of the axis, perpendicular to the
        // direction of a neighbouring ridge tile. Only for paths of three tiles or more.
        if width >= 3.0
            && let Some(&(dx, dy)) = NEIGHBOURS[..4]
                .iter()
                .find(|&&(dx, dy)| ridge_set.contains(&(x + dx, y + dy)))
        {
            let (px, py) = (-dy, dx);
            if let (Some(a), Some(b)) = (vertex(x + px, y + py), vertex(x - px, y - py)) {
                totals.cross_path.add((a - b).abs() as f32);
            }
        }
        // Smoothness along the axis: second difference over a straight run of ridge tiles.
        for &(dx, dy) in &NEIGHBOURS[..4] {
            if ridge_set.contains(&(x + dx, y + dy))
                && ridge_set.contains(&(x - dx, y - dy))
                && let (Some(a), Some(b), Some(c)) =
                    (vertex(x - dx, y - dy), vertex(x, y), vertex(x + dx, y + dy))
            {
                totals.curve_path.add((a - 2 * b + c).abs() as f32);
            }
        }
        // Slope along the axis: to adjacent ridge tiles.
        for &(dx, dy) in &NEIGHBOURS[..4] {
            if ridge_set.contains(&(x + dx, y + dy))
                && let (Some(a), Some(b)) = (vertex(x, y), vertex(x + dx, y + dy))
            {
                let step = if dx != 0 && dy != 0 {
                    std::f32::consts::SQRT_2
                } else {
                    1.0
                };
                totals.slope_path.add((a - b).abs() as f32 / step);
            }
        }
    }
}

/// Slope between neighbouring vertices anywhere in the square, as the baseline.
fn baseline_slopes(region: &Region, totals: &mut Totals) {
    for x in (0..N - 1).step_by(3) {
        for y in (0..N - 1).step_by(3) {
            totals
                .slope_all
                .add((region.height[x * N + y] - region.height[(x + 1) * N + y]).abs() as f32);
            totals
                .slope_all
                .add((region.height[x * N + y] - region.height[x * N + y + 1]).abs() as f32);
            if x >= 1 && y >= 1 && x + 2 < N && y + 2 < N {
                totals.cross_all.add(
                    (region.height[(x + 1) * N + y] - region.height[(x - 1) * N + y]).abs() as f32,
                );
            }
            if x + 2 < N {
                totals.curve_all.add(
                    (region.height[x * N + y] - 2 * region.height[(x + 1) * N + y]
                        + region.height[(x + 2) * N + y])
                        .abs() as f32,
                );
            }
        }
    }
}

fn top<K: Clone + Ord, V: Copy + Ord>(map: &BTreeMap<K, V>, n: usize) -> Vec<(K, V)> {
    let mut entries: Vec<_> = map.iter().map(|(k, v)| (k.clone(), *v)).collect();
    entries.sort_by_key(|entry| std::cmp::Reverse(entry.1));
    entries.truncate(n);
    entries
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let cache = args
        .get(1)
        .ok_or("usage: path_census CACHE_DIR [OUT.json]")?;
    let squares = SourceSquares::open(cache.as_str())?;
    let defs = WorldDefinitions::open(cache.as_str())?;
    let mut totals = Totals::default();
    let mut scanned = 0;
    for x in 0..=100 {
        for y in 0..=255 {
            let Some(region) = load_region(&squares, RegionCoord::new(x, y)) else {
                continue;
            };
            scanned += 1;
            analyse(&region, &mut totals);
            baseline_slopes(&region, &mut totals);
        }
    }
    eprintln!("scanned {scanned} map squares");

    // Overlay catalogue: what each overlay is and how path-like it is.
    use osrs_scene::terrain_build::FloorLookup;
    let mut catalogue = Vec::new();
    for (id, stats) in &totals.overlays {
        let floor = defs.floors().overlay(u32::from(*id) - 1);
        catalogue.push(json!({
            "overlay": id, "texture": floor.texture, "rgb": format!("{:06x}", floor.primary_rgb),
            "tiles": stats.tiles, "components": stats.components,
            "path_like_components": stats.path_like_components,
            "path_like_tile_share": if stats.tiles > 0 { stats.path_like_tiles as f64 / stats.tiles as f64 } else { 0.0 },
            "shape_share": stats.shapes.iter().map(|c| *c as f64 / stats.tiles.max(1) as f64).collect::<Vec<_>>(),
        }));
    }
    catalogue.sort_by_key(|v| std::cmp::Reverse(v["path_like_components"].as_u64().unwrap_or(0)));

    // Edge-pattern prior: how deterministic the real map's shape choice is.
    let mut prior = Vec::new();
    let (mut explained, mut total) = (0u64, 0u64);
    for (mask, choices) in &totals.prior {
        let sum: u64 = choices.values().sum();
        let best = choices.values().max().copied().unwrap_or(0);
        explained += best;
        total += sum;
        prior.push(json!({"mask": mask, "n": sum, "top": top(choices, 4)}));
    }
    let boundary_partial =
        totals.boundary[1] as f64 / (totals.boundary[0] + totals.boundary[1]).max(1) as f64;
    let interior_partial =
        totals.interior[1] as f64 / (totals.interior[0] + totals.interior[1]).max(1) as f64;
    let under_total =
        (totals.comps_underlay_zero + totals.comps_underlay_same + totals.comps_underlay_other)
            .max(1) as f64;
    let soft_hard: Vec<_> = top(&totals.flank_overlay, 25)
        .into_iter()
        .map(|((a, b), n)| {
            let sh = totals.flank_overlay_soft.get(&(a, b)).copied().unwrap_or([0, 0]);
            json!({"path": a, "meets": b, "contacts": n, "orthogonal_hard": sh[0], "orthogonal_soft": sh[1]})
        })
        .collect();
    let report = json!({
        "squares_scanned": scanned,
        "path_like": {
            "components": totals.comp_area.0.len(),
            "area": totals.comp_area.summary(),
            "length_tiles": totals.comp_length.summary(),
            "width_histogram_ridge_tiles": totals.widths,
            "width_histogram_per_component_median": totals.comp_width,
            "width_variation_cov": totals.width_cov.summary(),
        },
        "edges": {
            "partial_shape_share_on_boundary": boundary_partial,
            "partial_shape_share_in_interior": interior_partial,
            "neighbourhood_determinism": explained as f64 / total.max(1) as f64,
            "neighbourhood_patterns": prior.len(),
            "prior": prior,
        },
        "underlay": {
            "path_tiles_with_no_underlay": totals.comps_underlay_zero as f64 / under_total,
            "path_tiles_with_surrounding_underlay": totals.comps_underlay_same as f64 / under_total,
            "path_tiles_with_a_different_underlay": totals.comps_underlay_other as f64 / under_total,
            "underlay_under_path_top": top(&totals.under_path, 20),
            "underlay_beside_path_top": top(&totals.flank_underlay, 20),
        },
        "meets_other_overlays": soft_hard,
        "ends_and_junctions": {
            "endpoints": totals.endpoints, "junction_tiles": totals.junctions,
            "endpoint_shapes_top": top(&totals.endpoint_shapes, 12),
        },
        "ground": {
            "corner_height_span_in_own_tiles": totals.range_path.summary(),
            "own_span_over_window_span": totals.range_ratio.summary(),
            "bed_minus_surrounding_ground": totals.bed_vs_surround.summary(),
            "slope_along_path_per_tile": totals.slope_path.summary(),
            "slope_between_neighbouring_vertices_anywhere": totals.slope_all.summary(),
            "bend_along_path_second_difference": totals.curve_path.summary(),
            "bend_anywhere_second_difference": totals.curve_all.summary(),
            "cross_difference_one_tile_each_side_of_axis_wide_paths": totals.cross_path.summary(),
            "cross_difference_two_vertices_apart_anywhere": totals.cross_all.summary(),
        },
        "overlays": catalogue,
        "underlay_beneath_by_overlay": totals.under_by_overlay.iter().map(|(id, m)| (id.to_string(), top(m, 5))).collect::<BTreeMap<_, _>>(),
        "width_by_overlay": totals.width_by_overlay.iter().map(|(id, m)| (id.to_string(), m.clone())).collect::<BTreeMap<_, _>>(),
    });
    let text = osrs_hd::serde_json::to_string_pretty(&report)?;
    match args.get(2) {
        Some(path) => std::fs::write(path, text)?,
        None => println!("{text}"),
    }
    Ok(())
}
