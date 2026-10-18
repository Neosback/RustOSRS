//! Runtime terrain-path commit and worker projection.
//!
//! Path fitting stays in `terrain_path` / `spline_tool`, and pure semantic patch planning stays in
//! `app/path_commit.rs`. This module owns only the application boundary: plan against the persistent
//! editor session, commit through `EditRuntime`, then apply an already-validated worker projection.

use super::{EditStep, EditorApp, PathTile, TileKey};
use crate::app::edit_runtime::{EditExecution, EditExecutionError};
#[cfg(test)]
use crate::app::path_commit::build_terrain_path_patch;
use crate::app::path_commit::path_intent;
use crate::paint::TileValues;
use osrs_core::coords::{MapTile, SourcePlane};
use osrs_edit::{HeightVertexCoord, MapOp, MapPatch, MapPosition, MapQuery, TerrainTile};
use std::collections::HashSet;

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

impl EditorApp {
    /// Lay one fitted terrain path (the overlay, shape and rotation of every tile) as one
    /// persistent semantic transaction.
    pub(in crate::app) fn apply_terrain_path(&mut self, tiles: &[PathTile]) -> usize {
        if tiles.is_empty() {
            return 0;
        }
        let Some(plane) = SourcePlane::new(self.interaction.view_plane()) else {
            self.ui
                .notify("The path uses an invalid source plane".to_owned());
            return 0;
        };
        let view_plane = self.interaction.view_plane();
        let mut intents = Vec::new();
        for path_tile in tiles {
            // Only tiles the worker mirror holds can be projected.
            if self
                .current_tile((view_plane, path_tile.tile.0, path_tile.tile.1))
                .is_none()
            {
                continue;
            }
            intents.push(path_intent(*path_tile));
        }
        if intents.is_empty() {
            return 0;
        }
        let companion_underlay = if self.settings.prefs.spline.paint_companion_underlay {
            match self.settings.prefs.brush.underlay {
                crate::paint::IdChannel {
                    armed: true,
                    fill: crate::paint::Fill::Set(id),
                } if id > 0 => Some(id),
                _ => tiles
                    .first()
                    .and_then(|t| t.overlay)
                    .and_then(|ov| self.overlay_swatch(ov))
                    .and_then(|s| s.top_companion_underlay()),
            }
        } else {
            None
        };
        let patch = match osrs_edit::build_terrain_path_patch_with_underlay(
            self.session.document(),
            plane,
            &intents,
            companion_underlay,
        ) {
            Ok(patch) => patch,
            Err(error) => {
                self.ui.notify(format!(
                    "The path was not laid: invalid semantic commit data ({error:?})"
                ));
                return 0;
            }
        };
        let mut seen = HashSet::new();
        let keys: Vec<TileKey> = intents
            .iter()
            .filter(|intent| seen.insert(intent.tile))
            .map(|intent| (view_plane, intent.tile.x, intent.tile.y))
            .collect();
        commit_semantic_path(self, patch, keys, false, |count| {
            format!(
                "Changed {count} path tile{}",
                if count == 1 { "" } else { "s" }
            )
        })
    }
}

/// Smooth the terrain under a laid path as its own transaction (one more undo step, as in the
/// original editor): `smoothed` is the new height of each vertex on the viewed plane, and the
/// planes above move with it. Returns how many vertices changed.
pub(in crate::app) fn commit_path_height(
    app: &mut EditorApp,
    smoothed: &[((i32, i32), i32)],
) -> usize {
    let view_plane = app.interaction.view_plane();
    let Some(plane) = SourcePlane::new(view_plane) else {
        app.ui
            .notify("The path uses an invalid source plane".to_owned());
        return 0;
    };
    // Only vertices the worker mirror holds can be projected.
    let loaded: Vec<((i32, i32), i32)> = smoothed
        .iter()
        .copied()
        .filter(|((x, y), _)| app.current_tile((view_plane, *x, *y)).is_some())
        .collect();
    if loaded.is_empty() {
        return 0;
    }
    let patch = match osrs_edit::build_path_height_patch(app.session.document(), plane, &loaded) {
        Ok(patch) => patch,
        Err(error) => {
            app.ui.notify(format!(
                "The terrain was not smoothed: invalid semantic commit data ({error:?})"
            ));
            return 0;
        }
    };
    let keys = height_keys(&patch);
    commit_semantic_path(app, patch, keys, true, |count| {
        format!(
            "Smoothed the height of {count} vertex{}",
            if count == 1 { "" } else { "es" }
        )
    })
}

/// The `(plane, x, y)` of every vertex a patch gives a new height.
fn height_keys(patch: &MapPatch) -> Vec<TileKey> {
    let mut seen = HashSet::new();
    patch
        .ops()
        .iter()
        .filter_map(|op| match op {
            MapOp::SetHeight { coord, .. } => {
                Some((coord.plane.index().get(), coord.corner.x, coord.corner.y))
            }
            _ => None,
        })
        .filter(|key| seen.insert(*key))
        .collect()
}

/// Commit `patch` through the session and project it into the transitional mirrors for `keys`;
/// `include_height` says whether heights are projected too. Returns how many keys changed.
fn commit_semantic_path(
    app: &mut EditorApp,
    patch: MapPatch,
    keys: Vec<TileKey>,
    include_height: bool,
    summary: impl FnOnce(usize) -> String,
) -> usize {
    if patch.is_empty() {
        app.ui
            .notify("The path made no semantic changes".to_owned());
        return 0;
    }
    // Snapshot only the transitional values required for projection.
    let mut projection_input = Vec::with_capacity(keys.len());
    for key in &keys {
        let Some(before) = app.current_tile(*key) else {
            app.ui.notify(format!(
                "The path was not laid: tile ({}, {}) disappeared before projection",
                key.1, key.2
            ));
            return 0;
        };
        projection_input.push((*key, before));
    }
    let route_keys = keys;
    let execution = app.session.execute_named("path", patch, |document| {
        plan_path_projection(document, &projection_input, include_height)
    });
    let (receipt, projection) = match execution {
        Ok(EditExecution::Committed {
            receipt,
            projection,
        }) => (receipt, projection),
        Ok(EditExecution::NoChange) => {
            app.ui
                .notify("The path made no semantic changes".to_owned());
            return 0;
        }
        Err(EditExecutionError::Semantic(error)) => {
            app.ui.notify(format!(
                "The path was rejected by the semantic session: {error:?}"
            ));
            return 0;
        }
        Err(EditExecutionError::Projection(key)) => {
            app.ui.notify(format!(
                "The path commit was rolled back: semantic state at ({}, {}) cannot be represented by the legacy projection",
                key.1, key.2
            ));
            return 0;
        }
        Err(EditExecutionError::Rollback {
            projection: key,
            rollback,
        }) => {
            app.ui.notify(format!(
                "Path projection failed at ({}, {}) and semantic rollback failed: {rollback:?}",
                key.1, key.2
            ));
            return 0;
        }
        Err(EditExecutionError::Finalize(error)) => {
            app.ui.notify(format!(
                "Path projection succeeded but semantic finalize failed: {error:?}"
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
            app.terrain.height_edited.insert(*key);
        }
        app.terrain.edited.insert(*key, *after);
        edits.push(app.edit_for(*key, *after));
    }
    let count = route_keys.len();
    app.world.project_tile_edits(edits);
    app.world.invalidate_semantic_edit(&receipt);
    app.push_step(EditStep::semantic_path(route_keys, include_height));
    app.ui.notify(summary(count));
    count
}

#[cfg(test)]
mod tests;
