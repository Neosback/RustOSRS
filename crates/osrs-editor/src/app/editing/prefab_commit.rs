//! Committing one semantic map patch that changes terrain, heights and objects together (a placed
//! prefab, a generated building) as a single undoable step.
//!
//! The patch executes through the application edit gateway; the legacy tile mirror and the object
//! mirror are projected from the authoritative post-commit document inside the gateway's fallible
//! boundary, so a failure restores the exact pre-commit session. `EditorSession` records the patch
//! as one semantic transaction, and undo/redo derive projection requirements from that history.

use super::*;
use crate::app::edit_runtime::{EditExecution, EditExecutionError};
use crate::app::object_tool::{
    ObjectHistoryProjection, ObjectHistoryRoute, build_object_history_projection,
    spec_from_placement,
};
use osrs_edit::{MapOp, MapPatch};
use path::project_semantic_path_values;
use std::collections::BTreeSet;

struct Projection {
    tiles: Vec<(TileKey, TileValues, TileValues)>,
    objects: Option<ObjectHistoryProjection>,
}

/// Commit `patch` as one semantic transaction labelled `label` (shown as the notice). Returns
/// whether it committed; the reason is shown to the user when it did not.
pub(in crate::app) fn commit_map_patch(app: &mut EditorApp, patch: MapPatch, label: &str) -> bool {
    commit_patch(app, patch, label, false)
}

pub(in crate::app) fn apply_baseline_patch(
    app: &mut EditorApp,
    patch: MapPatch,
    label: &str,
) -> bool {
    commit_patch(app, patch, label, true)
}

fn commit_patch(app: &mut EditorApp, patch: MapPatch, label: &str, baseline: bool) -> bool {
    let mut keys = BTreeSet::new();
    let mut include_height = false;
    let (mut removed, mut added) = (Vec::new(), Vec::new());
    for op in patch.ops() {
        match *op {
            MapOp::SetTerrain { coord, .. } => {
                keys.insert((coord.plane.index().get(), coord.tile.x, coord.tile.y));
            }
            MapOp::SetHeight { coord, .. } => {
                include_height = true;
                keys.insert((coord.plane.index().get(), coord.corner.x, coord.corner.y));
            }
            MapOp::AddObject { placement } => added.push(spec_from_placement(placement)),
            MapOp::RemoveObject { placement } => removed.push(spec_from_placement(placement)),
        }
    }
    if keys.is_empty() && removed.is_empty() && added.is_empty() {
        app.ui.notify(format!("{label} made no changes"));
        return false;
    }
    // A piece removed and put back exactly changes nothing: the object route (what the worker
    // mirror and undo see) must describe the net change, not the two halves.
    let put_back: Vec<_> = removed
        .iter()
        .copied()
        .filter(|spec| added.contains(spec))
        .collect();
    removed.retain(|spec| !put_back.contains(spec));
    added.retain(|spec| !put_back.contains(spec));
    let keys: Vec<TileKey> = keys.into_iter().collect();
    let current: Vec<_> = keys
        .iter()
        .map(|&key| (key, app.current_tile(key)))
        .collect();
    let has_objects = !removed.is_empty() || !added.is_empty();
    let route = ObjectHistoryRoute::new(removed, added);

    let prepare = |document: &osrs_edit::MapDocument| {
        let mut tiles = Vec::with_capacity(current.len());
        for (key, current) in &current {
            let current = current
                .ok_or("part of the area is not loaded yet; move the camera over it first")?;
            let values = project_semantic_path_values(document, *key, current, include_height)
                .ok_or("the result cannot be represented by the legacy tile mirror")?;
            tiles.push((*key, current, values));
        }
        let objects = if has_objects {
            Some(
                build_object_history_projection(document, &route, true)
                    .map_err(|error| format!("{error:?}"))?,
            )
        } else {
            None
        };
        Ok::<_, String>(Projection { tiles, objects })
    };
    let execution = if baseline {
        app.session.execute_baseline(patch, prepare)
    } else {
        app.session.execute_named("prefab", patch, prepare)
    };
    let (receipt, projection) = match execution {
        Ok(EditExecution::Committed {
            receipt,
            projection,
        }) => (receipt, projection),
        Ok(EditExecution::NoChange) => {
            app.ui
                .notify(format!("{label} made no changes: the map already matches"));
            return false;
        }
        Err(EditExecutionError::Semantic(error)) => {
            app.ui
                .notify(format!("{label} was rejected by the map: {error:?}"));
            return false;
        }
        Err(EditExecutionError::Projection(message)) => {
            app.ui.notify(format!("{label} was rolled back: {message}"));
            return false;
        }
        Err(EditExecutionError::Rollback {
            projection,
            rollback,
        }) => {
            app.ui.notify(format!(
                "{label} failed and could not be rolled back cleanly: {projection} ({rollback:?})"
            ));
            return false;
        }
        Err(EditExecutionError::Finalize(error)) => {
            app.ui.notify(format!(
                "{label} applied but could not be finalized: {error:?}"
            ));
            return false;
        }
    };

    let Projection { tiles, objects } = projection;
    let mut edits = Vec::new();
    for (key, current, values) in tiles {
        if include_height {
            app.terrain.height_edited.insert(key);
        }
        if values != current {
            app.terrain.edited.insert(key, values);
            edits.push(app.edit_for(key, values));
        }
    }
    app.world.project_tile_edits(edits);
    if let Some(projection) = objects {
        app.world.project_loc_edits(projection.edits);
        app.clear_selection();
        app.interaction.set_pending_reselect(projection.reselect);
    }
    app.world.invalidate_semantic_edit(&receipt);
    app.ui.notify(label.to_owned());
    true
}
