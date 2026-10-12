//! Changing the map: strokes, undo and redo, and rebuilding what changed.
//!
//! A *stroke* is everything painted between pressing and releasing the left button (or one menu action,
//! or one laid path). It records, per tile, the value before and after, and is pushed as a single undo
//! step. Edits live in the world worker's edit layer (the cache is never written); after an edit the
//! regions whose geometry can change are rebuilt and replace the old ones when ready.

use super::EditorApp;
use crate::heights::{self, encode_height};
use crate::paint::{Brush, Fill, TileValues, Tool};
use eframe::egui;
use osrs_core::coords::RegionCoord;
use osrs_world::{LocEdit, LocSpec, TileEdit};
use std::collections::HashMap;
use std::time::Duration;
use web_time::Instant;

/// Tiles around an edit whose derived geometry can change: the underlay blend reaches 5 tiles
/// (`BLEND_RADIUS`) and lighting one more.
const REBUILD_REACH: i32 = 6;
/// Most undo steps kept.
const HISTORY_LIMIT: usize = 200;
/// How often a held sculpting brush applies again without moving.
const SCULPT_REPEAT: Duration = Duration::from_millis(110);

/// One tile of a path to lay: the overlay (when the path sets one) with its shape and rotation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PathTile {
    pub tile: (i32, i32),
    pub overlay: Option<u16>,
    pub shape: u8,
    pub rotation: u8,
}

/// `(plane, world x, world y)`.
pub type TileKey = (u8, i32, i32);

/// Before and after values of every tile one stroke changed.
#[derive(Debug, Clone, Default)]
pub struct EditStep {
    pub changes: Vec<(TileKey, TileValues, TileValues)>,
    /// Objects the step removed and added (one undo step covers a move: remove, add).
    pub removed: Vec<LocSpec>,
    pub added: Vec<LocSpec>,
}

/// A stroke being painted.
#[derive(Debug)]
pub struct Stroke {
    step: EditStep,
    index: HashMap<TileKey, usize>,
    last_tile: Option<(i32, i32)>,
    last_applied: Instant,
    /// The height under the cursor when the stroke began (what Flatten levels to).
    anchor: Option<i32>,
    seed: u64,
}

impl Default for Stroke {
    fn default() -> Self {
        Self {
            step: EditStep::default(),
            index: HashMap::new(),
            last_tile: None,
            last_applied: Instant::now(),
            anchor: None,
            seed: 0,
        }
    }
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
    /// The tile on the viewed plane as the map has it now: the editor's value if edited, else the picker's.
    pub(super) fn tile_now(&self, tile: (i32, i32)) -> Option<TileValues> {
        self.current_tile((self.view_plane, tile.0, tile.1))
    }

    pub(super) fn current_tile_values(&self, key: TileKey) -> Option<TileValues> {
        self.current_tile(key)
    }

    fn current_tile(&self, key: TileKey) -> Option<TileValues> {
        self.edited
            .get(&key)
            .copied()
            .or_else(|| crate::pick::tile_values(&self.infos, (key.1, key.2), key.0))
    }

    /// The height heights are stored relative to: nothing on plane 0, the plane below above it.
    fn base_height(&self, key: TileKey) -> i32 {
        if key.0 == 0 {
            0
        } else {
            self.current_tile((key.0 - 1, key.1, key.2))
                .map_or(0, |below| below.height)
        }
    }

    /// The nearest height the map file can store at this tile.
    fn quantize_height(&self, key: TileKey, desired: i32) -> i32 {
        let base = self.base_height(key);
        heights::decode_height(base, encode_height(base, desired))
    }

    fn edit_for(&self, key: TileKey, values: TileValues) -> TileEdit {
        let height = self
            .height_edited
            .contains(&key)
            .then(|| encode_height(self.base_height(key), values.height));
        TileEdit {
            plane: key.0,
            tile: (key.1, key.2),
            underlay: values.underlay,
            overlay: values.overlay,
            shape: values.shape,
            rotation: values.rotation,
            flags: values.flags,
            height,
        }
    }

    // ------------------------------------------------------------------ strokes

    pub(super) fn begin_stroke(&mut self) {
        if self.stroke.is_none() {
            self.stroke_counter += 1;
            self.stroke = Some(Stroke {
                seed: self.stroke_counter.wrapping_mul(0x9E37_79B9_7F4A_7C15),
                ..Stroke::default()
            });
        }
    }

    /// Record `after` for a tile in the open stroke and the edit layer. Returns `false` when nothing
    /// changed.
    fn record(
        &mut self,
        stroke: &mut Stroke,
        key: TileKey,
        mut after: TileValues,
        edits: &mut Vec<TileEdit>,
    ) -> bool {
        let Some(current) = self.current_tile(key) else {
            return false;
        };
        if after.height != current.height {
            after.height = self.quantize_height(key, after.height);
            self.height_edited.insert(key);
        }
        if after == current {
            return false;
        }
        match stroke.index.get(&key) {
            Some(&position) => stroke.step.changes[position].2 = after,
            None => {
                stroke.index.insert(key, stroke.step.changes.len());
                stroke.step.changes.push((key, current, after));
            }
        }
        self.edited.insert(key, after);
        edits.push(self.edit_for(key, after));
        true
    }

    /// Finish the open stroke: one undo step, if it changed anything (tiles or objects).
    pub(super) fn end_stroke(&mut self) {
        let Some(stroke) = self.stroke.take() else {
            return;
        };
        if stroke.step.changes.is_empty()
            && stroke.step.added.is_empty()
            && stroke.step.removed.is_empty()
        {
            return;
        }
        let count = stroke.step.changes.len();
        self.push_step(stroke.step);
        self.notice = Some((
            format!("Changed {count} tile{}", if count == 1 { "" } else { "s" }),
            Instant::now(),
        ));
    }

    /// One finished undo step (a stroke, or an object action).
    pub(super) fn push_step(&mut self, step: EditStep) {
        self.history.push(step);
        if self.history.len() > HISTORY_LIMIT {
            self.history.remove(0);
        }
        self.redo_stack.clear();
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
        let brush: Brush = self.prefs.brush.clone();
        let plane = self.view_plane;
        let mut edits = Vec::new();
        for (x, y) in brush.footprint_tiles(tile) {
            let key = (plane, x, y);
            if stroke.index.contains_key(&key) {
                continue;
            }
            let Some(before) = self.current_tile(key) else {
                continue;
            };
            let after = brush.apply(before);
            self.record(&mut stroke, key, after, &mut edits);
        }
        self.send_edits(edits);
        self.stroke = Some(stroke);
    }

    /// One application of the Height tool around `tile`.
    pub(super) fn sculpt_at(&mut self, tile: (i32, i32)) {
        let Some(mut stroke) = self.stroke.take() else {
            return;
        };
        let moved = stroke.last_tile != Some(tile);
        if !moved && stroke.last_applied.elapsed() < SCULPT_REPEAT {
            self.stroke = Some(stroke);
            return;
        }
        stroke.last_tile = Some(tile);
        stroke.last_applied = Instant::now();
        let plane = self.view_plane;
        if stroke.anchor.is_none() {
            stroke.anchor = self.current_tile((plane, tile.0, tile.1)).map(|v| v.height);
        }
        let tool = self.prefs.height_tool.clone();
        let (radius, shape) = (self.prefs.brush.radius, self.prefs.brush.footprint);
        let lookup = |t: (i32, i32)| self.current_tile((plane, t.0, t.1)).map(|v| v.height);
        let changes = heights::apply(
            &tool,
            tile,
            radius,
            shape,
            stroke.anchor.unwrap_or(0),
            stroke.seed,
            &lookup,
        );
        let mut edits = Vec::new();
        for (t, height) in changes {
            let key = (plane, t.0, t.1);
            let Some(mut after) = self.current_tile(key) else {
                continue;
            };
            after.height = height;
            self.record(&mut stroke, key, after, &mut edits);
        }
        self.send_edits(edits);
        self.stroke = Some(stroke);
    }

    /// Apply the Height tool's op to every selected tile (one undo step).
    pub(super) fn height_on_selection(&mut self) {
        let tiles: Vec<(i32, i32)> = self.selection_tiles.iter().copied().collect();
        if tiles.is_empty() {
            return;
        }
        self.begin_stroke();
        let Some(mut stroke) = self.stroke.take() else {
            return;
        };
        let plane = self.view_plane;
        let mut tool = self.prefs.height_tool.clone();
        tool.falloff = crate::heights::Falloff::Flat;
        // Flatten levels the whole selection to one height: the first selected tile's.
        let anchor = tiles
            .first()
            .and_then(|t| self.current_tile((plane, t.0, t.1)))
            .map_or(0, |v| v.height);
        let mut edits = Vec::new();
        for tile in tiles {
            let lookup = |t: (i32, i32)| self.current_tile((plane, t.0, t.1)).map(|v| v.height);
            let changes = heights::apply(
                &tool,
                tile,
                0,
                crate::paint::BrushShape::Square,
                anchor,
                stroke.seed,
                &lookup,
            );
            for (t, height) in changes {
                let key = (plane, t.0, t.1);
                if let Some(mut after) = self.current_tile(key) {
                    after.height = height;
                    self.record(&mut stroke, key, after, &mut edits);
                }
            }
        }
        self.send_edits(edits);
        self.stroke = Some(stroke);
        self.end_stroke();
    }

    /// Apply the brush to every selected tile (one undo step).
    pub(super) fn fill_selection(&mut self) {
        let tiles: Vec<(i32, i32)> = self.selection_tiles.iter().copied().collect();
        if tiles.is_empty() || self.prefs.brush.is_empty() {
            return;
        }
        self.begin_stroke();
        let Some(mut stroke) = self.stroke.take() else {
            return;
        };
        let brush = self.prefs.brush.clone();
        let plane = self.view_plane;
        let mut edits = Vec::new();
        for (x, y) in tiles {
            let key = (plane, x, y);
            if let Some(before) = self.current_tile(key) {
                let after = brush.apply(before);
                self.record(&mut stroke, key, after, &mut edits);
            }
        }
        self.send_edits(edits);
        self.stroke = Some(stroke);
        self.end_stroke();
    }

    /// Lay a path: each tile's overlay with its shape and rotation, the other armed channels of the
    /// brush, and spawned `added` objects (one undo step). Returns how many tiles changed.
    pub(super) fn apply_path(&mut self, tiles: &[PathTile], added: Vec<LocSpec>) -> usize {
        let brush = self.prefs.brush.clone();
        let loc_edits: Vec<LocEdit> = added.iter().map(|s| LocEdit::Add(*s)).collect();
        if tiles.is_empty() && loc_edits.is_empty() {
            return 0;
        }
        self.begin_stroke();
        let Some(mut stroke) = self.stroke.take() else {
            return 0;
        };
        let plane = self.view_plane;
        let mut edits = Vec::new();
        for path_tile in tiles {
            let key = (plane, path_tile.tile.0, path_tile.tile.1);
            let Some(before) = self.current_tile(key) else {
                continue;
            };
            let mut after = before;
            if let Some(id) = path_tile.overlay {
                after.overlay = id;
                after.shape = path_tile.shape;
                after.rotation = path_tile.rotation;
            }
            if brush.height.armed {
                after.height = brush.height.value;
            }
            if brush.flags.armed {
                after.flags = brush.flags.bits;
            }
            self.record(&mut stroke, key, after, &mut edits);
        }
        let count = edits.len();
        if !added.is_empty() {
            stroke.step.added = added;
        }
        self.send_edits(edits);
        self.send_loc_edits(loc_edits);
        self.stroke = Some(stroke);
        self.end_stroke();
        count
    }

    /// Remove a tile's overlay (its shape and rotation with it): one undo step.
    pub(super) fn clear_overlay_at(&mut self, tile: (i32, i32)) {
        let key = (self.view_plane, tile.0, tile.1);
        let Some(before) = self.current_tile(key) else {
            return;
        };
        let mut after = before;
        after.overlay = 0;
        after.shape = 0;
        after.rotation = 0;
        self.begin_stroke();
        let Some(mut stroke) = self.stroke.take() else {
            return;
        };
        let mut edits = Vec::new();
        self.record(&mut stroke, key, after, &mut edits);
        self.send_edits(edits);
        self.stroke = Some(stroke);
        self.end_stroke();
    }

    /// One click without a drag (the context menu's "paint here" and "sculpt here").
    pub(super) fn apply_brush_at(&mut self, tile: (i32, i32)) {
        match self.prefs.tool {
            Tool::Paint => {
                if self.prefs.brush.is_empty() {
                    self.notice = Some((
                        "Nothing armed: tick a channel in the drawer".to_owned(),
                        Instant::now(),
                    ));
                    return;
                }
                self.begin_stroke();
                self.paint_at(tile);
                self.end_stroke();
            }
            Tool::Height => {
                self.begin_stroke();
                self.sculpt_at(tile);
                self.end_stroke();
            }
            Tool::Spline | Tool::Select | Tool::Objects => {}
        }
    }

    // ----------------------------------------------------------------- rebuilds

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

    // --------------------------------------------------------------- undo / redo

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
            edits.push(self.edit_for(*key, values));
        }
        self.send_edits(edits);
        if !step.removed.is_empty() || !step.added.is_empty() {
            let (gone, came) = if forward {
                (&step.removed, &step.added)
            } else {
                (&step.added, &step.removed)
            };
            let mut loc_edits: Vec<LocEdit> = gone.iter().map(|s| LocEdit::Remove(*s)).collect();
            loc_edits.extend(came.iter().map(|s| LocEdit::Add(*s)));
            self.send_loc_edits(loc_edits);
            // The objects as they stand now are what a follow-up edit acts on.
            self.pending_reselect = came.clone();
            self.clear_selection();
        }
    }

    /// Hand object edits to the world worker and queue the regions they sit in for a rebuild.
    pub(super) fn send_loc_edits(&mut self, edits: Vec<LocEdit>) {
        if edits.is_empty() {
            return;
        }
        for edit in &edits {
            let spec = match edit {
                LocEdit::Remove(spec) | LocEdit::Add(spec) => spec,
            };
            let region = spec.region();
            if self.loaded.contains_key(&region) {
                self.dirty_regions.insert(region);
            }
        }
        self.streamer.edit_locs(edits);
    }

    pub(super) fn undo(&mut self) {
        if let Some(step) = self.history.pop() {
            self.revert(&step, false);
            self.notice = Some((
                if step.changes.is_empty() {
                    "Undid an object change".to_owned()
                } else {
                    format!("Undid {} tile change(s)", step.changes.len())
                },
                Instant::now(),
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

    // -------------------------------------------------------------------- input

    /// Paint or sculpt with the left button while a brush tool is active (called once per frame with
    /// the viewport's response, after the cursor's tile has been picked).
    pub(super) fn brush_input(&mut self, ctx: &egui::Context, response: &egui::Response) {
        let tool = self.prefs.tool;
        if !tool.uses_brush() || self.quick_open.is_some() || self.context_menu.is_some() {
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
                && let Some(values) = self.tile_now(tile)
            {
                self.eyedrop(values);
            }
            return;
        }
        self.begin_stroke();
        match tool {
            Tool::Paint => self.paint_at(tile),
            Tool::Height => self.sculpt_at(tile),
            Tool::Spline | Tool::Select | Tool::Objects => {}
        }
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
}
