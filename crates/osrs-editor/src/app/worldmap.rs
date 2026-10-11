//! The world map window, laid out like the game's: area selector, search, zoom steps, key panel.
//!
//! Data and rendering come from the client's own algorithm (`osrs_world::worldmap`); this file is
//! the viewer: smooth zoom between the client's steps, panning, icon and label placement with the
//! client's formulas, double-click to fly the 3D camera to a tile.

use super::EditorApp;
use crate::hud::Destination;
use crate::icons;
use eframe::egui;
use osrs_world::worldmap::{Area, Catalog, MapIcon};
use std::collections::HashSet;

/// The client's zoom steps: `(percent, pixels per tile)`.
const ZOOM_STEPS: [(i32, f32); 6] = [
    (25, 1.0),
    (37, 1.5),
    (50, 2.0),
    (75, 3.0),
    (100, 4.0),
    (200, 8.0),
];

/// World map window state.
pub struct WorldMapView {
    pub open: bool,
    area: usize,
    initialised: bool,
    /// Map tile (area coordinates) at the centre of the view.
    center: (f64, f64),
    zoom: f32,
    zoom_target: f32,
    key_open: bool,
    search: String,
    /// Element ids the player switched off in the key.
    hidden: HashSet<u32>,
    /// The `(area, pixels per tile)` the queued region work belongs to.
    queued_for: Option<(usize, i32)>,
}

impl WorldMapView {
    pub fn new() -> Self {
        Self {
            // Smoke-test hook: `RUSTOSRS_OPEN=worldmap`.
            open: std::env::var("RUSTOSRS_OPEN").is_ok_and(|value| value == "worldmap"),
            area: 0,
            initialised: false,
            center: (0.0, 0.0),
            zoom: 3.0,
            zoom_target: 3.0,
            key_open: true,
            search: String::new(),
            hidden: HashSet::new(),
            queued_for: None,
        }
    }

    fn step_zoom(&mut self, direction: i32) {
        let current = ZOOM_STEPS
            .iter()
            .position(|(_, zoom)| (*zoom - self.zoom_target).abs() < 0.01)
            .unwrap_or(3);
        let next = (current as i32 + direction).clamp(0, ZOOM_STEPS.len() as i32 - 1) as usize;
        self.zoom_target = ZOOM_STEPS[next].1;
    }

    fn zoom_percent(&self) -> i32 {
        ZOOM_STEPS
            .iter()
            .find(|(_, zoom)| (*zoom - self.zoom_target).abs() < 0.01)
            .map_or(100, |(percent, _)| *percent)
    }

    /// Show `area`, centred on its origin at its default zoom (`WorldMap.jump`).
    fn open_area(&mut self, catalog: &Catalog, area: usize) {
        self.area = area;
        let data = &catalog.areas[area];
        let (x, y) = data.origin_position();
        self.center = (f64::from(x), f64::from(y));
        let zoom = ZOOM_STEPS
            .iter()
            .find(|(percent, _)| *percent == data.zoom_percent)
            .map_or(4.0, |(_, zoom)| *zoom);
        self.zoom = zoom;
        self.zoom_target = zoom;
    }

    /// Centre on a world coordinate when the area shows it (`WorldMap.jump`).
    fn jump_to_world(&mut self, area: &Area, plane: i32, x: i32, y: i32) -> bool {
        if let Some((mx, my)) = area.position(plane, x, y) {
            self.center = (f64::from(mx), f64::from(my));
            true
        } else {
            false
        }
    }
}

/// `WorldMapRenderer.getPixelsPerTile` rounded up: the image scale the regions are rendered at.
fn pixels_per_tile(zoom: f32) -> i32 {
    (zoom.clamp(1.0, 8.0) - 0.0001).ceil().max(1.0) as i32
}

fn argb(colour: u32) -> egui::Color32 {
    egui::Color32::from_rgb((colour >> 16) as u8, (colour >> 8) as u8, colour as u8)
}

impl EditorApp {
    pub(super) fn toggle_world_map(&mut self) {
        self.world_map.open = !self.world_map.open;
    }

    pub(super) fn world_map_window(&mut self, ctx: &egui::Context) {
        if !self.world_map.open {
            return;
        }
        let Some(catalog) = self.assets.catalog.clone() else {
            egui::Window::new(format!("{} World map", icons::WORLD_MAP))
                .collapsible(false)
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(
                            self.assets
                                .status
                                .clone()
                                .unwrap_or_else(|| "Loading world map...".to_owned()),
                        );
                    });
                });
            return;
        };
        if !self.world_map.initialised {
            self.world_map.initialised = true;
            // Open on the camera's position when the main map shows it.
            let main = catalog.main_area();
            self.world_map.open_area(&catalog, main);
            let (x, z) = self.world_position();
            let (tx, ty) = ((x / 128.0).floor() as i32, (z / 128.0).floor() as i32);
            let plane = i32::from(self.view_plane);
            let area = &catalog.areas[main];
            self.world_map.jump_to_world(area, plane, tx, ty);
        }

        let dt = ctx.input(|input| input.stable_dt).min(0.1);
        // The client eases the zoom by 1/30 per 20 ms tick.
        let ease = (1.0 + 1.0 / 30.0_f32).powf(dt * 50.0);
        let map = &mut self.world_map;
        if map.zoom < map.zoom_target {
            map.zoom = (map.zoom * ease).min(map.zoom_target);
            ctx.request_repaint();
        } else if map.zoom > map.zoom_target {
            map.zoom = (map.zoom / ease).max(map.zoom_target);
            ctx.request_repaint();
        }

        let mut open = true;
        let mut teleport = None;
        egui::Window::new(format!("{} World map", icons::WORLD_MAP))
            .open(&mut open)
            .default_size([1040.0, 700.0])
            .resizable(true)
            .show(ctx, |ui| {
                self.world_map_toolbar(ui, &catalog);
                ui.add_space(4.0);
                if self.world_map.key_open {
                    egui::Panel::right("world_map_key")
                        .resizable(false)
                        .default_size(210.0)
                        .show(ui, |ui| self.world_map_key(ui, &catalog));
                }
                teleport = self.world_map_canvas(ui, &catalog);
            });
        self.world_map.open = open;
        if let Some((x, y, plane)) = teleport {
            self.teleport_to(Destination::Tile {
                x,
                y,
                plane: plane.clamp(0, 3) as u8,
            });
        }
    }

    fn world_map_toolbar(&mut self, ui: &mut egui::Ui, catalog: &Catalog) {
        ui.horizontal(|ui| {
            let current = self.world_map.area;
            let mut chosen = None;
            egui::ComboBox::from_id_salt("world_map_area")
                .width(220.0)
                .selected_text(catalog.areas[current].external_name.as_str())
                .show_ui(ui, |ui| {
                    for (index, area) in catalog.areas.iter().enumerate() {
                        if ui
                            .selectable_label(index == current, area.external_name.as_str())
                            .clicked()
                        {
                            chosen = Some(index);
                        }
                    }
                });
            if let Some(index) = chosen {
                self.world_map.open_area(catalog, index);
            }
            ui.add(
                egui::TextEdit::singleline(&mut self.world_map.search)
                    .hint_text(format!("{} Search the map", icons::SEARCH))
                    .desired_width(180.0),
            );
            ui.separator();
            if ui
                .button(icons::ZOOM_OUT)
                .on_hover_text("Zoom out")
                .clicked()
            {
                self.world_map.step_zoom(-1);
            }
            ui.label(format!("{}%", self.world_map.zoom_percent()));
            if ui.button(icons::ZOOM_IN).on_hover_text("Zoom in").clicked() {
                self.world_map.step_zoom(1);
            }
            ui.separator();
            if ui
                .button(icons::CROSSHAIR)
                .on_hover_text("Centre on the camera")
                .clicked()
            {
                let (x, z) = self.world_position();
                let (tx, ty) = ((x / 128.0).floor() as i32, (z / 128.0).floor() as i32);
                let plane = i32::from(self.view_plane);
                let main = self.world_map.area;
                if !self
                    .world_map
                    .jump_to_world(&catalog.areas[main], plane, tx, ty)
                {
                    // The camera is somewhere another area shows.
                    if let Some(index) = catalog
                        .areas
                        .iter()
                        .position(|area| area.contains_world(plane, tx, ty))
                    {
                        self.world_map.open_area(catalog, index);
                        let area = &catalog.areas[index];
                        self.world_map.jump_to_world(area, plane, tx, ty);
                    }
                }
            }
            ui.toggle_value(&mut self.world_map.key_open, "Key");
        });
    }

    /// The key: every kind of icon in this area with a switch, plus search results.
    fn world_map_key(&mut self, ui: &mut egui::Ui, catalog: &Catalog) {
        let area = self.world_map.area;
        let Some(icons_in_area) = self.assets.icons(area) else {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.weak("Reading icons...");
            });
            return;
        };
        let mut kinds: Vec<(u32, &str)> = Vec::new();
        let mut seen = HashSet::new();
        for icon in icons_in_area.iter() {
            if !seen.insert(icon.element) {
                continue;
            }
            if let Some(element) = catalog.elements.get(&icon.element)
                && let Some(name) = element.name.as_deref()
                && element.sprite >= 0
            {
                kinds.push((icon.element, name));
            }
        }
        kinds.sort_by(|a, b| a.1.cmp(b.1));
        kinds.dedup_by(|a, b| a.1 == b.1);

        let query = self.world_map.search.trim().to_lowercase();
        let mut jump = None;
        if !query.is_empty() {
            ui.strong("Search results");
            let mut shown = 0;
            let mut names = HashSet::new();
            for icon in icons_in_area.iter() {
                let Some(name) = catalog
                    .elements
                    .get(&icon.element)
                    .and_then(|element| element.name.as_deref())
                else {
                    continue;
                };
                if !name.to_lowercase().contains(&query) || !names.insert(name.to_owned()) {
                    continue;
                }
                if ui.selectable_label(false, name).clicked() {
                    jump = Some(name.to_owned());
                }
                shown += 1;
                if shown >= 14 {
                    break;
                }
            }
            if shown == 0 {
                ui.weak("No matches");
            }
            ui.separator();
        }
        if let Some(name) = jump {
            // Nearest icon of that name to the view centre.
            let center = self.world_map.center;
            let best = icons_in_area
                .iter()
                .filter(|icon| {
                    catalog
                        .elements
                        .get(&icon.element)
                        .and_then(|element| element.name.as_deref())
                        == Some(name.as_str())
                })
                .min_by(|a, b| {
                    let da =
                        (f64::from(a.x) - center.0).powi(2) + (f64::from(a.y) - center.1).powi(2);
                    let db =
                        (f64::from(b.x) - center.0).powi(2) + (f64::from(b.y) - center.1).powi(2);
                    da.total_cmp(&db)
                });
            if let Some(icon) = best {
                self.world_map.center = (f64::from(icon.x), f64::from(icon.y));
                self.world_map.zoom_target = 4.0;
            }
        }

        ui.strong("Key");
        egui::ScrollArea::vertical().show(ui, |ui| {
            for (id, name) in kinds {
                let sprite = catalog
                    .elements
                    .get(&id)
                    .map_or(-1, |element| element.sprite);
                ui.horizontal(|ui| {
                    let mut shown = !self.world_map.hidden.contains(&id);
                    if ui.checkbox(&mut shown, "").changed() {
                        if shown {
                            self.world_map.hidden.remove(&id);
                        } else {
                            self.world_map.hidden.insert(id);
                        }
                    }
                    if sprite >= 0
                        && let Some(texture) = self.assets.sprite(sprite as u32)
                    {
                        ui.image((texture.id(), texture.size_vec2()));
                    }
                    ui.label(name);
                });
            }
        });
    }

    /// Draws the map; returns the world tile and plane the user double-clicked.
    fn world_map_canvas(
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
    fn draw_map_icons(
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
            if self.world_map.hidden.contains(&icon.element) {
                continue;
            }
            let Some(element) = catalog.elements.get(&icon.element) else {
                continue;
            };
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
            {
                let size = match element.text_size {
                    0 => 11.0,
                    1 => 13.0,
                    _ => 15.0,
                };
                let colour = argb(element.color);
                let lines: Vec<&str> = name.split("<br>").collect();
                let line_height = size + 1.0;
                for (index, line) in lines.iter().enumerate() {
                    let at = position + egui::vec2(0.0, index as f32 * line_height);
                    {
                        let offset = egui::vec2(1.0, 1.0);
                        painter.text(
                            at + offset,
                            egui::Align2::CENTER_TOP,
                            line,
                            egui::FontId::proportional(size),
                            egui::Color32::BLACK,
                        );
                    }
                    painter.text(
                        at,
                        egui::Align2::CENTER_TOP,
                        line,
                        egui::FontId::proportional(size),
                        colour,
                    );
                }
            }
        }
    }
}
