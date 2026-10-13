use super::*;
use osrs_core::{
    coords::{MapTile, SourcePlane},
    ids::FloorUnderlayId,
};
use osrs_edit::{HeightVertexCoord, MapOp, MapPatch, MapPosition, TerrainTile};

fn plane() -> SourcePlane {
    SourcePlane::new(0).unwrap_or_else(|| unreachable!("plane zero is valid"))
}

fn vertex() -> HeightVertexCoord {
    HeightVertexCoord::new(MapTile::new(3200, 3200), plane())
}

fn position() -> MapPosition {
    MapPosition::new(MapTile::new(3200, 3200), plane())
}

fn region() -> RegionCoord {
    RegionCoord::new(50, 50)
}

fn session(height: i32) -> EditorSession {
    EditorSession::new(MapDocument::from_parts([], [(vertex(), height)], []))
}

fn snapshot(height: i32, underlay: u32) -> MapRegionSnapshot {
    MapRegionSnapshot::from_parts(
        region(),
        [(
            position(),
            TerrainTile {
                underlay: Some(FloorUnderlayId::new(underlay)),
                ..TerrainTile::default()
            },
        )],
        [(vertex(), height)],
        [],
    )
    .unwrap_or_else(|error| unreachable!("valid snapshot: {error:?}"))
}

fn height_patch(before: i32, after: i32) -> MapPatch {
    MapPatch::new([MapOp::SetHeight {
        coord: vertex(),
        before,
        after,
    }])
}

#[test]
fn successful_commit_updates_document_and_revision_once() {
    let mut session = session(0);

    let commit = session
        .commit(height_patch(0, 64))
        .unwrap_or_else(|error| unreachable!("valid commit: {error:?}"))
        .unwrap_or_else(|| unreachable!("non-empty patch creates a commit"));

    assert_eq!(session.document().height_at(vertex()), Some(64));
    assert_eq!(session.revision(), 1);
    assert_eq!(commit.revision, 1);
    assert_eq!(
        commit.impact,
        ChangeImpact::TERRAIN_GEOMETRY
            | ChangeImpact::NORMALS
            | ChangeImpact::LIGHTING
            | ChangeImpact::LOCATIONS
            | ChangeImpact::BOUNDS
    );
    assert!(session.can_undo());
    assert!(!session.can_redo());
}

#[test]
fn clean_region_ingestion_is_idempotent_and_replacement_advances_revision() {
    let mut session = EditorSession::default();
    let first = snapshot(-64, 7);

    assert_eq!(session.ingest_region(&first), RegionSyncOutcome::Changed);
    assert_eq!(session.revision(), 1);
    assert_eq!(session.document().height_at(vertex()), Some(-64));
    assert_eq!(
        session.document().terrain_at(position()).underlay,
        Some(FloorUnderlayId::new(7))
    );

    assert_eq!(session.ingest_region(&first), RegionSyncOutcome::Unchanged);
    assert_eq!(session.revision(), 1);

    let replacement = snapshot(-128, 8);
    assert_eq!(
        session.ingest_region(&replacement),
        RegionSyncOutcome::Changed
    );
    assert_eq!(session.revision(), 2);
    assert_eq!(session.document().height_at(vertex()), Some(-128));
    assert_eq!(
        session.document().terrain_at(position()).underlay,
        Some(FloorUnderlayId::new(8))
    );
}

#[test]
fn unloading_clean_region_removes_it_without_touching_history() {
    let mut session = EditorSession::default();
    assert_eq!(
        session.ingest_region(&snapshot(-64, 7)),
        RegionSyncOutcome::Changed
    );
    assert_eq!(session.unload_region(region()), RegionSyncOutcome::Changed);
    assert_eq!(session.revision(), 2);
    assert_eq!(session.document().height_at(vertex()), None);
    assert_eq!(
        session.document().terrain_at(position()),
        TerrainTile::default()
    );
    assert!(!session.can_undo());
    assert!(!session.can_redo());
    assert_eq!(
        session.unload_region(region()),
        RegionSyncOutcome::Unchanged
    );
}

#[test]
fn edited_region_is_not_overwritten_or_evicted_by_streaming() {
    let mut session = EditorSession::default();
    assert_eq!(
        session.ingest_region(&snapshot(0, 7)),
        RegionSyncOutcome::Changed
    );
    assert!(session.commit(height_patch(0, 64)).is_ok());
    assert_eq!(session.revision(), 2);

    assert_eq!(
        session.ingest_region(&snapshot(128, 8)),
        RegionSyncOutcome::PreservedDirty
    );
    assert_eq!(session.document().height_at(vertex()), Some(64));
    assert_eq!(
        session.document().terrain_at(position()).underlay,
        Some(FloorUnderlayId::new(7))
    );
    assert_eq!(session.revision(), 2);

    assert_eq!(
        session.unload_region(region()),
        RegionSyncOutcome::PreservedDirty
    );
    assert_eq!(session.document().height_at(vertex()), Some(64));
    assert_eq!(session.revision(), 2);
}

#[test]
fn reset_clears_loaded_state_history_and_revision() {
    let mut session = EditorSession::default();
    assert_eq!(
        session.ingest_region(&snapshot(0, 7)),
        RegionSyncOutcome::Changed
    );
    assert!(session.commit(height_patch(0, 64)).is_ok());
    session.reset();

    assert_eq!(session.revision(), 0);
    assert_eq!(session.document().height_at(vertex()), None);
    assert_eq!(
        session.document().terrain_at(position()),
        TerrainTile::default()
    );
    assert!(!session.can_undo());
    assert!(!session.can_redo());
}

#[test]
fn rejected_commit_changes_neither_document_history_nor_revision() {
    let mut session = session(0);

    assert!(session.commit(height_patch(999, 64)).is_err());
    assert_eq!(session.document().height_at(vertex()), Some(0));
    assert_eq!(session.revision(), 0);
    assert!(!session.can_undo());
    assert!(!session.can_redo());
}

#[test]
fn undo_and_redo_each_advance_revision_after_success() {
    let mut session = session(0);
    assert!(session.commit(height_patch(0, 64)).is_ok());

    let undo = session
        .undo()
        .unwrap_or_else(|error| unreachable!("valid undo: {error:?}"))
        .unwrap_or_else(|| unreachable!("undo entry exists"));
    assert_eq!(session.document().height_at(vertex()), Some(0));
    assert_eq!(undo.revision, 2);
    assert_eq!(session.revision(), 2);
    assert!(session.can_redo());

    let redo = session
        .redo()
        .unwrap_or_else(|error| unreachable!("valid redo: {error:?}"))
        .unwrap_or_else(|| unreachable!("redo entry exists"));
    assert_eq!(session.document().height_at(vertex()), Some(64));
    assert_eq!(redo.revision, 3);
    assert_eq!(session.revision(), 3);
}

#[test]
fn empty_commit_is_not_a_revision_and_preserves_redo() {
    let mut session = session(0);
    assert!(session.commit(height_patch(0, 64)).is_ok());
    assert!(session.undo().is_ok());
    assert!(session.can_redo());
    let revision = session.revision();

    assert_eq!(session.commit(MapPatch::default()), Ok(None));
    assert_eq!(session.revision(), revision);
    assert!(session.can_redo());
}

#[test]
fn empty_undo_and_redo_do_not_advance_revision() {
    let mut session = session(0);

    assert_eq!(session.undo(), Ok(None));
    assert_eq!(session.redo(), Ok(None));
    assert_eq!(session.revision(), 0);
}

#[test]
fn failed_undo_does_not_advance_revision_or_corrupt_history() {
    let mut session = session(0);
    assert!(session.commit(height_patch(0, 64)).is_ok());
    assert_eq!(session.revision(), 1);

    let out_of_band = height_patch(64, 128);
    assert!(session.document.apply_patch(&out_of_band).is_ok());

    assert!(session.undo().is_err());
    assert_eq!(session.document().height_at(vertex()), Some(128));
    assert_eq!(session.revision(), 1);
    assert!(session.can_undo());
    assert!(!session.can_redo());
}

fn paint_patch() -> MapPatch {
    MapPatch::new(vec![MapOp::SetTerrain {
        coord: position(),
        before: TerrainTile::default(),
        after: TerrainTile {
            underlay: Some(FloorUnderlayId::new(9)),
            ..TerrainTile::default()
        },
    }])
}

#[test]
fn untracked_apply_changes_the_document_without_history_and_protects_the_region() {
    let mut session = session(0);
    session.ingest_region(&snapshot(0, 3));
    let revision = session.revision();
    let patch = MapPatch::new(vec![MapOp::SetTerrain {
        coord: position(),
        before: session.document().terrain_at(position()),
        after: TerrainTile {
            underlay: Some(FloorUnderlayId::new(9)),
            ..TerrainTile::default()
        },
    }]);

    assert_eq!(session.apply_untracked(&patch), Ok(true));
    assert!(!session.can_undo(), "an opened project is not an undo step");
    assert_eq!(session.revision(), revision + 1);
    assert_eq!(
        session.document().terrain_at(position()).underlay,
        Some(FloorUnderlayId::new(9))
    );
    // The region is now edit-owned: streaming can no longer replace it.
    assert_eq!(
        session.ingest_region(&snapshot(0, 3)),
        RegionSyncOutcome::PreservedDirty
    );
    assert_eq!(session.dirty_regions().collect::<Vec<_>>(), vec![region()]);
}

#[test]
fn untracked_apply_of_an_empty_or_failing_patch_changes_nothing() {
    let mut session = session(0);
    assert_eq!(
        session.apply_untracked(&MapPatch::new(Vec::new())),
        Ok(false)
    );
    assert_eq!(session.revision(), 0);

    let stale = MapPatch::new(vec![MapOp::SetTerrain {
        coord: position(),
        before: TerrainTile {
            underlay: Some(FloorUnderlayId::new(1)),
            ..TerrainTile::default()
        },
        after: TerrainTile::default(),
    }]);
    assert!(session.apply_untracked(&stale).is_err());
    assert_eq!(session.revision(), 0);
    assert_eq!(session.dirty_regions().count(), 0);
}

#[test]
fn clearing_history_forgets_undo_but_not_the_document() {
    let mut session = session(0);
    session
        .commit(paint_patch())
        .unwrap_or_else(|e| unreachable!("{e:?}"));
    assert!(session.can_undo());
    session.clear_history();
    assert!(!session.can_undo());
    assert_eq!(
        session.document().terrain_at(position()).underlay,
        Some(FloorUnderlayId::new(9))
    );
}
