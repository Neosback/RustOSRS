//! The Inspector: what is under the cursor or selected, shown rather than listed. Tiles take the top
//! half of the panel and objects the bottom half, each with a picture (the tile and its eight
//! neighbours as the game draws them, an object's footprint and facing) above its fields. It only
//! inspects: the editing tools act on tiles and objects, the inspector never changes them.
//! The dock itself (rail, panel, tabs) is in `workspace.rs`; settings live in `settings_window.rs`.

mod collision;

use super::EditorApp;
use super::widgets::{OverlayFill, empty_state, paint_tile};
use crate::icons;
use crate::paint::TileValues;
use crate::pick::{self, Pick};
use eframe::egui::{self, Color32, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2};

/// Size of one tile in the neighbourhood picture.
const TILE_PX: f32 = 46.0;

impl EditorApp {
    /// The whole Inspector: tile above, object below, each in its own scrolling half.
    pub(super) fn inspector(&mut self, ui: &mut egui::Ui) {
        self.draw_inspector(ui, true);
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(super) fn inspection_inspector(&mut self, ui: &mut egui::Ui) {
        self.draw_inspector(ui, false);
    }

    fn draw_inspector(&mut self, ui: &mut egui::Ui, authoring: bool) {
        let width = ui.available_width();
        // The half with something to show gets the room; an empty one shrinks.
        let has_tile = if self.settings.prefs.inspector_follow {
            self.interaction.hover().tile.is_some() || self.interaction.last_hover_tile().is_some()
        } else {
            self.interaction.inspect_tile().is_some()
        };
        let has_object = self.interaction.selected().is_some()
            || !self.interaction.selection_objects().is_empty();
        let (tile_weight, object_weight) = (
            if has_tile { 1.0 } else { 0.55 },
            if has_object { 1.0 } else { 0.55 },
        );
        let room = (ui.available_height() - 14.0).max(300.0);
        let tile_height = (room * tile_weight / (tile_weight + object_weight)).max(150.0);
        let object_height = (room - tile_height).max(150.0);
        ui.allocate_ui(Vec2::new(width, tile_height), |ui| {
            ui.set_min_size(Vec2::new(width, tile_height));
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(icons::SELECT_TILES)
                        .size(15.0)
                        .color(ui.visuals().weak_text_color()),
                );
                ui.label(egui::RichText::new("Tile").strong());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    crate::theme::toggle(ui, &mut self.settings.prefs.inspector_follow, "Follow cursor")
                        .on_hover_text(
                            "On: the Inspector shows the tile under the cursor. Off: the tile you selected",
                        );
                });
            });
            ui.add_space(2.0);
            egui::ScrollArea::vertical()
                .id_salt("inspect_tile")
                .auto_shrink([false, false])
                .show(ui, |ui| self.tile_card(ui, authoring));
        });
        ui.separator();
        ui.allocate_ui(Vec2::new(width, object_height), |ui| {
            ui.set_min_size(Vec2::new(width, object_height));
            card_header(ui, icons::SELECT_OBJECTS, "Object");
            egui::ScrollArea::vertical()
                .id_salt("inspect_object")
                .auto_shrink([false, false])
                .show(ui, |ui| self.object_card(ui, authoring));
        });
    }

    /// How a tile's floors are filled in a picture: the underlay colour and the overlay's fill.
    fn tile_fills(
        &mut self,
        ctx: &egui::Context,
        values: &TileValues,
    ) -> (Option<Color32>, Option<OverlayFill>) {
        let underlay = (values.underlay != 0)
            .then(|| self.underlay_color(values.underlay))
            .flatten();
        let overlay = (values.overlay != 0)
            .then(|| {
                self.overlay_swatch(values.overlay)
                    .map(|swatch| self.overlay_fill(ctx, &swatch))
            })
            .flatten();
        (underlay, overlay)
    }

    /// The tile card: the tile and its neighbours, then its fields.
    fn tile_card(&mut self, ui: &mut egui::Ui, authoring: bool) {
        let ctx = ui.ctx().clone();
        // Following the cursor: the tile under it (the last one when it is over nothing). Otherwise
        // the tile that was selected.
        let hovered = self
            .interaction
            .hover()
            .tile
            .map(|pick| pick.tile)
            .or(self.interaction.last_hover_tile());
        let (tile, source) = if self.settings.prefs.inspector_follow {
            (hovered, "under the cursor")
        } else {
            (self.interaction.inspect_tile(), "selected")
        };
        let Some(tile) = tile else {
            if self.settings.prefs.inspector_follow {
                empty_state(
                    ui,
                    icons::SELECT_TILES,
                    "No tile",
                    "Move the cursor over the terrain.",
                );
            } else {
                empty_state(
                    ui,
                    icons::SELECT_TILES,
                    "No tile selected",
                    "Click a tile or an object, or turn on Follow cursor.",
                );
            }
            return;
        };
        let Some(values) = self.tile_now(tile) else {
            empty_state(
                ui,
                icons::SELECT_TILES,
                "No tile data",
                "That area is not loaded.",
            );
            return;
        };
        self.diagnostics
            .tile_inspector(ui, tile, self.interaction.view_plane());
        let selected_tiles = self.interaction.selection_tiles().len();
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(format!("{}, {}", tile.0, tile.1)).heading());
            ui.weak(format!(
                "plane {}  \u{b7}  {source}",
                self.interaction.view_plane()
            ));
        });
        if selected_tiles > 1 {
            ui.weak(format!("{selected_tiles} tiles selected"));
        }
        ui.add_space(4.0);
        // The tile in the middle of its eight neighbours.
        let (area, _) = ui.allocate_exact_size(Vec2::splat(TILE_PX * 3.0 + 6.0), Sense::hover());
        for dy in -1..=1 {
            for dx in -1..=1 {
                let at = (tile.0 + dx, tile.1 + dy);
                let cell = Rect::from_min_size(
                    Pos2::new(
                        area.left() + (dx + 1) as f32 * (TILE_PX + 2.0),
                        // North is up.
                        area.top() + (1 - dy) as f32 * (TILE_PX + 2.0),
                    ),
                    Vec2::splat(TILE_PX),
                );
                match self.tile_now(at) {
                    Some(v) => {
                        let (under, over) = self.tile_fills(&ctx, &v);
                        paint_tile(ui.painter(), cell, v.shape, v.rotation, under, over);
                    }
                    None => {
                        ui.painter()
                            .rect_filled(cell, 3.0, ui.visuals().extreme_bg_color);
                    }
                }
                if dx == 0 && dy == 0 {
                    ui.painter().rect_stroke(
                        cell.expand(1.0),
                        4.0,
                        Stroke::new(2.0, ui.visuals().selection.stroke.color),
                        StrokeKind::Outside,
                    );
                } else {
                    ui.painter()
                        .rect_filled(cell, 3.0, Color32::from_black_alpha(70));
                }
            }
        }
        ui.add_space(6.0);
        egui::Grid::new("tile_fields")
            .num_columns(2)
            .spacing([14.0, 5.0])
            .show(ui, |ui| {
                field(ui, "Overlay", &id_text(values.overlay));
                field(ui, "Underlay", &id_text(values.underlay));
                field(
                    ui,
                    "Shape",
                    &format!("{} \u{b7} rotation {}", values.shape, values.rotation),
                );
                field(
                    ui,
                    "Height",
                    &crate::paint::elevation(values.height).to_string(),
                );
                field(ui, "Flags", &format!("{:#04x}", values.flags));
                field(
                    ui,
                    "Region",
                    &crate::hud::region_id(tile.0 >> 6, tile.1 >> 6).to_string(),
                );
            });
        collision::show(
            ui,
            self.world
                .collision_inspection(osrs_core::coords::MapTile::new(tile.0, tile.1)),
            osrs_core::coords::MapTile::new(tile.0, tile.1),
            self.interaction.view_plane(),
        );
        ui.add_space(6.0);
        ui.horizontal_wrapped(|ui| {
            if authoring
                && ui
                    .button(format!("{}  Use as brush", icons::EYEDROPPER))
                    .on_hover_text("Fill the brush from this tile (I)")
                    .clicked()
            {
                self.eyedrop(values);
            }
            if ui
                .button(format!("{}  Copy", egui_phosphor::regular::COPY))
                .on_hover_text("Copy the tile's coordinates")
                .clicked()
            {
                ui.ctx().copy_text(format!("{}, {}", tile.0, tile.1));
            }
        });
    }

    /// The object card: the selection, the primary object's footprint and fields.
    fn object_card(&mut self, ui: &mut egui::Ui, authoring: bool) {
        let (objects, tiles) = (
            self.interaction.selection_objects().len(),
            self.interaction.selection_tiles().len(),
        );
        if objects == 0 && self.interaction.selected().is_none() {
            empty_state(
                ui,
                icons::SELECT_OBJECTS,
                "No object",
                "Click an object in the viewport.",
            );
            return;
        }
        let _ = tiles;
        if objects > 1 {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(format!("{objects} objects selected")).strong());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.small_button("Clear").on_hover_text("Esc").clicked() {
                        self.clear_selection();
                    }
                });
            });
            let names: Vec<(usize, String)> = self
                .interaction
                .selection_objects()
                .iter()
                .enumerate()
                .take(40)
                .map(|(i, o)| {
                    let name = self
                        .world
                        .infos()
                        .get(&o.region)
                        .and_then(|info| info.locs.get(o.index))
                        .map(|loc| {
                            format!(
                                "{}  \u{b7}  {}, {}",
                                loc.definition
                                    .name
                                    .as_deref()
                                    .filter(|n| !n.is_empty())
                                    .unwrap_or("Unnamed"),
                                loc.tile.0,
                                loc.tile.1
                            )
                        })
                        .unwrap_or_default();
                    (i, name)
                })
                .collect();
            for (index, name) in names {
                if ui.selectable_label(false, name).clicked() {
                    let selected = self.interaction.selection_objects().get(index).cloned();
                    self.interaction.set_primary_object(selected);
                }
            }
            if objects > 40 {
                ui.weak(format!("and {} more", objects - 40));
            }
            ui.separator();
        }
        let Some(object) = self.interaction.selected().clone() else {
            return;
        };
        let Some(loc) = self
            .world
            .infos()
            .get(&object.region)
            .and_then(|info| info.locs.get(object.index))
        else {
            return;
        };
        let name = loc
            .definition
            .name
            .as_deref()
            .filter(|name| !name.is_empty())
            .unwrap_or("Unnamed object");
        ui.label(egui::RichText::new(name).heading());
        ui.weak(format!(
            "Object {}  \u{b7}  {}",
            loc.definition.identity.id.get(),
            loc_type_name(loc.loc_type)
        ));
        ui.add_space(4.0);
        footprint(
            ui,
            loc.definition.size_x,
            loc.definition.size_y,
            loc.orientation,
        );
        ui.add_space(6.0);
        egui::Grid::new("object_fields")
            .num_columns(2)
            .spacing([14.0, 5.0])
            .show(ui, |ui| {
                field(ui, "Tile", &format!("{}, {}", loc.tile.0, loc.tile.1));
                field(ui, "Plane", &loc.plane.to_string());
                field(ui, "Facing", facing_name(loc.orientation));
                field(ui, "Type", &loc.loc_type.to_string());
                field(
                    ui,
                    "Size",
                    &format!("{} \u{d7} {}", loc.definition.size_x, loc.definition.size_y),
                );
                field(ui, "Draw level", &loc.level.to_string());
            });
        ui.add_space(8.0);
        ui.horizontal_wrapped(|ui| {
            let pick = Pick {
                object: Some(object.clone()),
                ..Pick::default()
            };
            ui.menu_button("Copy", |ui| self.copy_menu(ui, &pick));
            if authoring {
                self.run_plugins(|plugin, cx| plugin.selection_ui(ui, cx));
            }
            if ui.button("Deselect").clicked() {
                self.interaction.set_primary_object(None);
            }
        });
        let text = pick::describe_object(self.world.infos(), &object);
        egui::CollapsingHeader::new(egui::RichText::new("Raw data").weak())
            .default_open(false)
            .show(ui, |ui| {
                ui.add(egui::Label::new(egui::RichText::new(text).monospace().size(11.5)).wrap());
            });
    }
}

/// An inspector half's title line.
fn card_header(ui: &mut egui::Ui, icon: &str, title: &str) {
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(icon)
                .size(15.0)
                .color(ui.visuals().weak_text_color()),
        );
        ui.label(egui::RichText::new(title).strong());
    });
    ui.add_space(2.0);
}

fn id_text(id: u16) -> String {
    if id == 0 {
        "none".to_owned()
    } else {
        id.to_string()
    }
}

fn field(ui: &mut egui::Ui, label: &str, value: &str) {
    ui.weak(label);
    ui.label(value);
    ui.end_row();
}

/// OSRS orientations: 0 faces west, 1 north, 2 east, 3 south.
fn facing_name(orientation: u8) -> &'static str {
    match orientation & 3 {
        0 => "west",
        1 => "north",
        2 => "east",
        _ => "south",
    }
}

/// The object's footprint (north up) with an arrow for the way it faces.
fn footprint(ui: &mut egui::Ui, size_x: u16, size_y: u16, orientation: u8) {
    let (mut columns, mut rows) = (u32::from(size_x.max(1)), u32::from(size_y.max(1)));
    if orientation & 1 == 1 {
        std::mem::swap(&mut columns, &mut rows);
    }
    let (columns, rows) = (columns.min(10), rows.min(10));
    let cell = (88.0 / columns.max(rows) as f32).clamp(14.0, 30.0);
    let area = Vec2::new(columns as f32 * cell, rows as f32 * cell);
    // Room around the cells for the arrow.
    let pad = Vec2::splat(16.0);
    let (rect, response) = ui.allocate_exact_size(area + pad * 2.0, Sense::hover());
    let body = Rect::from_min_size(rect.min + pad, area);
    let accent = ui.visuals().selection.stroke.color;
    let painter = ui.painter();
    for y in 0..rows {
        for x in 0..columns {
            let c = Rect::from_min_size(
                body.min + Vec2::new(x as f32 * cell, y as f32 * cell),
                Vec2::splat(cell),
            )
            .shrink(1.0);
            painter.rect_filled(c, 2.0, accent.gamma_multiply(0.28));
        }
    }
    painter.rect_stroke(body, 2.0, Stroke::new(1.5, accent), StrokeKind::Outside);
    // The arrow from the middle towards the facing edge (screen y is down, north is up).
    let direction = match orientation & 3 {
        0 => Vec2::new(-1.0, 0.0),
        1 => Vec2::new(0.0, -1.0),
        2 => Vec2::new(1.0, 0.0),
        _ => Vec2::new(0.0, 1.0),
    };
    let centre = body.center();
    let tip = centre + direction * (area.x.min(area.y) * 0.5 + 8.0);
    painter.line_segment([centre, tip], Stroke::new(2.0, accent));
    let side = Vec2::new(-direction.y, direction.x) * 4.5;
    painter.add(egui::Shape::convex_polygon(
        vec![
            tip + direction * 4.0,
            tip - direction * 3.0 + side,
            tip - direction * 3.0 - side,
        ],
        accent,
        Stroke::NONE,
    ));
    response.on_hover_text("Footprint, north up. The arrow is the way the object faces.");
}

/// The loc type's name (the client's `ObjectType` shapes).
fn loc_type_name(loc_type: u8) -> &'static str {
    match loc_type {
        0 => "Straight wall",
        1 => "Diagonal wall piece",
        2 => "Corner wall",
        3 => "Square corner wall",
        4 => "Wall decoration",
        5 => "Wall decoration (offset)",
        6 | 7 => "Wall decoration (diagonal)",
        8 => "Wall decoration (double)",
        9 => "Diagonal wall",
        10 => "Scenery",
        11 => "Scenery (diagonal)",
        12..=21 => "Roof",
        22 => "Ground decoration",
        _ => "Unknown type",
    }
}
