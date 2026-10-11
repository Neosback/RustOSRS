//! Bottom status bar: interaction hints on the left, resource usage on the right.

use super::EditorApp;
use eframe::egui;

const FLY_HINTS: &[(&str, &str)] = &[
    ("Click", "select"),
    ("Right drag", "look"),
    ("WASD", "fly"),
    ("Scroll", "speed"),
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

impl EditorApp {
    pub(super) fn status_bar(&mut self, ui: &mut egui::Ui) {
        egui::Panel::bottom("status").show(ui, |ui| {
            ui.horizontal(|ui| {
                if let Some((message, shown)) = &self.notice
                    && shown.elapsed().as_secs_f32() < 6.0
                {
                    ui.label(message.as_str());
                    ui.separator();
                    ui.ctx()
                        .request_repaint_after(std::time::Duration::from_millis(500));
                }
                ui.spacing_mut().item_spacing.x = 4.0;
                for (key, action) in match self.camera.mode {
                    crate::camera::CameraMode::Fly => FLY_HINTS,
                    crate::camera::CameraMode::Orbit => ORBIT_HINTS,
                } {
                    crate::theme::key_hint(ui, key, action);
                }
                if let Some(status) = &self.status {
                    ui.separator();
                    ui.colored_label(egui::Color32::LIGHT_RED, status);
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let resident: usize = self
                        .loaded
                        .values()
                        .flatten()
                        .map(|stats| stats.vertices)
                        .sum();
                    let regions = self.loaded.values().flatten().count();
                    ui.weak(format!(
                        "{} region{}{}  \u{b7}  {:.1}M vertices  \u{b7}  {} zones",
                        regions,
                        if regions == 1 { "" } else { "s" },
                        if self.in_flight.is_empty() {
                            String::new()
                        } else {
                            format!(" (+{} loading)", self.in_flight.len())
                        },
                        resident as f32 / 1.0e6,
                        self.renderer.resident_zone_count()
                    ));
                    ui.separator();
                    ui.weak(
                        self.stats
                            .memory_text(self.renderer.resident_vertex_bytes()),
                    );
                });
            });
        });
    }
}
