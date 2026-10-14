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
        let after = project_semantic_path_values(query, *key, *before, include_height)
            .ok_or(*key)?;
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
mod tests {
    use super::*;
    use crate::app::edit_runtime::{EditExecution, EditExecutionError, EditRuntime};
    use crate::session::EditorSession;
    use osrs_core::ids::{FloorOverlayId, FloorUnderlayId};
    use osrs_edit::{
        MapDocument, MapOp, MapPatch, OverlayRotation, OverlayShape, TerrainOverlay,
    };

    fn plane(value: u8) -> SourcePlane {
        SourcePlane::new(value).unwrap_or_else(|| unreachable!("test plane is valid"))
    }

    fn height_patch(coord: HeightVertexCoord, before: i32, after: i32) -> MapPatch {
        MapPatch::new([MapOp::SetHeight {
            coord,
            before,
            after,
        }])
    }

    fn runtime_with_redo() -> (EditRuntime, MapTile) {
        let prior = MapTile::new(3200, 3200);
        let edited = MapTile::new(3264, 3200);
        let mut runtime = EditRuntime::new(EditorSession::new(MapDocument::from_parts(
            [],
            [
                (HeightVertexCoord::new(prior, plane(0)), 0),
                (HeightVertexCoord::new(edited, plane(0)), 0),
            ],
            [],
        )));
        runtime
            .commit(height_patch(
                HeightVertexCoord::new(prior, plane(0)),
                0,
                32,
            ))
            .unwrap_or_else(|error| unreachable!("valid setup commit: {error:?}"));
        runtime
            .undo()
            .unwrap_or_else(|error| unreachable!("valid setup undo: {error:?}"));
        assert!(runtime.can_redo());
        (runtime, edited)
    }

    fn path_patch(runtime: &EditRuntime, tile: MapTile) -> MapPatch {
        let intent = TerrainPathIntent {
            tile: (tile.x, tile.y),
            overlay: Some(7),
            shape: 0,
            rotation: 0,
            height: None,
            flags: None,
        };
        build_terrain_path_patch(runtime.document(), plane(0), &[intent])
            .unwrap_or_else(|error| unreachable!("valid Path patch: {error:?}"))
    }

    #[test]
    fn upper_plane_path_height_requires_the_semantic_lower_plane_base() {
        let tile = MapTile::new(3200, 3200);
        let document =
            MapDocument::from_parts([], [(HeightVertexCoord::new(tile, plane(1)), 100)], []);

        assert_eq!(
            quantize_path_height(&document, plane(1), tile, 64),
            Err("the semantic lower-plane height is missing")
        );
    }

    #[test]
    fn failed_path_projection_restores_exact_pre_transaction_state() {
        let (mut runtime, edited) = runtime_with_redo();
        let before = runtime.clone();
        let patch = path_patch(&runtime, edited);
        let coord = MapPosition::new(edited, plane(0));
        let invalid_key = (u8::MAX, edited.x, edited.y);
        let result = runtime.execute(patch, |document| {
            assert_eq!(
                document
                    .terrain_at(coord)
                    .overlay
                    .map(|overlay| overlay.id.get()),
                Some(7)
            );
            plan_path_projection(
                document,
                &[(invalid_key, TileValues::default())],
                false,
            )
        });

        assert_eq!(result, Err(EditExecutionError::Projection(invalid_key)));
        assert_eq!(runtime, before);
    }

    #[test]
    fn successful_path_projection_uses_post_commit_state_and_finalizes_once() {
        let (mut runtime, edited) = runtime_with_redo();
        let revision_before = runtime.revision();
        let patch = path_patch(&runtime, edited);
        let key = (0, edited.x, edited.y);
        let coord = MapPosition::new(edited, plane(0));
        let result = runtime
            .execute(patch, |document| {
                assert_eq!(
                    document
                        .terrain_at(coord)
                        .overlay
                        .map(|overlay| overlay.id.get()),
                    Some(7)
                );
                plan_path_projection(document, &[(key, TileValues::default())], false)
            })
            .unwrap_or_else(|error| unreachable!("Path gateway execution succeeds: {error:?}"));

        let EditExecution::Committed { projection, .. } = result else {
            unreachable!("non-empty Path patch commits")
        };
        assert_eq!(
            projection,
            vec![(
                key,
                TileValues::default(),
                TileValues {
                    overlay: 7,
                    ..TileValues::default()
                }
            )]
        );
        assert_eq!(runtime.revision(), revision_before + 1);
        assert_eq!(
            runtime
                .document()
                .terrain_at(coord)
                .overlay
                .map(|overlay| overlay.id.get()),
            Some(7)
        );
        assert!(runtime.can_undo());
        assert!(!runtime.can_redo());
    }

    #[test]
    fn one_path_patch_is_one_persistent_session_revision() {
        let tile = MapTile::new(3200, 3200);
        let coord = MapPosition::new(tile, plane(0));
        let overlay = TerrainOverlay {
            id: FloorOverlayId::new(7),
            shape: OverlayShape::new(0).unwrap_or_else(|| unreachable!("shape zero is valid")),
            rotation: OverlayRotation::new(0)
                .unwrap_or_else(|| unreachable!("rotation zero is valid")),
        };
        let document =
            MapDocument::from_parts([], [(HeightVertexCoord::new(tile, plane(0)), 0)], []);
        let mut session = EditorSession::new(document);
        let intent = TerrainPathIntent {
            tile: (3200, 3200),
            overlay: Some(7),
            shape: 0,
            rotation: 0,
            height: None,
            flags: None,
        };
        let patch = build_terrain_path_patch(session.document(), plane(0), &[intent])
            .unwrap_or_else(|error| unreachable!("valid path patch: {error:?}"));

        let commit = session
            .commit(patch)
            .unwrap_or_else(|error| unreachable!("path commit should succeed: {error:?}"))
            .unwrap_or_else(|| unreachable!("path changes semantic terrain"));

        assert_eq!(commit.revision, 1);
        assert_eq!(session.revision(), 1);
        assert!(session.can_undo());
        assert_eq!(session.document().terrain_at(coord).overlay, Some(overlay));
    }

    #[test]
    fn semantic_path_projection_tracks_undo_redo_and_optional_height() {
        let tile = MapTile::new(3200, 3200);
        let key = (0, 3200, 3200);
        let coord = MapPosition::new(tile, plane(0));
        let vertex = HeightVertexCoord::new(tile, plane(0));
        let before_terrain = TerrainTile {
            underlay: Some(FloorUnderlayId::new(3)),
            overlay: None,
            flags: 1,
        };
        let before = TileValues {
            underlay: 3,
            height: -96,
            flags: 1,
            ..TileValues::default()
        };
        let mut session = EditorSession::new(MapDocument::from_parts(
            [(coord, before_terrain)],
            [(vertex, -96)],
            [],
        ));
        let intent = TerrainPathIntent {
            tile: (3200, 3200),
            overlay: Some(7),
            shape: 4,
            rotation: 2,
            height: Some(-32),
            flags: Some(5),
        };
        let patch = build_terrain_path_patch(session.document(), plane(0), &[intent])
            .unwrap_or_else(|error| unreachable!("valid path patch: {error:?}"));
        assert!(session.commit(patch).is_ok());

        let terrain_only = project_semantic_path_values(session.document(), key, before, false)
            .unwrap_or_else(|| unreachable!("semantic Path terrain is projectable"));
        assert_eq!(terrain_only.overlay, 7);
        assert_eq!(terrain_only.shape, 4);
        assert_eq!(terrain_only.rotation, 2);
        assert_eq!(terrain_only.flags, 5);
        assert_eq!(terrain_only.height, -96);

        let committed = project_semantic_path_values(session.document(), key, before, true)
            .unwrap_or_else(|| unreachable!("semantic Path state is projectable"));
        assert_eq!(committed.height, -32);

        assert!(session.undo().is_ok());
        let undone = project_semantic_path_values(session.document(), key, committed, true)
            .unwrap_or_else(|| unreachable!("semantic Path undo is projectable"));
        assert_eq!(undone, before);

        assert!(session.redo().is_ok());
        let redone = project_semantic_path_values(session.document(), key, undone, true)
            .unwrap_or_else(|| unreachable!("semantic Path redo is projectable"));
        assert_eq!(redone, committed);
    }

    #[test]
    fn semantic_path_history_marker_carries_no_legacy_tile_authority() {
        let key = (0, 3200, 3200);
        let step = EditStep::semantic_path(vec![key], true);

        assert!(step.changes.is_empty());
        assert!(step.removed.is_empty());
        assert!(step.added.is_empty());
        assert!(step.semantic_paint.is_none());
        assert!(step.semantic_height.is_none());
        assert!(step.semantic_object.is_none());
        let path = step
            .semantic_path
            .as_ref()
            .unwrap_or_else(|| unreachable!("Path marker should be present"));
        assert_eq!(path.keys, vec![key]);
        assert!(path.include_height);
    }

    #[test]
    fn consecutive_path_transactions_follow_semantic_history_order() {
        let tile = MapTile::new(3200, 3200);
        let coord = MapPosition::new(tile, plane(0));
        let vertex = HeightVertexCoord::new(tile, plane(0));
        let mut session = EditorSession::new(MapDocument::from_parts([], [(vertex, 0)], []));
        let first = TerrainPathIntent {
            tile: (3200, 3200),
            overlay: Some(7),
            shape: 1,
            rotation: 0,
            height: None,
            flags: None,
        };
        let second = TerrainPathIntent {
            tile: (3200, 3200),
            overlay: Some(8),
            shape: 2,
            rotation: 3,
            height: Some(-32),
            flags: Some(5),
        };

        let first_patch = build_terrain_path_patch(session.document(), plane(0), &[first])
            .unwrap_or_else(|error| unreachable!("valid first Path patch: {error:?}"));
        assert!(session.commit(first_patch).is_ok());
        let second_patch = build_terrain_path_patch(session.document(), plane(0), &[second])
            .unwrap_or_else(|error| unreachable!("valid second Path patch: {error:?}"));
        assert!(session.commit(second_patch).is_ok());
        assert_eq!(
            session
                .document()
                .terrain_at(coord)
                .overlay
                .map(|overlay| overlay.id.get()),
            Some(8)
        );
        assert_eq!(session.document().height_at(vertex), Some(-32));

        assert!(session.undo().is_ok());
        assert_eq!(
            session
                .document()
                .terrain_at(coord)
                .overlay
                .map(|overlay| overlay.id.get()),
            Some(7)
        );
        assert_eq!(session.document().height_at(vertex), Some(0));

        assert!(session.undo().is_ok());
        assert_eq!(session.document().terrain_at(coord), TerrainTile::default());

        assert!(session.redo().is_ok());
        assert_eq!(
            session
                .document()
                .terrain_at(coord)
                .overlay
                .map(|overlay| overlay.id.get()),
            Some(7)
        );
        assert!(session.redo().is_ok());
        assert_eq!(
            session
                .document()
                .terrain_at(coord)
                .overlay
                .map(|overlay| overlay.id.get()),
            Some(8)
        );
        assert_eq!(session.document().height_at(vertex), Some(-32));
    }
}
