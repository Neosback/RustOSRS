//! World map window and the compass HUD.
//!
//! The world map shows every map region as one pixel per tile (see `osrs_world::assets`). Drag to
//! pan, scroll to zoom, double-click a tile to fly there. The compass is the game's own compass
//! sprite, rotated with the camera; clicking it faces north.

use super::EditorApp;
use crate::assets::sprite;
use crate::hud::Destination;
use eframe::egui;

const MIN_ZOOM: f32 = 0.25;
const MAX_ZOOM: f32 = 16.0;
const REGION_TILES: f32 = 64.0;

/// World map window state.
pub struct WorldMapView {
    pub open: bool,
    /// World tile at the centre of the window.
    center: (f64, f64),
    /// Screen pixels per tile.
    zoom: f32,
    centered_once: bool,
}

impl WorldMapView {
    pub fn new() -> Self {
        Self {
            // Smoke-test hook: `RUSTOSRS_OPEN=worldmap`.
            open: std::env::var("RUSTOSRS_OPEN").is_ok_and(|value| value == "worldmap"),
            center: (3222.0, 3218.0),
            zoom: 2.0,
            centered_once: false,
        }
    }
}

impl EditorApp {
    pub(super) fn toggle_world_map(&mut self) {
        self.world_map.open = !self.world_map.open;
    }

    pub(super) fn world_map_window(&mut self, ctx: &egui::Context) {
        if !self.world_map.open {
            return;
        }
        if !self.world_map.centered_once {
            self.world_map.centered_once = true;
            let (x, z) = self.world_position();
            self.world_map.center = (x / 128.0, z / 128.0);
        }
        let region = self.camera_region();
        self.assets.ensure_world_map(region);

        let mut open = true;
        let mut teleport = None;
        egui::Window::new(format!("{} World map", crate::icons::WORLD_MAP))
            .open(&mut open)
            .default_size([900.0, 640.0])
            .resizable(true)
            .show(ctx, |ui| {
                let available = ui.available_size();
                let (rect, response) =
                    ui.allocate_exact_size(available, egui::Sense::click_and_drag());
                let painter = ui.painter_at(rect);
                painter.rect_filled(rect, 0.0, egui::Color32::from_rgb(12, 20, 40));

                let map = &mut self.world_map;
                if response.dragged() {
                    let delta = response.drag_delta();
                    map.center.0 -= f64::from(delta.x / map.zoom);
                    map.center.1 += f64::from(delta.y / map.zoom);
                }
                let pointer = response.hover_pos();
                if response.hovered() {
                    let scroll =
                        ctx.input(|i| i.smooth_scroll_delta.y + (i.zoom_delta() - 1.0) * 400.0);
                    if scroll != 0.0 {
                        // Zoom about the cursor: keep the tile under it fixed.
                        let anchor = pointer.unwrap_or(rect.center());
                        let before = screen_to_tile(map, rect, anchor);
                        map.zoom = (map.zoom * (1.0 + scroll * 0.002)).clamp(MIN_ZOOM, MAX_ZOOM);
                        let after = screen_to_tile(map, rect, anchor);
                        map.center.0 += before.0 - after.0;
                        map.center.1 += before.1 - after.1;
                    }
                }

                // Region textures that intersect the window.
                let zoom = map.zoom;
                let half = rect.size() / (2.0 * zoom);
                let (min_x, max_x) = (
                    ((map.center.0 - f64::from(half.x)) / 64.0).floor() as i32,
                    ((map.center.0 + f64::from(half.x)) / 64.0).floor() as i32,
                );
                let (min_y, max_y) = (
                    ((map.center.1 - f64::from(half.y)) / 64.0).floor() as i32,
                    ((map.center.1 + f64::from(half.y)) / 64.0).floor() as i32,
                );
                for rx in min_x..=max_x {
                    for ry in min_y..=max_y {
                        let Some(texture) = self.assets.map_tiles.get(&(rx, ry)) else {
                            continue;
                        };
                        // Top-left screen corner of the region (its north-west tile corner).
                        let top_left = tile_to_screen(
                            &self.world_map,
                            rect,
                            (f64::from(rx) * 64.0, f64::from(ry + 1) * 64.0),
                        );
                        let region_rect = egui::Rect::from_min_size(
                            top_left,
                            egui::vec2(REGION_TILES * zoom, REGION_TILES * zoom),
                        );
                        painter.image(
                            texture.id(),
                            region_rect,
                            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                            egui::Color32::WHITE,
                        );
                    }
                }

                // Camera marker.
                let (cx, cz) = self.world_position();
                let marker = tile_to_screen(&self.world_map, rect, (cx / 128.0, cz / 128.0));
                if rect.contains(marker) {
                    let yaw = self.camera.yaw;
                    let tip = marker + egui::vec2(-yaw.sin(), -yaw.cos()) * 14.0;
                    painter.circle_filled(marker, 4.0, egui::Color32::from_rgb(255, 200, 0));
                    painter.line_segment(
                        [marker, tip],
                        egui::Stroke::new(2.0, egui::Color32::from_rgb(255, 200, 0)),
                    );
                }

                if let Some(pointer) = pointer {
                    let tile = screen_to_tile(&self.world_map, rect, pointer);
                    let (tx, ty) = (tile.0.floor() as i32, tile.1.floor() as i32);
                    painter.text(
                        rect.left_bottom() + egui::vec2(8.0, -8.0),
                        egui::Align2::LEFT_BOTTOM,
                        format!(
                            "{tx}, {ty}   region {}   (double-click to go there)",
                            crate::hud::region_id(tx.div_euclid(64), ty.div_euclid(64))
                        ),
                        egui::FontId::monospace(13.0),
                        egui::Color32::WHITE,
                    );
                    if response.double_clicked() {
                        teleport = Some((tx, ty));
                    }
                }
                if !self.assets.map_done {
                    painter.text(
                        rect.right_top() + egui::vec2(-8.0, 8.0),
                        egui::Align2::RIGHT_TOP,
                        format!("loading map... {} regions", self.assets.map_tiles.len()),
                        egui::FontId::proportional(13.0),
                        egui::Color32::WHITE,
                    );
                }
            });
        self.world_map.open = open;
        if let Some((x, y)) = teleport {
            self.teleport_to(Destination::Tile {
                x,
                y,
                plane: self.view_plane,
            });
        }
    }

    /// Compass HUD: the game's compass sprite, turned with the camera, in the viewport's corner.
    pub(super) fn compass_hud(&mut self, ui: &egui::Ui, rect: egui::Rect) {
        if !self.prefs.show_compass {
            return;
        }
        let yaw = self.camera.yaw;
        let Some(texture) = self.assets.sprite(sprite::COMPASS) else {
            return;
        };
        let texture_id = texture.id();
        let size = texture.size_vec2() * 2.0;
        let center = rect.right_top() + egui::vec2(-size.x * 0.5 - 14.0, size.y * 0.5 + 14.0);
        let response = ui.interact(
            egui::Rect::from_center_size(center, size),
            egui::Id::new("compass_hud"),
            egui::Sense::click(),
        );
        // A sprite turned about its centre: build the rotated quad by hand.
        let (sin, cos) = yaw.sin_cos();
        let corner = |x: f32, y: f32| {
            let (dx, dy) = (x * size.x * 0.5, y * size.y * 0.5);
            center + egui::vec2(dx * cos + dy * sin, -dx * sin + dy * cos)
        };
        let mut mesh = egui::epaint::Mesh::with_texture(texture_id);
        let uv = [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)];
        for (index, (x, y)) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)]
            .into_iter()
            .enumerate()
        {
            mesh.vertices.push(egui::epaint::Vertex {
                pos: corner(x, y),
                uv: egui::pos2(uv[index].0, uv[index].1),
                color: egui::Color32::WHITE,
            });
        }
        mesh.indices.extend([0, 1, 2, 0, 2, 3]);
        ui.painter()
            .with_clip_rect(rect)
            .add(egui::Shape::mesh(mesh));
        if response.on_hover_text("Face north").clicked() {
            self.camera.yaw = 0.0;
        }
    }
}

fn tile_to_screen(map: &WorldMapView, rect: egui::Rect, tile: (f64, f64)) -> egui::Pos2 {
    egui::pos2(
        rect.center().x + ((tile.0 - map.center.0) as f32) * map.zoom,
        rect.center().y - ((tile.1 - map.center.1) as f32) * map.zoom,
    )
}

fn screen_to_tile(map: &WorldMapView, rect: egui::Rect, position: egui::Pos2) -> (f64, f64) {
    (
        map.center.0 + f64::from((position.x - rect.center().x) / map.zoom),
        map.center.1 - f64::from((position.y - rect.center().y) / map.zoom),
    )
}
