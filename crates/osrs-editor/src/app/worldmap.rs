//! The world map window, laid out like the game's: area selector, search, zoom steps, key panel.
//!
//! Data and rendering come from the client's own algorithm (`osrs_world::worldmap`); this file is
//! the viewer: smooth zoom between the client's steps, panning, icon and label placement with the
//! client's formulas, double-click to fly the 3D camera to a tile.

mod canvas;
mod key;

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
    /// Element categories the player switched off in the key.
    hidden: HashSet<i32>,
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

/// What the player calls an element: its label, its menu name, or the object that shows it.
fn element_name(
    catalog: &Catalog,
    names: &std::collections::HashMap<u32, String>,
    id: u32,
) -> Option<String> {
    let element = catalog.elements.get(&id)?;
    catalog
        .category_names
        .get(&element.category)
        .cloned()
        .or_else(|| element.name.clone())
        .or_else(|| element.key_name.clone())
        .or_else(|| names.get(&id).cloned())
}

fn argb(colour: u32) -> egui::Color32 {
    egui::Color32::from_rgb((colour >> 16) as u8, (colour >> 8) as u8, colour as u8)
}

impl EditorApp {
    pub(super) fn toggle_world_map(&mut self) {
        self.ui.world_map.open = !self.ui.world_map.open;
    }

    pub(super) fn world_map_window(&mut self, ctx: &egui::Context) {
        if !self.ui.world_map.open {
            return;
        }
        let Some(catalog) = self.assets.catalog.clone() else {
            let status = self
                .assets
                .status
                .clone()
                .unwrap_or_else(|| "Loading world map...".to_owned());
            let ((), close) = super::modal::Modal::new(
                "world_map_loading",
                format!("{} World map", icons::WORLD_MAP),
            )
            .size(300.0, 0.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(status);
                });
            });
            if close {
                self.ui.world_map.open = false;
            }
            return;
        };
        if !self.ui.world_map.initialised {
            self.ui.world_map.initialised = true;
            // Open on the camera's position when the main map shows it.
            let main = catalog.main_area();
            self.ui.world_map.open_area(&catalog, main);
            let (x, z) = self.world_position();
            let (tx, ty) = ((x / 128.0).floor() as i32, (z / 128.0).floor() as i32);
            let plane = i32::from(self.interaction.view_plane());
            let area = &catalog.areas[main];
            self.ui.world_map.jump_to_world(area, plane, tx, ty);
        }

        let dt = ctx.input(|input| input.stable_dt).min(0.1);
        // The client eases the zoom by 1/30 per 20 ms tick.
        let ease = (1.0 + 1.0 / 30.0_f32).powf(dt * 50.0);
        let map = &mut self.ui.world_map;
        if map.zoom < map.zoom_target {
            map.zoom = (map.zoom * ease).min(map.zoom_target);
            ctx.request_repaint();
        } else if map.zoom > map.zoom_target {
            map.zoom = (map.zoom / ease).max(map.zoom_target);
            ctx.request_repaint();
        }

        let mut teleport = None;
        let ((), close) = super::modal::Modal::new(
            "world_map_window",
            format!("{} World map", icons::WORLD_MAP),
        )
        .size(1040.0, 680.0)
        .show(ctx, |ui| {
            self.world_map_toolbar(ui, &catalog);
            ui.add_space(4.0);
            if self.ui.world_map.key_open {
                egui::Panel::right("world_map_key")
                    .resizable(false)
                    .default_size(210.0)
                    .show(ui, |ui| self.world_map_key(ui, &catalog));
            }
            teleport = self.world_map_canvas(ui, &catalog);
        });
        if close {
            self.ui.world_map.open = false;
        }
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
            let current = self.ui.world_map.area;
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
                self.ui.world_map.open_area(catalog, index);
            }
            ui.add(
                egui::TextEdit::singleline(&mut self.ui.world_map.search)
                    .hint_text(format!("{} Search the map", icons::SEARCH))
                    .desired_width(180.0),
            );
            ui.separator();
            if ui
                .button(icons::ZOOM_OUT)
                .on_hover_text("Zoom out")
                .clicked()
            {
                self.ui.world_map.step_zoom(-1);
            }
            ui.label(format!("{}%", self.ui.world_map.zoom_percent()));
            if ui.button(icons::ZOOM_IN).on_hover_text("Zoom in").clicked() {
                self.ui.world_map.step_zoom(1);
            }
            ui.separator();
            if ui
                .button(icons::CROSSHAIR)
                .on_hover_text("Centre on the camera")
                .clicked()
            {
                let (x, z) = self.world_position();
                let (tx, ty) = ((x / 128.0).floor() as i32, (z / 128.0).floor() as i32);
                let plane = i32::from(self.interaction.view_plane());
                let main = self.ui.world_map.area;
                if !self
                    .ui
                    .world_map
                    .jump_to_world(&catalog.areas[main], plane, tx, ty)
                {
                    // The camera is somewhere another area shows.
                    if let Some(index) = catalog
                        .areas
                        .iter()
                        .position(|area| area.contains_world(plane, tx, ty))
                    {
                        self.ui.world_map.open_area(catalog, index);
                        let area = &catalog.areas[index];
                        self.ui.world_map.jump_to_world(area, plane, tx, ty);
                    }
                }
            }
            ui.toggle_value(&mut self.ui.world_map.key_open, "Key");
        });
    }
}
