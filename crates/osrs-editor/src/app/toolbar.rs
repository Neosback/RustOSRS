//! Toolbar under the menu bar: the controls used all the time, as buttons.
//!
//! Left to right: plane, camera mode, plugin controls (grid, ...), then teleport and the side
//! panel toggle on the right. Each control also lives in a menu or panel; the toolbar is only a
//! shortcut.

use super::EditorApp;
use crate::camera::CameraMode;
use eframe::egui;

impl EditorApp {
    pub(super) fn toolbar(&mut self, ui: &mut egui::Ui) {
        egui::Panel::top("toolbar").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label("Plane");
                for plane in 0..4_u8 {
                    ui.selectable_value(&mut self.view_plane, plane, plane.to_string());
                }
                ui.separator();
                ui.selectable_value(&mut self.camera.mode, CameraMode::Fly, "Fly");
                ui.selectable_value(&mut self.camera.mode, CameraMode::Orbit, "Orbit");
                ui.separator();
                self.run_plugins(|plugin, cx| plugin.toolbar(ui, cx));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.toggle_value(&mut self.panels.properties, "Panel");
                    ui.toggle_value(&mut self.panels.stats_overlay, "Stats");
                    if ui.button("Teleport").clicked() {
                        self.open_teleport();
                    }
                });
            });
        });
    }
}
