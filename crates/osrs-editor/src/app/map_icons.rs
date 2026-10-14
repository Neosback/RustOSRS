//! Map icons (banks, shops, altars ...) as billboards over their tiles in the 3D view: the same icons
//! the minimap and the world map draw, placed in the world so you can see what is where while flying
//! around. Off by default (Overlays menu or the Interface settings).

use super::EditorApp;
use crate::interact;
use eframe::egui::{self, Color32, Rect, Stroke, Vec2};

/// Icons further than this (tiles) from the camera are not drawn, and fade out over the last third.
const REACH_TILES: f32 = 48.0;
/// How far above the ground an icon floats (local units; up is negative).
const LIFT: f64 = 150.0;

impl EditorApp {
    pub(super) fn draw_map_icons_3d(
        &mut self,
        painter: &egui::Painter,
        viewport: Rect,
        pixels_per_point: f32,
        width: u32,
        height: u32,
    ) {
        if !self.prefs.map_icons_3d {
            return;
        }
        let Some(catalog) = self.assets.catalog.clone() else {
            return;
        };
        let plane = self.interaction.view_plane;
        let (rx, ry) = self.camera_region();
        let (camera_x, camera_y) = self.camera_tile();
        // Collect first: asking for a region may request it from the worker.
        let mut regions = Vec::new();
        for dx in -1..=1 {
            for dy in -1..=1 {
                let region = (rx + dx, ry + dy);
                if let Some(Some(data)) = self.assets.minimap_region(region, plane) {
                    regions.push(data);
                }
            }
        }
        let mut icons: Vec<(f32, Color32, egui::TextureId, Vec2, egui::Pos2, egui::Pos2)> =
            Vec::new();
        for region in &regions {
            for icon in &region.icons {
                let tile = (
                    region.region.0 * 64 + i32::from(icon.x),
                    region.region.1 * 64 + i32::from(icon.y),
                );
                let distance = ((tile.0 - camera_x) as f32).hypot((tile.1 - camera_y) as f32);
                if distance > REACH_TILES {
                    continue;
                }
                let Some(element) = catalog.elements.get(&icon.element) else {
                    continue;
                };
                if element.sprite < 0 {
                    continue;
                }
                let Some(corners) = interact::tile_corners(&self.infos, tile, plane) else {
                    continue;
                };
                let ground = corners.iter().map(|c| c[1]).sum::<f64>() / 4.0;
                let (x, z) = (
                    f64::from(tile.0) * 128.0 + 64.0,
                    f64::from(tile.1) * 128.0 + 64.0,
                );
                let (Some(top), Some(foot)) = (
                    self.screen_point(
                        [x, ground - LIFT, z],
                        viewport,
                        pixels_per_point,
                        width,
                        height,
                    ),
                    self.screen_point([x, ground, z], viewport, pixels_per_point, width, height),
                ) else {
                    continue;
                };
                if !viewport.expand(40.0).contains(top) {
                    continue;
                }
                let fade = ((REACH_TILES - distance) / (REACH_TILES / 3.0)).clamp(0.0, 1.0);
                // The texture is looked up last: it borrows the assets mutably.
                let Some(texture) = self.assets.sprite(element.sprite as u32) else {
                    continue;
                };
                icons.push((
                    distance,
                    Color32::from_white_alpha((255.0 * fade) as u8),
                    texture.id(),
                    texture.size_vec2(),
                    top,
                    foot,
                ));
            }
        }
        // Far ones first, so near icons draw over them.
        icons.sort_by(|a, b| b.0.total_cmp(&a.0));
        for (_, tint, texture, size, top, foot) in icons {
            painter.line_segment(
                [top, foot],
                Stroke::new(1.0, Color32::from_white_alpha(tint.a() / 3)),
            );
            painter.circle_filled(foot, 2.0, Color32::from_white_alpha(tint.a() / 2));
            let rect = Rect::from_center_size(top, size);
            painter.circle_filled(
                top,
                size.max_elem() * 0.6,
                Color32::from_black_alpha(tint.a() / 3),
            );
            painter.image(
                texture,
                rect,
                Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                tint,
            );
        }
    }
}
