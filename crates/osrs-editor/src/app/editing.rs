//! Changing the map: strokes, undo and redo, and rebuilding what changed.
//!
//! A *stroke* is everything painted between pressing and releasing the left button (or one menu action,
//! or one laid path). It records, per tile, the value before and after, and is pushed as a single undo
//! step. Edits live in the world worker's edit layer (the cache is never written); after an edit the
//! regions whose geometry can change are rebuilt and replace the old ones when ready.

use super::EditorApp;
use super::paint_commit::{PaintCommitError, TerrainPaintIntent, build_terrain_paint_patch};
use super::path_commit::{TerrainPathIntent, build_terrain_path_patch};
use crate::heights::{self, encode_height};
use crate::paint::{Brush, Fill, TileValues, Tool};
use eframe::egui;
use osrs_core::coords::{MapTile, RegionCoord, SourcePlane};
use osrs_core::ids::{FloorOverlayId, FloorUnderlayId};
use osrs_edit::{
    HeightVertexCoord, MapDocument, MapPatch, MapPosition, MapQuery, OverlayRotation, OverlayShape,
    TerrainOverlay, TerrainTile,
};
use osrs_world::{LocEdit, LocSpec, TileEdit};
use std::collections::{BTreeMap, HashMap, HashSet};
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

fn semantic_terrain(values: TileValues) -> Option<TerrainTile> {
    let underlay = (values.underlay != 0).then(|| FloorUnderlayId::new(u32::from(values.underlay)));
    let overlay = if values.overlay == 0 {
        None
    } else {
        Some(TerrainOverlay {
            id: FloorOverlayId::new(u32::from(values.overlay)),
            shape: OverlayShape::new(values.shape)?,
            rotation: OverlayRotation::new(values.rotation)?,
        })
    };
    Some(TerrainTile {
        underlay,
        overlay,
        flags: values.flags,
    })
}

fn project_semantic_terrain(values: &mut TileValues, terrain: TerrainTile) -> Option<()> {
    values.underlay = match terrain.underlay {
        Some(id) => u16::try_from(id.get()).ok()?,
        None => 0,
    };
    match terrain.overlay {
        Some(overlay) => {
            values.overlay = u16::try_from(overlay.id.get()).ok()?;
            values.shape = overlay.shape.get();
            values.rotation = overlay.rotation.get();
        }
        None => {
            values.overlay = 0;
            values.shape = 0;
            values.rotation = 0;
        }
    }
    values.flags = terrain.flags;
    Some(())
}

fn paint_intent_from_brush(
    brush: &Brush,
    tile: (i32, i32),
    after: TileValues,
) -> TerrainPaintIntent {
    let mut intent = TerrainPaintIntent::new(tile);
    if brush.underlay.armed {
        intent.underlay = Some(match brush.underlay.fill {
            Fill::Clear | Fill::Set(0) => None,
            Fill::Set(id) => Some(FloorUnderlayId::new(u32::from(id))),
        });
    }
    if brush.overlay.armed {
        intent.overlay = Some(match brush.overlay.fill {
            Fill::Clear | Fill::Set(0) => None,
            Fill::Set(id) => Some(FloorOverlayId::new(u32::from(id))),
        });
    }
    if brush.shape.armed && after.overlay != 0 {
        intent.overlay_shape = Some((
            OverlayShape::new(after.shape)
                .unwrap_or_else(|| unreachable!("Brush::apply clamps overlay shape")),
            OverlayRotation::new(after.rotation)
                .unwrap_or_else(|| unreachable!("Brush::apply clamps overlay rotation")),
        ));
    }
    intent.height = brush.height.armed.then_some(after.height);
    intent.flags = brush.flags.armed.then_some(after.flags);
    intent
}

fn build_paint_stroke_patch(
    query: &impl MapQuery,
    brush: &Brush,
    changes: &[(TileKey, TileValues, TileValues)],
) -> Result<MapPatch, PaintCommitError> {
    let mut by_plane = BTreeMap::<SourcePlane, Vec<TerrainPaintIntent>>::new();
    for (key, _, after) in changes {
        let plane = SourcePlane::new(key.0)
            .unwrap_or_else(|| unreachable!("editor stroke planes are always validated"));
        by_plane
            .entry(plane)
            .or_default()
            .push(paint_intent_from_brush(brush, (key.1, key.2), *after));
    }

    let mut ops = Vec::new();
    for (plane, intents) in by_plane {
        let patch = build_terrain_paint_patch(query, plane, &intents)?;
        ops.extend_from_slice(patch.ops());
    }
    Ok(MapPatch::new(ops))
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
    /// The exact Paint brush captured for this stroke. `None` keeps non-Paint strokes on the legacy
    /// commit path until their own migration checkpoints.
    paint_brush: Option<Brush>,
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
        for (key, before, after) in &mut stroke.step.changes {
            let Some(plane) = SourcePlane::new(key.0) else {
                continue;
            };
            let position = MapPosition::new(MapTile::new(key.1, key.2), plane);
            let vertex = HeightVertexCoord::new(MapTile::new(key.1, key.2), plane);
            let terrain = self.session.document().terrain_at(position);
            let semantic_height = self.session.document().height_at(vertex);
            let mut projected = *after;

            if brush.underlay.armed {
                projected.underlay = match brush.underlay.fill {
                    Fill::Clear | Fill::Set(0) => 0,
                    Fill::Set(id) => id,
                };
            }
            if brush.overlay.armed {
                match brush.overlay.fill {
                    Fill::Clear | Fill::Set(0) => {
                        projected.overlay = 0;
                        projected.shape = 0;
                        projected.rotation = 0;
                    }
                    Fill::Set(id) => {
                        projected.overlay = id;
                        if let Some(overlay) = terrain.overlay {
                            projected.shape = overlay.shape.get();
                            projected.rotation = overlay.rotation.get();
                        } else {
                            projected.shape = 0;
                            projected.rotation = 0;
                        }
                    }
                }
            } else if brush.shape.armed {
                if let Some(overlay) = terrain.overlay {
                    projected.shape = overlay.shape.get();
                    projected.rotation = overlay.rotation.get();
                } else {
                    projected.shape = before.shape;
                    projected.rotation = before.rotation;
                }
            }
            if brush.height.armed
                && let Some(height) = semantic_height
            {
                projected.height = height;
            }
            if brush.flags.armed {
                projected.flags = terrain.flags;
            }

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

    /// Finish the open stroke. Paint strokes commit exactly once to the semantic session before the
    /// current world/renderer projection and legacy compatibility history are finalized. Other tools
    /// remain on the existing legacy path until their own migration checkpoints.
    pub(super) fn end_stroke(&mut self) {
        let Some(mut stroke) = self.stroke.take() else {
            return;
        };
        if stroke.step.changes.is_empty()
            && stroke.step.added.is_empty()
            && stroke.step.removed.is_empty()
        {
            return;
        }

        if let Some(brush) = stroke.paint_brush.clone() {
            let patch = match build_paint_stroke_patch(
                self.session.document(),
                &brush,
                &stroke.step.changes,
            ) {
                Ok(patch) => patch,
                Err(error) => {
                    self.revert(&stroke.step, false);
                    self.notice = Some((
                        format!("Paint stroke was not committed: {error:?}"),
                        Instant::now(),
                    ));
                    return;
                }
            };

            match self.session.commit(patch) {
                Ok(Some(_)) => self.project_paint_stroke(&mut stroke, &brush),
                Ok(None) => {
                    self.project_paint_stroke(&mut stroke, &brush);
                    self.notice = Some((
                        "Paint stroke made no semantic changes".to_owned(),
                        Instant::now(),
                    ));
                    return;
                }
                Err(error) => {
                    self.revert(&stroke.step, false);
                    self.notice = Some((
                        format!("Paint stroke was rejected by the semantic session: {error:?}"),
                        Instant::now(),
                    ));
                    return;
                }
            }
        }

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
        let brush = stroke
            .paint_brush
            .get_or_insert_with(|| self.prefs.brush.clone())
            .clone();
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

    /// Lay a normal terrain path through the semantic edit-domain boundary.
    ///
    /// The domain patch is the canonical description of this commit. The patched semantic snapshot
    /// is then projected into the existing world edit layer and legacy undo stack so rendering and
    /// editor-wide undo behavior remain unchanged while migration proceeds one tool at a time.
    pub(super) fn apply_terrain_path(&mut self, tiles: &[PathTile]) -> usize {
        if tiles.is_empty() {
            return 0;
        }
        let Some(plane) = SourcePlane::new(self.view_plane) else {
            self.notice = Some((
                "The path uses an invalid source plane".to_owned(),
                Instant::now(),
            ));
            return 0;
        };

        let brush = self.prefs.brush.clone();
        let mut terrain = Vec::new();
        let mut heights = Vec::new();
        let mut intents = Vec::new();
        let mut snapshotted = HashSet::new();

        for path_tile in tiles {
            let key = (self.view_plane, path_tile.tile.0, path_tile.tile.1);
            let Some(before) = self.current_tile(key) else {
                continue;
            };
            let Some(before_terrain) = semantic_terrain(before) else {
                self.notice = Some((
                    format!(
                        "The path was not laid: tile {:?} has invalid terrain shape data",
                        path_tile.tile
                    ),
                    Instant::now(),
                ));
                return 0;
            };
            let map_tile = MapTile::new(path_tile.tile.0, path_tile.tile.1);
            if snapshotted.insert(path_tile.tile) {
                terrain.push((MapPosition::new(map_tile, plane), before_terrain));
                heights.push((HeightVertexCoord::new(map_tile, plane), before.height));
            }
            let height = brush
                .height
                .armed
                .then(|| self.quantize_height(key, brush.height.value));
            let flags = brush.flags.armed.then_some(brush.flags.bits);
            intents.push(TerrainPathIntent::from_path_tile(*path_tile, height, flags));
        }

        if intents.is_empty() {
            return 0;
        }

        let mut document = MapDocument::from_parts(terrain, heights, []);
        let patch = match build_terrain_path_patch(&document, plane, &intents) {
            Ok(patch) => patch,
            Err(error) => {
                self.notice = Some((
                    format!("The path was not laid: invalid semantic commit data ({error:?})"),
                    Instant::now(),
                ));
                return 0;
            }
        };
        if patch.is_empty() {
            return 0;
        }
        if let Err(error) = document.apply_patch(&patch) {
            self.notice = Some((
                format!("The path was not laid: semantic patch validation failed ({error:?})"),
                Instant::now(),
            ));
            return 0;
        }

        let mut projected = HashSet::new();
        let mut final_tiles = Vec::new();
        for intent in &intents {
            if !projected.insert(intent.tile) {
                continue;
            }
            let key = (self.view_plane, intent.tile.0, intent.tile.1);
            let Some(mut after) = self.current_tile(key) else {
                continue;
            };
            let map_tile = MapTile::new(intent.tile.0, intent.tile.1);
            let position = MapPosition::new(map_tile, plane);
            if project_semantic_terrain(&mut after, document.terrain_at(position)).is_none() {
                self.notice = Some((
                    "The path was not laid: semantic terrain IDs exceed the legacy adapter range"
                        .to_owned(),
                    Instant::now(),
                ));
                return 0;
            }
            let vertex = HeightVertexCoord::new(map_tile, plane);
            if let Some(height) = document.height_at(vertex) {
                after.height = height;
            }
            final_tiles.push((key, after));
        }

        self.begin_stroke();
        let Some(mut stroke) = self.stroke.take() else {
            return 0;
        };
        let mut edits = Vec::new();
        for (key, after) in final_tiles {
            self.record(&mut stroke, key, after, &mut edits);
        }
        let count = edits.len();
        self.send_edits(edits);
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
    use crate::session::EditorSession;

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
    fn one_paint_stroke_is_one_semantic_session_history_entry() {
        let brush = Brush::default();
        let before = TileValues::default();
        let after = brush.apply(before);
        let changes = vec![
            ((0, 3200, 3200), before, after),
            ((1, 3200, 3200), before, after),
        ];
        let mut session = EditorSession::new(MapDocument::new());
        let patch = build_paint_stroke_patch(session.document(), &brush, &changes)
            .unwrap_or_else(|error| unreachable!("valid Paint stroke: {error:?}"));

        assert_eq!(patch.len(), 2);
        assert!(
            session
                .commit(patch)
                .unwrap_or_else(|error| unreachable!("valid semantic commit: {error:?}"))
                .is_some()
        );
        assert_eq!(session.revision(), 1);
        assert!(session.can_undo());

        for plane in [0, 1] {
            let source_plane =
                SourcePlane::new(plane).unwrap_or_else(|| unreachable!("test plane is valid"));
            let position = MapPosition::new(MapTile::new(3200, 3200), source_plane);
            assert_eq!(
                session
                    .document()
                    .terrain_at(position)
                    .overlay
                    .map(|overlay| overlay.id),
                Some(FloorOverlayId::new(1))
            );
        }

        assert!(
            session
                .undo()
                .unwrap_or_else(|error| unreachable!("valid semantic undo: {error:?}"))
                .is_some()
        );
        assert_eq!(session.revision(), 2);
        for plane in [0, 1] {
            let source_plane =
                SourcePlane::new(plane).unwrap_or_else(|| unreachable!("test plane is valid"));
            let position = MapPosition::new(MapTile::new(3200, 3200), source_plane);
            assert_eq!(
                session.document().terrain_at(position),
                TerrainTile::default()
            );
        }
    }
}
