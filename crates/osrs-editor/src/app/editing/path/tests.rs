use super::*;
use crate::app::edit_runtime::{EditExecution, EditExecutionError, EditRuntime};
use crate::session::EditorSession;
use osrs_core::ids::{FloorOverlayId, FloorUnderlayId};
use osrs_edit::TerrainPathIntent;
use osrs_edit::{MapDocument, MapOp, MapPatch, OverlayRotation, OverlayShape, TerrainOverlay};

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
        .commit(height_patch(HeightVertexCoord::new(prior, plane(0)), 0, 32))
        .unwrap_or_else(|error| unreachable!("valid setup commit: {error:?}"));
    runtime
        .undo()
        .unwrap_or_else(|error| unreachable!("valid setup undo: {error:?}"));
    assert!(runtime.can_redo());
    (runtime, edited)
}

fn path_patch(runtime: &EditRuntime, tile: MapTile) -> MapPatch {
    let intent = TerrainPathIntent {
        tile,
        overlay: Some(7),
        shape: 0,
        rotation: 0,
    };
    build_terrain_path_patch(runtime.document(), plane(0), &[intent])
        .unwrap_or_else(|error| unreachable!("valid Path patch: {error:?}"))
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
        plan_path_projection(document, &[(invalid_key, TileValues::default())], false)
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
        rotation: OverlayRotation::new(0).unwrap_or_else(|| unreachable!("rotation zero is valid")),
    };
    let document = MapDocument::from_parts([], [(HeightVertexCoord::new(tile, plane(0)), 0)], []);
    let mut session = EditorSession::new(document);
    let intent = TerrainPathIntent {
        tile: osrs_core::coords::MapTile::new(3200, 3200),
        overlay: Some(7),
        shape: 0,
        rotation: 0,
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
fn semantic_path_projection_tracks_undo_redo_and_the_separate_height_step() {
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
        tile: osrs_core::coords::MapTile::new(3200, 3200),
        overlay: Some(7),
        shape: 4,
        rotation: 2,
    };
    let patch = build_terrain_path_patch(session.document(), plane(0), &[intent])
        .unwrap_or_else(|error| unreachable!("valid path patch: {error:?}"));
    assert!(session.commit(patch).is_ok());

    // The overlay step keeps the tile's flags and height.
    let laid = project_semantic_path_values(session.document(), key, before, false)
        .unwrap_or_else(|| unreachable!("semantic Path terrain is projectable"));
    assert_eq!(laid.overlay, 7);
    assert_eq!(laid.shape, 4);
    assert_eq!(laid.rotation, 2);
    assert_eq!(laid.flags, 1);
    assert_eq!(laid.height, -96);

    // Smoothing the terrain is its own step.
    let heights =
        osrs_edit::build_path_height_patch(session.document(), plane(0), &[((3200, 3200), -53)])
            .unwrap_or_else(|error| unreachable!("valid height patch: {error:?}"));
    assert!(session.commit(heights).is_ok());
    assert_eq!(session.revision(), 2, "overlay and height are two steps");
    let smoothed = project_semantic_path_values(session.document(), key, laid, true)
        .unwrap_or_else(|| unreachable!("semantic Path state is projectable"));
    assert_eq!(smoothed.height, -53);

    assert!(session.undo().is_ok());
    let undone_height = project_semantic_path_values(session.document(), key, smoothed, true)
        .unwrap_or_else(|| unreachable!("semantic height undo is projectable"));
    assert_eq!(undone_height, laid);
    assert!(session.undo().is_ok());
    let undone = project_semantic_path_values(session.document(), key, undone_height, true)
        .unwrap_or_else(|| unreachable!("semantic Path undo is projectable"));
    assert_eq!(undone, before);

    assert!(session.redo().is_ok());
    assert!(session.redo().is_ok());
    let redone = project_semantic_path_values(session.document(), key, undone, true)
        .unwrap_or_else(|| unreachable!("semantic Path redo is projectable"));
    assert_eq!(redone, smoothed);
}

#[test]
fn consecutive_path_transactions_follow_semantic_history_order() {
    let tile = MapTile::new(3200, 3200);
    let coord = MapPosition::new(tile, plane(0));
    let vertex = HeightVertexCoord::new(tile, plane(0));
    let mut session = EditorSession::new(MapDocument::from_parts([], [(vertex, 0)], []));
    let first = TerrainPathIntent {
        tile: osrs_core::coords::MapTile::new(3200, 3200),
        overlay: Some(7),
        shape: 1,
        rotation: 0,
    };
    let second = TerrainPathIntent {
        tile: osrs_core::coords::MapTile::new(3200, 3200),
        overlay: Some(8),
        shape: 2,
        rotation: 3,
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
    assert_eq!(session.document().height_at(vertex), Some(0));

    assert!(session.undo().is_ok());
    assert_eq!(
        session
            .document()
            .terrain_at(coord)
            .overlay
            .map(|overlay| overlay.id.get()),
        Some(7)
    );

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
}

#[test]
fn projection_with_height_covers_graded_vertices_outside_the_path_tiles() {
    let tile = MapTile::new(3201, 3200);
    let vertex = HeightVertexCoord::new(tile, plane(0));
    let document = MapDocument::from_parts([], [(vertex, -64)], []);
    let current = [(
        (0, 3201, 3200),
        TileValues {
            height: -16,
            ..TileValues::default()
        },
    )];
    let projected = plan_path_projection(&document, &current, true)
        .unwrap_or_else(|key| unreachable!("projection of {key:?} is representable"));
    assert_eq!(projected[0].2.height, -64, "the graded height is projected");
    // Without height the worker's own height is preserved (the old Path behaviour).
    let unchanged = plan_path_projection(&document, &current, false)
        .unwrap_or_else(|key| unreachable!("projection of {key:?} is representable"));
    assert_eq!(unchanged[0].2.height, -16);
}
