//! Applying the brush to the map: strokes, undo and redo, and rebuilding what changed.
//!
//! A *stroke* is everything painted between pressing and releasing the left button (or one
//! context-menu "paint here"). It records, per tile, the value before and after; releasing pushes it
//! as one undo step. Edits live in the world worker's edit layer (the cache is never written); after
//! an edit the regions whose geometry can change are rebuilt and replace the old ones when ready.

use super::EditorApp;
use crate::paint::{Brush, TileValues, Tool};
use eframe::egui;
use osrs_core::coords::RegionCoord;
use osrs_world::TileEdit;
use std::collections::HashMap;

/// Tiles around an edit whose derived geometry can change: the underlay blend reaches 5 tiles
/// (`BLEND_RADIUS`) and lighting one more.
const REBUILD_REACH: i32 = 6;
/// Most undo steps kept.
const HISTORY_LIMIT: usize = 200;

/// `(plane, world x, world y)`.
pub type TileKey = (u8, i32, i32);

/// Before and after values of every tile one stroke changed.
#[derive(Debug, Clone, Default)]
pub struct EditStep {
    pub changes: Vec<(TileKey, TileValues, TileValues)>,
}

/// A stroke being painted.
#[derive(Debug, Default)]
pub struct Stroke {
    step: EditStep,
    index: HashMap<TileKey, usize>,
    last_tile: Option<(i32, i32)>,
    warned_height: bool,
}

fn edit_for(key: TileKey, values: TileValues) -> TileEdit {
    TileEdit {
        plane: key.0,
        tile: (key.1, key.2),
        underlay: values.underlay,
        overlay: values.overlay,
        shape: values.shape,
        rotation: values.rotation,
        flags: values.flags,
    }
}

/// Whether two tiles differ in a channel the edit layer stores (heights are not edited yet).
fn differs(a: TileValues, b: TileValues) -> bool {
    (a.underlay, a.overlay, a.shape, a.rotation, a.flags)
        != (b.underlay, b.overlay, b.shape, b.rotation, b.flags)
}

/// The regions whose geometry can change when `tile` does.
pub fn affected_regions(tile: (i32, i32)) -> Vec<(i32, i32)> {
    let low = (
        (tile.0 - REBUILD_REACH).div_euclid(64),
        (tile.1 - REBUILD_REACH).div_euclid(64),
    );
    let high = (
        (tile.0 + REBUILD_REACH).div_euclid(64),
        (tile.1 + REBUILD_REACH).div_euclid(64),
    );
    let mut regions = Vec::new();
    for x in low.0..=high.0 {
        for y in low.1..=high.1 {
            regions.push((x, y));
        }
    }
    regions
}

impl EditorApp {
    /// The tile as the map currently has it: the editor's value if it was edited, else the picker's.
    fn current_tile(&self, key: TileKey) -> Option<TileValues> {
        self.edited
            .get(&key)
            .copied()
            .or_else(|| crate::pick::tile_values(&self.infos, (key.1, key.2), key.0))
    }

    /// The brush as the active tool uses it: the Flags tool paints only flags, the Height tool only
    /// height; the Paint tool paints whatever is armed.
    fn tool_brush(&self) -> Brush {
        let mut brush = self.prefs.brush.clone();
        match self.prefs.tool {
            Tool::Flags => {
                brush.overlay.armed = false;
                brush.underlay.armed = false;
                brush.shape.armed = false;
                brush.height.armed = false;
            }
            Tool::Height => {
                brush.overlay.armed = false;
                brush.underlay.armed = false;
                brush.shape.armed = false;
                brush.flags.armed = false;
            }
            Tool::Paint | Tool::Select => {}
        }
        brush
    }

    /// Start a stroke (left button pressed with a brush tool).
    pub(super) fn begin_stroke(&mut self) {
        if self.stroke.is_none() {
            self.stroke = Some(Stroke::default());
        }
    }

    /// Paint the brush footprint around `tile` into the open stroke.
    pub(super) fn paint_at(&mut self, tile: (i32, i32)) {
        let Some(mut stroke) = self.stroke.take() else {
            return;
        };
        if stroke.last_tile == Some(tile) {
            self.stroke = Some(stroke);
            return;
        }
        stroke.last_tile = Some(tile);
        let brush = self.tool_brush();
        if brush.height.armed && !stroke.warned_height {
            stroke.warned_height = true;
            self.notice = Some((
                "Height is not applied yet; the other armed channels are".to_owned(),
                std::time::Instant::now(),
            ));
        }
        let plane = self.view_plane;
        let mut edits = Vec::new();
        for (x, y) in brush.footprint_tiles(tile) {
            let key = (plane, x, y);
            if let Some(&position) = stroke.index.get(&key) {
                // Already painted in this stroke: Set modes are idempotent, so leave it.
                let _ = position;
                continue;
            }
            let Some(before) = self.current_tile(key) else {
                continue;
            };
            let after = brush.apply(before);
            if !differs(before, after) {
                continue;
            }
            stroke.index.insert(key, stroke.step.changes.len());
            stroke.step.changes.push((key, before, after));
            self.edited.insert(key, after);
            edits.push(edit_for(key, after));
        }
        self.send_edits(edits);
        self.stroke = Some(stroke);
    }

    /// Finish the open stroke: one undo step, if it changed anything.
    pub(super) fn end_stroke(&mut self) {
        let Some(stroke) = self.stroke.take() else {
            return;
        };
        if stroke.step.changes.is_empty() {
            return;
        }
        let count = stroke.step.changes.len();
        self.history.push(stroke.step);
        if self.history.len() > HISTORY_LIMIT {
            self.history.remove(0);
        }
        self.redo_stack.clear();
        self.notice = Some((
            format!("Painted {count} tile{}", if count == 1 { "" } else { "s" }),
            std::time::Instant::now(),
        ));
    }

    /// One click without a drag (the context menu's "paint here").
    pub(super) fn apply_brush_at(&mut self, tile: (i32, i32)) {
        if self.tool_brush().is_empty() {
            self.notice = Some((
                "Nothing armed: tick a channel in the drawer".to_owned(),
                std::time::Instant::now(),
            ));
            return;
        }
        self.begin_stroke();
        self.paint_at(tile);
        self.end_stroke();
    }

    fn send_edits(&mut self, edits: Vec<TileEdit>) {
        if edits.is_empty() {
            return;
        }
        for edit in &edits {
            for region in affected_regions(edit.tile) {
                if self.loaded.contains_key(&region) {
                    self.dirty_regions.insert(region);
                }
            }
        }
        self.streamer.edit(edits);
    }

    /// Request a rebuild of every dirty region that is not already being built; the old geometry
    /// stays on screen until the new one arrives.
    pub(super) fn flush_dirty_regions(&mut self) {
        let ready: Vec<(i32, i32)> = self
            .dirty_regions
            .iter()
            .copied()
            .filter(|key| !self.in_flight.contains(key))
            .collect();
        for key in ready {
            self.dirty_regions.remove(&key);
            self.in_flight.insert(key);
            self.streamer.request(RegionCoord::new(key.0, key.1));
        }
    }

    pub(super) fn can_undo(&self) -> bool {
        !self.history.is_empty()
    }

    pub(super) fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    fn revert(&mut self, step: &EditStep, forward: bool) {
        let mut edits = Vec::new();
        for (key, before, after) in &step.changes {
            let values = if forward { *after } else { *before };
            self.edited.insert(*key, values);
            edits.push(edit_for(*key, values));
        }
        self.send_edits(edits);
    }

    pub(super) fn undo(&mut self) {
        if let Some(step) = self.history.pop() {
            self.revert(&step, false);
            self.notice = Some((
                format!("Undid {} tile change(s)", step.changes.len()),
                std::time::Instant::now(),
            ));
            self.redo_stack.push(step);
        }
    }

    pub(super) fn redo(&mut self) {
        if let Some(step) = self.redo_stack.pop() {
            self.revert(&step, true);
            self.history.push(step);
        }
    }

    /// Ctrl/Cmd+Z and Ctrl/Cmd+Shift+Z.
    pub(super) fn edit_actions(&mut self, ctx: &egui::Context) {
        let undo = ctx.input(|i| self.keymap.pressed(i, "edit.undo"));
        let redo = ctx.input(|i| self.keymap.pressed(i, "edit.redo"));
        if undo {
            self.undo();
        }
        if redo {
            self.redo();
        }
    }

    /// Paint with the left button while a brush tool is active (called once per frame with the
    /// viewport's response, after the cursor's tile has been picked).
    pub(super) fn brush_input(&mut self, ctx: &egui::Context, response: &egui::Response) {
        if self.prefs.tool == Tool::Select
            || self.quick_open.is_some()
            || self.context_menu.is_some()
        {
            self.end_stroke();
            return;
        }
        let down = ctx.input(|i| i.pointer.primary_down());
        let alt = ctx.input(|i| i.modifiers.alt);
        if !down {
            self.end_stroke();
            return;
        }
        if !response.is_pointer_button_down_on() {
            return;
        }
        let Some(tile) = self.hover.tile.map(|pick| pick.tile) else {
            return;
        };
        if alt {
            // Alt + click samples instead of painting.
            if self.stroke.is_none()
                && let Some(values) = crate::pick::tile_values(&self.infos, tile, self.view_plane)
            {
                let key = (self.view_plane, tile.0, tile.1);
                self.eyedrop(self.edited.get(&key).copied().unwrap_or(values));
            }
            return;
        }
        self.begin_stroke();
        self.paint_at(tile);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_interior_tile_rebuilds_only_its_own_region() {
        assert_eq!(affected_regions((3222, 3218)), vec![(50, 50)]);
    }

    #[test]
    fn a_tile_near_a_region_corner_rebuilds_the_neighbours_too() {
        let regions = affected_regions((3200, 3200));
        assert_eq!(regions.len(), 4);
        assert!(regions.contains(&(49, 49)) && regions.contains(&(50, 50)));
        assert_eq!(affected_regions((3263, 3230)).len(), 2);
    }

    #[test]
    fn height_only_changes_are_not_a_stored_difference() {
        let a = TileValues::default();
        let b = TileValues { height: 40, ..a };
        assert!(!differs(a, b));
        assert!(differs(a, TileValues { overlay: 3, ..a }));
    }
}
