//! Script window emits text to run; it never executes product commands while drawing.
use super::UiRuntime;
use eframe::egui;
impl UiRuntime {
    /// The Script window: a text box, Run, the log and the command reference.
    pub(in crate::app) fn script_window(
        &mut self,
        ctx: &egui::Context,
        runner: &crate::app::script::ScriptRunner,
    ) -> Option<String> {
        if !self.script_open {
            return None;
        }
        let mut run = None;
        let ((), close) = crate::app::modal::Modal::new(
            "script_window",
            format!("{} Script", crate::icons::SCRIPT),
        )
        .size(520.0, 0.0)
        .show(ctx, |ui| {
            ui.add(
                egui::TextEdit::multiline(&mut self.script_text)
                    .code_editor()
                    .desired_rows(8)
                    .desired_width(f32::INFINITY)
                    .hint_text("tool paint\noverlay 12\nradius 3"),
            );
            ui.horizontal(|ui| {
                if ui
                    .add(crate::theme::primary_button(format!(
                        "{}  Run",
                        crate::icons::PLAY
                    )))
                    .clicked()
                {
                    let text = self.script_text.clone();
                    run = Some(text);
                }
                if runner.is_running() {
                    ui.spinner();
                    ui.weak("running");
                }
            });
            for message in &runner.log {
                ui.colored_label(ui.visuals().warn_fg_color, message);
            }
            egui::CollapsingHeader::new("Commands")
                .default_open(false)
                .show(ui, |ui| {
                    ui.add(egui::Label::new(
                        egui::RichText::new(crate::app::script::HELP)
                            .monospace()
                            .size(11.5),
                    ));
                });
        });
        if close {
            self.script_open = false;
        }
        run
    }
}
