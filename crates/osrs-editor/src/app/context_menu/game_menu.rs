//! The game-style context menu.

use super::*;

impl EditorApp {
    /// The game's menu: `Choose Option`, entries, `Cancel`.
    pub(super) fn game_menu(
        &self,
        ctx: &egui::Context,
        position: egui::Pos2,
        pick: &Pick,
    ) -> Outcome {
        let entries = interact::entries(self.world.infos(), pick);
        let mut outcome = Outcome::default();
        let brown = egui::Color32::from_rgb(93, 84, 71);
        let header = egui::Color32::from_rgb(38, 34, 28);
        let area = egui::Area::new(egui::Id::new("viewport_context_menu"))
            .order(egui::Order::Foreground)
            .fixed_pos(position)
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(brown)
                    .stroke(egui::Stroke::new(1.0, egui::Color32::BLACK))
                    .inner_margin(egui::Margin::same(2))
                    .show(ui, |ui| {
                        ui.set_width(210.0);
                        ui.spacing_mut().item_spacing.y = 0.0;
                        egui::Frame::new()
                            .fill(header)
                            .inner_margin(egui::Margin::symmetric(6, 3))
                            .show(ui, |ui| {
                                ui.set_min_width(ui.available_width());
                                ui.label(
                                    egui::RichText::new("Choose Option")
                                        .color(egui::Color32::from_rgb(220, 205, 170))
                                        .size(13.0),
                                );
                            });
                        let line = |ui: &mut egui::Ui, job: egui::text::LayoutJob| -> bool {
                            let galley = ui.painter().layout_job(job);
                            let size = egui::vec2(
                                ui.available_width().max(galley.size().x + 12.0),
                                galley.size().y + 6.0,
                            );
                            let (rect, response) =
                                ui.allocate_exact_size(size, egui::Sense::click());
                            if response.hovered() {
                                ui.painter().rect_filled(
                                    rect,
                                    0.0,
                                    egui::Color32::from_white_alpha(46),
                                );
                            }
                            ui.painter().galley(
                                rect.min + egui::vec2(6.0, 3.0),
                                galley,
                                egui::Color32::WHITE,
                            );
                            response.clicked()
                        };
                        for entry in &entries {
                            if line(ui, interact::entry_job(entry, 13.5, None)) {
                                outcome.effect = Some(entry.effect.clone());
                                outcome.close = true;
                            }
                        }
                        let cancel = interact::Entry {
                            action: "Cancel".to_owned(),
                            target: None,
                            effect: interact::Effect::Cancel,
                        };
                        if line(ui, interact::entry_job(&cancel, 13.5, None)) {
                            outcome.close = true;
                        }
                    });
            });
        outcome.close |=
            ctx.input(|i| i.pointer.any_pressed()) && !area.response.contains_pointer();
        outcome
    }
}
