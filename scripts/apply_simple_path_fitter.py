from pathlib import Path
import re

ROOT = Path('.')

fitter = r'''//! Deterministic overlay-shape fitting for the Spline tool.
//!
//! The fitter has no obstacle routing or map mutation. It takes a dense polyline, samples the ideal
//! path band on an 8x8 grid per touched tile, and picks only shapes that the audited OSRS terrain
//! geometry can actually render. A short deterministic relaxation then improves shared edges.

use super::overlay_triangles;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

const SAMPLE_GRID: usize = 8;
const SAMPLE_COUNT: usize = SAMPLE_GRID * SAMPLE_GRID;
const REFINE_PASSES: usize = 6;
const CENTER_REJECT_MARGIN: f32 = 0.75;
const WEST: usize = 0;
const EAST: usize = 1;
const SOUTH: usize = 2;
const NORTH: usize = 3;

/// One fitted map-file overlay shape. `shape` is the stored 0..=11 value, not the renderer's
/// internal shape-table row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FittedTile {
    pub tile_x: i32,
    pub tile_y: i32,
    pub shape: u8,
    pub orientation: u8,
}

#[derive(Debug, Clone)]
struct Candidate {
    shape: Option<u8>,
    orientation: u8,
    covered: [bool; SAMPLE_COUNT],
    edges: [[bool; SAMPLE_GRID]; 4],
}

#[derive(Debug, Clone)]
struct TileState {
    ideal: [bool; SAMPLE_COUNT],
    candidate: usize,
}

/// Uniform Catmull-Rom smoothing with clamped endpoints and twelve samples per control segment.
/// Two points remain a straight segment exactly.
pub fn smooth_catmull_rom(points: &[(f32, f32)]) -> Vec<(f32, f32)> {
    if points.len() < 3 {
        return points.to_vec();
    }
    let mut out = Vec::with_capacity((points.len() - 1) * 12 + 1);
    for i in 0..points.len() - 1 {
        let p0 = points[i.saturating_sub(1)];
        let p1 = points[i];
        let p2 = points[i + 1];
        let p3 = points[(i + 2).min(points.len() - 1)];
        for step in 0..12 {
            let t = step as f32 / 12.0;
            let t2 = t * t;
            let t3 = t2 * t;
            out.push((
                catmull_component(p0.0, p1.0, p2.0, p3.0, t, t2, t3),
                catmull_component(p0.1, p1.1, p2.1, p3.1, t, t2, t3),
            ));
        }
    }
    out.push(points[points.len() - 1]);
    out
}

fn catmull_component(p0: f32, p1: f32, p2: f32, p3: f32, t: f32, t2: f32, t3: f32) -> f32 {
    0.5 * (2.0 * p1
        + (p2 - p0) * t
        + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * t2
        + (3.0 * p1 - p0 - 3.0 * p2 + p3) * t3)
}

/// Fit OSRS overlay shapes to a dense path polyline.
///
/// Unlike the old planner this never reroutes. The output follows the supplied line exactly.
/// Candidate tiles come from segment-local expanded boxes rather than one giant global rectangle,
/// which preserves coverage while avoiding pathological work for long diagonal paths.
pub fn fit_path_overlay(points: &[(f32, f32)], width: f32, edge_weight: f32) -> Vec<FittedTile> {
    if points.is_empty() {
        return Vec::new();
    }
    let width = if width.is_finite() { width.max(0.05) } else { 1.0 };
    let half = width * 0.5;
    let edge_weight = if edge_weight.is_finite() {
        edge_weight.max(0.0)
    } else {
        0.0
    };
    let candidates = candidates();
    let mut states = BTreeMap::<(i32, i32), TileState>::new();

    for tile in candidate_tiles(points, half) {
        let center = (tile.0 as f32 + 0.5, tile.1 as f32 + 0.5);
        if distance_to_path(center, points) > half + CENTER_REJECT_MARGIN {
            continue;
        }
        let ideal = ideal_mask(tile, points, half);
        if !ideal.iter().any(|covered| *covered) {
            continue;
        }
        let candidate = best_independent(&ideal, candidates);
        states.insert(tile, TileState { ideal, candidate });
    }

    if edge_weight > 0.0 {
        let keys: Vec<_> = states.keys().copied().collect();
        for _ in 0..REFINE_PASSES {
            let mut changed = false;
            for key in &keys {
                let next = best_with_neighbors(*key, &states, candidates, edge_weight);
                if let Some(state) = states.get_mut(key)
                    && state.candidate != next
                {
                    state.candidate = next;
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
    }

    states
        .into_iter()
        .filter_map(|((tile_x, tile_y), state)| {
            let candidate = &candidates[state.candidate];
            candidate.shape.map(|shape| FittedTile {
                tile_x,
                tile_y,
                shape,
                orientation: candidate.orientation,
            })
        })
        .collect()
}

fn candidate_tiles(points: &[(f32, f32)], half: f32) -> BTreeSet<(i32, i32)> {
    let mut tiles = BTreeSet::new();
    let pad = half + 1.0;
    if points.len() == 1 {
        add_box(&mut tiles, points[0], points[0], pad);
        return tiles;
    }
    for pair in points.windows(2) {
        add_box(&mut tiles, pair[0], pair[1], pad);
    }
    tiles
}

fn add_box(tiles: &mut BTreeSet<(i32, i32)>, a: (f32, f32), b: (f32, f32), pad: f32) {
    let x0 = (a.0.min(b.0) - pad).floor() as i32;
    let x1 = (a.0.max(b.0) + pad).ceil() as i32;
    let y0 = (a.1.min(b.1) - pad).floor() as i32;
    let y1 = (a.1.max(b.1) + pad).ceil() as i32;
    for x in x0..=x1 {
        for y in y0..=y1 {
            tiles.insert((x, y));
        }
    }
}

fn ideal_mask(tile: (i32, i32), points: &[(f32, f32)], half: f32) -> [bool; SAMPLE_COUNT] {
    let mut ideal = [false; SAMPLE_COUNT];
    for sy in 0..SAMPLE_GRID {
        for sx in 0..SAMPLE_GRID {
            let point = (
                tile.0 as f32 + (sx as f32 + 0.5) / SAMPLE_GRID as f32,
                tile.1 as f32 + (sy as f32 + 0.5) / SAMPLE_GRID as f32,
            );
            ideal[sy * SAMPLE_GRID + sx] = distance_to_path(point, points) <= half;
        }
    }
    ideal
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

fn distance(a: (f32, f32), b: (f32, f32)) -> f32 {
    ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt()
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

fn candidates() -> &'static [Candidate] {
    static CANDIDATES: OnceLock<Vec<Candidate>> = OnceLock::new();
    CANDIDATES.get_or_init(|| {
        let mut out = Vec::with_capacity(46);
        out.push(Candidate {
            shape: None,
            orientation: 0,
            covered: [false; SAMPLE_COUNT],
            edges: [[false; SAMPLE_GRID]; 4],
        });
        if let Some(full) = sampled_candidate(0, 0) {
            out.push(full);
        }
        for shape in 1..=11 {
            for orientation in 0..=3 {
                if let Some(candidate) = sampled_candidate(shape, orientation) {
                    out.push(candidate);
                }
            }
        }
        out
    })
}

fn sampled_candidate(shape: u8, orientation: u8) -> Option<Candidate> {
    let triangles = overlay_triangles(shape, orientation);
    if triangles.is_empty() {
        return None;
    }
    let covered = std::array::from_fn(|index| {
        let sx = index % SAMPLE_GRID;
        let sy = index / SAMPLE_GRID;
        let point = (
            (sx as f32 + 0.5) / SAMPLE_GRID as f32,
            (sy as f32 + 0.5) / SAMPLE_GRID as f32,
        );
        covered_by(point, triangles)
    });
    let edges = std::array::from_fn(|side| {
        std::array::from_fn(|sample| {
            let along = (sample as f32 + 0.5) / SAMPLE_GRID as f32;
            let point = match side {
                WEST => (1.0 / 128.0, along),
                EAST => (127.0 / 128.0, along),
                SOUTH => (along, 1.0 / 128.0),
                NORTH => (along, 127.0 / 128.0),
                _ => unreachable!(),
            };
            covered_by(point, triangles)
        })
    });
    Some(Candidate {
        shape: Some(shape),
        orientation,
        covered,
        edges,
    })
}

fn covered_by(point: (f32, f32), triangles: &[[[f32; 2]; 3]]) -> bool {
    triangles.iter().any(|triangle| point_in_triangle(point, *triangle))
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

fn best_independent(ideal: &[bool; SAMPLE_COUNT], candidates: &[Candidate]) -> usize {
    let mut best = 0;
    let mut best_score = agreement(ideal, &candidates[0].covered);
    for (index, candidate) in candidates.iter().enumerate().skip(1) {
        let score = agreement(ideal, &candidate.covered);
        if score > best_score {
            best = index;
            best_score = score;
        }
    }
    best
}

fn best_with_neighbors(
    key: (i32, i32),
    states: &BTreeMap<(i32, i32), TileState>,
    candidates: &[Candidate],
    edge_weight: f32,
) -> usize {
    let Some(state) = states.get(&key) else {
        return 0;
    };
    let mut best = 0;
    let mut best_score = score_candidate(key, state, 0, states, candidates, edge_weight);
    for index in 1..candidates.len() {
        let score = score_candidate(key, state, index, states, candidates, edge_weight);
        if score > best_score {
            best = index;
            best_score = score;
        }
    }
    best
}

fn score_candidate(
    key: (i32, i32),
    state: &TileState,
    candidate_index: usize,
    states: &BTreeMap<(i32, i32), TileState>,
    candidates: &[Candidate],
    edge_weight: f32,
) -> f32 {
    let candidate = &candidates[candidate_index];
    let mut mismatch = 0_u32;
    for (neighbor_key, side, opposite) in [
        ((key.0 - 1, key.1), WEST, EAST),
        ((key.0 + 1, key.1), EAST, WEST),
        ((key.0, key.1 - 1), SOUTH, NORTH),
        ((key.0, key.1 + 1), NORTH, SOUTH),
    ] {
        if let Some(neighbor) = states.get(&neighbor_key) {
            let neighbor_candidate = &candidates[neighbor.candidate];
            mismatch += edge_mismatch(&candidate.edges[side], &neighbor_candidate.edges[opposite]);
        }
    }
    agreement(&state.ideal, &candidate.covered) as f32 - edge_weight * mismatch as f32
}

fn agreement(left: &[bool; SAMPLE_COUNT], right: &[bool; SAMPLE_COUNT]) -> u32 {
    left.iter()
        .zip(right)
        .filter(|(left, right)| left == right)
        .count() as u32
}

fn edge_mismatch(left: &[bool; SAMPLE_GRID], right: &[bool; SAMPLE_GRID]) -> u32 {
    left.iter()
        .zip(right)
        .filter(|(left, right)| left != right)
        .count() as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn candidate_set_is_empty_full_plus_all_shaped_rotations() {
        assert_eq!(candidates().len(), 46);
        assert!(candidates()[0].shape.is_none());
        assert_eq!(candidates()[1].shape, Some(0));
        assert!(candidates()[1].covered.iter().all(|covered| *covered));
    }

    #[test]
    fn two_points_remain_a_straight_segment() {
        let points = [(1.25, 2.5), (8.75, 4.0)];
        assert_eq!(smooth_catmull_rom(&points), points);
    }

    #[test]
    fn catmull_rom_keeps_exact_endpoints() {
        let points = [(0.0, 0.0), (2.0, 3.0), (6.0, 4.0)];
        let smooth = smooth_catmull_rom(&points);
        assert_eq!(smooth.first().copied(), Some(points[0]));
        assert_eq!(smooth.last().copied(), Some(points[2]));
        assert_eq!(smooth.len(), 25);
    }

    #[test]
    fn fit_is_deterministic_and_uses_only_valid_stored_shapes() {
        let line = smooth_catmull_rom(&[(0.25, 0.5), (4.0, 2.0), (8.75, 2.75)]);
        let first = fit_path_overlay(&line, 2.5, 0.75);
        let second = fit_path_overlay(&line, 2.5, 0.75);
        assert_eq!(first, second);
        assert!(!first.is_empty());
        assert!(first.iter().all(|tile| tile.shape <= 11 && tile.orientation <= 3));
    }

    #[test]
    fn negative_world_coordinates_are_supported() {
        let fitted = fit_path_overlay(&[(-4.5, -2.25), (-0.25, -2.25)], 1.5, 0.5);
        assert!(!fitted.is_empty());
        assert!(fitted.iter().any(|tile| tile.tile_x < 0 && tile.tile_y < 0));
    }
}
'''
Path('crates/osrs-editor/src/terrain_path/fitter.rs').write_text(fitter)

Path('crates/osrs-editor/src/terrain_path/mod.rs').write_text(r'''//! Deterministic terrain-path overlay fitting for the Spline tool.
//!
//! The current path tool intentionally has no obstacle routing, network pathfinding, walls, noise,
//! or auto-connect behavior. It smooths the user's control polyline and fits the exact OSRS overlay
//! shape geometry to that band.

mod fitter;
mod shapes;

pub use fitter::{FittedTile, fit_path_overlay, smooth_catmull_rom};
pub use shapes::{overlay_triangles, shape_triangles};
''')

paint_path = Path('crates/osrs-editor/src/paint.rs')
paint = paint_path.read_text()
start = paint.index('/// What the path tool lays along the line.')
end = paint.index('/// Which floors the palettes list.', start)
replacement = r'''/// How the simplified Spline tool fits an overlay band to the user's control line.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SplineSettings {
    /// Total path-band width in tiles.
    pub width: f32,
    /// Shared-edge agreement weight. Zero fits each tile independently.
    pub edge_weight: f32,
    /// Control points land on tile centres (off: where you click).
    pub snap_points: bool,
}

impl Default for SplineSettings {
    fn default() -> Self {
        Self {
            width: 3.0,
            edge_weight: 0.75,
            snap_points: false,
        }
    }
}

'''
paint_path.write_text(paint[:start] + replacement + paint[end:])

shelf_path = Path('crates/osrs-editor/src/app/shelf.rs')
shelf = shelf_path.read_text()
shelf = shelf.replace('use crate::paint::{BrushShape, PathMode, Tool};\n', 'use crate::paint::{BrushShape, Tool};\n')
shelf = shelf.replace('use crate::terrain_path::{AutoConnectPolicy, ObstaclePolicy};\n', '')
start = shelf.index('    /// The terrain-path planner and wall-band controls used by the Spline tool.')
end = shelf.rfind('\n}')
path_section = r'''    /// Deterministic width/shape fitting controls used by the Spline tool.
    fn path_section(&mut self, ui: &mut egui::Ui) {
        crate::theme::section_heading(ui, "Path");
        let spline = &mut self.prefs.spline;
        ui.checkbox(&mut spline.snap_points, "Snap points to tile centres")
            .on_hover_text("Place control points at tile centres instead of the exact cursor position");

        ui.add_space(4.0);
        ui.label(egui::RichText::new("Width").weak());
        ui.add(
            egui::Slider::new(&mut spline.width, 0.5..=24.0)
                .max_decimals(1)
                .suffix(" tiles"),
        );
        ui.label(egui::RichText::new("Edge continuity").weak());
        ui.add(egui::Slider::new(&mut spline.edge_weight, 0.0..=2.0).max_decimals(2))
            .on_hover_text("0 fits every tile independently; higher values prefer neighbouring shapes whose shared edges agree");
        ui.add_space(6.0);
        ui.weak("The path follows the line you draw. Obstacle routing and walls are disabled for now.");
    }
'''
shelf_path.write_text(shelf[:start] + path_section + shelf[end:])

spline_path = Path('crates/osrs-editor/src/app/spline_tool.rs')
spline = spline_path.read_text()
click_marker = '/// A click this close (in tiles) to a control point grabs it instead of adding a new one.\n'
header_end = spline.index(click_marker)
header = r'''//! The Spline tool: click control points, preview a fitted OSRS overlay band, press Enter.
//!
//! There is deliberately no obstacle routing or wall mode here. The control line is smoothed with
//! deterministic Catmull-Rom sampling and the resulting band is fitted against the exact OSRS
//! shaped-terrain geometry. The fitted tiles are committed as one semantic patch / one undo step.

use super::EditorApp;
use super::editing::PathTile;
use crate::interact;
use crate::paint::Tool;
use crate::style::ColorKey;
use crate::terrain_path::{self, FittedTile};
use eframe::egui::{self, Color32, Pos2, Rect, Stroke};

'''
spline = header + spline[header_end:]
spline = spline.replace('/// Spacing of the polyline used for drawing and for walls.\nconst STEP: f64 = 0.25;\n', '')

old_preview = re.compile(r'#\[derive\(Debug, Default, Clone\)\]\nstruct Preview \{.*?\n\}\n', re.S)
new_preview = r'''#[derive(Debug, Default, Clone)]
struct Preview {
    key: u64,
    overlay_id: u16,
    paints: Vec<FittedTile>,
    /// The exact smoothed centre line used by the fitter.
    lines: Vec<Vec<(f32, f32)>>,
}
'''
spline, count = old_preview.subn(new_preview, spline, count=1)
if count != 1:
    raise SystemExit('Preview replacement failed')

fnv_start = spline.index('fn fnv(text: &str) -> u64 {')
distance_start = spline.index('fn distance(a: (f32, f32), b: (f32, f32)) -> f32 {', fnv_start)
new_fnv = r'''fn fnv(text: &str) -> u64 {
    let mut hash = FNV_OFFSET;
    for byte in text.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

'''
spline = spline[:fnv_start] + new_fnv + spline[distance_start:]

plan_start = spline.index('fn to_points(points: &[(f32, f32)]) -> Vec<PathPoint> {')
worker_start = spline.index('#[cfg(not(target_arch = "wasm32"))]\nimpl PlanWorker {', plan_start)
new_plan = r'''/// Everything a fit is made from, copied off the editor so another thread can make it.
struct PlanInput {
    key: u64,
    points: Vec<(f32, f32)>,
    width: f32,
    edge_weight: f32,
    overlay_id: u16,
}

impl PlanInput {
    fn make(self) -> Preview {
        let line = terrain_path::smooth_catmull_rom(&self.points);
        let paints = terrain_path::fit_path_overlay(&line, self.width, self.edge_weight);
        Preview {
            key: self.key,
            overlay_id: self.overlay_id,
            paints,
            lines: vec![line],
        }
    }
}

'''
spline = spline[:plan_start] + new_plan + spline[worker_start:]

count_start = spline.index('    /// Tiles the current path would change.')
overlay_start = spline.index('    /// The overlay id the path lays, when the brush sets one.', count_start)
new_count = r'''    /// Tiles the current path would change.
    pub(super) fn spline_tile_count(&self) -> usize {
        self.spline.current_preview().map_or(0, |preview| preview.paints.len())
    }

'''
spline = spline[:count_start] + new_count + spline[overlay_start:]

profile_start = spline.index('    /// A profile for the planner:', overlay_start)
plan_input_comment = spline.index('    /// Everything the current path is planned from, or `None` when there is nothing to plan.', profile_start)
spline = spline[:profile_start] + spline[plan_input_comment:]

plan_fn_start = spline.index('    fn plan_input(&self) -> Option<PlanInput> {', plan_input_comment)
update_comment = spline.index('    /// Keep the preview in step with the points and options.', plan_fn_start)
new_plan_input = r'''    fn plan_input(&self) -> Option<PlanInput> {
        if self.spline.points.len() < 2 {
            return None;
        }
        let overlay_id = self.laying_overlay()?;
        let spline = &self.prefs.spline;
        let width = spline.width.clamp(0.5, 24.0);
        let edge_weight = spline.edge_weight.clamp(0.0, 2.0);
        let key = fnv(&format!(
            "{:?}|{}|{}|{}|{}|{}",
            self.spline.points,
            width.to_bits(),
            edge_weight.to_bits(),
            overlay_id,
            self.view_plane,
            spline.snap_points,
        ));
        Some(PlanInput {
            key,
            points: self.spline.points.clone(),
            width,
            edge_weight,
            overlay_id,
        })
    }

'''
spline = spline[:plan_fn_start] + new_plan_input + spline[update_comment:]

report_start = spline.index('    /// Everything about the path in one block of text, to paste into a chat:')
input_comment = spline.index('    /// Add, move, insert and remove control points; Enter lays the path.', report_start)
new_report = r'''    /// Everything about the current fitted path in one copyable block.
    pub(super) fn spline_report(&self) -> String {
        let spline = &self.prefs.spline;
        let raw: Vec<String> = self
            .spline
            .points
            .iter()
            .map(|p| format!("{:.3},{:.3}", p.0, p.1))
            .collect();
        let mut text = format!(
            "PATH plane={} points={}\nwidth={:.2} edge_weight={:.2} overlay={:?}\n",
            self.view_plane,
            raw.join(";"),
            spline.width,
            spline.edge_weight,
            self.prefs.brush.overlay.fill,
        );
        if let Some(preview) = &self.spline.preview {
            text.push_str(&format!("fitted {} tiles\n", preview.paints.len()));
        }
        text
    }

'''
spline = spline[:report_start] + new_report + spline[input_comment:]

commit_start = spline.index('    /// Lay the previewed path as one undo step.')
draw_comment = spline.index('    /// The path preview:', commit_start)
new_commit = r'''    /// Lay the previewed fitted path as one semantic patch / one undo step.
    pub(super) fn commit_spline(&mut self) {
        if self.laying_overlay().is_none() {
            self.notice = Some((
                "Choose an overlay to lay the path with".to_owned(),
                web_time::Instant::now(),
            ));
            return;
        }
        self.spline_plan_now();
        let Some(preview) = self.spline.preview.take() else {
            return;
        };
        if preview.paints.is_empty() {
            self.spline.preview = Some(preview);
            self.notice = Some((
                "The fitted path has no overlay tiles".to_owned(),
                web_time::Instant::now(),
            ));
            return;
        }
        let tiles: Vec<PathTile> = preview
            .paints
            .iter()
            .map(|paint| PathTile {
                tile: (paint.tile_x, paint.tile_y),
                overlay: Some(preview.overlay_id),
                shape: paint.shape,
                rotation: paint.orientation,
            })
            .collect();
        let changed = self.apply_terrain_path(&tiles);
        self.spline.points.clear();
        self.notice = Some((
            format!("Laid a fitted path over {changed} tiles"),
            web_time::Instant::now(),
        ));
    }

'''
spline = spline[:commit_start] + new_commit + spline[draw_comment:]

spline = re.sub(
    r'\n            for tile in &preview\.wall_tiles \{\n                tile_polygon\(\*tile, 0, 0\);\n            \}',
    '',
    spline,
    count=1,
)
old_fallback = re.compile(
    r'''            _ => to_line\(&terrain_path::smooth_line\(\n                &to_points\(&self\.spline\.points\),\n                self\.prefs\.spline\.planner\.smoothness,\n                STEP,\n            \)\),''',
    re.S,
)
spline, count = old_fallback.subn('            _ => terrain_path::smooth_catmull_rom(&self.spline.points),', spline, count=1)
if count != 1:
    raise SystemExit('draw fallback replacement failed')

tests = spline.find('\n#[cfg(test)]\nmod tests {')
if tests != -1:
    spline = spline[:tests] + '\n'
spline_path.write_text(spline)

editing_path = Path('crates/osrs-editor/src/app/editing.rs')
editing = editing_path.read_text()
legacy_start = editing.find('    /// Lay a path: each tile\'s overlay with its shape and rotation, the other armed channels of the')
if legacy_start != -1:
    legacy_end = editing.index('    /// Remove a tile\'s overlay', legacy_start)
    editing = editing[:legacy_start] + editing[legacy_end:]
editing_path.write_text(editing)

# Record the changed direction in the living handoff without pretending the old generator remains active.
handoff_path = Path('DEV_HANDOFF.md')
handoff = handoff_path.read_text()
handoff = handoff.replace(
    'The current spline/path generator is intentionally **not** being redesigned yet. Its replacement comes later, after the edit-domain, history/query, view-synchronization, and persistence boundaries are stable.',
    'The spline/path direction changed on 2026-10-12: wall generation and obstacle/network pathfinding are disabled for now. The active tool is a deliberately small deterministic pipeline: clamped Catmull-Rom smoothing -> ideal band sampling -> exact OSRS overlay-shape fitting -> bounded shared-edge relaxation -> one semantic `MapPatch` commit.'
)
status_marker = '## Current branch status / next checkpoint\n'
if status_marker in handoff:
    status = r'''## Current branch status / next checkpoint

Active branch: `refactor/editor-path-commit-domain`.

### Simplified path-generator direction

The previous terrain planner is no longer part of the active Spline-tool runtime. The current branch intentionally disables:

- Walls mode and wall/tower object emission
- obstacle avoidance and rerouting
- A*/cost-grid pathfinding
- path-network auto-connect behavior
- wander/noise/width-variation generation
- preserve-existing-overlay topology logic

The active path algorithm is intentionally small:

1. Control points are smoothed with clamped uniform Catmull-Rom, 12 samples per segment.
2. Candidate tiles are collected from segment-local boxes expanded by half the requested width. This is equivalent to considering every tile that may touch the band without scanning a huge global bounding rectangle.
3. Each candidate tile samples an 8x8 ideal coverage mask against exact point-to-polyline distance.
4. Shape candidates are sampled from `terrain_path::shape_triangles`, which itself uses audited `osrs_scene::ShapedTerrainSurface` geometry. Stored map shapes remain `0..=11`.
5. EMPTY wins exact ties; the first pass maximizes 8x8 area agreement independently.
6. Up to six deterministic coordinate-descent passes add shared-edge mismatch cost controlled by `edge_weight`.
7. Empty fits are omitted.
8. The result still commits through the semantic `MapPatch` boundary and remains one undo step.

Safe simplifications/improvements over the supplied `PathOverlayFitter` idea:

- signed world coordinates are supported directly; there is no `mapWidth/mapLength` clamp assumption
- exact renderer-derived shape triangles are reused rather than duplicating the shape tables or rotation formulas
- segment-local candidate boxes avoid pathological global-bounding-box work on long diagonals
- width and edge weight are finite/clamped at the editor boundary
- no randomness, annealing, topology repair, or additional heuristics were added

The old planner/router/wall source files remain in the repository temporarily as uncompiled historical code while this replacement proves itself. They are not wired into the active `terrain_path` module and should not be revived implicitly.

Next checkpoint after this fitter is validated should be a focused quality pass on its visual output and fixtures only. Do not reintroduce obstacle routing or walls without an explicit decision.

'''
    handoff = handoff[:handoff.index(status_marker)] + status
handoff_path.write_text(handoff)

# Remove this temporary migration machinery from the source commit.
Path('scripts/apply_simple_path_fitter.py').unlink(missing_ok=True)
Path('.github/workflows/editor-simple-path-fitter.yml').unlink(missing_ok=True)
