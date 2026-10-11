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
}

#[derive(Debug)]
struct Preview {
    key: u64,
    tiles: Vec<((i32, i32), TileChoice)>,
}

fn key_of(points: &[(f32, f32)], params: PathParams) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    let mut mix = |value: u64| {
        hash ^= value;
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    };
    mix(params.width.to_bits().into());
    mix(params.style as u64);
    mix(params.curve as u64);
    mix(u64::from(params.closed));
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
        }
    }

    /// Tiles the current path would change.
    pub(super) fn spline_tile_count(&self) -> usize {
        self.spline.preview.as_ref().map_or(0, |p| p.tiles.len())
    }

    /// Keep the preview in step with the points and options.
    pub(super) fn spline_update(&mut self) {
        let params = self.path_params();
        let key = key_of(&self.spline.points, params);
        if self.spline.preview.as_ref().is_some_and(|p| p.key == key) {
            return;
        }
        let tiles = if self.spline.points.is_empty() {
            Vec::new()
        } else {
            autotile::generate(&self.spline.points, params)
        };
        self.spline.preview = Some(Preview { key, tiles });
    }

    /// Add, move, insert and remove control points; Enter lays the path.
    pub(super) fn spline_input(&mut self, ctx: &egui::Context, response: &egui::Response) {
        if self.prefs.tool != Tool::Spline {
            self.spline.drag = None;
            return;
        }
        self.spline_update();
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
        self.spline_update();
        let tiles = self
            .spline
            .preview
            .as_ref()
            .map(|p| p.tiles.clone())
            .unwrap_or_default();
        let brush = &self.prefs.brush;
        if !(brush.overlay.armed || brush.underlay.armed) {
            self.notice = Some((
                "Choose an overlay or underlay to lay the path with".to_owned(),
                std::time::Instant::now(),
            ));
            return;
        }
        if tiles.is_empty() {
            return;
        }
        let grade = self.prefs.spline.ramp.then(|| {
            let line = autotile::path_polyline(&self.spline.points, self.path_params(), STEP);
            self.ramp_heights().map(|(first, last)| super::editing::PathGrade { line, first, last })
        });
        let changed = self.apply_path(&tiles, grade.flatten());
        self.spline.points.clear();
        self.spline.preview = None;
        self.notice = Some((
            format!("Laid a path over {changed} tiles"),
            std::time::Instant::now(),
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
        let outline = Color32::from_rgba_unmultiplied(255, 200, 80, 230);
        let ghost = Color32::from_rgba_unmultiplied(120, 200, 255, 200);

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
        let overlay_color = Color32::from_rgba_unmultiplied(255, 176, 56, 120);
        let edge = Stroke::new(0.75, Color32::from_rgba_unmultiplied(255, 236, 190, 150));
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
        let line = autotile::path_polyline(&self.spline.points, self.path_params(), STEP);
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
