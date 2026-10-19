//! Changing the map: strokes, semantic commits, and rebuilding what changed.
//!
//! A *stroke* is everything painted between pressing and releasing the left button (or one menu
//! action). Stroke construction stays here; terrain-path commit lives in `editing/path.rs`, and
//! semantic undo/redo projection lives in `editing/history.rs` so the editor shell does not
//! accumulate another god module.

#[cfg(test)]
mod controller_tests;
mod history;
mod paste;
mod path;
mod plan;
mod prefab_commit;
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
use osrs_core::coords::{MapTile, SourcePlane};
use osrs_edit::HeightVertexCoord;
use osrs_world::TileEdit;
pub(super) use path::commit_path_height;
use plan::{build_paint_stroke_patch, project_height_values, project_paint_values};
pub(super) use prefab_commit::{apply_baseline_patch, commit_map_patch};
use std::collections::{HashMap, HashSet};
use std::time::Duration;
use web_time::Instant;

/// Tiles around an edit whose derived geometry can change: the underlay blend reaches 5 tiles
/// (`BLEND_RADIUS`) and lighting one more.
const REBUILD_REACH: i32 = osrs_scene::terrain_appearance::APPEARANCE_REACH;
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

/// One transient before/after tile sample collected while a Paint or Height gesture is open.
type StrokeChange = (TileKey, TileValues, TileValues);

/// A stroke being painted.
///
/// This is preview/gesture state only. Completed chronological history lives in `EditorSession`.
#[derive(Debug)]
pub struct Stroke {
    changes: Vec<StrokeChange>,
    index: HashMap<TileKey, usize>,
    last_tile: Option<(i32, i32)>,
    last_applied: Instant,
    /// Paint settings latched at the gesture's first sample; Height gestures use height_spec.
    paint_brush: Option<Brush>,
    /// The exact Height settings captured on the first applied sample of this stroke.
    height_spec: Option<HeightStrokeSpec>,
    /// Ordered Height brush centers. Repeated centers are intentional for held sculpting.
    height_centers: Vec<MapTile>,
    /// The height under the cursor when the stroke began (what Flatten levels to).
    anchor: Option<i32>,
    seed: u64,
    newly_explicit_heights: HashSet<TileKey>,
}

impl Stroke {
    /// Whether the stroke changed nothing at all.
    fn is_empty(&self) -> bool {
        self.changes.is_empty()
    }
}

impl Default for Stroke {
    fn default() -> Self {
        Self {
            changes: Vec::new(),
            index: HashMap::new(),
            last_tile: None,
            last_applied: Instant::now(),
            paint_brush: None,
            height_spec: None,
            height_centers: Vec::new(),
            anchor: None,
            seed: 0,
            newly_explicit_heights: HashSet::new(),
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

/// Transitional terrain preview/projection state. Authoritative map state and chronology live only
/// in `EditorSession`; this adapter can be rebuilt from semantic state plus worker/source data.
#[derive(Default)]
pub(super) struct TerrainController {
    edited: HashMap<TileKey, TileValues>,
    height_edited: HashSet<TileKey>,
    stroke: Option<Stroke>,
    stroke_counter: u64,
}

impl TerrainController {
    pub(in crate::app) fn reset(&mut self) {
        *self = Self::default();
    }
    pub(in crate::app) fn cancel_stroke(&mut self, infos: &crate::pick::Infos) -> Vec<TileEdit> {
        let Some(stroke) = self.stroke.take() else {
            return Vec::new();
        };
        for key in &stroke.newly_explicit_heights {
            self.height_edited.remove(key);
        }
        for (key, before, _) in &stroke.changes {
            self.edited.insert(*key, *before);
        }
        stroke
            .changes
            .into_iter()
            .map(|(key, before, _)| self.edit_for(infos, key, before))
            .collect()
    }
    fn current_tile(&self, infos: &crate::pick::Infos, key: TileKey) -> Option<TileValues> {
        self.edited
            .get(&key)
            .copied()
            .or_else(|| crate::pick::tile_values(infos, (key.1, key.2), key.0))
    }

    /// The height heights are stored relative to: nothing on plane 0, the plane below above it.
    fn base_height(&self, infos: &crate::pick::Infos, key: TileKey) -> i32 {
        if key.0 == 0 {
            0
        } else {
            self.current_tile(infos, (key.0 - 1, key.1, key.2))
                .map_or(0, |below| below.height)
        }
    }

    /// The nearest height the map file can store at this tile.
    fn quantize_height(&self, infos: &crate::pick::Infos, key: TileKey, desired: i32) -> i32 {
        let base = self.base_height(infos, key);
        heights::decode_height(base, encode_height(base, desired))
    }

    fn edit_for(&self, _infos: &crate::pick::Infos, key: TileKey, values: TileValues) -> TileEdit {
        let height = self.height_edited.contains(&key).then_some(values.height);
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
        infos: &crate::pick::Infos,
        stroke: &mut Stroke,
        key: TileKey,
        mut after: TileValues,
        edits: &mut Vec<TileEdit>,
    ) -> bool {
        let Some(current) = self.current_tile(infos, key) else {
            return false;
        };
        if after.height != current.height {
            after.height = self.quantize_height(infos, key, after.height);
            if self.height_edited.insert(key) {
                stroke.newly_explicit_heights.insert(key);
            }
        }
        if after == current {
            return false;
        }
        match stroke.index.get(&key) {
            Some(&position) => stroke.changes[position].2 = after,
            None => {
                stroke.index.insert(key, stroke.changes.len());
                stroke.changes.push((key, current, after));
            }
        }
        self.edited.insert(key, after);
        edits.push(self.edit_for(infos, key, after));
        true
    }
}

impl EditorApp {
    /// Preview queries are for presentation only; planners read the authoritative `MapQuery`.
    pub(super) fn tile_now(&self, tile: (i32, i32)) -> Option<TileValues> {
        self.current_tile((self.interaction.view_plane(), tile.0, tile.1))
    }
    fn current_tile(&self, key: TileKey) -> Option<TileValues> {
        self.terrain.current_tile(self.world.infos(), key)
    }
    fn quantize_height(&self, key: TileKey, desired: i32) -> i32 {
        self.terrain
            .quantize_height(self.world.infos(), key, desired)
    }
    fn edit_for(&self, key: TileKey, values: TileValues) -> TileEdit {
        self.terrain.edit_for(self.world.infos(), key, values)
    }
    pub(in crate::app) fn cancel_stroke(&mut self) {
        let edits = self.terrain.cancel_stroke(self.world.infos());
        self.send_edits(edits);
    }

    /// Restore transient stroke tile values after a commit attempt fails.
    ///
    /// This restores visible tile values only. User cancellation remains the path that also clears
    /// newly-explicit Height preview markers.
    fn revert_stroke_preview(&mut self, stroke: &Stroke) {
        let mut edits = Vec::with_capacity(stroke.changes.len());
        for (key, before, _) in &stroke.changes {
            self.terrain.edited.insert(*key, *before);
            edits.push(self.edit_for(*key, *before));
        }
        self.send_edits(edits);
    }

    /// Reconcile the already-visible Paint preview with the authoritative semantic result. Most
    /// strokes are already identical; this mainly canonicalizes overlay shape/rotation when an
    /// overlay is created on a semantically empty tile.
    fn project_paint_stroke(&mut self, stroke: &mut Stroke, brush: &Brush) {
        let mut edits = Vec::new();
        for (key, _, after) in &mut stroke.changes {
            let Some(projected) =
                project_paint_values(self.session.document(), brush, *key, *after)
            else {
                continue;
            };
            if projected != *after {
                self.terrain.edited.insert(*key, projected);
                edits.push(self.edit_for(*key, projected));
            }
            *after = projected;
        }
        self.send_edits(edits);
        stroke.changes.retain(|(_, before, after)| before != after);
    }

    /// Reconcile the already-visible Height preview with authoritative semantic heights.
    ///
    /// Values are prepared before mutation so an adapter failure cannot leave a partially projected
    /// Height stroke.
    fn project_height_stroke(&mut self, stroke: &mut Stroke) -> Result<(), &'static str> {
        let mut projected = Vec::with_capacity(stroke.changes.len());
        for (index, (key, _, after)) in stroke.changes.iter().enumerate() {
            let values = project_height_values(self.session.document(), *key, *after)
                .ok_or("a Height preview vertex is missing from semantic state")?;
            projected.push((index, *key, values));
        }

        let mut edits = Vec::new();
        for (index, key, values) in projected {
            let after = &mut stroke.changes[index].2;
            self.terrain.height_edited.insert(key);
            if values != *after {
                self.terrain.edited.insert(key, values);
                edits.push(self.edit_for(key, values));
            }
            *after = values;
        }
        self.send_edits(edits);
        stroke.changes.retain(|(_, before, after)| before != after);
        Ok(())
    }

    /// Paint the brush footprint around `tile` into the open stroke.
    pub(super) fn paint_at(&mut self, tile: (i32, i32)) {
        let Some(mut stroke) = self.terrain.stroke.take() else {
            return;
        };
        if stroke.last_tile == Some(tile) {
            self.terrain.stroke = Some(stroke);
            return;
        }
        stroke.last_tile = Some(tile);
        let brush = stroke
            .paint_brush
            .get_or_insert_with(|| self.settings.prefs.brush.clone())
            .clone();
        let plane = self.interaction.view_plane();
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
            self.terrain
                .record(self.world.infos(), &mut stroke, key, after, &mut edits);
        }
        self.send_edits(edits);
        self.terrain.stroke = Some(stroke);
    }

    /// One application of the Height tool around `tile`.
    pub(super) fn sculpt_at(&mut self, tile: (i32, i32)) {
        let Some(mut stroke) = self.terrain.stroke.take() else {
            return;
        };
        let moved = stroke.last_tile != Some(tile);
        if !moved && stroke.last_applied.elapsed() < SCULPT_REPEAT {
            self.terrain.stroke = Some(stroke);
            return;
        }
        stroke.last_tile = Some(tile);
        stroke.last_applied = Instant::now();
        let plane = self.interaction.view_plane();
        if stroke.anchor.is_none() {
            stroke.anchor = self.current_tile((plane, tile.0, tile.1)).map(|v| v.height);
        }
        let source_plane = SourcePlane::new(plane)
            .unwrap_or_else(|| unreachable!("editor view planes are always validated"));
        let center = MapTile::new(tile.0, tile.1);
        let captured_spec = HeightStrokeSpec::new(
            source_plane,
            self.settings.prefs.height_tool.clone(),
            self.settings.prefs.brush.radius,
            self.settings.prefs.brush.footprint,
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
            self.terrain
                .record(self.world.infos(), &mut stroke, key, after, &mut edits);
        }
        self.send_edits(edits);
        self.terrain.stroke = Some(stroke);
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
        let tool = self.settings.prefs.tool;
        if !tool.uses_brush() || self.ui.quick_open.is_some() || self.ui.context_menu.is_some() {
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
        let Some(tile) = self.interaction.hover().tile.map(|pick| pick.tile) else {
            return;
        };
        if alt {
            // Alt + click samples instead of painting.
            if self.terrain.stroke.is_none()
                && let Some(values) = self.tile_now(tile)
            {
                self.eyedrop(values);
            }
            return;
        }
        self.terrain.begin_stroke();
        match tool {
            Tool::Paint => self.paint_at(tile),
            Tool::Height => self.sculpt_at(tile),
            Tool::Spline | Tool::Select | Tool::Objects => {}
        }
    }
}
