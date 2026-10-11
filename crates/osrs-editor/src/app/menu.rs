//! Menu bar and the controls reference window.

use super::EditorApp;
use crate::camera::CameraMode;
use crate::keymap::Binding;
use eframe::egui;

/// Mouse controls (keys are listed from the keymap).
const MOUSE_CONTROLS: &[(&str, &str)] = &[
    ("Left click", "select the object under the cursor"),
    ("Right drag", "look around (fly) / orbit (orbit camera)"),
    ("Shift + right drag, middle drag", "pan"),
    (
        "Shift + right click",
        "copy menu for the object / tile under the cursor",
    ),
    ("Scroll", "fly speed (fly) / zoom (orbit)"),
    ("Shift + movement keys", "move faster"),
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
                ui.menu_button("Edit", |ui| {
                    if ui.button("Keyboard shortcuts...").clicked() {
                        self.keys_window_open = true;
                        ui.close();
                    }
                });
                ui.menu_button("View", |ui| {
                    ui.checkbox(&mut self.prefs.properties_panel, "Side panel");
                    ui.checkbox(&mut self.prefs.dock_right, "Dock side panel on the right");
                    ui.checkbox(&mut self.prefs.stats_overlay, "Statistics overlay");
                    ui.checkbox(&mut self.prefs.show_compass, "Compass");
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
                        self.keys_window_open = true;
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
                .radio_value(&mut self.prefs.theme, index, *name)
                .clicked()
            {
                crate::theme::set_theme(ui.ctx(), crate::theme::ALL[index]);
            }
        }
    }

    /// Controls reference with editable keyboard shortcuts (click a binding, press the new keys).
    pub(super) fn keys_window(&mut self, ctx: &egui::Context) {
        let mut open = self.keys_window_open;
        if !open {
            self.capturing_binding = None;
            return;
        }
        // Capture: the first key pressed while a row is waiting becomes its binding.
        if let Some(id) = self.capturing_binding {
            if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                self.capturing_binding = None;
            } else if let Some(binding) = ctx.input(Binding::capture) {
                self.keymap.set(id, binding);
                self.capturing_binding = None;
            }
        }
        egui::Window::new("Controls and shortcuts")
            .open(&mut open)
            .collapsible(false)
            .default_width(460.0)
            .show(ctx, |ui| {
                ui.strong("Mouse");
                egui::Grid::new("mouse_controls")
                    .num_columns(2)
                    .show(ui, |ui| {
                        for (input, action) in MOUSE_CONTROLS {
                            ui.label(*input);
                            ui.label(*action);
                            ui.end_row();
                        }
                    });
                ui.separator();
                ui.horizontal(|ui| {
                    ui.strong("Keyboard");
                    ui.weak("click a shortcut, then press the new keys (Esc cancels)");
                    if ui.small_button("Reset all").clicked() {
                        self.keymap.reset_all();
                    }
                });
                egui::ScrollArea::vertical()
                    .max_height(360.0)
                    .show(ui, |ui| {
                        let actions: Vec<_> = self.keymap.actions().to_vec();
                        let mut category = "";
                        egui::Grid::new("key_bindings")
                            .num_columns(3)
                            .spacing([12.0, 4.0])
                            .show(ui, |ui| {
                                for action in actions {
                                    if action.category != category {
                                        category = action.category;
                                        ui.end_row();
                                        ui.strong(category);
                                        ui.end_row();
                                    }
                                    ui.label(action.label);
                                    let waiting = self.capturing_binding == Some(action.id);
                                    let text = if waiting {
                                        "press keys...".to_owned()
                                    } else {
                                        self.keymap
                                            .binding(action.id)
                                            .map_or_else(|| "unbound".to_owned(), |b| b.describe())
                                    };
                                    if ui.selectable_label(waiting, text).clicked() {
                                        self.capturing_binding = Some(action.id);
                                    }
                                    ui.horizontal(|ui| {
                                        if self.keymap.is_overridden(action.id)
                                            && ui.small_button("reset").clicked()
                                        {
                                            self.keymap.reset(action.id);
                                        }
                                        if let Some(binding) = self.keymap.binding(action.id) {
                                            let conflicts =
                                                self.keymap.conflicts(action.id, binding);
                                            if !conflicts.is_empty() {
                                                ui.colored_label(
                                                    egui::Color32::from_rgb(240, 160, 60),
                                                    format!("also: {}", conflicts.join(", ")),
                                                );
                                            }
                                        }
                                    });
                                    ui.end_row();
                                }
                            });
                    });
            });
        self.keys_window_open = open;
    }
}
