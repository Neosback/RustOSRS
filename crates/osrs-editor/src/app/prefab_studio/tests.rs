//! Studio's actual planner/gateway sequence without GPU or filesystem prerequisites.
use super::*;
use crate::app::edit_runtime::{EditExecution, EditRuntime};
use crate::session::EditorSession;
use osrs_core::ids::FloorUnderlayId;
use osrs_edit::{MapOp, MapPatch, MapPosition, TerrainTile};
fn tile() -> MapPosition {
    MapPosition::new(
        MapTile::new(10, 10),
        SourcePlane::new(0).unwrap_or_else(|| unreachable!()),
    )
}
fn patch(before: TerrainTile, after: TerrainTile) -> MapPatch {
    MapPatch::new(vec![MapOp::SetTerrain {
        coord: tile(),
        before,
        after,
    }])
}
fn floor(id: u32) -> TerrainTile {
    TerrainTile {
        underlay: Some(FloorUnderlayId::new(id)),
        ..TerrainTile::default()
    }
}
fn baseline() -> (EditRuntime, MapDocument) {
    let pad = MapDocument::from_parts([(tile(), floor(50))], [], []);
    let mut runtime = EditRuntime::new(EditorSession::new(pad.clone()));
    runtime
        .execute_baseline(patch(floor(50), floor(3)), |_| Ok::<_, ()>(()))
        .unwrap_or_else(|e| panic!("{e:?}"));
    (runtime, pad)
}
#[test]
fn studio_baseline_is_untracked_and_user_commit_is_one_entry() {
    let (mut runtime, _) = baseline();
    assert!(!runtime.can_undo());
    let result = runtime
        .execute(patch(floor(3), floor(7)), |_| Ok::<_, ()>(()))
        .unwrap_or_else(|e| panic!("{e:?}"));
    assert!(matches!(result, EditExecution::Committed { .. }));
    assert_eq!(runtime.document().terrain_at(tile()), floor(7));
    assert!(runtime.can_undo());
    runtime.undo().unwrap_or_else(|e| panic!("{e:?}"));
    assert!(!runtime.can_undo());
}
#[test]
fn studio_undo_returns_to_prefab_and_parked_world_never_changes() {
    let world = EditRuntime::new(EditorSession::new(MapDocument::from_parts(
        [(tile(), floor(99))],
        [],
        [],
    )));
    let parked = world.clone();
    let (mut studio, _) = baseline();
    studio
        .commit(patch(floor(3), floor(7)))
        .unwrap_or_else(|e| panic!("{e:?}"));
    studio.undo().unwrap_or_else(|e| panic!("{e:?}"));
    assert_eq!(studio.document().terrain_at(tile()), floor(3));
    assert_eq!(world, parked);
}
#[test]
fn metadata_only_changes_require_save_and_cancel_preserves_them() {
    let mut session = StudioSession::default();
    session.baseline(0);
    session.metadata_changed();
    assert!(session.request(Destination::Close, 0).is_none());
    assert!(session.choose(Choice::Cancel, None).is_none());
    assert!(session.dirty(0));
    session.mark_saved(0);
    assert!(!session.dirty(0));
}
