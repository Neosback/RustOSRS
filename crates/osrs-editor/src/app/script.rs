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
paint X Y              apply the brush at a tile (waits until it is loaded)
spline X,Y X,Y ...     set the path's control points (tile coordinates)
lay                    lay the path
select object X Y      the nearest named object within 4 tiles (waits for it)
select tiles X0 Y0 X1 Y1
menu                   open the context menu on the selected object
move DX DY             move the selected objects
count                  log how many objects the camera's region has
hud tile|stats|minimap|outline on|off|toggle
window settings|plugins|script|map|teleport|none   open a window (or close all)
grid                   toggle the tile grid
camera fly|orbit
focus                  move the camera to the selection
undo | redo | delete | turn | copy
wait N                 pause N frames; at N waits until frame N
status TEXT
# a comment";

/// Lines waiting to run, and when the script may continue.
#[derive(Default)]
pub struct ScriptRunner {
    queue: VecDeque<String>,
    wait_until: u64,
    /// Frames the current line has been waiting for what it needs (a loading region ...).
    retries: u32,
    /// What the last runs said (errors are one line each, with the line text).
    pub log: Vec<String>,
}

impl ScriptRunner {
    pub fn is_running(&self) -> bool {
        !self.queue.is_empty()
    }
}

/// What one command did: finished, or needs another frame (something is still loading).
enum Step {
    Done,
    Retry,
}

/// Frames a command waits for what it needs before giving up.
const RETRY_LIMIT: u32 = 900;

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
            match self.script_command(&line, ctx) {
                Ok(Step::Done) => self.script.retries = 0,
                Ok(Step::Retry) => {
                    self.script.retries += 1;
                    if self.script.retries > RETRY_LIMIT {
                        self.script.retries = 0;
                        let message = format!("{line}: still waiting after {RETRY_LIMIT} frames");
                        self.script.log.push(message.clone());
                        self.status = Some(format!("script: {message}"));
                    } else {
                        self.script.queue.push_front(line);
                        return;
                    }
                }
                Err(error) => {
                    self.script.retries = 0;
                    self.script.log.push(format!("{line}: {error}"));
                    self.status = Some(format!("script: {line}: {error}"));
                }
            }
        }
    }

    fn script_command(&mut self, line: &str, ctx: &egui::Context) -> Result<Step, String> {
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
            "plane" => self.interaction.view_plane = number::<u8>(words.next(), "plane")?.min(3),
            "teleport" => {
                let x = number(words.next(), "x")?;
                let y = number(words.next(), "y")?;
                let plane = words.next().map_or(Ok(self.interaction.view_plane), |p| {
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
                if crate::pick::tile_values(&self.infos, (x, y), self.interaction.view_plane)
                    .is_none()
                {
                    return Ok(Step::Retry);
                }
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
            "focus" => self.focus_selection(),
            "window" => {
                let which = words.next();
                self.settings_open = which == Some("settings");
                self.plugins_open = which == Some("plugins");
                self.script_open = which == Some("script");
                self.world_map.open = which == Some("map");
                self.teleport.open = which == Some("teleport");
                if !matches!(
                    which,
                    Some("settings" | "plugins" | "script" | "map" | "teleport" | "none")
                ) {
                    return Err(format!("unknown window {which:?}"));
                }
            }
            "camera" => {
                self.interaction.camera.mode = match words.next() {
                    Some("fly") => crate::camera::CameraMode::Fly,
                    Some("orbit") => crate::camera::CameraMode::Orbit,
                    other => return Err(format!("unknown camera {other:?}")),
                };
            }
            "spline" => {
                let mut points = Vec::new();
                for pair in words {
                    let (x, y) = pair.split_once(',').ok_or("points are X,Y")?;
                    points.push((
                        number::<f32>(Some(x), "x")? + 0.5,
                        number::<f32>(Some(y), "y")? + 0.5,
                    ));
                }
                self.interaction.spline.points = points;
            }
            "lay" => {
                if self.interaction.spline.points.is_empty() {
                    return Err("no path points".to_owned());
                }
                self.commit_spline();
            }
            "select" => match words.next() {
                Some("object") => {
                    let x: i32 = number(words.next(), "x")?;
                    let y: i32 = number(words.next(), "y")?;
                    return Ok(if self.select_nearest_object((x, y)) {
                        Step::Done
                    } else {
                        Step::Retry
                    });
                }
                Some("tiles") => {
                    let x0: i32 = number(words.next(), "x0")?;
                    let y0: i32 = number(words.next(), "y0")?;
                    let x1: i32 = number(words.next(), "x1")?;
                    let y1: i32 = number(words.next(), "y1")?;
                    for x in x0.min(x1)..=x0.max(x1) {
                        for y in y0.min(y1)..=y0.max(y1) {
                            self.interaction.selection_tiles.insert((x, y));
                        }
                    }
                }
                other => return Err(format!("select object or tiles, not {other:?}")),
            },
            "menu" => {
                let Some(object) = self.interaction.selected.clone() else {
                    return Ok(Step::Retry);
                };
                let anchor = self
                    .infos
                    .get(&object.region)
                    .and_then(|info| info.locs.get(object.index))
                    .map(|loc| crate::pick::TilePick {
                        tile: loc.tile,
                        at: (loc.tile.0 as f32 + 0.5, loc.tile.1 as f32 + 0.5),
                    });
                self.interaction.context_pick = crate::pick::Pick {
                    object: Some(object),
                    tile: anchor,
                };
                self.context_menu = Some(egui::pos2(400.0, 300.0));
            }
            "move" => {
                let dx: i32 = number(words.next(), "dx")?;
                let dy: i32 = number(words.next(), "dy")?;
                if self.interaction.selected.is_none() {
                    return Ok(Step::Retry);
                }
                let specs = self.selected_specs();
                self.move_objects(specs, (dx, dy));
            }
            "count" => {
                let region = self.camera_region();
                let n = self.infos.get(&region).map_or(0, |info| info.locs.len());
                eprintln!("smoke: region {region:?} has {n} objects");
                self.notice = Some((format!("region {region:?}: {n} objects"), Instant::now()));
            }
            "undo" => {
                if !self.can_undo() {
                    return Ok(Step::Retry);
                }
                self.undo();
            }
            "redo" => self.redo(),
            "delete" | "turn" | "copy" if self.interaction.selected.is_none() => {
                return Ok(Step::Retry);
            }
            "delete" => self.delete_selected_objects(),
            "turn" => self.rotate_selected_objects(1),
            "copy" => self.duplicate_selected_objects(),
            "wait" => {
                self.script.wait_until = self.frames + number::<u64>(words.next(), "frames")?;
            }
            "at" => {
                self.script.wait_until = number::<u64>(words.next(), "frame")?;
            }
            "status" => {
                let text: Vec<&str> = words.collect();
                self.notice = Some((text.join(" "), Instant::now()));
            }
            other => return Err(format!("unknown command '{other}'")),
        }
        ctx.request_repaint();
        Ok(Step::Done)
    }

    /// Select the named object nearest `tile` (within 4 tiles); `false` while none is loaded yet.
    fn select_nearest_object(&mut self, tile: (i32, i32)) -> bool {
        let found = self
            .infos
            .iter()
            .flat_map(|(region, info)| {
                info.locs
                    .iter()
                    .enumerate()
                    .filter_map(move |(index, loc)| {
                        let named = loc
                            .definition
                            .name
                            .as_deref()
                            .is_some_and(|n| !n.is_empty());
                        let distance = (loc.tile.0 - tile.0).abs().max((loc.tile.1 - tile.1).abs());
                        (named && distance <= 4).then_some((
                            distance,
                            crate::pick::ObjectPick {
                                region: *region,
                                index,
                            },
                        ))
                    })
            })
            .min_by_key(|(distance, _)| *distance)
            .map(|(_, pick)| pick);
        match found {
            Some(pick) => {
                self.select_only(pick);
                true
            }
            None => false,
        }
    }

    /// The script the old `RUSTOSRS_*` smoke-test variables describe, so they keep working.
    pub(super) fn env_script() -> String {
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
    pub(super) fn script_window(&mut self, ctx: &egui::Context) {
        if !self.script_open {
            return;
        }
        let ((), close) =
            super::modal::Modal::new("script_window", format!("{} Script", crate::icons::SCRIPT))
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
