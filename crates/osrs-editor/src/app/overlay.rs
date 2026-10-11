//! Viewport overlays: statistics readout and the plane switcher.

use super::EditorApp;
use crate::hud;
use eframe::egui;

const OVERLAY_MARGIN: f32 = 10.0;

fn overlay_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(egui::Color32::from_black_alpha(165))
        .corner_radius(6.0)
        .inner_margin(egui::Margin::symmetric(10, 7))
}

impl EditorApp {
    pub(super) fn viewport_overlays(&mut self, ui: &egui::Ui, rect: egui::Rect) {
        let ctx = ui.ctx().clone();
        self.minimap_hud(ui, rect);
        self.run_plugins(|plugin, cx| plugin.overlay(&ctx, rect, cx));

        if self.prefs.stats_overlay {
            let (region_x, region_y) = self.camera_region();
            let (tile_x, tile_y) = self.camera_tile();
            egui::Area::new(egui::Id::new("stats_overlay"))
                .order(egui::Order::Background)
                .fixed_pos(rect.min + egui::vec2(OVERLAY_MARGIN, OVERLAY_MARGIN))
                .interactable(false)
                .show(&ctx, |ui| {
                    overlay_frame().show(ui, |ui| {
                        ui.style_mut().override_text_style = Some(egui::TextStyle::Small);
                        ui.style_mut().override_font_id =
                            Some(egui::FontId::monospace(12.0));
                        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
                        ui.spacing_mut().item_spacing.y = 2.0;
                        ui.colored_label(egui::Color32::WHITE, self.stats.fps_text());
                        ui.colored_label(egui::Color32::WHITE, self.stats.timing_text());
                        ui.colored_label(egui::Color32::WHITE, self.stats.phases_text());
                        ui.colored_label(
                            egui::Color32::WHITE,
                            format!(
                                "tile {tile_x}, {tile_y}, {}   region {region_x}, {region_y} (id {})",
                                self.view_plane,
                                hud::region_id(region_x, region_y)
                            ),
                        );
                    });
                });
        }
    }
}
