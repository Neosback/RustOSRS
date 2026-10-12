//! The spline tool: click tiles to lay control points, see the shaped path, press Enter.
//!
//! Points are tile integers on the viewed plane. The curve is a uniform Catmull-Rom spline through
//! them; each style turns it into per-tile overlay shapes and rotations (Walls spawns castle walls
//! instead), drawn as a preview and laid as one undo step. Clicking within two tiles of a point
//! grabs it for dragging; Enter lays the path, Esc clears it.

use super::EditorApp;
use crate::autotile::{
    self, EdgeStyle, TileChoice, shape_triangles,
    walls::{WallKit, emit_walls},
};
use crate::interact;
use crate::paint::Tool;
use crate::style::ColorKey;
use eframe::egui::{self, Color32, Pos2, Rect, Stroke};
use std::collections::{HashMap, HashSet};

/// A click this close (in tiles) to a control point grabs it instead of adding a new one.
const GRAB_DISTANCE: f32 = 2.0;
/// Spacing of the preview curve.
const PREVIEW_STEP: f32 = 0.5;

#[derive(Debug, Default)]
pub struct SplineState {
    /// Control points in world tile coordinates.
    pub points: Vec<(i32, i32)>,
    drag: Option<usize>,
    preview: Option<Preview>,
}

impl SplineState {
    pub(super) fn clear(&mut self) {
        self.points.clear();
        self.preview = None;
        self.drag = None;
    }
}

#[derive(Debug, Default)]
struct Preview {
    key: u64,
    tiles: Vec<((i32, i32), TileChoice)>,
    /// The points the tiles were made from.
    points: Vec<(i32, i32)>,
    /// Per-tile band tangents for wall emission (Walls style only; empty otherwise).
    tangents: HashMap<(i32, i32), (f32, f32)>,
}

fn key_of(points: &[(i32, i32)], width: f32, style: EdgeStyle, terrain: u64, wall: u64) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    let mut mix = |value: u64| {
        hash ^= value;
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    };
    mix(width.to_bits().into());
    mix(style as u64);
    mix(wall);
    mix(terrain);
    for (x, y) in points {
        mix(*x as u64);
        mix(*y as u64);
    }
    hash
}

/// The wall-kit settings folded into the preview key so the preview refreshes when they change.
fn wall_key(spline: &crate::paint::SplineSettings) -> u64 {
    let mut key = u64::from(spline.wall_side);
    let mut mix = |value: u64| {
        key = key
            .wrapping_mul(0x0100_0000_01b3)
            .wrapping_add(value.wrapping_mul(0x9E37_79B9_7F4A_7C15));
    };
    mix(u64::from(spline.wall_top));
    mix(u64::from(spline.wall_door));
    mix(u64::from(spline.wall_diagonal));
    mix(spline.wall_thickness.to_bits().into());
    mix(spline.wall_height.to_bits().into());
    mix(u64::from(spline.wall_towers));
    mix(spline.tower_size.to_bits().into());
    key
}

fn distance(a: (i32, i32), b: (i32, i32)) -> f32 {
    let (dx, dy) = (a.0 - b.0, a.1 - b.1);
    ((dx * dx + dy * dy) as f32).sqrt()
}

impl EditorApp {
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

    /// Keep the preview in step with the points and options.
    pub(super) fn spline_update(&mut self) {
        self.spline_refresh();
    }

    fn spline_refresh(&mut self) {
        let spline = &self.prefs.spline;
        let (width, style) = (spline.width, spline.style);
        let overlay = self.laying_overlay();
        let key = key_of(
            &self.spline.points,
            width,
            style,
            (self.edited.len() as u64) << 32
                ^ self.infos_version
                ^ u64::from(overlay.unwrap_or(0)) << 48,
            wall_key(&self.prefs.spline),
        );
        if self.spline.preview.as_ref().is_some_and(|p| p.key == key) {
            return;
        }
        if self.spline.points.len() < 2 {
            self.spline.preview = None;
            return;
        }
        if style == EdgeStyle::Walls {
            // Castle walls: band tiles straight from the drawn points.
            let plan = autotile::generate_walls(
                &self.spline.points,
                spline.wall_thickness,
                if spline.wall_towers {
                    spline.tower_size
                } else {
                    1.0
                },
            );
            self.spline.preview = Some(Preview {
                key,
                tiles: plan.tiles,
                points: self.spline.points.clone(),
                tangents: plan.tangents,
            });
            return;
        }
        let tiles = autotile::generate(&self.spline.points, style, width);
        self.spline.preview = Some(Preview {
            key,
            tiles,
            points: self.spline.points.clone(),
            tangents: HashMap::new(),
        });
    }

    /// Everything about the path in one block of text, to paste into a chat.
    pub(super) fn spline_report(&self) -> String {
        let points = self
            .spline
            .preview
            .as_ref()
            .map_or(&self.spline.points, |p| &p.points);
        let raw: Vec<String> = self
            .spline
            .points
            .iter()
            .map(|p| format!("{},{}", p.0, p.1))
            .collect();
        let mut text = format!(
            "PATH width={:.2} style={:?} points={}\n",
            self.prefs.spline.width,
            self.prefs.spline.style,
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
            self.prefs.spline.width,
            points
                .iter()
                .map(|p| format!("{},{}", p.0, p.1))
                .collect::<Vec<_>>()
                .join(";")
        ));
        if self.prefs.spline.style == crate::autotile::EdgeStyle::Walls {
            let wall = &self.prefs.spline;
            text.push_str(&format!(
                "walls side={} top={} door={} diagonal={} thickness={:.1} height={:.1} towers={} tower_size={:.1}\n",
                wall.wall_side,
                wall.wall_top,
                wall.wall_door,
                wall.wall_diagonal,
                wall.wall_thickness,
                wall.wall_height,
                wall.wall_towers,
                wall.tower_size,
            ));
        }
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
            text.push_str(&format!("tiles={}:", preview.tiles.len()));
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

    /// Add and move control points; Enter lays the path, Esc clears it.
    pub(super) fn spline_input(&mut self, ctx: &egui::Context, response: &egui::Response) {
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
            if ctx.input(|i| i.key_pressed(egui::Key::Escape)) && !self.spline.points.is_empty() {
                self.spline.clear();
                return;
            }
        }
        let Some(hit) = self.hover.tile else {
            if !ctx.input(|i| i.pointer.primary_down()) {
                self.spline.drag = None;
            }
            return;
        };
        let at = hit.tile;
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
        if ctx.input(|i| i.pointer.primary_pressed()) && response.is_pointer_button_down_on() {
            if modifiers.alt {
                return;
            }
            match nearest {
                Some(index) => self.spline.drag = Some(index),
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
        if self.prefs.spline.style == EdgeStyle::Walls {
            self.commit_walls();
            return;
        }
        self.spline_refresh();
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
        let changed = self.apply_path(&tiles, Vec::new());
        self.spline.points.clear();
        self.spline.preview = None;
        self.notice = Some((
            format!("Laid a path over {changed} tiles"),
            web_time::Instant::now(),
        ));
    }

    /// Lay a castle wall: band overlay paint plus spawned wall objects, one undo step. The
    /// overlay is optional (walls still spawn).
    fn commit_walls(&mut self) {
        self.spline_refresh();
        let (tiles, tangents) = self
            .spline
            .preview
            .as_ref()
            .map(|preview| (preview.tiles.clone(), preview.tangents.clone()))
            .unwrap_or_default();
        let spline = &self.prefs.spline;
        let kit = WallKit {
            side: spline.wall_side,
            top: spline.wall_top,
            door: spline.wall_door,
            diagonal: spline.wall_diagonal,
        };
        let height = spline.wall_height.max(1.0).round() as u32;
        let band: HashSet<(i32, i32)> = tiles.iter().map(|(tile, _)| *tile).collect();
        let walls = emit_walls(&band, &tangents, &kit, height, self.view_plane);
        if tiles.is_empty() && walls.is_empty() {
            return;
        }
        let wall_count = walls.len();
        let changed = self.apply_path(&tiles, walls);
        self.spline.points.clear();
        self.spline.preview = None;
        let summary = if kit.side == 0 {
            format!("Laid a path over {changed} tiles (set a wall Side ID to spawn walls)")
        } else if changed == 0 {
            format!("Spawned {wall_count} walls")
        } else {
            format!("Laid a path over {changed} tiles and spawned {wall_count} walls")
        };
        self.notice = Some((summary, web_time::Instant::now()));
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
        let outline = self.prefs.colors.get(ColorKey::PathLine);

        if self.spline.points.is_empty() {
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
        let points = self
            .spline
            .preview
            .as_ref()
            .map_or(&self.spline.points, |p| &p.points);
        let line = autotile::evaluate(points, PREVIEW_STEP);
        let screen: Vec<Pos2> = line.iter().filter_map(|p| ground(*p, 10.0)).collect();
        if screen.len() >= 2 {
            painter.add(egui::Shape::line(
                screen.clone(),
                Stroke::new(4.5, Color32::from_black_alpha(110)),
            ));
            painter.add(egui::Shape::line(screen, Stroke::new(2.5, outline)));
        }
        // The control points.
        for (index, point) in self.spline.points.iter().enumerate() {
            let centered = (point.0 as f32 + 0.5, point.1 as f32 + 0.5);
            if let Some(position) = ground(centered, 12.0) {
                let active = self.spline.drag == Some(index);
                painter.circle_filled(position, if active { 8.0 } else { 6.5 }, Color32::WHITE);
                painter.circle_stroke(position, 7.5, Stroke::new(2.0, outline));
            }
        }
    }
}
