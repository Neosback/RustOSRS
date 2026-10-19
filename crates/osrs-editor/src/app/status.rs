//! Bottom status bar: interaction hints on the left, resource usage on the right.

use super::EditorApp;
use eframe::egui;

/// Window width below which the status bar drops its right-hand figures.
const NARROW_STATUS: f32 = 900.0;

const FLY_HINTS: &[(&str, &str)] = &[
    ("Click", "select"),
    ("Right drag", "look"),
    ("WASD", "fly"),
    ("Scroll", "zoom"),
    ("Right drag + scroll", "speed"),
    ("Shift", "boost"),
    ("Ctrl", "slow"),
    ("Shift + right click", "menu"),
    ("F", "focus"),
    ("T", "teleport"),
];
const ORBIT_HINTS: &[(&str, &str)] = &[
    ("Click", "select"),
    ("Right drag", "orbit"),
    ("Shift + right drag", "pan"),
    ("Scroll", "zoom"),
    ("Shift + right click", "menu"),
    ("F", "focus"),
    ("T", "teleport"),
];
const GAME_HINTS: &[(&str, &str)] = &[
    ("Click", "walk"),
    ("Right drag", "orbit"),
    ("Scroll", "zoom"),
    ("Right click", "menu"),
    ("WASD / Arrows", "turn"),
];

impl EditorApp {
    pub(super) fn status_bar(&mut self, ui: &mut egui::Ui) {
        egui::Panel::bottom("status").show(ui, |ui| {
            ui.horizontal(|ui| {
                // On narrow windows the figures on the right are dropped and a long notice is
                // cut with an ellipsis, so the two never draw over each other.
                let narrow = ui.ctx().content_rect().width() < NARROW_STATUS;
                if let Some(message) = self.ui.notice() {
                    ui.add(egui::Label::new(message).truncate());
                    ui.separator();
                    ui.ctx()
                        .request_repaint_after(std::time::Duration::from_millis(500));
                }
                ui.spacing_mut().item_spacing.x = 4.0;
                // The hints (left) and the figures (right) share one line; each gives way before they meet.
                let width = ui.ctx().content_rect().width();
                let notice_up = self.ui.notice().is_some();
                let wide = width >= if notice_up { 1900.0 } else { 1500.0 };
                for (key, action) in match self.interaction.camera().mode {
                    crate::camera::CameraMode::Fly => FLY_HINTS,
                    crate::camera::CameraMode::Orbit => ORBIT_HINTS,
                    crate::camera::CameraMode::Game => GAME_HINTS,
                } {
                    if wide || *key == "Click" {
                        crate::theme::key_hint(ui, key, action);
                    }
                }
                if let Some(status) = self.ui.status() {
                    ui.separator();
                    ui.colored_label(egui::Color32::LIGHT_RED, status);
                }
                if narrow {
                    return;
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let resident = self.world.resident_vertices();
                    let regions = self.world.resident_regions();
                    ui.weak(format!(
                        "{} region{}{}  \u{b7}  {:.1}M vertices  \u{b7}  {} zones",
                        regions,
                        if regions == 1 { "" } else { "s" },
                        if self.world.in_flight_count() == 0 {
                            String::new()
                        } else {
                            format!(" (+{} loading)", self.world.in_flight_count())
                        },
                        resident as f32 / 1.0e6,
                        self.renderer.resident_zone_count()
                    ));
                    if width >= 1300.0 {
                        ui.separator();
                        ui.weak(
                            self.diagnostics
                                .stats()
                                .memory_text(self.renderer.resident_vertex_bytes()),
                        );
                    }
                    if let Some((build, _)) = &self.project.cache_build() {
                        ui.separator();
                        ui.label(egui::RichText::new(format!("build {build}")).strong());
                    }
                });
            });
        });
    }
}
