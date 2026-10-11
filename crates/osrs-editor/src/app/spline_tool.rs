//! The spline tool: click points on the terrain, see the path the auto-tiler would lay, press Enter.
//!
//! The control points sit on tile centres. The path is a Catmull-Rom spline through them, `width`
//! tiles wide; the auto-tiler (`autotile.rs`) turns it into per-tile overlay shapes and rotations,
//! which are drawn as a preview and laid as one undo step.

use super::EditorApp;
use crate::autotile::{self, PathParams, TileChoice, shape_triangles};
use crate::interact;
use crate::paint::Tool;
use eframe::egui::{self, Color32, Pos2, Rect, Stroke};

/// A click this close (in tiles) to a control point grabs it instead of adding a new one.
const GRAB_DISTANCE: f32 = 0.9;

#[derive(Debug, Default)]
pub struct SplineState {
    /// Control points in world tile coordinates (tile centres).
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
    mix(u64::from(params.hard_edges));
    for (x, y) in points {
        mix(x.to_bits().into());
        mix(y.to_bits().into());
    }
    hash
}

impl EditorApp {
    fn path_params(&self) -> PathParams {
        PathParams {
            width: self.prefs.spline.width,
            hard_edges: self.prefs.spline.hard_edges,
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

    /// Add, move and remove control points; Enter lays the path.
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
            if ctx
                .input(|i| i.key_pressed(egui::Key::Backspace) || i.key_pressed(egui::Key::Delete))
            {
                self.spline.points.pop();
            }
        }
        let Some(tile) = self.hover.tile.map(|pick| pick.tile) else {
            if !ctx.input(|i| i.pointer.primary_down()) {
                self.spline.drag = None;
            }
            return;
        };
        let centre = (tile.0 as f32 + 0.5, tile.1 as f32 + 0.5);
        let modifiers = ctx.input(|i| i.modifiers);
        let nearest = self
            .spline
            .points
            .iter()
            .enumerate()
            .map(|(i, p)| {
                (
                    i,
                    ((p.0 - centre.0).powi(2) + (p.1 - centre.1).powi(2)).sqrt(),
                )
            })
            .filter(|(_, d)| *d <= GRAB_DISTANCE)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(i, _)| i);
        if ctx.input(|i| i.pointer.primary_pressed()) && response.is_pointer_button_down_on() {
            if modifiers.alt {
                return;
            }
            match nearest {
                Some(index) if modifiers.command => {
                    self.spline.points.remove(index);
                }
                Some(index) => self.spline.drag = Some(index),
                None => {
                    self.spline.points.push(centre);
                    self.spline.drag = Some(self.spline.points.len() - 1);
                }
            }
        }
        if ctx.input(|i| i.pointer.primary_down()) {
            if let Some(index) = self.spline.drag
                && let Some(point) = self.spline.points.get_mut(index)
            {
                *point = centre;
            }
        } else {
            self.spline.drag = None;
        }
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
                "Tick Overlay or Underlay to choose what the path is laid with".to_owned(),
                std::time::Instant::now(),
            ));
            return;
        }
        if tiles.is_empty() {
            return;
        }
        let changed = self.apply_path(&tiles);
        self.spline.points.clear();
        self.spline.preview = None;
        self.notice = Some((
            format!("Laid a path over {changed} tiles"),
            std::time::Instant::now(),
        ));
    }

    /// The path preview: the shaped tiles, the spline and its control points.
    pub(super) fn draw_spline(
        &self,
        painter: &egui::Painter,
        viewport: Rect,
        pixels_per_point: f32,
        width: u32,
        height: u32,
    ) {
        if self.prefs.tool != Tool::Spline || self.spline.points.is_empty() {
            return;
        }
        let plane = self.view_plane;
        let project = |p: [f64; 3]| self.screen_point(p, viewport, pixels_per_point, width, height);
        // A fixed, bright tint so the preview reads against any ground.
        let overlay_color = Color32::from_rgba_unmultiplied(255, 176, 56, 120);
        let edge = Stroke::new(0.75, Color32::from_rgba_unmultiplied(255, 236, 190, 150));
        let outline = Color32::from_rgba_unmultiplied(255, 200, 80, 220);
        if let Some(preview) = &self.spline.preview {
            for (tile, choice) in &preview.tiles {
                let Some(corners) = interact::tile_corners(&self.infos, *tile, plane) else {
                    continue;
                };
                let to_screen = |c: [f64; 3]| project(c);
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
                    to_screen(lerp(south, north, v))
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
        // The spline itself and its control points, lifted a little above the ground.
        let ground = |p: (f32, f32)| -> Option<Pos2> {
            let tile = (p.0.floor() as i32, p.1.floor() as i32);
            let corners = interact::tile_corners(&self.infos, tile, plane)?;
            let y = corners.iter().map(|c| c[1]).sum::<f64>() / 4.0 - 10.0;
            project([f64::from(p.0) * 128.0, y, f64::from(p.1) * 128.0])
        };
        let line = autotile::spline_polyline(&self.spline.points, 0.5);
        let screen: Vec<Pos2> = line.iter().filter_map(|p| ground(*p)).collect();
        if screen.len() >= 2 {
            painter.add(egui::Shape::line(screen, Stroke::new(2.0, outline)));
        }
        for (index, point) in self.spline.points.iter().enumerate() {
            if let Some(position) = ground(*point) {
                let active = self.spline.drag == Some(index);
                painter.circle_filled(position, if active { 7.0 } else { 6.0 }, Color32::WHITE);
                painter.circle_stroke(position, 6.5, Stroke::new(2.0, outline));
                painter.text(
                    position + egui::vec2(10.0, -10.0),
                    egui::Align2::LEFT_CENTER,
                    (index + 1).to_string(),
                    egui::FontId::proportional(11.0),
                    Color32::WHITE,
                );
            }
        }
    }
}
