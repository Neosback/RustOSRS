//! The Object tab of the inspector: the selected object as a card. The dock itself (rail, panel,
//! tabs) is in `workspace.rs`; settings live in `settings_window.rs`.

use super::EditorApp;
use crate::pick::{self, Pick};
use eframe::egui;

impl EditorApp {
    pub(super) fn selection_section(&mut self, ui: &mut egui::Ui) {
        let Some(object) = self.selected.clone() else {
            ui.add_space(24.0);
            ui.vertical_centered(|ui| {
                ui.label(
                    egui::RichText::new(crate::icons::SELECT)
                        .size(28.0)
                        .color(ui.visuals().widgets.noninteractive.fg_stroke.color),
                );
                ui.label(egui::RichText::new("Nothing selected").strong());
                ui.weak("Click an object in the viewport to inspect it.");
            });
            return;
        };
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
