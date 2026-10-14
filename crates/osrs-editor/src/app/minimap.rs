//! Minimap HUD: the game's resizable-mode minimap, built from the cache like the client does.
//!
//! One setting (`show_minimap`) shows the whole assembly: the frame ring (sprite 1177), the scene
//! image composited through the circular alpha mask (1178) and rotated with the camera, the
//! compass (169 under mask 1179) and the world-map globe orb (1438/1439). The scene image comes
//! from `osrs_world::minimap` (tile colours, wall lines, map scenes and map icons, 4 pixels per
//! tile).

use super::EditorApp;
use crate::assets::sprite;
use crate::hud::Destination;
use eframe::egui;
use osrs_world::{SpriteImage, minimap::MinimapRegion};
use std::sync::Arc;

/// Sprite ids of the resizable-mode minimap (`SpriteID.RESIZEABLE_MODE_*`, `MINIMAP_ORB_WORLD_MAP_*`).
mod ids {
    pub const FRAME: u32 = 1177;
    pub const MAP_MASK: u32 = 1178;
    pub const COMPASS_MASK: u32 = 1179;
    pub const GLOBE_FRAME: u32 = 1438;
    pub const GLOBE: u32 = 1439;
    pub const GLOBE_HOVER: u32 = 1440;
}

/// Where the map circle and the compass sit inside the frame sprite (measured from the sprite's
/// transparent holes) and where the globe orb hangs.
const MAP_AT: (f32, f32) = (24.0, 8.0);
const COMPASS_AT: (f32, f32) = (5.0, 5.0);
const GLOBE_AT: (f32, f32) = (3.0, 124.0);
/// Minimap pixels per tile (the client's default).
const PX_PER_TILE: f32 = 4.0;

/// Textures re-uploaded as the camera moves.
#[derive(Default)]
pub struct MinimapHud {
    map: Option<egui::TextureHandle>,
    compass: Option<egui::TextureHandle>,
}

struct Sprites {
    frame: Arc<SpriteImage>,
    map_mask: Arc<SpriteImage>,
    compass_mask: Arc<SpriteImage>,
    compass: Arc<SpriteImage>,
}

impl EditorApp {
    /// The minimap assembly in the viewport's top-right corner.
    pub(super) fn minimap_hud(&mut self, ui: &egui::Ui, rect: egui::Rect) {
        if !self.prefs.show_minimap {
            return;
        }
        let scale = self.prefs.minimap_size.scale();
        let (Some(frame), Some(map_mask), Some(compass_mask), Some(compass)) = (
            self.assets.sprite_image(ids::FRAME),
            self.assets.sprite_image(ids::MAP_MASK),
            self.assets.sprite_image(ids::COMPASS_MASK),
            self.assets.sprite_image(sprite::COMPASS),
        ) else {
            return;
        };
        let sprites = Sprites {
            frame,
            map_mask,
            compass_mask,
            compass,
        };
        let ctx = ui.ctx().clone();
        let frame_size =
            egui::vec2(sprites.frame.width as f32, sprites.frame.height as f32) * scale;
        let origin = egui::pos2(rect.right() - frame_size.x - 10.0, rect.top() + 10.0);
        let at = |x: f32, y: f32| origin + egui::vec2(x, y) * scale;
        let yaw = self.interaction.camera.yaw;
        let (camera_x, camera_z) = self.world_position();
        let (tile_x, tile_y) = (camera_x / 128.0, camera_z / 128.0);
        let plane = self.interaction.view_plane;

        // Scene image.
        let regions = self.minimap_regions(tile_x, tile_y, plane);
        let map_image = compose_map(&sprites.map_mask, &regions, (tile_x, tile_y), yaw);
        let map_texture = match &mut self.minimap_hud.map {
            Some(texture) => {
                texture.set(map_image, egui::TextureOptions::NEAREST);
                texture.clone()
            }
            slot => slot
                .insert(ctx.load_texture("minimap", map_image, egui::TextureOptions::NEAREST))
                .clone(),
        };
        let map_size = egui::vec2(
            sprites.map_mask.width as f32,
            sprites.map_mask.height as f32,
        ) * scale;
        let map_rect = egui::Rect::from_min_size(at(MAP_AT.0, MAP_AT.1), map_size);
        let painter = ui.painter().with_clip_rect(rect);
        let full_uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
        painter.image(map_texture.id(), map_rect, full_uv, egui::Color32::WHITE);
        self.draw_minimap_icons(&painter, &regions, map_rect, (tile_x, tile_y), yaw, scale);

        // Frame ring.
        let Some(frame_texture) = self.assets.sprite(ids::FRAME).map(|t| t.id()) else {
            return;
        };
        painter.image(
            frame_texture,
            egui::Rect::from_min_size(origin, frame_size),
            full_uv,
            egui::Color32::WHITE,
        );

        // Compass: the sprite turned by the camera yaw, cut by its mask.
        let compass_image = compose_compass(&sprites.compass, &sprites.compass_mask, yaw);
        let compass_texture = match &mut self.minimap_hud.compass {
            Some(texture) => {
                texture.set(compass_image, egui::TextureOptions::NEAREST);
                texture.clone()
            }
            slot => slot
                .insert(ctx.load_texture(
                    "minimap-compass",
                    compass_image,
                    egui::TextureOptions::NEAREST,
                ))
                .clone(),
        };
        let compass_rect = egui::Rect::from_min_size(
            at(COMPASS_AT.0, COMPASS_AT.1),
            egui::vec2(
                sprites.compass_mask.width as f32,
                sprites.compass_mask.height as f32,
            ) * scale,
        );
        painter.image(
            compass_texture.id(),
            compass_rect,
            full_uv,
            egui::Color32::WHITE,
        );

        // Globe orb: opens the world map.
        let globe_rect =
            egui::Rect::from_min_size(at(GLOBE_AT.0, GLOBE_AT.1), egui::vec2(30.0, 30.0) * scale);
        let globe = ui.interact(
            globe_rect,
            egui::Id::new("minimap_globe"),
            egui::Sense::click(),
        );
        let planet = if globe.hovered() {
            ids::GLOBE_HOVER
        } else {
            ids::GLOBE
        };
        if let Some(texture) = self.assets.sprite(ids::GLOBE_FRAME).map(|t| t.id()) {
            painter.image(texture, globe_rect, full_uv, egui::Color32::WHITE);
        }
        if let Some(texture) = self.assets.sprite(planet).map(|t| t.id()) {
            let inner = egui::Rect::from_min_size(
                globe_rect.min + egui::vec2(4.0, 4.0) * scale,
                egui::vec2(22.0, 22.0) * scale,
            );
            painter.image(texture, inner, full_uv, egui::Color32::WHITE);
        }
        if globe.on_hover_text("World map").clicked() {
            self.toggle_world_map();
        }

        // Compass click faces north; clicking the map goes there (as the world map does).
        let compass_response = ui.interact(
            compass_rect,
            egui::Id::new("minimap_compass"),
            egui::Sense::click(),
        );
        if compass_response.on_hover_text("Face north").clicked() {
            self.interaction.camera.yaw = 0.0;
        }
        let map_response =
            ui.interact(map_rect, egui::Id::new("minimap_map"), egui::Sense::click());
        if let Some(pointer) = map_response.hover_pos() {
            let centre = map_rect.center();
            let (dx, dy) = (
                (pointer.x - centre.x) / scale,
                (pointer.y - centre.y) / scale,
            );
            if dx * dx + dy * dy <= 75.0 * 75.0 {
                let (tx, ty) = screen_to_tile(dx, dy, (tile_x, tile_y), yaw);
                map_response.clone().on_hover_text(format!(
                    "{}, {}   click to go there",
                    tx.floor() as i32,
                    ty.floor() as i32
                ));
                if map_response.clicked() {
                    self.teleport_to(Destination::Tile {
                        x: tx.floor() as i32,
                        y: ty.floor() as i32,
                        plane,
                    });
                }
            }
        }
    }

    /// The 3x3 regions around the camera (requested on first use), row-major from the south-west.
    fn minimap_regions(
        &mut self,
        tile_x: f64,
        tile_y: f64,
        plane: u8,
    ) -> [[Option<Arc<MinimapRegion>>; 3]; 3] {
        let (rx, ry) = (
            (tile_x / 64.0).floor() as i32,
            (tile_y / 64.0).floor() as i32,
        );
        let mut regions: [[Option<Arc<MinimapRegion>>; 3]; 3] = Default::default();
        for (column, row_regions) in regions.iter_mut().enumerate() {
            for (row, slot) in row_regions.iter_mut().enumerate() {
                let region = (rx + column as i32 - 1, ry + row as i32 - 1);
                // Only regions the circle can reach need to load.
                let reach = 19.5 + 1.0;
                let near_x = (f64::from(region.0) * 64.0 - reach
                    ..f64::from(region.0 + 1) * 64.0 + reach)
                    .contains(&tile_x);
                let near_y = (f64::from(region.1) * 64.0 - reach
                    ..f64::from(region.1 + 1) * 64.0 + reach)
                    .contains(&tile_y);
                if near_x && near_y {
                    *slot = self.assets.minimap_region(region, plane).flatten();
                }
            }
        }
        regions
    }

    /// Map icons (bank, shops, altars ...) of the loaded regions, centred on their tiles.
    fn draw_minimap_icons(
        &mut self,
        painter: &egui::Painter,
        regions: &[[Option<Arc<MinimapRegion>>; 3]; 3],
        map_rect: egui::Rect,
        camera: (f64, f64),
        yaw: f32,
        scale: f32,
    ) {
        let Some(catalog) = self.assets.catalog.clone() else {
            return;
        };
        let (sin, cos) = yaw.sin_cos();
        let centre = map_rect.center();
        for region in regions.iter().flatten().flatten() {
            for icon in &region.icons {
                let (wx, wy) = (
                    f64::from(region.region.0) * 64.0 + f64::from(icon.x) + 0.5 - camera.0,
                    f64::from(region.region.1) * 64.0 + f64::from(icon.y) + 0.5 - camera.1,
                );
                // Forward is (-sin, cos), right is (cos, sin).
                let right = wx as f32 * cos + wy as f32 * sin;
                let forward = -(wx as f32) * sin + wy as f32 * cos;
                let (dx, dy) = (right * PX_PER_TILE, -forward * PX_PER_TILE);
                if dx * dx + dy * dy > 72.0 * 72.0 {
                    continue;
                }
                let Some(element) = catalog.elements.get(&icon.element) else {
                    continue;
                };
                if element.sprite < 0 {
                    continue;
                }
                let Some(texture) = self.assets.sprite(element.sprite as u32) else {
                    continue;
                };
                let size = texture.size_vec2() * scale;
                painter.image(
                    texture.id(),
                    egui::Rect::from_center_size(centre + egui::vec2(dx, dy) * scale, size),
                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                    egui::Color32::WHITE,
                );
            }
        }
    }
}

/// Tile under a minimap offset (pixels right/down of the centre).
fn screen_to_tile(dx: f32, dy: f32, camera: (f64, f64), yaw: f32) -> (f64, f64) {
    let (sin, cos) = yaw.sin_cos();
    let (right, forward) = (dx / PX_PER_TILE, -dy / PX_PER_TILE);
    (
        camera.0 + f64::from(right * cos - forward * sin),
        camera.1 + f64::from(right * sin + forward * cos),
    )
}

/// The scene image seen through the circular mask, turned so the camera looks up
/// (`drawRotatedMaskedCenteredAround`).
fn compose_map(
    mask: &SpriteImage,
    regions: &[[Option<Arc<MinimapRegion>>; 3]; 3],
    camera: (f64, f64),
    yaw: f32,
) -> egui::ColorImage {
    let (width, height) = (mask.width as usize, mask.height as usize);
    let mut pixels = vec![egui::Color32::TRANSPARENT; width * height];
    let (base_x, base_y) = (
        (camera.0 / 64.0).floor() as i32 - 1,
        (camera.1 / 64.0).floor() as i32 - 1,
    );
    for y in 0..height {
        for x in 0..width {
            // Index 0 of the mask sprite (alpha 0) is the inside of the circle.
            if mask.rgba[(y * width + x) * 4 + 3] != 0 {
                continue;
            }
            let dx = x as f32 + 0.5 - width as f32 / 2.0;
            let dy = y as f32 + 0.5 - height as f32 / 2.0;
            let (tx, ty) = screen_to_tile(dx, dy, camera, yaw);
            let (rx, ry) = ((tx / 64.0).floor() as i32, (ty / 64.0).floor() as i32);
            let (column, row) = (rx - base_x, ry - base_y);
            if !(0..3).contains(&column) || !(0..3).contains(&row) {
                continue;
            }
            let Some(region) = &regions[column as usize][row as usize] else {
                continue;
            };
            let local_x = ((tx - f64::from(rx) * 64.0) * 4.0).floor() as usize;
            let local_y = ((ty - f64::from(ry) * 64.0) * 4.0).floor() as usize;
            let source = ((255 - local_y.min(255)) * 256 + local_x.min(255)) * 4;
            let rgba = &region.rgba[source..source + 4];
            if rgba[3] != 0 {
                pixels[y * width + x] = egui::Color32::from_rgb(rgba[0], rgba[1], rgba[2]);
            }
        }
    }
    egui::ColorImage {
        size: [width, height],
        source_size: egui::vec2(width as f32, height as f32),
        pixels,
    }
}

/// The compass texture turned by the camera yaw, inside the compass mask.
fn compose_compass(compass: &SpriteImage, mask: &SpriteImage, yaw: f32) -> egui::ColorImage {
    let (width, height) = (mask.width as usize, mask.height as usize);
    let mut pixels = vec![egui::Color32::TRANSPARENT; width * height];
    let (sin, cos) = yaw.sin_cos();
    let (source_width, source_height) = (compass.width as f32, compass.height as f32);
    for y in 0..height {
        for x in 0..width {
            if mask.rgba[(y * width + x) * 4 + 3] != 0 {
                continue;
            }
            let dx = x as f32 + 0.5 - width as f32 / 2.0;
            let dy = y as f32 + 0.5 - height as f32 / 2.0;
            // Turn the sprite with the camera (the north marker swings the other way).
            let sx = dx * cos - dy * sin + source_width / 2.0;
            let sy = dx * sin + dy * cos + source_height / 2.0;
            if sx < 0.0 || sy < 0.0 || sx >= source_width || sy >= source_height {
                continue;
            }
            let at = (sy as usize * compass.width as usize + sx as usize) * 4;
            let rgba = &compass.rgba[at..at + 4];
            if rgba[3] != 0 {
                pixels[y * width + x] = egui::Color32::from_rgb(rgba[0], rgba[1], rgba[2]);
            }
        }
    }
    egui::ColorImage {
        size: [width, height],
        source_size: egui::vec2(width as f32, height as f32),
        pixels,
    }
}
