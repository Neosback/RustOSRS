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
        let compact = ui.ctx().content_rect().width() < super::workspace::COMPACT_WIDTH;
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
                    if compact {
                        icons::FLY.to_owned()
                    } else {
                        format!("{} Fly", icons::FLY)
                    },
                )
                .on_hover_text("Fly camera: right drag looks, WASD moves");
                ui.selectable_value(
                    &mut self.camera.mode,
                    CameraMode::Orbit,
                    if compact {
                        icons::ORBIT.to_owned()
                    } else {
                        format!("{} Orbit", icons::ORBIT)
                    },
                )
                .on_hover_text(
                    "Orbit camera: right drag orbits, shift+right drag pans, scroll zooms",
                );
                ui.separator();
                if self.prefs.tool != crate::paint::Tool::Select
                    || self.can_undo()
                    || self.can_redo()
                {
                    if ui
                        .add_enabled(
                            self.can_undo(),
                            egui::Button::new(egui_phosphor::regular::ARROW_U_UP_LEFT),
                        )
                        .on_hover_text("Undo (Cmd+Z)")
                        .clicked()
                    {
                        self.undo();
                    }
                    if ui
                        .add_enabled(
                            self.can_redo(),
                            egui::Button::new(egui_phosphor::regular::ARROW_U_UP_RIGHT),
                        )
                        .on_hover_text("Redo (Cmd+Shift+Z)")
                        .clicked()
                    {
                        self.redo();
                    }
                    ui.separator();
                }
                self.run_plugins(|plugin, cx| plugin.toolbar(ui, cx));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.toggle_value(&mut self.prefs.stats_overlay, icons::STATS)
                        .on_hover_text("Statistics overlay");
                    ui.toggle_value(&mut self.world_map.open, icons::WORLD_MAP)
                        .on_hover_text("World map (M): double-click a tile to go there");
                    let label = if compact {
                        icons::TELEPORT.to_owned()
                    } else {
                        format!("{} Teleport", icons::TELEPORT)
                    };
                    if ui
                        .button(label)
                        .on_hover_text("Go to a tile or region (T)")
                        .clicked()
                    {
                        self.open_teleport();
                    }
                });
            });
        });
    }
}
