//! Menu bar and the controls reference window.

use super::EditorApp;
use crate::camera::CameraMode;
use eframe::egui;

/// Mouse and keyboard reference shown by Help > Controls.
const CONTROLS: &[(&str, &str)] = &[
    ("Left click", "select the object under the cursor"),
    (
        "Right click",
        "copy menu for the object / tile under the cursor",
    ),
    ("Drag", "look around (fly) / orbit (orbit camera)"),
    ("Shift+drag, right drag", "pan (orbit camera)"),
    ("C", "switch between fly and orbit camera"),
    ("F", "focus the selected object"),
    ("W A S D", "fly (Shift = fast)"),
    ("Q / E", "down / up"),
    ("Scroll", "fly speed (fly) / zoom (orbit)"),
    ("1 - 4", "show plane 0 - 3"),
    ("T", "teleport"),
    ("N", "toggle the Properties panel"),
];

impl EditorApp {
    pub(super) fn menu_bar(&mut self, ui: &mut egui::Ui) {
        egui::Panel::top("menu").show(ui, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Quit").clicked() {
                        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
                ui.menu_button("View", |ui| {
                    ui.checkbox(&mut self.panels.properties, "Properties panel   N");
                    ui.checkbox(&mut self.panels.stats_overlay, "Statistics overlay");
                    ui.menu_button("Camera", |ui| self.camera_mode_picker(ui));
                    self.run_plugins(|plugin, cx| {
                        if plugin.has_menu() {
                            ui.menu_button(plugin.title(), |ui| plugin.menu(ui, cx));
                        }
                    });
                    ui.separator();
                    ui.label("Plane");
                    for plane in 0..4_u8 {
                        ui.radio_value(&mut self.view_plane, plane, format!("z = {plane}"));
                    }
                    ui.separator();
                    ui.menu_button("Theme", |ui| self.theme_picker(ui));
                });
                ui.menu_button("Go", |ui| {
                    if ui.button("Teleport...   T").clicked() {
                        self.open_teleport();
                        ui.close();
                    }
                });
                ui.menu_button("Help", |ui| {
                    if ui.button("Controls").clicked() {
                        self.panels.help = true;
                        ui.close();
                    }
                });
            });
        });
    }

    /// Fly / orbit camera choice.
    pub(super) fn camera_mode_picker(&mut self, ui: &mut egui::Ui) {
        ui.radio_value(&mut self.camera.mode, CameraMode::Fly, "Fly   C");
        ui.radio_value(&mut self.camera.mode, CameraMode::Orbit, "Orbit   C");
    }

    /// Theme choices (shared by the View menu and the Properties panel).
    pub(super) fn theme_picker(&mut self, ui: &mut egui::Ui) {
        for (index, name) in crate::theme::NAMES.iter().enumerate() {
            if ui
                .radio_value(&mut self.theme_index, index, *name)
                .clicked()
            {
                crate::theme::set_theme(ui.ctx(), crate::theme::ALL[index]);
            }
        }
    }

    pub(super) fn help_window(&mut self, ctx: &egui::Context) {
        let mut open = self.panels.help;
        egui::Window::new("Controls")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                egui::Grid::new("controls").num_columns(2).show(ui, |ui| {
                    for (keys, action) in CONTROLS {
                        ui.strong(*keys);
                        ui.label(*action);
                        ui.end_row();
                    }
                    for plugin in &self.plugins {
                        for (key, action) in plugin.hotkeys() {
                            ui.strong(key.name());
                            ui.label(*action);
                            ui.end_row();
                        }
                    }
                });
            });
        self.panels.help = open;
    }
}
