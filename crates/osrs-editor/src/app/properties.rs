//! Right-hand Properties dock, organised in collapsible sections (Selection, View, Shading,
//! Scene, Interface). New tools add their own section here.

use super::EditorApp;
use crate::camera::CameraMode;
use crate::pick::{self, Pick};
use crate::settings::{ColorblindChoice, PanelTab, ProfileChoice};
use eframe::egui;
use osrs_hd::{
    environment::{DefaultSky, FogDepthMode},
    shadows::{ShadowFiltering, ShadowMode, ShadowResolution},
};

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
        let hd = self.prefs.render_profile == ProfileChoice::Hd;
        egui::Grid::new("shading_grid")
            .num_columns(2)
            .spacing([8.0, 6.0])
            .show(ui, |ui| {
                ui.label("Render profile");
                egui::ComboBox::from_id_salt("render_profile")
                    .selected_text(self.prefs.render_profile.label())
                    .show_ui(ui, |ui| {
                        for profile in ProfileChoice::ALL {
                            ui.selectable_value(
                                &mut self.prefs.render_profile,
                                profile,
                                profile.label(),
                            );
                        }
                    });
                ui.end_row();

                if hd {
                    ui.label("HD exposure");
                    ui.add(egui::Slider::new(&mut self.prefs.hd_exposure, 0.25..=4.0));
                    ui.end_row();

                    ui.label("HD vignette");
                    ui.add(egui::Slider::new(&mut self.prefs.hd_vignette, 0.0..=1.0));
                    ui.end_row();
                } else {
                    ui.label("Brightness");
                    ui.add(egui::Slider::new(&mut self.prefs.brightness, 0.5..=1.0));
                    ui.end_row();

                    ui.label("Fog depth");
                    ui.add(
                        egui::Slider::new(&mut self.prefs.fog_depth_tiles, 0..=100)
                            .suffix(" tiles"),
                    );
                    ui.end_row();
                }

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
        if hd {
            self.hd_shading_sections(ui);
        } else {
            ui.checkbox(&mut self.prefs.remove_color_banding, "Remove color banding");
            ui.checkbox(&mut self.prefs.bright_textures, "Bright textures");
            ui.weak("Defaults match RuneLite's GPU plugin. Fog uses the sky colour.");
        }
    }

    /// The 117 HD sections of Properties > Shading (mirroring `HdPluginConfig`): Lighting and
    /// Environment.
    fn hd_shading_sections(&mut self, ui: &mut egui::Ui) {
        ui.separator();
        section(ui, "Lighting", true, |ui| {
            ui.checkbox(
                &mut self.prefs.hd_environment.atmospheric_lighting,
                "Atmospheric lighting",
            )
            .on_hover_text("Change environmental lighting based on the current area.");
            ui.checkbox(&mut self.prefs.hd_normal_mapping, "Normal mapping")
                .on_hover_text("Affects how light interacts with certain materials.");
            ui.checkbox(
                &mut self.prefs.hd_environment.flashing_effects,
                "Lightning flashes",
            )
            .on_hover_text("Flashing lightning in storm areas (photosensitivity warning).");
        });
        section(ui, "Shadows", true, |ui| {
            let shadows = &mut self.prefs.hd_shadows;
            egui::Grid::new("hd_shadow_grid")
                .num_columns(2)
                .spacing([8.0, 6.0])
                .show(ui, |ui| {
                    ui.label("Shadow mode");
                    egui::ComboBox::from_id_salt("hd_shadow_mode")
                        .selected_text(shadows.mode.label())
                        .show_ui(ui, |ui| {
                            for mode in ShadowMode::ALL {
                                ui.selectable_value(&mut shadows.mode, mode, mode.label());
                            }
                        });
                    ui.end_row();

                    if shadows.enabled() {
                        ui.label("Shadow quality");
                        egui::ComboBox::from_id_salt("hd_shadow_resolution")
                            .selected_text(shadows.resolution.label())
                            .show_ui(ui, |ui| {
                                for resolution in ShadowResolution::ALL {
                                    ui.selectable_value(
                                        &mut shadows.resolution,
                                        resolution,
                                        resolution.label(),
                                    );
                                }
                            });
                        ui.end_row();

                        ui.label("Shadow filtering");
                        egui::ComboBox::from_id_salt("hd_shadow_filtering")
                            .selected_text(shadows.filtering.label())
                            .show_ui(ui, |ui| {
                                for filtering in ShadowFiltering::ALL {
                                    ui.selectable_value(
                                        &mut shadows.filtering,
                                        filtering,
                                        filtering.label(),
                                    );
                                }
                            });
                        ui.end_row();

                        ui.label("Shadow distance");
                        ui.add(
                            egui::Slider::new(&mut shadows.distance_tiles, 4..=184)
                                .suffix(" tiles"),
                        );
                        ui.end_row();
                    }
                });
            if shadows.enabled() {
                ui.checkbox(&mut shadows.transparency, "Shadow transparency")
                    .on_hover_text("Take model transparency into account.");
                ui.checkbox(&mut shadows.terrain_shadows, "Terrain shadows")
                    .on_hover_text("Allow terrain to cast shadows. May cause artifacts on slopes.");
                ui.checkbox(&mut shadows.roof_shadows, "Roof shadows")
                    .on_hover_text("Always cast shadows from roofs, even when they are hidden.");
                let bytes = self.renderer.hd_shadow_map_bytes();
                if bytes > 0 {
                    ui.weak(format!(
                        "Shadow maps: {:.0} MiB",
                        bytes as f64 / 1_048_576.0
                    ));
                }
            }
        });
        section(ui, "Environment", true, |ui| {
            let environment = &mut self.prefs.hd_environment;
            egui::Grid::new("hd_environment_grid")
                .num_columns(2)
                .spacing([8.0, 6.0])
                .show(ui, |ui| {
                    ui.label("Current area");
                    ui.label(
                        self.hd
                            .current_name()
                            .unwrap_or_else(|| "(loading)".to_owned()),
                    );
                    ui.end_row();

                    ui.label("Fog depth mode");
                    egui::ComboBox::from_id_salt("hd_fog_depth_mode")
                        .selected_text(environment.fog_depth_mode.label())
                        .show_ui(ui, |ui| {
                            for mode in FogDepthMode::ALL {
                                ui.selectable_value(
                                    &mut environment.fog_depth_mode,
                                    mode,
                                    mode.label(),
                                );
                            }
                        });
                    ui.end_row();

                    if environment.fog_depth_mode == FogDepthMode::Static {
                        ui.label("Static fog depth");
                        ui.add(
                            egui::Slider::new(&mut environment.static_fog_depth, 0.0..=100.0)
                                .suffix(" tiles"),
                        );
                        ui.end_row();
                    }

                    ui.label("Default sky");
                    egui::ComboBox::from_id_salt("hd_default_sky")
                        .selected_text(environment.default_sky.label())
                        .show_ui(ui, |ui| {
                            for sky in DefaultSky::ALL {
                                ui.selectable_value(&mut environment.default_sky, sky, sky.label());
                            }
                        });
                    ui.end_row();
                });
            ui.checkbox(&mut environment.ground_fog, "Ground fog");
            ui.checkbox(&mut environment.override_sky, "Override sky colour")
                .on_hover_text("Force the default sky colour in all environments.");
            ui.checkbox(
                &mut environment.decouple_sky_and_water,
                "Decouple sky and water colour",
            );
            ui.checkbox(&mut environment.poh_themes, "Player-owned house themes");
            ui.checkbox(&mut environment.legacy_tob, "Legacy Theatre of Blood");
        });
        ui.weak(
            "HD is a work in progress: materials, normal maps, specular, ground blending and \
             area environments and shadows are in. Water, lights and the sky are still to come. \
             Animated objects still use Reference shading.",
        );
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
