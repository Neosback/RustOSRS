//! Changing the map: strokes, semantic commits, and rebuilding what changed.
//!
//! A *stroke* is everything painted between pressing and releasing the left button (or one menu
//! action). Stroke construction stays here; terrain-path commit lives in `editing/path.rs`, and
//! chronological undo/redo routing lives in `editing/history.rs` so the editor shell does not
//! accumulate another god module.

mod history;
mod path;
mod plan;
#[cfg(test)]
mod tests;

mod actions;
mod commit;

use super::EditorApp;
use super::paint_commit::overlay_clear_brush;
use crate::heights::commit::{HeightStrokeSpec, build_terrain_height_patch};
use crate::heights::{self, encode_height};
use crate::paint::{Brush, TileValues, Tool};
use eframe::egui;
pub(super) use history::EditStep;
use history::{SemanticHeightHistory, SemanticPaintHistory};
use osrs_core::coords::{MapTile, SourcePlane};
use osrs_edit::HeightVertexCoord;
use osrs_world::TileEdit;
use plan::{build_paint_stroke_patch, project_height_values, project_paint_values};
use std::collections::HashMap;
use std::time::Duration;
use web_time::Instant;

/// Tiles around an edit whose derived geometry can change: the underlay blend reaches 5 tiles
/// (`BLEND_RADIUS`) and lighting one more.
const REBUILD_REACH: i32 = 6;
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

/// A stroke being painted.
#[derive(Debug)]
pub struct Stroke {
    step: EditStep,
    index: HashMap<TileKey, usize>,
    last_tile: Option<(i32, i32)>,
    last_applied: Instant,
    /// The exact Paint brush captured for this stroke. `None` keeps non-Paint strokes on the legacy
    /// commit path until their own migration checkpoints.
    paint_brush: Option<Brush>,
    /// The exact Height settings captured on the first applied sample of this stroke.
    height_spec: Option<HeightStrokeSpec>,
    /// Ordered Height brush centers. Repeated centers are intentional for held sculpting.
    height_centers: Vec<MapTile>,
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
            paint_brush: None,
            height_spec: None,
            height_centers: Vec::new(),
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

    /// Reconcile the already-visible Paint preview with the authoritative semantic result. Most
    /// strokes are already identical; this mainly canonicalizes overlay shape/rotation when an
    /// overlay is created on a semantically empty tile.
    fn project_paint_stroke(&mut self, stroke: &mut Stroke, brush: &Brush) {
        let mut edits = Vec::new();
        for (key, _, after) in &mut stroke.step.changes {
            let Some(projected) =
                project_paint_values(self.session.document(), brush, *key, *after)
            else {
                continue;
            };
            if projected != *after {
                self.edited.insert(*key, projected);
                edits.push(self.edit_for(*key, projected));
            }
            *after = projected;
        }
        self.send_edits(edits);
        stroke
            .step
            .changes
            .retain(|(_, before, after)| before != after);
    }

    /// Reconcile the already-visible Height preview with authoritative semantic heights.
    ///
    /// Values are prepared before mutation so an adapter failure cannot leave a partially projected
    /// Height stroke.
    fn project_height_stroke(&mut self, stroke: &mut Stroke) -> Result<(), &'static str> {
        let mut projected = Vec::with_capacity(stroke.step.changes.len());
        for (index, (key, _, after)) in stroke.step.changes.iter().enumerate() {
            let values = project_height_values(self.session.document(), *key, *after)
                .ok_or("a Height preview vertex is missing from semantic state")?;
            projected.push((index, *key, values));
        }

        let mut edits = Vec::new();
        for (index, key, values) in projected {
            let after = &mut stroke.step.changes[index].2;
            self.height_edited.insert(key);
            if values != *after {
                self.edited.insert(key, values);
                edits.push(self.edit_for(key, values));
            }
            *after = values;
        }
        self.send_edits(edits);
        stroke
            .step
            .changes
            .retain(|(_, before, after)| before != after);
        Ok(())
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
        let brush = stroke
            .paint_brush
            .get_or_insert_with(|| self.prefs.brush.clone())
            .clone();
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
        let source_plane = SourcePlane::new(plane)
            .unwrap_or_else(|| unreachable!("editor view planes are always validated"));
        let center = MapTile::new(tile.0, tile.1);
        let captured_spec = HeightStrokeSpec::new(
            source_plane,
            self.prefs.height_tool.clone(),
            self.prefs.brush.radius,
            self.prefs.brush.footprint,
            stroke.anchor.unwrap_or(0),
            stroke.seed,
        );
        let spec = stroke.height_spec.get_or_insert(captured_spec).clone();
        stroke.height_centers.push(center);
        let lookup = |t: (i32, i32)| self.current_tile((plane, t.0, t.1)).map(|v| v.height);
        let changes = spec.apply_sample(center, &lookup);
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

    // ----------------------------------------------------------------- rebuilds

    fn send_edits(&mut self, edits: Vec<TileEdit>) {
        if edits.is_empty() {
            return;
        }
        self.world
            .mirror_tile_edits(edits, |edit| affected_regions(edit.tile));
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
