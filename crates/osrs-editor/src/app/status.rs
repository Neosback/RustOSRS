//! Bottom status bar: interaction hints on the left, resource usage on the right.

use super::EditorApp;
use eframe::egui;

const FLY_HINT: &str = "click select   right drag look   WASD fly   scroll speed   shift+right click menu   F focus   T teleport";
const ORBIT_HINT: &str = "click select   right drag orbit   shift+right drag pan   scroll zoom   shift+right click menu   F focus   T teleport";

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
                ui.weak(match self.camera.mode {
                    crate::camera::CameraMode::Fly => FLY_HINT,
                    crate::camera::CameraMode::Orbit => ORBIT_HINT,
                });
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
