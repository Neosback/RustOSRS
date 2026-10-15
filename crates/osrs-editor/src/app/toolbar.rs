//! Toolbar under the menu bar: the controls used all the time, as buttons.
//!
//! Left to right: plane, camera mode, undo and redo, plugin controls (grid, ...), then the overlays
//! menu and the world map on the right. Each control also lives in a menu or panel; the toolbar is only a
//! shortcut.

use super::EditorApp;
use crate::camera::CameraMode;
use crate::icons;
use eframe::egui;

impl EditorApp {
    pub(super) fn toolbar(&mut self, ui: &mut egui::Ui) {
        let mut camera_mode = self.interaction.camera().mode;
        let mut view_plane = self.interaction.view_plane();
        let compact = ui.ctx().content_rect().width() < super::workspace::COMPACT_WIDTH;
        egui::Panel::top("toolbar").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label("Plane");
                for plane in 0..4_u8 {
                    ui.selectable_value(&mut view_plane, plane, plane.to_string())
                        .on_hover_text(format!("Show plane {plane}"));
                }
                ui.separator();
                ui.selectable_value(
                    &mut camera_mode,
                    CameraMode::Fly,
                    if compact {
                        icons::FLY.to_owned()
                    } else {
                        format!("{} Fly", icons::FLY)
                    },
                )
                .on_hover_text("Fly camera: right drag looks, WASD moves");
                ui.selectable_value(
                    &mut camera_mode,
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
                self.interaction.set_camera_mode(camera_mode);
                self.interaction.set_view_plane(view_plane);
                ui.separator();
                if self.settings.prefs.tool != crate::paint::Tool::Select
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
                    ui.toggle_value(&mut self.ui.world_map.open, icons::WORLD_MAP)
                        .on_hover_text("World map (M): double-click a tile to go there");
                    // What the viewport shows on top of the world, in one menu.
                    ui.menu_button(icons::OVERLAYS, |ui| {
                        ui.set_min_width(190.0);
                        ui.checkbox(&mut self.settings.prefs.tile_info_hud, "Tile info")
                            .on_hover_text("The tile under the cursor, in the corner");
                        ui.checkbox(&mut self.settings.prefs.hover_tile, "Tile outline")
                            .on_hover_text(
                                "Outline the tile (or the brush footprint) under the cursor",
                            );
                        ui.checkbox(&mut self.settings.prefs.stats_overlay, "Statistics")
                            .on_hover_text("Frame rate, timings and the camera tile");
                        ui.checkbox(&mut self.settings.prefs.show_minimap, "Minimap");
                        ui.checkbox(
                            &mut self.settings.prefs.map_icons_3d,
                            "Map icons in the world",
                        );
                        ui.checkbox(&mut self.settings.prefs.mouse_tooltips, "Action tooltip")
                            .on_hover_text("The game's action text next to the cursor");
                        ui.separator();
                        if ui.button("More in the View panel...").clicked() {
                            self.settings.prefs.rail_tab = crate::settings::RailTab::View;
                            self.settings.prefs.view_tab = crate::settings::ViewTab::Overlays;
                            self.settings.prefs.properties_panel = true;
                            ui.close();
                        }
                    })
                    .response
                    .on_hover_text("What the viewport shows");
                });
            });
        });
    }
}
