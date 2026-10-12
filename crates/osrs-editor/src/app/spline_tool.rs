//! The path tool: click points on the terrain, see the path the planner would lay, press Enter.
//!
//! The points become a one-edge network that the terrain path planner (`terrain_path`, a port of
//! Terraini's) turns into overlay tiles: the line is smoothed, routed round obstacles, made to
//! wander, given a width that varies along it and a rough edge, and every tile gets the overlay
//! shape that real maps use for its neighbourhood. The result is drawn as a preview and laid as one
//! undo step. In Walls mode the same line becomes a band of tiles with wall objects on its edges.
//!
//! Dragging a point re-plans live (off the UI thread where there are threads); Shift + click
//! inserts a point on the nearest stretch; Cmd + click or Delete removes the point under the
//! cursor; Backspace removes the last.

use super::EditorApp;
use super::editing::PathTile;
use crate::interact;
use crate::paint::{PathMode, Tool};
use crate::style::ColorKey;
use crate::terrain_path::{
    self, PathEnvironment, PathPlanner, PathPoint, PathWarning, SceneGrid, Severity, TilePaint,
    walls::{WallKit, WallPlan, emit_walls, generate_walls},
};
use eframe::egui::{self, Color32, Pos2, Rect, Stroke};
use std::collections::{HashMap, HashSet};

/// A click this close (in tiles) to a control point grabs it instead of adding a new one.
const GRAB_DISTANCE: f32 = 0.9;
/// Spacing of the polyline used for drawing and for walls.
const STEP: f64 = 0.25;

#[derive(Debug, Default)]
pub struct SplineState {
    /// Control points in world tile coordinates.
    pub points: Vec<(f32, f32)>,
    drag: Option<usize>,
    preview: Option<Preview>,
    /// The plan being made on another thread, with the key it is for.
    job: Option<(u64, std::sync::mpsc::Receiver<Preview>)>,
    ctx: Option<egui::Context>,
}

#[derive(Debug, Default, Clone)]
struct Preview {
    key: u64,
    paints: Vec<TilePaint>,
    warnings: Vec<PathWarning>,
    /// The centre line as planned (or the smoothed drawn line when there is no plan).
    lines: Vec<Vec<(f32, f32)>>,
    /// Walls mode: the band's tiles and their tangents.
    wall_tiles: Vec<(i32, i32)>,
    tangents: HashMap<(i32, i32), (f32, f32)>,
}

fn fnv(text: &str) -> u64 {
    terrain_path::fnv(text)
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

fn to_points(points: &[(f32, f32)]) -> Vec<PathPoint> {
    points
        .iter()
        .map(|p| PathPoint::new(f64::from(p.0), f64::from(p.1)))
        .collect()
}

fn to_line(points: &[PathPoint]) -> Vec<(f32, f32)> {
    points.iter().map(|p| (p.x as f32, p.y as f32)).collect()
}

/// Everything a plan is made from, copied off the editor so another thread can make it.
struct PlanInput {
    key: u64,
    mode: PathMode,
    plane: u8,
    points: Vec<(f32, f32)>,
    settings: terrain_path::PathSettings,
    profile: terrain_path::PathProfile,
    env: PathEnvironment,
    wall_thickness: f32,
    tower_size: f32,
}

impl PlanInput {
    fn make(self) -> Preview {
        let points = to_points(&self.points);
        match self.mode {
            PathMode::Terrain => {
                let plan = PathPlanner.plan_stroke(&points, self.plane, &self.profile, &self.settings, &self.env);
                Preview {
                    key: self.key,
                    paints: plan.paints,
                    warnings: plan.warnings,
                    lines: plan.lines.iter().map(|l| to_line(l)).collect(),
                    ..Preview::default()
                }
            }
            PathMode::Walls => {
                let line = terrain_path::smooth_line(&points, self.settings.smoothness, STEP);
                let line = to_line(&line);
                let WallPlan { tiles, tangents } =
                    generate_walls(&line, &self.points, self.wall_thickness, self.tower_size);
                Preview {
                    key: self.key,
                    lines: vec![line],
                    wall_tiles: tiles,
                    tangents,
                    ..Preview::default()
                }
            }
        }
    }
}

impl EditorApp {
    /// Tiles the current path would change.
    pub(super) fn spline_tile_count(&self) -> usize {
        self.spline
            .preview
            .as_ref()
            .map_or(0, |p| p.paints.len().max(p.wall_tiles.len()))
    }

    /// What the planner has to say about the previewed path (reroutes, blocked tiles ...), one
    /// line per kind of note.
    pub(super) fn spline_notes(&self) -> Vec<(Severity, String)> {
        let Some(preview) = &self.spline.preview else {
            return Vec::new();
        };
        let mut seen: Vec<(Severity, &'static str, String, usize)> = Vec::new();
        for warning in &preview.warnings {
            match seen.iter_mut().find(|n| n.1 == warning.code) {
                Some(note) => note.3 += 1,
                None => seen.push((warning.severity, warning.code, warning.message.clone(), 1)),
            }
        }
        seen.into_iter()
            .map(|(severity, code, message, count)| {
                let text = match code {
                    "REROUTED" => "Went round obstacles".to_owned(),
                    "ROUTE_BLOCKED" => "Could not route round the obstacles; it follows the line you drew".to_owned(),
                    "OUT_OF_BOUNDS" => "Part of the path is off the loaded map".to_owned(),
                    "OBSTACLE_CONFLICT" => "Crosses a solid object or a steep slope".to_owned(),
                    "EXISTING_OVERLAY" => format!("Kept {count} tile(s) that already have a different overlay"),
                    "TOPOLOGY_SEAM" => format!("{count} seam(s) between shapes do not meet"),
                    "TOPOLOGY_COMPONENT" => "The path is in separate pieces".to_owned(),
                    _ => message,
                };
                (severity, text)
            })
            .collect()
    }

    /// The overlay id the path lays, when the brush sets one.
    fn laying_overlay(&self) -> Option<u16> {
        let brush = &self.prefs.brush;
        match brush.overlay.fill {
            crate::paint::Fill::Set(id) if brush.overlay.armed => Some(id),
            _ => None,
        }
    }

    /// A profile for the planner: the tool's profile laying the brush's overlay.
    fn path_profile(&self, overlay: u16) -> terrain_path::PathProfile {
        let mut profile = self.prefs.spline.profile.clone();
        profile.primary_overlay_id = overlay.max(1);
        profile.sanitized()
    }

    /// The tiles round the path that already carry an overlay (a path keeps them and joins onto
    /// them).
    fn existing_paints(&self, points: &[(f32, f32)], reach: f32) -> Vec<TilePaint> {
        let plane = self.view_plane;
        let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
        for p in points {
            x0 = x0.min(p.0);
            y0 = y0.min(p.1);
            x1 = x1.max(p.0);
            y1 = y1.max(p.1);
        }
        let mut paints = Vec::new();
        for x in (x0 - reach).floor() as i32..=(x1 + reach).ceil() as i32 {
            for y in (y0 - reach).floor() as i32..=(y1 + reach).ceil() as i32 {
                if let Some(values) = self.current_tile_values((plane, x, y))
                    && values.overlay != 0
                    && values.shape <= 11
                    && values.rotation <= 3
                {
                    paints.push(TilePaint {
                        tile_x: x,
                        tile_y: y,
                        plane,
                        overlay_id: values.overlay,
                        shape: values.shape,
                        orientation: values.rotation,
                        coverage: 1.0,
                        profile_id: "existing".to_owned(),
                    });
                }
            }
        }
        paints
    }

    /// Everything the current path is planned from, or `None` when there is nothing to plan.
    fn plan_input(&self) -> Option<PlanInput> {
        let spline = &self.prefs.spline;
        if self.spline.points.len() < 2 {
            return None;
        }
        let overlay = self.laying_overlay();
        if spline.mode == PathMode::Terrain && overlay.is_none() {
            return None;
        }
        let settings = spline.planner.sanitized();
        let profile = self.path_profile(overlay.unwrap_or(1));
        let key = fnv(&format!(
            "{:?}|{:?}|{:?}|{:?}|{}|{}|{}|{}|{:?}",
            self.spline.points,
            settings,
            profile,
            spline.mode,
            self.view_plane,
            self.infos_version,
            self.edited.len(),
            spline.wall_thickness.to_bits() ^ spline.tower_size.to_bits(),
            (spline.wall_towers, spline.snap_points),
        ));
        let reach = (profile.base_width * settings.width_scale) as f32
            + settings.route_corridor as f32
            + settings.snap_radius as f32
            + 6.0;
        let existing = if spline.mode == PathMode::Terrain {
            self.existing_paints(&self.spline.points, reach)
        } else {
            Vec::new()
        };
        let grid = SceneGrid::new(self.infos.clone());
        let env = PathEnvironment {
            bounds: grid.bounds(),
            cost_grid: std::sync::Arc::new(grid),
            existing_paints: existing,
        };
        Some(PlanInput {
            key,
            mode: spline.mode,
            plane: self.view_plane,
            points: self.spline.points.clone(),
            settings,
            profile,
            env,
            wall_thickness: spline.wall_thickness,
            tower_size: if spline.wall_towers {
                spline.tower_size
            } else {
                1.0
            },
        })
    }

    /// Keep the preview in step with the points and options.
    pub(super) fn spline_update(&mut self) {
        // A plan finished on its thread?
        if let Some((key, receiver)) = &self.spline.job {
            match receiver.try_recv() {
                Ok(preview) => {
                    let _ = key;
                    self.spline.job = None;
                    self.spline.preview = Some(preview);
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => return,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => self.spline.job = None,
            }
        }
        let Some(input) = self.plan_input() else {
            self.spline.preview = None;
            return;
        };
        if self.spline.preview.as_ref().is_some_and(|p| p.key == input.key) {
            return;
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let (sender, receiver) = std::sync::mpsc::channel();
            self.spline.job = Some((input.key, receiver));
            let repaint = self.spline.ctx.clone();
            std::thread::spawn(move || {
                let _ = sender.send(input.make());
                if let Some(ctx) = repaint {
                    ctx.request_repaint();
                }
            });
        }
        #[cfg(target_arch = "wasm32")]
        {
            self.spline.preview = Some(input.make());
        }
    }

    /// Plan now, on this thread, so what is laid is what the points and options say.
    fn spline_plan_now(&mut self) {
        self.spline.job = None;
        match self.plan_input() {
            Some(input) => {
                if self.spline.preview.as_ref().is_none_or(|p| p.key != input.key) {
                    self.spline.preview = Some(input.make());
                }
            }
            None => self.spline.preview = None,
        }
    }

    /// Everything about the path in one block of text, to paste into a chat: the points, the
    /// options and what the planner made of them.
    pub(super) fn spline_report(&self) -> String {
        let spline = &self.prefs.spline;
        let raw: Vec<String> = self
            .spline
            .points
            .iter()
            .map(|p| format!("{:.3},{:.3}", p.0, p.1))
            .collect();
        let mut text = format!(
            "PATH mode={:?} plane={} points={}\n",
            spline.mode,
            self.view_plane,
            raw.join(";")
        );
        text.push_str(&format!(
            "overlay={:?} build={}\n",
            self.prefs.brush.overlay.fill,
            self.cache_build
                .as_ref()
                .map_or("?".to_owned(), |(b, _)| b.to_string())
        ));
        text.push_str(&format!("settings={:?}\nprofile={:?}\n", spline.planner, spline.profile));
        if let Some(preview) = &self.spline.preview {
            text.push_str(&format!(
                "planned {} tiles, {} walls band tiles\n",
                preview.paints.len(),
                preview.wall_tiles.len()
            ));
            for warning in &preview.warnings {
                text.push_str(&format!(
                    "{:?} {} {}\n",
                    warning.severity, warning.code, warning.message
                ));
            }
        }
        text
    }

    /// Add, move, insert and remove control points; Enter lays the path.
    pub(super) fn spline_input(&mut self, ctx: &egui::Context, response: &egui::Response) {
        self.spline.ctx.get_or_insert_with(|| ctx.clone());
        if self.prefs.tool != Tool::Spline {
            self.spline.drag = None;
            return;
        }
        self.spline_update();
        if self.quick_open.is_some() || self.context_menu.is_some() {
            return;
        }
        if !self.keys_blocked(ctx) {
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
        let at = if self.prefs.spline.snap_points {
            (hit.tile.0 as f32 + 0.5, hit.tile.1 as f32 + 0.5)
        } else {
            hit.at
        };
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
        if !self.keys_blocked(ctx) && ctx.input(|i| i.key_pressed(egui::Key::Delete)) {
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
                    for i in 0..points.len() - 1 {
                        let d = segment_distance(at, points[i], points[i + 1]);
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

    /// Lay the previewed path as one undo step.
    pub(super) fn commit_spline(&mut self) {
        if self.prefs.spline.mode == PathMode::Walls {
            self.commit_walls();
            return;
        }
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
                "The path has no tiles to lay (it may be blocked or already covered)".to_owned(),
                web_time::Instant::now(),
            ));
            return;
        }
        let tiles: Vec<PathTile> = preview
            .paints
            .iter()
            .map(|p| PathTile {
                tile: (p.tile_x, p.tile_y),
                overlay: Some(p.overlay_id),
                shape: p.shape,
                rotation: p.orientation,
            })
            .collect();
        let changed = self.apply_path(&tiles, Vec::new());
        self.spline.points.clear();
        self.notice = Some((
            format!("Laid a path over {changed} tiles"),
            web_time::Instant::now(),
        ));
    }

    /// Lay a castle wall: band overlay paint plus spawned wall objects, one undo step. The overlay
    /// is optional (walls still spawn).
    fn commit_walls(&mut self) {
        self.spline_plan_now();
        let Some(preview) = self.spline.preview.take() else {
            return;
        };
        let spline = &self.prefs.spline;
        let kit = WallKit {
            side: spline.wall_side,
            top: spline.wall_top,
            door: spline.wall_door,
            diagonal: spline.wall_diagonal,
        };
        let height = spline.wall_height.max(1.0).round() as u32;
        let overlay = self.laying_overlay();
        let band: HashSet<(i32, i32)> = preview.wall_tiles.iter().copied().collect();
        let walls = emit_walls(&band, &preview.tangents, &kit, height, self.view_plane);
        if band.is_empty() && walls.is_empty() {
            return;
        }
        let tiles: Vec<PathTile> = preview
            .wall_tiles
            .iter()
            .map(|tile| PathTile {
                tile: *tile,
                overlay,
                shape: 0,
                rotation: 0,
            })
            .collect();
        let wall_count = walls.len();
        let changed = self.apply_path(&tiles, walls);
        self.spline.points.clear();
        let summary = if kit.side == 0 {
            format!("Laid a band over {changed} tiles (set a wall Side ID to spawn walls)")
        } else if changed == 0 {
            format!("Spawned {wall_count} walls")
        } else {
            format!("Laid a band over {changed} tiles and spawned {wall_count} walls")
        };
        self.notice = Some((summary, web_time::Instant::now()));
    }

    /// The path preview: the planned tiles, the centre line, a ghost segment to the cursor, the
    /// control points.
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

        // The planned tiles.
        let overlay_color = self.prefs.colors.get(ColorKey::PathFill);
        let edge = Stroke::new(0.75, self.prefs.colors.get(ColorKey::PathEdge));
        if let Some(preview) = &self.spline.preview {
            let mut tile_polygon = |tile: (i32, i32), shape: u8, rotation: u8| {
                let Some(corners) = interact::tile_corners(&self.infos, tile, plane) else {
                    return;
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
                for triangle in terrain_path::overlay_triangles(shape, rotation) {
                    if let (Some(a), Some(b), Some(c)) = (
                        at(triangle[0][0], triangle[0][1]),
                        at(triangle[1][0], triangle[1][1]),
                        at(triangle[2][0], triangle[2][1]),
                    ) {
                        painter.add(egui::Shape::convex_polygon(vec![a, b, c], overlay_color, edge));
                    }
                }
            };
            for paint in &preview.paints {
                tile_polygon((paint.tile_x, paint.tile_y), paint.shape, paint.orientation);
            }
            for tile in &preview.wall_tiles {
                tile_polygon(*tile, 0, 0);
            }
        }

        // The centre line.
        let drawn: Vec<(f32, f32)> = match &self.spline.preview {
            Some(preview) if !preview.lines.is_empty() => {
                preview.lines.iter().flatten().copied().collect()
            }
            _ => to_line(&terrain_path::smooth_line(
                &to_points(&self.spline.points),
                self.prefs.spline.planner.smoothness,
                STEP,
            )),
        };
        let screen: Vec<Pos2> = drawn.iter().filter_map(|p| ground(*p, 10.0)).collect();
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
