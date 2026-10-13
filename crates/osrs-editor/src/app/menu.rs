//! Menu bar and the controls reference window.

use super::EditorApp;
use crate::camera::CameraMode;
use crate::icons;
use eframe::egui;

impl EditorApp {
    pub(super) fn menu_bar(&mut self, ui: &mut egui::Ui) {
        egui::Panel::top("menu").show(ui, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                ui.menu_button("File", |ui| {
                    self.cache_menu(ui);
                    self.project_menu(ui);
                    self.export_menu(ui);
                    ui.separator();
                    if ui.button("Quit").clicked() {
                        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
                ui.menu_button("Edit", |ui| {
                    if ui
                        .add_enabled(
                            self.can_undo(),
                            egui::Button::new(format!(
                                "Undo   {}",
                                self.keymap.describe("edit.undo")
                            )),
                        )
                        .clicked()
                    {
                        self.undo();
                        ui.close();
                    }
                    if ui
                        .add_enabled(
                            self.can_redo(),
                            egui::Button::new(format!(
                                "Redo   {}",
                                self.keymap.describe("edit.redo")
                            )),
                        )
                        .clicked()
                    {
                        self.redo();
                        ui.close();
                    }
                    ui.separator();
                    if ui
                        .button(format!(
                            "{} Settings...   {}",
                            icons::SETTINGS,
                            self.keymap.describe("settings.open")
                        ))
                        .clicked()
                    {
                        self.open_settings(None);
                        self.settings_focus_search = true;
                        ui.close();
                    }
                    if ui.button("Keyboard shortcuts...").clicked() {
                        self.open_settings(Some(crate::settings::SettingsPage::Controls));
                        ui.close();
                    }
                });
                ui.menu_button("View", |ui| {
                    ui.checkbox(&mut self.prefs.properties_panel, "Side panel");
                    ui.checkbox(&mut self.prefs.stats_overlay, "Statistics overlay");
                    ui.checkbox(&mut self.prefs.show_minimap, "Minimap");
                    ui.menu_button("Camera", |ui| self.camera_mode_picker(ui));
                    self.run_plugins(|plugin, cx| {
                        if plugin.has_menu() {
                            ui.menu_button(plugin.title(), |ui| plugin.menu(ui, cx));
                        }
                    });
                    ui.separator();
                    ui.label("Plane");
                    for plane in 0..4_u8 {
                        ui.radio_value(
                            &mut self.interaction.view_plane,
                            plane,
                            format!("z = {plane}"),
                        );
                    }
                    ui.separator();
                    if ui
                        .button(format!("{} Script...", icons::SCRIPT))
                        .on_hover_text("Run editor commands from text")
                        .clicked()
                    {
                        self.script_open = true;
                        ui.close();
                    }
                    if ui
                        .button(format!("{} Settings...", icons::SETTINGS))
                        .clicked()
                    {
                        self.open_settings(None);
                        ui.close();
                    }
                });
                if ui
                    .button("Plugins")
                    .on_hover_text("Switch plugins on and off, and your own tools")
                    .clicked()
                {
                    self.plugins_open = true;
                }
                ui.menu_button("Go", |ui| {
                    if ui.button("World map...").clicked() {
                        self.toggle_world_map();
                        ui.close();
                    }
                    if ui.button("Teleport...   T").clicked() {
                        self.open_teleport();
                        ui.close();
                    }
                });
                ui.menu_button("Help", |ui| {
                    if ui.button("Controls and shortcuts").clicked() {
                        self.open_settings(Some(crate::settings::SettingsPage::Controls));
                        ui.close();
                    }
                });
            });
        });
    }

    /// Fly / orbit camera choice.
    pub(super) fn camera_mode_picker(&mut self, ui: &mut egui::Ui) {
        ui.radio_value(
            &mut self.interaction.camera.mode,
            CameraMode::Fly,
            "Fly   C",
        );
        ui.radio_value(
            &mut self.interaction.camera.mode,
            CameraMode::Orbit,
            "Orbit   C",
        );
    }
}
