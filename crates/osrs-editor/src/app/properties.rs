//! The Object tab of the inspector: the selected object as a card. The dock itself (rail, panel,
//! tabs) is in `workspace.rs`; settings live in `settings_window.rs`.

use super::EditorApp;
use crate::pick::{self, Pick};
use eframe::egui;

impl EditorApp {
    /// The Selection tab: counts, the selected tiles, the objects, and the primary object's card.
    pub(super) fn selection_section(&mut self, ui: &mut egui::Ui) {
        let (objects, tiles) = (self.selection_objects.len(), self.selection_tiles.len());
        if objects == 0 && tiles == 0 && self.selected.is_none() {
            super::widgets::empty_state(
                ui,
                crate::icons::SELECT,
                "Nothing selected",
                "Click, or drag a box, in the viewport.",
            );
            return;
        }
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(format!(
                    "{objects} object{} \u{b7} {tiles} tile{}",
                    if objects == 1 { "" } else { "s" },
                    if tiles == 1 { "" } else { "s" },
                ))
                .strong(),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.small_button("Clear").on_hover_text("Esc").clicked() {
                    self.clear_selection();
                }
            });
        });
        if tiles > 0 {
            crate::theme::section_heading(ui, "Tiles");
            let (mut min, mut max) = ((i32::MAX, i32::MAX), (i32::MIN, i32::MIN));
            for (x, y) in &self.selection_tiles {
                min = (min.0.min(*x), min.1.min(*y));
                max = (max.0.max(*x), max.1.max(*y));
            }
            ui.weak(format!(
                "{} \u{d7} {} tiles wide, from {}, {} to {}, {}",
                max.0 - min.0 + 1,
                max.1 - min.1 + 1,
                min.0,
                min.1,
                max.0,
                max.1
            ));
            ui.weak("Pick a tool to act on them: Paint fills them, Height levels or smooths them.");
        }
        if objects > 1 {
            crate::theme::section_heading(ui, "Objects");
            let names: Vec<(usize, String)> = self
                .selection_objects
                .iter()
                .enumerate()
                .take(40)
                .map(|(i, o)| {
                    let name = self
                        .infos
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
                    self.selected = self.selection_objects.get(index).cloned();
                }
            }
            if objects > 40 {
                ui.weak(format!("and {} more", objects - 40));
            }
        }
        let Some(object) = self.selected.clone() else {
            return;
        };
        crate::theme::section_heading(ui, "Object");
        if let Some(loc) = self
            .infos
            .get(&object.region)
            .and_then(|info| info.locs.get(object.index))
        {
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
            crate::theme::section_heading(ui, "Placement");
            egui::Grid::new("object_fields")
                .num_columns(2)
                .spacing([14.0, 5.0])
                .show(ui, |ui| {
                    field(ui, "Tile", &format!("{}, {}", loc.tile.0, loc.tile.1));
                    field(ui, "Plane", &loc.plane.to_string());
                    field(ui, "Orientation", &loc.orientation.to_string());
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
                self.run_plugins(|plugin, cx| plugin.selection_ui(ui, cx));
                if ui.button("Deselect").clicked() {
                    self.selected = None;
                }
            });
        }
        let text = pick::describe_object(&self.infos, &object);
        egui::CollapsingHeader::new(egui::RichText::new("Raw data").weak())
            .default_open(false)
            .show(ui, |ui| {
                ui.add(egui::Label::new(egui::RichText::new(text).monospace().size(11.5)).wrap());
            });
    }
}

fn field(ui: &mut egui::Ui, label: &str, value: &str) {
    ui.weak(label);
    ui.label(value);
    ui.end_row();
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
