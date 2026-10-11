//! Right-hand Properties dock, organised in collapsible sections (Selection, View, Shading,
//! Scene, Interface). New tools add their own section here.

use super::EditorApp;
use crate::camera::CameraMode;
use crate::pick::{self, Pick};
use crate::settings::{ColorblindChoice, PanelTab};
use eframe::egui;

fn section(ui: &mut egui::Ui, title: &str, default_open: bool, body: impl FnOnce(&mut egui::Ui)) {
    egui::CollapsingHeader::new(egui::RichText::new(title).strong())
        .default_open(default_open)
        .show(ui, body);
}

impl EditorApp {
    /// Side panel: a tab strip (Object, View, Shading, Tools) over one scrolling page, docked on
    /// the right or left (View menu).
    pub(super) fn properties_panel(&mut self, ui: &mut egui::Ui) {
        if !self.prefs.properties_panel {
            return;
        }
        let panel = if self.prefs.dock_right {
            egui::Panel::right("side_panel")
        } else {
            egui::Panel::left("side_panel")
        };
        panel.resizable(true).default_size(300.0).show(ui, |ui| {
            ui.horizontal(|ui| {
                for (tab, label) in [
                    (PanelTab::Object, "Object"),
                    (PanelTab::View, "View"),
                    (PanelTab::Shading, "Shading"),
                    (PanelTab::Tools, "Tools"),
                ] {
                    ui.selectable_value(&mut self.prefs.side_tab, tab, label);
                }
            });
            ui.separator();
            egui::ScrollArea::vertical().show(ui, |ui| match self.prefs.side_tab {
                PanelTab::Object => self.selection_section(ui),
                PanelTab::View => {
                    self.view_section(ui);
                    ui.separator();
                    section(ui, "Interface", true, |ui| self.theme_picker(ui));
                }
                PanelTab::Shading => self.shading_section(ui),
                PanelTab::Tools => {
                    section(ui, "Scene", true, |ui| self.scene_section(ui));
                    self.run_plugins(|plugin, cx| {
                        if plugin.has_properties() {
                            section(ui, plugin.title(), true, |ui| plugin.properties(ui, cx));
                        }
                    });
                }
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
            self.run_plugins(|plugin, cx| plugin.selection_ui(ui, cx));
            if ui.button("Deselect").clicked() {
                self.selected = None;
            }
        });
    }

    fn view_section(&mut self, ui: &mut egui::Ui) {
        ui.checkbox(&mut self.prefs.show_minimap, "Minimap");
        ui.horizontal(|ui| {
            ui.label("Minimap size");
            self.minimap_size_picker_inline(ui);
        });
        ui.checkbox(&mut self.prefs.mouse_tooltips, "Mouse tooltips");
        ui.checkbox(&mut self.prefs.hover_tile, "Highlight hovered tile");
        ui.checkbox(&mut self.prefs.animate_objects, "Animate objects");
        ui.add_space(4.0);
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

                ui.label("Camera");
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.camera.mode, CameraMode::Fly, "Fly");
                    ui.selectable_value(&mut self.camera.mode, CameraMode::Orbit, "Orbit");
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
                ui.add(egui::Slider::new(&mut self.prefs.load_radius, 0..=4).suffix(" regions"));
                ui.end_row();
            });
    }

    fn shading_section(&mut self, ui: &mut egui::Ui) {
        egui::Grid::new("shading_grid")
            .num_columns(2)
            .spacing([8.0, 6.0])
            .show(ui, |ui| {
                ui.label("Brightness");
                ui.add(egui::Slider::new(&mut self.prefs.brightness, 0.5..=1.0));
                ui.end_row();

                ui.label("Fog depth");
                ui.add(
                    egui::Slider::new(&mut self.prefs.fog_depth_tiles, 0..=100).suffix(" tiles"),
                );
                ui.end_row();

                ui.label("Draw distance");
                ui.add(
                    egui::Slider::new(&mut self.prefs.draw_distance_tiles, 8..=184)
                        .suffix(" tiles"),
                );
                ui.end_row();

                ui.label("Colorblind mode");
                egui::ComboBox::from_id_salt("colorblind_mode")
                    .selected_text(format!("{:?}", self.prefs.colorblind_mode))
                    .show_ui(ui, |ui| {
                        for mode in ColorblindChoice::ALL {
                            ui.selectable_value(
                                &mut self.prefs.colorblind_mode,
                                mode,
                                format!("{mode:?}"),
                            );
                        }
                    });
                ui.end_row();

                ui.label("Colorblind strength");
                ui.add(egui::Slider::new(
                    &mut self.prefs.colorblind_intensity,
                    0.0..=100.0,
                ));
                ui.end_row();
            });
        ui.checkbox(&mut self.prefs.remove_color_banding, "Remove color banding");
        ui.checkbox(&mut self.prefs.bright_textures, "Bright textures");
        ui.weak("Defaults match RuneLite's GPU plugin. Fog uses the sky colour.");
    }

    fn scene_section(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("Sky colour");
            ui.color_edit_button_rgb(&mut self.prefs.sky_color);
        });
        if ui
            .checkbox(
                &mut self.prefs.smooth_terrain,
                "Smooth terrain (not in the game)",
            )
            .changed()
        {
            self.rebuild_all();
        }
    }
}
