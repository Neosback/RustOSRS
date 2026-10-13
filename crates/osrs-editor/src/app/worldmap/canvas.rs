//! The world map canvas: tiles, icons, and pointer interaction.

use super::*;

impl EditorApp {
    /// Draws the map; returns the world tile and plane the user double-clicked.
    pub(super) fn world_map_canvas(
        &mut self,
        ui: &mut egui::Ui,
        catalog: &Catalog,
    ) -> Option<(i32, i32, i32)> {
        let available = ui.available_size();
        let (rect, response) = ui.allocate_exact_size(available, egui::Sense::click_and_drag());
        let painter = ui.painter_at(rect);
        let area_index = self.world_map.area;
        let area = &catalog.areas[area_index];
        painter.rect_filled(rect, 0.0, argb(area.background));

        // Pan and zoom.
        let zoom = self.world_map.zoom;
        if response.dragged() {
            let delta = response.drag_delta();
            self.world_map.center.0 -= f64::from(delta.x / zoom);
            self.world_map.center.1 += f64::from(delta.y / zoom);
        }
        if response.hovered() {
            let scroll = ui.input(|input| input.smooth_scroll_delta.y);
            let pinch = ui.input(|input| input.zoom_delta());
            if scroll > 0.5 || pinch > 1.01 {
                self.world_map.step_zoom(1);
            } else if scroll < -0.5 || pinch < 0.99 {
                self.world_map.step_zoom(-1);
            }
        }
        let zoom = self.world_map.zoom;
        let center = self.world_map.center;
        let to_screen = |x: f64, y: f64| {
            egui::pos2(
                rect.center().x + ((x - center.0) as f32) * zoom,
                rect.center().y - ((y - center.1) as f32) * zoom,
            )
        };
        let to_tile = |position: egui::Pos2| {
            (
                center.0 + f64::from((position.x - rect.center().x) / zoom),
                center.1 - f64::from((position.y - rect.center().y) / zoom),
            )
        };

        // New view settings drop the queued region work.
        let ppt = pixels_per_tile(zoom);
        if self.world_map.queued_for != Some((area_index, ppt)) {
            self.world_map.queued_for = Some((area_index, ppt));
            self.assets.new_map_view();
        }

        // The overview picture underneath while regions load.
        if let Some(overview) = self.assets.overview(area_index) {
            let top_left = to_screen(
                f64::from(area.low_x) * 64.0,
                f64::from(area.high_y + 1) * 64.0,
            );
            let bottom_right = to_screen(
                f64::from(area.high_x + 1) * 64.0,
                f64::from(area.low_y) * 64.0,
            );
            painter.image(
                overview.id(),
                egui::Rect::from_min_max(top_left, bottom_right),
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                egui::Color32::WHITE,
            );
        }

        // Region images, nearest the centre first.
        let half = rect.size() / (2.0 * zoom);
        let min_x = ((center.0 - f64::from(half.x)) / 64.0).floor() as i32;
        let max_x = ((center.0 + f64::from(half.x)) / 64.0).floor() as i32;
        let min_y = ((center.1 - f64::from(half.y)) / 64.0).floor() as i32;
        let max_y = ((center.1 + f64::from(half.y)) / 64.0).floor() as i32;
        let mut visible = Vec::new();
        for rx in min_x.max(area.low_x)..=max_x.min(area.high_x) {
            for ry in min_y.max(area.low_y)..=max_y.min(area.high_y) {
                if area.has_region(rx, ry) {
                    visible.push((rx, ry));
                }
            }
        }
        visible.sort_by(|a, b| {
            let da = (f64::from(a.0) * 64.0 + 32.0 - center.0).powi(2)
                + (f64::from(a.1) * 64.0 + 32.0 - center.1).powi(2);
            let db = (f64::from(b.0) * 64.0 + 32.0 - center.0).powi(2)
                + (f64::from(b.1) * 64.0 + 32.0 - center.1).powi(2);
            da.total_cmp(&db)
        });
        let mut loading = false;
        for (rx, ry) in visible {
            let top_left = to_screen(f64::from(rx) * 64.0, f64::from(ry + 1) * 64.0);
            let region_rect =
                egui::Rect::from_min_size(top_left, egui::vec2(64.0 * zoom, 64.0 * zoom));
            match self.assets.region((area_index, rx, ry, ppt)) {
                Some(Some(texture)) => {
                    painter.image(
                        texture.id(),
                        region_rect,
                        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                        egui::Color32::WHITE,
                    );
                }
                Some(None) => {}
                None => loading = true,
            }
        }

        // Icons and labels.
        if let Some(icons_in_area) = self.assets.icons(area_index) {
            self.draw_map_icons(&painter, rect, catalog, &icons_in_area, zoom, &to_screen);
        }

        // Camera marker.
        let (cx, cz) = self.world_position();
        let (tile_x, tile_y) = ((cx / 128.0).floor() as i32, (cz / 128.0).floor() as i32);
        if let Some((mx, my)) = area.position(i32::from(self.view_plane), tile_x, tile_y) {
            let marker = to_screen(f64::from(mx) + 0.5, f64::from(my) + 0.5);
            if rect.contains(marker) {
                let yaw = self.camera.yaw;
                let tip = marker + egui::vec2(-yaw.sin(), -yaw.cos()) * 16.0;
                let gold = egui::Color32::from_rgb(255, 204, 0);
                painter.line_segment([marker, tip], egui::Stroke::new(2.0, gold));
                painter.circle(
                    marker,
                    4.5,
                    gold,
                    egui::Stroke::new(1.5, egui::Color32::BLACK),
                );
            }
        }

        // Status line and double-click teleport.
        let mut teleport = None;
        if let Some(pointer) = response.hover_pos() {
            let tile = to_tile(pointer);
            let (tx, ty) = (tile.0.floor() as i32, tile.1.floor() as i32);
            let world = area.world_coord(tx, ty);
            let text = match world {
                Some(coord) => format!(
                    "{}, {}, {}   region {}   double-click to go there",
                    coord.x,
                    coord.y,
                    coord.plane,
                    crate::hud::region_id(coord.x.div_euclid(64), coord.y.div_euclid(64))
                ),
                None => "Outside this map".to_owned(),
            };
            painter.text(
                rect.left_bottom() + egui::vec2(8.0, -8.0),
                egui::Align2::LEFT_BOTTOM,
                text,
                egui::FontId::monospace(12.0),
                egui::Color32::WHITE,
            );
            if response.double_clicked()
                && let Some(coord) = world
            {
                teleport = Some((coord.x, coord.y, coord.plane));
            }
        }
        if loading || self.assets.region_requests_in_flight() > 0 {
            painter.text(
                rect.right_bottom() + egui::vec2(-8.0, -8.0),
                egui::Align2::RIGHT_BOTTOM,
                "loading...",
                egui::FontId::proportional(12.0),
                egui::Color32::WHITE,
            );
            ui.ctx().request_repaint();
        }
        teleport
    }

    /// `WorldMapRegion.drawNonLinkMapIcons` / `drawMapLinks` with the client's placement.
    pub(super) fn draw_map_icons(
        &mut self,
        painter: &egui::Painter,
        rect: egui::Rect,
        catalog: &Catalog,
        icons_in_area: &[MapIcon],
        zoom: f32,
        to_screen: &dyn Fn(f64, f64) -> egui::Pos2,
    ) {
        let margin = 40.0;
        let clip = rect.expand(margin);
        for icon in icons_in_area {
            let Some(element) = catalog.elements.get(&icon.element) else {
                continue;
            };
            if self.world_map.hidden.contains(&element.category) {
                continue;
            }
            // Scenery icons sit half a tile left of their tile, vertically centred; fixed
            // icons at the tile's top-left corner.
            let position = if icon.link {
                to_screen(f64::from(icon.x), f64::from(icon.y + 1))
            } else {
                to_screen(f64::from(icon.x), f64::from(icon.y) + 0.5) - egui::vec2(zoom * 0.5, 0.0)
            };
            let position = egui::pos2(position.x.trunc(), position.y.trunc());
            if !clip.contains(position) {
                continue;
            }
            if element.sprite >= 0
                && let Some(texture) = self.assets.sprite(element.sprite as u32)
            {
                let size = texture.size_vec2();
                let dx = match element.horizontal {
                    0 => -(size.x / 2.0).trunc(),
                    1 => 0.0,
                    _ => -size.x,
                };
                let dy = match element.vertical {
                    0 => -(size.y / 2.0).trunc(),
                    2 => 0.0,
                    _ => -size.y,
                };
                painter.image(
                    texture.id(),
                    egui::Rect::from_min_size(position + egui::vec2(dx, dy), size),
                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                    egui::Color32::WHITE,
                );
            }
            // Labels: small at 75% and up, medium from 50%, large always.
            let min_zoom = match element.text_size {
                0 => 3.0,
                1 => 2.0,
                _ => 0.0,
            };
            if let Some(name) = element.name.as_deref()
                && zoom >= min_zoom
                && let Some((texture, size, offset)) =
                    self.assets
                        .label(catalog, element.text_size, name, element.color)
            {
                painter.image(
                    texture,
                    egui::Rect::from_min_size(position + offset, size),
                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                    egui::Color32::WHITE,
                );
            }
        }
    }
}
