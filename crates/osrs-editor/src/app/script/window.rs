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
}
