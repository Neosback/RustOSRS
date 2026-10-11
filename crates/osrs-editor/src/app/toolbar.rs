//! Toolbar under the menu bar: the controls used all the time, as buttons.
//!
//! Left to right: plane, camera mode, plugin controls (grid, ...), then teleport and the side
//! panel toggle on the right. Each control also lives in a menu or panel; the toolbar is only a
//! shortcut.

use super::EditorApp;
use crate::camera::CameraMode;
use crate::icons;
use eframe::egui;

impl EditorApp {
    pub(super) fn toolbar(&mut self, ui: &mut egui::Ui) {
        egui::Panel::top("toolbar").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label("Plane");
                for plane in 0..4_u8 {
                    ui.selectable_value(&mut self.view_plane, plane, plane.to_string())
                        .on_hover_text(format!("Show plane {plane}"));
                }
                ui.separator();
                ui.selectable_value(
                    &mut self.camera.mode,
                    CameraMode::Fly,
                    format!("{} Fly", icons::FLY),
                )
                .on_hover_text("Fly camera: right drag looks, WASD moves");
                ui.selectable_value(
                    &mut self.camera.mode,
                    CameraMode::Orbit,
                    format!("{} Orbit", icons::ORBIT),
                )
                .on_hover_text(
                    "Orbit camera: right drag orbits, shift+right drag pans, scroll zooms",
                );
                ui.separator();
                self.run_plugins(|plugin, cx| plugin.toolbar(ui, cx));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .button(icons::SETTINGS)
                        .on_hover_text("Settings (search every option)")
                        .clicked()
                    {
                        self.open_settings(None);
                        self.settings_focus_search = true;
                    }
                    ui.toggle_value(&mut self.prefs.properties_panel, icons::SIDE_PANEL)
                        .on_hover_text("Side panel");
                    ui.toggle_value(&mut self.prefs.stats_overlay, icons::STATS)
                        .on_hover_text("Statistics overlay");
                    ui.toggle_value(&mut self.world_map.open, icons::WORLD_MAP)
                        .on_hover_text("World map (M): double-click a tile to go there");
                    if ui
                        .button(format!("{} Teleport", icons::TELEPORT))
                        .on_hover_text("Go to a tile or region")
                        .clicked()
                    {
                        self.open_teleport();
                    }
                });
            });
        });
    }
}
