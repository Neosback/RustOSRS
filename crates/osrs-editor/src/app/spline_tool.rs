//! The spline tool: click points on the terrain, see the path the auto-tiler would lay, press Enter.
//!
//! Control points sit exactly where you click (not snapped to tiles), so curves stay smooth. The path is a
//! centripetal Catmull-Rom spline through them (or straight lines), `width` tiles wide; the auto-tiler
//! (`autotile.rs`) turns it into per-tile overlay shapes and rotations, drawn as a preview and laid as
//! one undo step. Dragging a point re-previews live; Shift + click inserts a point on the nearest
//! stretch; Cmd + click or Delete removes the point under the cursor; Backspace removes the last.

use super::EditorApp;
use crate::autotile::{self, PathParams, TileChoice, shape_triangles};
use crate::interact;
use crate::paint::Tool;
use crate::style::ColorKey;
use eframe::egui::{self, Color32, Pos2, Rect, Stroke};

/// A click this close (in tiles) to a control point grabs it instead of adding a new one.
const GRAB_DISTANCE: f32 = 0.9;
/// Spacing of the polyline used for drawing and rasterising.
const STEP: f32 = 0.25;

#[derive(Debug, Default)]
pub struct SplineState {
    /// Control points in world tile coordinates.
    pub points: Vec<(f32, f32)>,
    drag: Option<usize>,
    preview: Option<Preview>,
    /// When the points or options last changed (the refined tiling waits for things to settle).
    changed_at: Option<web_time::Instant>,
    /// The refined tiling being made on another thread, with the key it is for.
    job: Option<(u64, std::sync::mpsc::Receiver<autotile::Refined>)>,
    ctx: Option<egui::Context>,
}

/// How long the path must sit still before it is slid to its cleanest position (a search that is
/// too slow to run on every drag frame).
const REFINE_DELAY: std::time::Duration = std::time::Duration::from_millis(220);

#[derive(Debug)]
struct Preview {
    key: u64,
    tiles: Vec<((i32, i32), TileChoice)>,
    /// Whether the path was slid to its cleanest position (otherwise it is as drawn).
    refined: bool,
    /// The points and options the tiles were made from (the refined ones when `refined`).
    points: Vec<(f32, f32)>,
    params: PathParams,
}

fn key_of(points: &[(f32, f32)], params: PathParams, terrain: u64) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    let mut mix = |value: u64| {
        hash ^= value;
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    };
    mix(params.width.to_bits().into());
    mix(params.style as u64);
    mix(params.curve as u64);
    mix(u64::from(params.closed));
    mix(params.snap.to_bits().into());
    mix(params.straighten.to_bits().into());
    mix(0);
    mix(terrain);
    for (x, y) in points {
        mix(x.to_bits().into());
        mix(y.to_bits().into());
    }
    hash
}

fn distance(a: (f32, f32), b: (f32, f32)) -> f32 {
    ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt()
}

/// Distance from `p` to the segment `a`-`b`.
fn segment_distance(p: (f32, f32), a: (f32, f32), b: (f32, f32)) -> f32 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let length_squared = dx * dx + dy * dy;
    let t = if length_squared > 0.0 {
        (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / length_squared).clamp(0.0, 1.0)
    } else {
        0.0
    };
    distance(p, (a.0 + dx * t, a.1 + dy * t))
}

impl EditorApp {
    fn path_params(&self) -> PathParams {
        let spline = &self.prefs.spline;
        PathParams {
            width: spline.width,
            style: spline.style,
            curve: spline.curve,
            closed: spline.closed,
            snap: spline.snap,
            straighten: spline.straighten,
        }
    }

    /// Tiles the current path would change.
    pub(super) fn spline_tile_count(&self) -> usize {
        self.spline.preview.as_ref().map_or(0, |p| p.tiles.len())
    }

    /// The overlay id the path lays, when the brush sets one.
    fn laying_overlay(&self) -> Option<u16> {
        let brush = &self.prefs.brush;
        match brush.overlay.fill {
            crate::paint::Fill::Set(id) if brush.overlay.armed => Some(id),
            _ => None,
        }
    }

    /// Keep the preview in step with the points and options: at once for the path as drawn, and
    /// again, slid to its cleanest position, once it has sat still for a moment.
    pub(super) fn spline_update(&mut self) {
        self.spline_refresh(false);
    }

    fn spline_refresh(&mut self, force_refine: bool) {
        let params = self.path_params();
        let overlay = self.laying_overlay();
        let key = key_of(
            &self.spline.points,
            params,
            (self.edited.len() as u64) << 32
                ^ self.infos_version
                ^ u64::from(overlay.unwrap_or(0)) << 48,
        );
        let current = self.spline.preview.as_ref().filter(|p| p.key == key);
        if current.is_some_and(|p| p.refined) {
            return;
        }
        if current.is_none() {
            self.spline.changed_at = Some(web_time::Instant::now());
        }
        let settled = self
            .spline
            .changed_at
            .is_none_or(|at| at.elapsed() >= REFINE_DELAY)
            && self.spline.drag.is_none();
        let refine = !self.spline.points.is_empty() && (force_refine || settled);
        if current.is_some() && !refine {
            return;
        }
        let plane = self.view_plane;
        let existing = |tile: (i32, i32)| {
            let id = overlay?;
            let values = self.current_tile_values((plane, tile.0, tile.1))?;
            (values.overlay == id).then_some((values.shape, values.rotation))
        };
        let tuning = autotile::Tuning::default();
        let mut refined_now = refine;
        let (points, used, tiles) = if self.spline.points.is_empty() {
            (Vec::new(), params, Vec::new())
        } else if refine && !force_refine && cfg!(not(target_arch = "wasm32")) {
            // The search takes a few hundred milliseconds: do it off the UI thread and keep
            // showing the path as drawn until it lands.
            let mut finished = None;
            if let Some((job_key, receiver)) = &self.spline.job
                && *job_key == key
            {
                match receiver.try_recv() {
                    Ok(result) => finished = Some(result),
                    Err(std::sync::mpsc::TryRecvError::Empty) => return,
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {}
                }
            }
            match finished {
                Some(result) => {
                    self.spline.job = None;
                    result
                }
                None => {
                    // Snapshot what the search needs from the editor: the path and the tiles
                    // around it that already carry the overlay.
                    let points = self.spline.points.clone();
                    let known = self.overlay_snapshot(overlay, &points, params.width);
                    let (sender, receiver) = std::sync::mpsc::channel();
                    self.spline.job = Some((key, receiver));
                    let repaint = self.spline.ctx.clone();
                    std::thread::spawn(move || {
                        let lookup = |tile: (i32, i32)| known.get(&tile).copied();
                        let result = autotile::generate_refined(&points, params, &lookup, &tuning);
                        let _ = sender.send(result);
                        if let Some(ctx) = repaint {
                            ctx.request_repaint();
                        }
                    });
                    return;
                }
            }
        } else if refine {
            let known = self.overlay_snapshot(overlay, &self.spline.points, params.width);
            let lookup = |tile: (i32, i32)| known.get(&tile).copied();
            autotile::generate_refined(&self.spline.points, params, &lookup, &tuning)
        } else {
            refined_now = false;
            (
                self.spline.points.clone(),
                params,
                autotile::generate(&self.spline.points, params, &existing),
            )
        };
        self.spline.preview = Some(Preview {
            key,
            tiles,
            refined: refined_now,
            points,
            params: used,
        });
    }

    /// Everything about the path in one block of text, to paste into a chat: the first line is a
    /// `PATH ...` case the auto-tiler playground can read back (`data/user-paths.txt`), the rest is
    /// what the editor made of it and the ground under it.
    pub(super) fn spline_report(&self) -> String {
        let (points, used) = self.drawn_path();
        let raw: Vec<String> = self
            .spline
            .points
            .iter()
            .map(|p| format!("{:.3},{:.3}", p.0, p.1))
            .collect();
        let mut text = format!(
            "PATH width={:.2} style={:?} curve={:?} closed={} snap={:.2} straighten={:.2} points={}\n",
            self.prefs.spline.width,
            self.prefs.spline.style,
            self.prefs.spline.curve,
            self.prefs.spline.closed,
            self.prefs.spline.snap,
            self.prefs.spline.straighten,
            raw.join(";")
        );
        let brush = &self.prefs.brush;
        text.push_str(&format!(
            "plane={} overlay={:?} build={}\n",
            self.view_plane,
            brush.overlay.fill,
            self.cache_build
                .as_ref()
                .map_or("?".to_owned(), |(b, _)| b.to_string())
        ));
        text.push_str(&format!(
            "tiled as: width={:.2} points={}\n",
            used.width,
            points
                .iter()
                .map(|p| format!("{:.3},{:.3}", p.0, p.1))
                .collect::<Vec<_>>()
                .join(";")
        ));
        if let Some(preview) = &self.spline.preview {
            let plane = self.view_plane;
            let heights: Vec<i32> = preview
                .tiles
                .iter()
                .filter_map(|(t, _)| {
                    self.current_tile_values((plane, t.0, t.1))
                        .map(|v| v.height)
                })
                .collect();
            if let (Some(low), Some(high)) = (heights.iter().min(), heights.iter().max()) {
                text.push_str(&format!(
                    "ground height under the path (client units): {low} to {high}\n"
                ));
            }
            text.push_str(&format!(
                "refined={} tiles={}:",
                preview.refined,
                preview.tiles.len()
            ));
            for (tile, choice) in &preview.tiles {
                match choice.shape_rotation() {
                    Some((shape, rotation)) => {
                        text.push_str(&format!(" {},{}:{shape}/{rotation}", tile.0, tile.1));
                    }
                    None => text.push_str(&format!(" {},{}:keep", tile.0, tile.1)),
                }
            }
            text.push('\n');
        }
        text
    }

    /// The tiles around the path that already carry `overlay`, with their shape and rotation (a
    /// snapshot the background search can read).
    fn overlay_snapshot(
        &self,
        overlay: Option<u16>,
        points: &[(f32, f32)],
        width: f32,
    ) -> std::collections::HashMap<(i32, i32), (u8, u8)> {
        let mut known = std::collections::HashMap::new();
        let Some(id) = overlay else {
            return known;
        };
        let reach = width + 8.0;
        let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
        for p in points {
            x0 = x0.min(p.0);
            y0 = y0.min(p.1);
            x1 = x1.max(p.0);
            y1 = y1.max(p.1);
        }
        let plane = self.view_plane;
        for x in (x0 - reach).floor() as i32..=(x1 + reach).ceil() as i32 {
            for y in (y0 - reach).floor() as i32..=(y1 + reach).ceil() as i32 {
                if let Some(values) = self.current_tile_values((plane, x, y))
                    && values.overlay == id
                {
                    known.insert((x, y), (values.shape, values.rotation));
                }
            }
        }
        known
    }

    /// The points and options the previewed tiles were made from (so the drawn curve matches them).
    fn drawn_path(&self) -> (Vec<(f32, f32)>, PathParams) {
        match &self.spline.preview {
            Some(p) if !p.points.is_empty() => (p.points.clone(), p.params),
            _ => (self.spline.points.clone(), self.path_params()),
        }
    }

    /// Add, move, insert and remove control points; Enter lays the path.
    pub(super) fn spline_input(&mut self, ctx: &egui::Context, response: &egui::Response) {
        self.spline.ctx.get_or_insert_with(|| ctx.clone());
        if self.prefs.tool != Tool::Spline {
            self.spline.drag = None;
            return;
        }
        self.spline_update();
        if self.spline.preview.as_ref().is_some_and(|p| !p.refined) {
            ctx.request_repaint_after(REFINE_DELAY);
        }
        if self.quick_open.is_some() || self.context_menu.is_some() {
            return;
        }
        if !ctx.egui_wants_keyboard_input() {
            if ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
                self.commit_spline();
            }
            if ctx.input(|i| i.key_pressed(egui::Key::Backspace)) {
                self.spline.points.pop();
            }
        }
        let Some(hit) = self.hover.tile else {
            if !ctx.input(|i| i.pointer.primary_down()) {
                self.spline.drag = None;
            }
            return;
        };
        let at = hit.at;
        let modifiers = ctx.input(|i| i.modifiers);
        let nearest = self
            .spline
            .points
            .iter()
            .enumerate()
            .map(|(i, p)| (i, distance(*p, at)))
            .filter(|(_, d)| *d <= GRAB_DISTANCE)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(i, _)| i);
        if !ctx.egui_wants_keyboard_input() && ctx.input(|i| i.key_pressed(egui::Key::Delete)) {
            match nearest {
                Some(index) => {
                    self.spline.points.remove(index);
                }
                None => {
                    self.spline.points.pop();
                }
            }
        }
        if ctx.input(|i| i.pointer.primary_pressed()) && response.is_pointer_button_down_on() {
            if modifiers.alt {
                return;
            }
            match nearest {
                Some(index) if modifiers.command => {
                    self.spline.points.remove(index);
                }
                Some(index) => self.spline.drag = Some(index),
                None if modifiers.shift && self.spline.points.len() >= 2 => {
                    // Insert on the stretch of the control polygon nearest the click.
                    let points = &self.spline.points;
                    let mut best = (f32::MAX, 0);
                    let count = if self.prefs.spline.closed && points.len() >= 3 {
                        points.len()
                    } else {
                        points.len() - 1
                    };
                    for i in 0..count {
                        let d = segment_distance(at, points[i], points[(i + 1) % points.len()]);
                        if d < best.0 {
                            best = (d, i);
                        }
                    }
                    self.spline.points.insert(best.1 + 1, at);
                    self.spline.drag = Some(best.1 + 1);
                }
                None => {
                    self.spline.points.push(at);
                    self.spline.drag = Some(self.spline.points.len() - 1);
                }
            }
        }
        if ctx.input(|i| i.pointer.primary_down()) {
            if let Some(index) = self.spline.drag
                && let Some(point) = self.spline.points.get_mut(index)
            {
                *point = at;
            }
        } else {
            self.spline.drag = None;
        }
    }

    /// The path's ramp: the terrain height at the first and last control points.
    fn ramp_heights(&self) -> Option<(i32, i32)> {
        let first = *self.spline.points.first()?;
        let last = *self.spline.points.last()?;
        let height = |p: (f32, f32)| {
            self.tile_now((p.0.floor() as i32, p.1.floor() as i32))
                .map(|t| t.height)
        };
        Some((height(first)?, height(last)?))
    }

    /// Lay the previewed path as one undo step.
    pub(super) fn commit_spline(&mut self) {
        self.spline_refresh(true);
        let tiles = self
            .spline
            .preview
            .as_ref()
            .map(|p| p.tiles.clone())
            .unwrap_or_default();
        let brush = &self.prefs.brush;
        if !brush.overlay.armed {
            self.notice = Some((
                "Choose an overlay to lay the path with".to_owned(),
                web_time::Instant::now(),
            ));
            return;
        }
        if tiles.is_empty() {
            return;
        }
        let grade = self.prefs.spline.ramp.then(|| {
            let (points, params) = self.drawn_path();
            let line = autotile::path_polyline(&points, params, STEP);
            self.ramp_heights()
                .map(|(first, last)| super::editing::PathGrade { line, first, last })
        });
        let changed = self.apply_path(&tiles, grade.flatten());
        self.spline.points.clear();
        self.spline.preview = None;
        self.notice = Some((
            format!("Laid a path over {changed} tiles"),
            web_time::Instant::now(),
        ));
    }

    /// The path preview: the shaped tiles, the spline, a ghost segment to the cursor, the control points.
    pub(super) fn draw_spline(
        &self,
        painter: &egui::Painter,
        viewport: Rect,
        pixels_per_point: f32,
        width: u32,
        height: u32,
    ) {
        if self.prefs.tool != Tool::Spline {
            return;
        }
        let plane = self.view_plane;
        let project = |p: [f64; 3]| self.screen_point(p, viewport, pixels_per_point, width, height);
        // A point on the ground at tile coordinates, lifted a little.
        let ground = |p: (f32, f32), lift: f64| -> Option<Pos2> {
            let tile = (p.0.floor() as i32, p.1.floor() as i32);
            let corners = interact::tile_corners(&self.infos, tile, plane)?;
            // Bilinear height inside the tile.
            let (u, v) = (f64::from(p.0 - p.0.floor()), f64::from(p.1 - p.1.floor()));
            let y = corners[0][1] * (1.0 - u) * (1.0 - v)
                + corners[1][1] * u * (1.0 - v)
                + corners[2][1] * u * v
                + corners[3][1] * (1.0 - u) * v;
            project([f64::from(p.0) * 128.0, y - lift, f64::from(p.1) * 128.0])
        };
        let cursor = self.hover.tile.map(|pick| pick.at);
        let outline = self.prefs.colors.get(ColorKey::PathLine);
        let ghost = self.prefs.colors.get(ColorKey::PathGhost);

        if self.spline.points.is_empty() {
            if let Some(at) = cursor
                && let Some(position) = ground(at, 8.0)
            {
                painter.circle_stroke(position, 9.0, Stroke::new(2.0, ghost));
                painter.text(
                    position + egui::vec2(14.0, -12.0),
                    egui::Align2::LEFT_CENTER,
                    "Click to start the path",
                    egui::FontId::proportional(12.0),
                    Color32::WHITE,
                );
            }
            return;
        }

        // The shaped tiles.
        let overlay_color = self.prefs.colors.get(ColorKey::PathFill);
        let edge = Stroke::new(0.75, self.prefs.colors.get(ColorKey::PathEdge));
        if let Some(preview) = &self.spline.preview {
            for (tile, choice) in &preview.tiles {
                let Some(corners) = interact::tile_corners(&self.infos, *tile, plane) else {
                    continue;
                };
                // Map a tile-local (u, v) in 0..1 to the tile's four corners (bilinear).
                let at = |u: f32, v: f32| -> Option<Pos2> {
                    let (u, v) = (f64::from(u), f64::from(v));
                    let lerp = |a: [f64; 3], b: [f64; 3], t: f64| {
                        [
                            a[0] + (b[0] - a[0]) * t,
                            a[1] + (b[1] - a[1]) * t,
                            a[2] + (b[2] - a[2]) * t,
                        ]
                    };
                    let south = lerp(corners[0], corners[1], u);
                    let north = lerp(corners[3], corners[2], u);
                    project(lerp(south, north, v))
                };
                match choice {
                    TileChoice::Keep => {}
                    TileChoice::Full => {
                        if let (Some(a), Some(b), Some(c), Some(d)) =
                            (at(0.0, 0.0), at(1.0, 0.0), at(1.0, 1.0), at(0.0, 1.0))
                        {
                            painter.add(egui::Shape::convex_polygon(
                                vec![a, b, c, d],
                                overlay_color,
                                edge,
                            ));
                        }
                    }
                    TileChoice::Shape { shape, rotation } => {
                        for (triangle, is_overlay) in shape_triangles(*shape, *rotation) {
                            if !is_overlay {
                                continue;
                            }
                            if let (Some(a), Some(b), Some(c)) = (
                                at(triangle[0][0], triangle[0][1]),
                                at(triangle[1][0], triangle[1][1]),
                                at(triangle[2][0], triangle[2][1]),
                            ) {
                                painter.add(egui::Shape::convex_polygon(
                                    vec![a, b, c],
                                    overlay_color,
                                    edge,
                                ));
                            }
                        }
                    }
                }
            }
        }

        // The spline itself.
        let (drawn_points, drawn_params) = self.drawn_path();
        let line = autotile::path_polyline(&drawn_points, drawn_params, STEP);
        let screen: Vec<Pos2> = line.iter().filter_map(|p| ground(*p, 10.0)).collect();
        if screen.len() >= 2 {
            painter.add(egui::Shape::line(
                screen.clone(),
                Stroke::new(4.5, Color32::from_black_alpha(110)),
            ));
            painter.add(egui::Shape::line(screen, Stroke::new(2.5, outline)));
        }
        // A ghost stretch from the last point to the cursor while no point is being dragged.
        if self.spline.drag.is_none()
            && let (Some(last), Some(at)) = (self.spline.points.last(), cursor)
            && let (Some(a), Some(b)) = (ground(*last, 10.0), ground(at, 10.0))
        {
            painter.add(egui::Shape::dashed_line(
                &[a, b],
                Stroke::new(1.5, ghost),
                6.0,
                5.0,
            ));
            painter.circle_stroke(b, 5.0, Stroke::new(1.5, ghost));
        }
        // The control points.
        for (index, point) in self.spline.points.iter().enumerate() {
            if let Some(position) = ground(*point, 12.0) {
                let active = self.spline.drag == Some(index);
                painter.circle_filled(position, if active { 8.0 } else { 6.5 }, Color32::WHITE);
                painter.circle_stroke(position, 7.5, Stroke::new(2.0, outline));
                painter.text(
                    position + egui::vec2(11.0, -11.0),
                    egui::Align2::LEFT_CENTER,
                    (index + 1).to_string(),
                    egui::FontId::proportional(11.5),
                    Color32::WHITE,
                );
            }
        }
    }
}
