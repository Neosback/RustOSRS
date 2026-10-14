//! Runtime terrain-path commit and worker projection.
//!
//! Path fitting stays in `terrain_path` / `spline_tool`, and pure semantic patch planning stays in
//! `app/path_commit.rs`. This module owns only the application boundary: plan against the persistent
//! editor session, commit through `EditRuntime`, then apply an already-validated worker projection.

use super::{EditStep, EditorApp, PathTile, TileKey};
use crate::app::edit_runtime::{EditExecution, EditExecutionError};
use crate::app::path_commit::{TerrainPathIntent, build_terrain_path_patch};
use crate::heights::{decode_height, encode_height};
use crate::paint::TileValues;
use osrs_core::coords::{MapTile, SourcePlane};
use osrs_edit::{HeightVertexCoord, MapPosition, MapQuery, TerrainTile};
use std::collections::HashSet;
use web_time::Instant;

/// One already-validated Path projection from authoritative semantic state into the transitional
/// preview/worker mirrors.
type PathProjection = Vec<(TileKey, TileValues, TileValues)>;

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

/// Project one authoritative semantic Path result into the transitional worker tile value.
///
/// Terrain channels always come from `MapQuery`. Height comes from semantic state only when the
/// Path gesture armed Height; otherwise the worker's current height is deliberately preserved.
pub(super) fn project_semantic_path_values(
    query: &impl MapQuery,
    key: TileKey,
    current: TileValues,
    include_height: bool,
) -> Option<TileValues> {
    let plane = SourcePlane::new(key.0)?;
    let tile = MapTile::new(key.1, key.2);
    let mut values = current;
    project_semantic_terrain(&mut values, query.terrain_at(MapPosition::new(tile, plane)))?;
    if include_height {
        values.height = query.height_at(HeightVertexCoord::new(tile, plane))?;
    }
    Some(values)
}

/// Prepare the complete Path projection from the authoritative post-commit document.
///
/// The caller snapshots only the legacy values needed by the adapter before entering the gateway.
/// Any unrepresentable semantic result fails while `EditRuntime` still owns the active checkpoint,
/// allowing exact semantic restoration without cloning the whole `MapDocument`.
fn plan_path_projection(
    query: &impl MapQuery,
    current: &[(TileKey, TileValues)],
    include_height: bool,
) -> Result<PathProjection, TileKey> {
    let mut projected = Vec::with_capacity(current.len());
    for (key, before) in current {
        let after =
            project_semantic_path_values(query, *key, *before, include_height).ok_or(*key)?;
        projected.push((*key, *before, after));
    }
    Ok(projected)
}

fn quantize_path_height(
    query: &impl MapQuery,
    plane: SourcePlane,
    tile: MapTile,
    desired: i32,
) -> Result<i32, &'static str> {
    let plane_index = plane.index().get();
    let base = if plane_index == 0 {
        0
    } else {
        let below = SourcePlane::new(plane_index - 1)
            .unwrap_or_else(|| unreachable!("a non-zero source plane always has a lower plane"));
        query
            .height_at(HeightVertexCoord::new(tile, below))
            .ok_or("the semantic lower-plane height is missing")?
    };
    Ok(decode_height(base, encode_height(base, desired)))
}

impl EditorApp {
    /// Lay one fitted terrain path as one persistent semantic transaction.
    pub(in crate::app) fn apply_terrain_path(&mut self, tiles: &[PathTile]) -> usize {
        if tiles.is_empty() {
            return 0;
        }
        let Some(plane) = SourcePlane::new(self.interaction.view_plane) else {
            self.notice = Some((
                "The path uses an invalid source plane".to_owned(),
                Instant::now(),
            ));
            return 0;
        };

        let brush = self.prefs.brush.clone();
        let mut intents = Vec::new();
        for path_tile in tiles {
            let key = (
                self.interaction.view_plane,
                path_tile.tile.0,
                path_tile.tile.1,
            );
            if self.current_tile(key).is_none() {
                continue;
            }
            let map_tile = MapTile::new(path_tile.tile.0, path_tile.tile.1);
            let height = if brush.height.armed {
                match quantize_path_height(
                    self.session.document(),
                    plane,
                    map_tile,
                    brush.height.value,
                ) {
                    Ok(height) => Some(height),
                    Err(message) => {
                        self.notice = Some((
                            format!("The path was not laid at {:?}: {message}", path_tile.tile),
                            Instant::now(),
                        ));
                        return 0;
                    }
                }
            } else {
                None
            };
            let flags = brush.flags.armed.then_some(brush.flags.bits);
            intents.push(TerrainPathIntent::from_path_tile(*path_tile, height, flags));
        }

        if intents.is_empty() {
            return 0;
        }

        let patch = match build_terrain_path_patch(self.session.document(), plane, &intents) {
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
            self.notice = Some((
                "The path made no semantic changes".to_owned(),
                Instant::now(),
            ));
            return 0;
        }

        // Snapshot only the transitional values required for projection. Duplicate fitted path tiles
        // share one route key and one worker projection, matching the previous preflight behavior.
        let mut seen = HashSet::new();
        let mut projection_input = Vec::new();
        for intent in &intents {
            if !seen.insert(intent.tile) {
                continue;
            }
            let key = (self.interaction.view_plane, intent.tile.0, intent.tile.1);
            let Some(before) = self.current_tile(key) else {
                self.notice = Some((
                    format!(
                        "The path was not laid: tile {:?} disappeared before projection",
                        intent.tile
                    ),
                    Instant::now(),
                ));
                return 0;
            };
            projection_input.push((key, before));
        }
        let route_keys: Vec<_> = projection_input.iter().map(|(key, _)| *key).collect();

        let execution = self.session.execute(patch, |document| {
            plan_path_projection(document, &projection_input, brush.height.armed)
        });
        let projection = match execution {
            Ok(EditExecution::Committed { projection, .. }) => projection,
            Ok(EditExecution::NoChange) => {
                self.notice = Some((
                    "The path made no semantic changes".to_owned(),
                    Instant::now(),
                ));
                return 0;
            }
            Err(EditExecutionError::Semantic(error)) => {
                self.notice = Some((
                    format!("The path was rejected by the semantic session: {error:?}"),
                    Instant::now(),
                ));
                return 0;
            }
            Err(EditExecutionError::Projection(key)) => {
                self.notice = Some((
                    format!(
                        "The path commit was rolled back: semantic state at ({}, {}) cannot be represented by the legacy projection",
                        key.1, key.2
                    ),
                    Instant::now(),
                ));
                return 0;
            }
            Err(EditExecutionError::Rollback {
                projection: key,
                rollback,
            }) => {
                self.notice = Some((
                    format!(
                        "Path projection failed at ({}, {}) and semantic rollback failed: {rollback:?}",
                        key.1, key.2
                    ),
                    Instant::now(),
                ));
                return 0;
            }
            Err(EditExecutionError::Finalize(error)) => {
                self.notice = Some((
                    format!("Path projection succeeded but semantic finalize failed: {error:?}"),
                    Instant::now(),
                ));
                return 0;
            }
        };

        let mut edits = Vec::with_capacity(projection.len());
        for (key, before, after) in &projection {
            if after == before {
                continue;
            }
            if after.height != before.height {
                self.height_edited.insert(*key);
            }
            self.edited.insert(*key, *after);
            edits.push(self.edit_for(*key, *after));
        }
        let count = route_keys.len();
        self.send_edits(edits);
        self.push_step(EditStep::semantic_path(route_keys, brush.height.armed));
        self.notice = Some((
            format!(
                "Changed {count} path tile{}",
                if count == 1 { "" } else { "s" }
            ),
            Instant::now(),
        ));
        count
    }
}

#[cfg(test)]
mod tests;
