//! The Script window and the start-up script from the environment.

use super::*;

impl EditorApp {
    /// The script the old `RUSTOSRS_*` smoke-test variables describe, so they keep working.
    pub(in crate::app) fn env_script() -> String {
        let var = |name: &str| std::env::var(name).ok();
        let mut lines: Vec<String> = Vec::new();
        if let Some(value) = var("RUSTOSRS_TELEPORT") {
            lines.push(format!("teleport {}", value.replace(',', " ")));
        }
        if let Some(value) = var("RUSTOSRS_SPLINE") {
            lines.push(format!("spline {}", value.replace(';', " ")));
        }
        if let Some(value) = var("RUSTOSRS_PAINT") {
            lines.push(format!("paint {}", value.replace(',', " ")));
        }
        if let Some(value) = var("RUSTOSRS_SELECT") {
            lines.push(format!("select object {}", value.replace(',', " ")));
            if std::env::var_os("RUSTOSRS_CONTEXT").is_some() {
                lines.push("menu".to_owned());
            }
        }
        if let Some(value) = var("RUSTOSRS_SELECT_TILES") {
            lines.push(format!("select tiles {}", value.replace(',', " ")));
        }
        if let Some(frame) = var("RUSTOSRS_LAY_AT") {
            lines.push(format!("at {frame}"));
            lines.push("lay".to_owned());
        }
        if let Some(value) = var("RUSTOSRS_OBJECT_OP")
            && let Some((frame, op)) = value.split_once(':')
        {
            lines.push(format!("at {}", frame.trim()));
            lines.push(op.trim().to_owned());
            lines.push("wait 80".to_owned());
            lines.push("count".to_owned());
        }
        if let Some(frame) = var("RUSTOSRS_UNDO_AT") {
            lines.push(format!("at {frame}"));
            lines.push("undo".to_owned());
        }
        if let Some(text) = var("RUSTOSRS_SCRIPT") {
            lines.push(text);
        }
        lines.join("\n")
    }

    /// The Script window: a text box, Run, the log and the command reference.
    pub(in crate::app) fn script_window(&mut self, ctx: &egui::Context) {
        if !self.script_open {
            return;
        }
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
                    self.run_script(&text);
                }
                if self.script.is_running() {
                    ui.spinner();
                    ui.weak("running");
                }
            });
            for message in &self.script.log {
                ui.colored_label(ui.visuals().warn_fg_color, message);
            }
            egui::CollapsingHeader::new("Commands")
                .default_open(false)
                .show(ui, |ui| {
                    ui.add(egui::Label::new(
                        egui::RichText::new(HELP).monospace().size(11.5),
                    ));
                });
        });
        if close {
            self.script_open = false;
        }
    }
}
