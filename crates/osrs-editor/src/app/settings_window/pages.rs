//! The pages that are not plain row tables: plugin options, colours, controls, advanced.

use super::EditorApp;
use super::catalog::MOUSE_CONTROLS;
use crate::settings::Settings;
use eframe::egui;
use egui_phosphor::regular as ph;

impl EditorApp {
    /// The options of the plugins that are on (switch plugins on and off in the Plugins window).
    pub(super) fn plugin_options_page(&mut self, ui: &mut egui::Ui) {
        let mut any = false;
        self.run_plugins(|plugin, cx| {
            if plugin.has_properties() {
                any = true;
                crate::theme::section_heading(ui, plugin.title());
                plugin.properties(ui, cx);
            }
        });
        if !any {
            ui.weak("No plugin that is on has options.");
        }
    }

    /// The colours the viewport draws its indicators, the grid and the HUD with.
    pub(super) fn colors_page(&mut self, ui: &mut egui::Ui) {
        use crate::style::{ColorGroup, ColorKey};
        ui.weak(
            "Every outline, the grid and the HUD read these. The alpha is the opacity. \
             The UI theme (panels, text) is on the Interface page.",
        );
        ui.add_space(4.0);
        if ui.button("Reset all colours").clicked() {
            self.prefs.colors.reset_all();
        }
        for group in ColorGroup::ALL {
            crate::theme::section_heading(ui, group.label());
            egui::Grid::new(("colors", group.label()))
                .num_columns(3)
                .spacing([14.0, 6.0])
                .show(ui, |ui| {
                    for key in ColorKey::ALL.iter().filter(|k| k.group() == group) {
                        ui.label(key.label());
                        let [r, g, b, a] = self.prefs.colors.rgba(*key);
                        let mut color = egui::Color32::from_rgba_unmultiplied(r, g, b, a);
                        if ui.color_edit_button_srgba(&mut color).changed() {
                            let [r, g, b, a] = color.to_srgba_unmultiplied();
                            self.prefs.colors.set(*key, [r, g, b, a]);
                        }
                        if ui
                            .add_enabled(
                                !self.prefs.colors.is_default(*key),
                                egui::Button::new(ph::ARROW_COUNTER_CLOCKWISE).small(),
                            )
                            .on_hover_text("Back to the default")
                            .clicked()
                        {
                            self.prefs.colors.reset(*key);
                        }
                        ui.end_row();
                    }
                });
        }
    }

    /// Mouse controls and the editable keyboard shortcuts.
    pub(super) fn controls_page(&mut self, ui: &mut egui::Ui) {
        crate::theme::section_heading(ui, "Mouse");
        egui::Grid::new("mouse_controls")
            .num_columns(2)
            .spacing([14.0, 6.0])
            .min_col_width(190.0)
            .show(ui, |ui| {
                for (input, action) in MOUSE_CONTROLS {
                    ui.weak(*input);
                    ui.label(*action);
                    ui.end_row();
                }
            });
        crate::theme::section_heading(ui, "Keyboard");
        ui.horizontal(|ui| {
            ui.weak("Click a shortcut, then press the new keys. Esc cancels.");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Reset all").clicked() {
                    self.keymap.reset_all();
                }
            });
        });
        ui.add_space(4.0);
        let actions: Vec<_> = self.keymap.actions().to_vec();
        let mut categories: Vec<&str> = Vec::new();
        for action in &actions {
            if !categories.contains(&action.category) {
                categories.push(action.category);
            }
        }
        for category in categories {
            ui.add_space(6.0);
            ui.label(egui::RichText::new(category).strong());
            egui::Grid::new(("key_bindings", category))
                .num_columns(3)
                .spacing([14.0, 5.0])
                .min_col_width(190.0)
                .show(ui, |ui| {
                    for action in actions.iter().filter(|a| a.category == category) {
                        ui.weak(action.label);
                        let waiting = self.capturing_binding == Some(action.id);
                        let text = if waiting {
                            "Press keys...".to_owned()
                        } else {
                            self.keymap
                                .binding(action.id)
                                .map_or_else(|| "Unbound".to_owned(), |b| b.describe())
                        };
                        if ui
                            .add_sized(
                                [150.0, 24.0],
                                egui::Button::new(egui::RichText::new(text).monospace().size(12.5))
                                    .selected(waiting),
                            )
                            .clicked()
                        {
                            self.capturing_binding = Some(action.id);
                        }
                        ui.horizontal(|ui| {
                            if self.keymap.is_overridden(action.id)
                                && ui.small_button("Reset").clicked()
                            {
                                self.keymap.reset(action.id);
                            }
                            if let Some(binding) = self.keymap.binding(action.id) {
                                let conflicts = self.keymap.conflicts(action.id, binding);
                                if !conflicts.is_empty() {
                                    ui.colored_label(
                                        ui.visuals().warn_fg_color,
                                        format!("also: {}", conflicts.join(", ")),
                                    );
                                }
                            }
                        });
                        ui.end_row();
                    }
                });
        }
    }

    /// Reset buttons and where the settings file lives.
    pub(super) fn advanced_page(&mut self, ui: &mut egui::Ui) {
        crate::theme::section_heading(ui, "Reset");
        ui.horizontal(|ui| {
            if ui
                .button("Reset the window layout")
                .on_hover_text("Side panel, minimap and statistics back to their defaults.")
                .clicked()
            {
                let defaults = Settings::default();
                self.prefs.properties_panel = defaults.properties_panel;
                self.prefs.dock_right = defaults.dock_right;
                self.prefs.stats_overlay = defaults.stats_overlay;
                self.prefs.show_minimap = defaults.show_minimap;
                self.prefs.minimap_size = defaults.minimap_size;
                self.prefs.rail_tab = defaults.rail_tab;
            }
            if ui
                .button("Reset the HD options")
                .on_hover_text(
                    "Lighting, fog, sky, shadows, water and brightness of the HD profile.",
                )
                .clicked()
            {
                let defaults = Settings::default();
                self.prefs.hd_brightness = defaults.hd_brightness;
                self.prefs.hd_exposure = defaults.hd_exposure;
                self.prefs.hd_vignette = defaults.hd_vignette;
                self.prefs.hd_normal_mapping = defaults.hd_normal_mapping;
                self.prefs.hd_parallax_mapping = defaults.hd_parallax_mapping;
                self.prefs.hd_underwater_caustics = defaults.hd_underwater_caustics;
                self.prefs.hd_environment = defaults.hd_environment;
                self.prefs.hd_shadows = defaults.hd_shadows;
            }
        });
        ui.add_space(6.0);
        crate::theme::section_heading(ui, "Settings file");
        match crate::settings::path() {
            Some(path) => {
                ui.label(path.display().to_string());
            }
            None => {
                ui.weak("not saved (no settings path)");
            }
        }
        ui.weak(
            "Settings are saved automatically; unknown or missing fields never break the file.",
        );
    }
}

/// One entry of the settings page list: icon and title, left aligned, filled when selected.
pub(super) fn nav_item(
    ui: &mut egui::Ui,
    selected: bool,
    icon: &str,
    title: &str,
) -> egui::Response {
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 30.0), egui::Sense::click());
    if ui.is_rect_visible(rect) {
        let visuals = ui.visuals();
        let fill = if selected {
            visuals.selection.bg_fill
        } else if response.hovered() {
            visuals.widgets.hovered.weak_bg_fill
        } else {
            egui::Color32::TRANSPARENT
        };
        ui.painter().rect_filled(rect, 6.0, fill);
        let color = if selected {
            visuals.selection.stroke.color
        } else {
            visuals.widgets.inactive.fg_stroke.color
        };
        let font = egui::TextStyle::Body.resolve(ui.style());
        ui.painter().text(
            rect.left_center() + egui::vec2(10.0, 0.0),
            egui::Align2::LEFT_CENTER,
            icon,
            font.clone(),
            color,
        );
        ui.painter().text(
            rect.left_center() + egui::vec2(36.0, 0.0),
            egui::Align2::LEFT_CENTER,
            title,
            font,
            color,
        );
    }
    response
}
