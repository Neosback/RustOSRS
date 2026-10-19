//! Viewport overlays: statistics readout and the plane switcher.

use super::EditorApp;
use crate::hud;
use eframe::egui;

use crate::style::ColorKey;

const OVERLAY_MARGIN: f32 = 10.0;

fn overlay_frame(fill: egui::Color32) -> egui::Frame {
    egui::Frame::new()
        .fill(fill)
        .corner_radius(6.0)
        .inner_margin(egui::Margin::symmetric(10, 7))
}

impl EditorApp {
    pub(super) fn viewport_overlays(&mut self, ui: &egui::Ui, rect: egui::Rect) {
        let ctx = ui.ctx().clone();
        let phase = web_time::Instant::now();
        self.minimap_hud(ui, rect);
        super::gameframe::draw(self, ui, rect);
        self.diagnostics.phase("minimap", phase);
        self.run_plugins(|plugin, cx| plugin.overlay(&ctx, rect, cx));

        let tile_lines = if self.settings.prefs.tile_info_hud {
            self.tile_info_lines()
        } else {
            None
        };
        let (background, text, label) = (
            self.settings.prefs.colors.get(ColorKey::HudBackground),
            self.settings.prefs.colors.get(ColorKey::HudText),
            self.settings.prefs.colors.get(ColorKey::HudLabel),
        );
        // The HUDs sit in the viewport's corner, to the right of the picker rail when the rail is
        // parked there (they are drawn beneath it, never over it).
        let left = match self.ui.rail_rect {
            Some(rail) if rail.left() < rect.min.x + 160.0 => rail.right() + OVERLAY_MARGIN,
            _ => rect.min.x + OVERLAY_MARGIN,
        };
        let mut hide_tile_hud = false;
        self.ui.hud_rect = None;
        if self.settings.prefs.stats_overlay || tile_lines.is_some() {
            let (region_x, region_y) = self.camera_region();
            let (tile_x, tile_y) = self.camera_tile();
            let stats = self.settings.prefs.stats_overlay;
            let hud = egui::Area::new(egui::Id::new("stats_overlay"))
                .order(egui::Order::Background)
                .fixed_pos(egui::pos2(left, rect.min.y + OVERLAY_MARGIN))
                .show(&ctx, |ui| {
                    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
                    ui.spacing_mut().item_spacing.y = 6.0;
                    if stats {
                        overlay_frame(background).show(ui, |ui| {
                            ui.style_mut().override_text_style = Some(egui::TextStyle::Small);
                            ui.style_mut().override_font_id = Some(egui::FontId::monospace(12.0));
                            ui.spacing_mut().item_spacing.y = 2.0;
                            ui.colored_label(text, self.diagnostics.stats().fps_text());
                            // A narrow viewport keeps only the frame rate and the camera tile.
                            if rect.width() >= 760.0 {
                                ui.colored_label(text, self.diagnostics.stats().timing_text());
                                ui.colored_label(text, self.diagnostics.stats().phases_text());
                            }
                            ui.colored_label(
                                text,
                                format!(
                                    "camera {tile_x}, {tile_y}, {}   region {region_x}, {region_y} (id {})",
                                    self.interaction.view_plane(),
                                    hud::region_id(region_x, region_y)
                                ),
                            );
                        });
                    }
                    if let Some(lines) = &tile_lines {
                        overlay_frame(background).show(ui, |ui| {
                            ui.horizontal_top(|ui| {
                                egui::Grid::new("tile_hud")
                                    .num_columns(2)
                                    .spacing([10.0, 2.0])
                                    .show(ui, |ui| {
                                        for (name, value) in lines {
                                            ui.label(
                                                egui::RichText::new(name).size(12.0).color(label),
                                            );
                                            ui.label(
                                                egui::RichText::new(value)
                                                    .size(12.0)
                                                    .monospace()
                                                    .color(text),
                                            );
                                            ui.end_row();
                                        }
                                    });
                                if ui
                                    .add(
                                        egui::Button::new(
                                            egui::RichText::new(crate::icons::HIDE).color(label),
                                        )
                                        .small()
                                        .frame(false),
                                    )
                                    .on_hover_text("Hide the tile info (the eye menu in the toolbar brings it back)")
                                    .clicked()
                                {
                                    hide_tile_hud = true;
                                }
                            });
                        });
                    }
                });
            self.ui.hud_rect = Some(hud.response.rect);
            if hide_tile_hud {
                self.settings.prefs.tile_info_hud = false;
            }
        }
    }
}
