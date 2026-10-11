//! Bottom status bar: interaction hints on the left, resource usage on the right.

use super::EditorApp;
use eframe::egui;

const HINT: &str =
    "LMB select   drag look   WASD fly   scroll speed   1-4 plane   T teleport   N properties";

impl EditorApp {
    pub(super) fn status_bar(&mut self, ui: &mut egui::Ui) {
        egui::Panel::bottom("status").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.weak(HINT);
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
                    ui.label(format!(
                        "{} regions{}  {:.1}M vertices  {} zones",
                        self.loaded.values().flatten().count(),
                        if self.in_flight.is_empty() {
                            String::new()
                        } else {
                            format!(" (+{} loading)", self.in_flight.len())
                        },
                        resident as f32 / 1.0e6,
                        self.renderer.resident_zone_count()
                    ));
                    ui.separator();
                    ui.label(
                        self.stats
                            .memory_text(self.renderer.resident_vertex_bytes()),
                    );
                });
            });
        });
    }
}
