//! A small script language for the editor: one command per line, run from the Script window
//! (View menu), from a user tool's button, or at start-up (`RUSTOSRS_SCRIPT="tool paint; radius 2"`).
//!
//! Every command is something the interface already does, so a script is a recorded way of using it:
//! switch tool, set the brush, go somewhere, toggle overlays, undo. `wait N` pauses N frames (the
//! script resumes on its own), which is how smoke tests line things up with the world loading.

use super::EditorApp;
use crate::hud::Destination;
use crate::paint::{Fill, Tool};
use eframe::egui;
use std::collections::VecDeque;
use web_time::Instant;

/// The command reference shown in the Script window.
pub const HELP: &str = "\
tool select|paint|spline|height|objects
plane 0..3
teleport X Y [plane]
overlay ID|none        underlay ID|none        shape S [R]
radius N               arm overlay|underlay|shape|height|mask on|off
paint X Y              apply the brush at a tile
hud tile|stats|minimap|outline on|off|toggle
grid                   toggle the tile grid
camera fly|orbit
undo | redo | delete | turn | copy
wait N                 pause N frames
status TEXT
# a comment";

/// Lines waiting to run, and when the script may continue.
#[derive(Default)]
pub struct ScriptRunner {
    queue: VecDeque<String>,
    wait_until: u64,
    /// What the last runs said (errors are one line each, with the line text).
    pub log: Vec<String>,
}

impl ScriptRunner {
    pub fn is_running(&self) -> bool {
        !self.queue.is_empty()
    }
}

fn on_off(word: Option<&str>, current: bool) -> Result<bool, String> {
    match word {
        Some("on") => Ok(true),
        Some("off") => Ok(false),
        Some("toggle") | None => Ok(!current),
        Some(other) => Err(format!("expected on, off or toggle, not '{other}'")),
    }
}

fn number<T: std::str::FromStr>(word: Option<&str>, what: &str) -> Result<T, String> {
    word.ok_or_else(|| format!("{what} is missing"))?
        .parse()
        .map_err(|_| format!("{what} is not a number"))
}

impl EditorApp {
    /// Queue a script (`;` also separates commands) and clear the log.
    pub(super) fn run_script(&mut self, text: &str) {
        self.script.log.clear();
        for line in text.lines().flat_map(|line| line.split(';')) {
            let line = line.trim();
            if !line.is_empty() && !line.starts_with('#') {
                self.script.queue.push_back(line.to_owned());
            }
        }
    }

    /// Run queued commands until the queue is empty or a `wait` holds it (once per frame).
    pub(super) fn script_frame(&mut self, ctx: &egui::Context) {
        while self.frames >= self.script.wait_until {
            let Some(line) = self.script.queue.pop_front() else {
                return;
            };
            if let Err(error) = self.script_command(&line, ctx) {
                self.script.log.push(format!("{line}: {error}"));
                self.status = Some(format!("script: {line}: {error}"));
            }
        }
    }

    fn script_command(&mut self, line: &str, ctx: &egui::Context) -> Result<(), String> {
        let mut words = line.split_whitespace();
        let command = words.next().unwrap_or("").to_ascii_lowercase();
        match command.as_str() {
            "tool" => {
                let tool = match words.next() {
                    Some("select") => Tool::Select,
                    Some("paint") => Tool::Paint,
                    Some("spline") => Tool::Spline,
                    Some("height") => Tool::Height,
                    Some("objects") => Tool::Objects,
                    other => return Err(format!("unknown tool {other:?}")),
                };
                self.set_tool(tool);
            }
            "plane" => self.view_plane = number::<u8>(words.next(), "plane")?.min(3),
            "teleport" => {
                let x = number(words.next(), "x")?;
                let y = number(words.next(), "y")?;
                let plane = words.next().map_or(Ok(self.view_plane), |p| {
                    p.parse::<u8>()
                        .map(|p| p.min(3))
                        .map_err(|_| "plane is not a number".to_owned())
                })?;
                self.teleport_to(Destination::Tile { x, y, plane });
            }
            "overlay" | "underlay" => {
                let fill = match words.next() {
                    Some("none") => Fill::Clear,
                    other => Fill::Set(number(other, "id")?),
                };
                let channel = if command == "overlay" {
                    &mut self.prefs.brush.overlay
                } else {
                    &mut self.prefs.brush.underlay
                };
                channel.fill = fill;
                channel.armed = true;
            }
            "shape" => {
                self.prefs.brush.shape.shape =
                    number::<u8>(words.next(), "shape")?.min(crate::paint::MAX_SHAPE);
                if let Some(rotation) = words.next() {
                    self.prefs.brush.shape.rotation = rotation
                        .parse::<u8>()
                        .map_err(|_| "rotation is not a number".to_owned())?
                        & 3;
                }
                self.prefs.brush.shape.armed = true;
            }
            "radius" => self.prefs.brush.radius = number::<u8>(words.next(), "radius")?.min(16),
            "arm" => {
                let channel = words.next().ok_or("which channel?")?;
                let on = on_off(words.next(), true)?;
                let brush = &mut self.prefs.brush;
                match channel {
                    "overlay" => brush.overlay.armed = on,
                    "underlay" => brush.underlay.armed = on,
                    "shape" => brush.shape.armed = on,
                    "height" => brush.height.armed = on,
                    "mask" => brush.flags.armed = on,
                    other => return Err(format!("unknown channel {other}")),
                }
            }
            "paint" => {
                let x = number(words.next(), "x")?;
                let y = number(words.next(), "y")?;
                self.apply_brush_at((x, y));
            }
            "hud" => {
                let which = words.next().ok_or("which hud?")?;
                let prefs = &mut self.prefs;
                let slot = match which {
                    "tile" => &mut prefs.tile_info_hud,
                    "stats" => &mut prefs.stats_overlay,
                    "minimap" => &mut prefs.show_minimap,
                    "outline" => &mut prefs.hover_tile,
                    other => return Err(format!("unknown hud {other}")),
                };
                *slot = on_off(words.next(), *slot)?;
            }
            "grid" => self.plugin_action("grid.toggle"),
            "camera" => {
                self.camera.mode = match words.next() {
                    Some("fly") => crate::camera::CameraMode::Fly,
                    Some("orbit") => crate::camera::CameraMode::Orbit,
                    other => return Err(format!("unknown camera {other:?}")),
                };
            }
            "undo" => self.undo(),
            "redo" => self.redo(),
            "delete" => self.delete_selected_objects(),
            "turn" => self.rotate_selected_objects(1),
            "copy" => self.duplicate_selected_objects(),
            "wait" => {
                self.script.wait_until = self.frames + number::<u64>(words.next(), "frames")?;
            }
            "status" => {
                let text: Vec<&str> = words.collect();
                self.notice = Some((text.join(" "), Instant::now()));
            }
            other => return Err(format!("unknown command '{other}'")),
        }
        ctx.request_repaint();
        Ok(())
    }

    /// The Script window: a text box, Run, the log and the command reference.
    pub(super) fn script_window(&mut self, ctx: &egui::Context) {
        if !self.script_open {
            return;
        }
        let mut open = true;
        egui::Window::new(format!("{} Script", crate::icons::SCRIPT))
            .id(egui::Id::new("script_window"))
            .open(&mut open)
            .default_size([420.0, 380.0])
            .show(ctx, |ui| {
                ui.add(
                    egui::TextEdit::multiline(&mut self.script_text)
                        .code_editor()
                        .desired_rows(8)
                        .desired_width(f32::INFINITY)
                        .hint_text("tool paint\noverlay 12\nradius 3"),
                );
                ui.horizontal(|ui| {
                    if ui.button(format!("{}  Run", crate::icons::PLAY)).clicked() {
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
        self.script_open = open;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn on_off_words_and_numbers_parse_with_clear_errors() {
        assert_eq!(on_off(Some("on"), false), Ok(true));
        assert_eq!(on_off(Some("toggle"), true), Ok(false));
        assert_eq!(on_off(None, true), Ok(false));
        assert!(on_off(Some("maybe"), true).is_err());
        assert_eq!(number::<u8>(Some("7"), "n"), Ok(7));
        assert!(number::<u8>(Some("x"), "n").is_err());
        assert!(number::<u8>(None, "n").is_err());
    }
}
