//! Right-hand Properties dock, organised in collapsible sections (Selection, View, Shading,
//! Scene, Interface). New tools add their own section here.

use super::EditorApp;
use crate::pick::{self, Pick};
use eframe::egui;
use osrs_render::gpu::ColorblindMode;

fn section(ui: &mut egui::Ui, title: &str, default_open: bool, body: impl FnOnce(&mut egui::Ui)) {
    egui::CollapsingHeader::new(egui::RichText::new(title).strong())
        .default_open(default_open)
        .show(ui, body);
}

impl EditorApp {
    pub(super) fn properties_panel(&mut self, ui: &mut egui::Ui) {
        if !self.panels.properties {
            return;
        }
        egui::Panel::right("properties")
            .resizable(true)
            .default_size(300.0)
            .show(ui, |ui| {
                ui.heading("Properties");
                ui.separator();
                egui::ScrollArea::vertical().show(ui, |ui| {
                    section(ui, "Selection", true, |ui| self.selection_section(ui));
                    section(ui, "View", true, |ui| self.view_section(ui));
                    section(ui, "Shading", true, |ui| self.shading_section(ui));
                    section(ui, "Scene", true, |ui| self.scene_section(ui));
                    section(ui, "Interface", false, |ui| self.theme_picker(ui));
                });
            });
    }

    fn selection_section(&mut self, ui: &mut egui::Ui) {
        let Some(object) = self.selected.clone() else {
            ui.weak("Click an object in the viewport to inspect it.");
            return;
        };
        let text = pick::describe_object(&self.infos, &object);
        egui::ScrollArea::vertical()
            .max_height(220.0)
            .show(ui, |ui| {
                ui.add(egui::Label::new(egui::RichText::new(text).monospace().size(12.0)).wrap());
            });
        ui.horizontal_wrapped(|ui| {
            let pick = Pick {
                object: Some(object),
                ..Pick::default()
            };
            ui.menu_button("Copy", |ui| self.copy_menu(ui, &pick));
            if ui.button("Deselect").clicked() {
                self.selected = None;
            }
        });
    }

    fn view_section(&mut self, ui: &mut egui::Ui) {
        egui::Grid::new("view_grid")
            .num_columns(2)
            .spacing([8.0, 6.0])
            .show(ui, |ui| {
                ui.label("Plane");
                ui.horizontal(|ui| {
                    for plane in 0..4_u8 {
                        ui.selectable_value(&mut self.view_plane, plane, plane.to_string());
                    }
                });
                ui.end_row();

                ui.label("Field of view");
                ui.add(egui::Slider::new(&mut self.camera.fov_degrees, 30.0..=100.0).suffix("°"));
                ui.end_row();

                ui.label("Fly speed");
                ui.add(
                    egui::Slider::new(&mut self.camera.speed, 50.0..=20_000.0).logarithmic(true),
                );
                ui.end_row();

                ui.label("Load radius");
                ui.add(egui::Slider::new(&mut self.stream_radius, 0..=4).suffix(" regions"));
                ui.end_row();
            });
    }

    fn shading_section(&mut self, ui: &mut egui::Ui) {
        egui::Grid::new("shading_grid")
            .num_columns(2)
            .spacing([8.0, 6.0])
            .show(ui, |ui| {
                ui.label("Brightness");
                ui.add(egui::Slider::new(&mut self.brightness, 0.5..=1.0));
                ui.end_row();

                ui.label("Fog depth");
                ui.add(egui::Slider::new(&mut self.fog.depth_tiles, 0..=100).suffix(" tiles"));
                ui.end_row();

                ui.label("Draw distance");
                ui.add(
                    egui::Slider::new(&mut self.fog.draw_distance_tiles, 8..=184).suffix(" tiles"),
                );
                ui.end_row();

                ui.label("Colorblind mode");
                egui::ComboBox::from_id_salt("colorblind_mode")
                    .selected_text(format!("{:?}", self.colorblind.mode))
                    .show_ui(ui, |ui| {
                        for mode in [
                            ColorblindMode::None,
                            ColorblindMode::Protanope,
                            ColorblindMode::Deuteranope,
                            ColorblindMode::Tritanope,
                        ] {
                            ui.selectable_value(
                                &mut self.colorblind.mode,
                                mode,
                                format!("{mode:?}"),
                            );
                        }
                    });
                ui.end_row();

                ui.label("Colorblind strength");
                ui.add(egui::Slider::new(
                    &mut self.colorblind.intensity,
                    0.0..=100.0,
                ));
                ui.end_row();
            });
        ui.checkbox(&mut self.remove_color_banding, "Remove color banding");
        ui.checkbox(&mut self.bright_textures, "Bright textures");
        ui.weak("Defaults match RuneLite's GPU plugin. Fog uses the sky colour.");
    }

    fn grid_toggles_inline(&mut self, ui: &mut egui::Ui) {
        ui.checkbox(&mut self.grid.tiles, "Tiles");
        ui.checkbox(&mut self.grid.zones, "Zones");
        ui.checkbox(&mut self.grid.regions, "Regions");
    }

    fn scene_section(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("Sky colour");
            ui.color_edit_button_rgb(&mut self.sky_color);
        });
        ui.label("Grid");
        ui.horizontal(|ui| self.grid_toggles_inline(ui));
        if ui
            .checkbox(&mut self.smooth_terrain, "Smooth terrain (not in the game)")
            .changed()
        {
            self.rebuild_all();
        }
    }
}
