//! Real session/history behavior across map and appearance transactions.
#![allow(clippy::unwrap_used)]
use super::*;
use osrs_core::coords::{MapTile, SourcePlane};
use osrs_edit::{HeightVertexCoord, MapDocument, MapOp, MapPatch};
use osrs_hd::theme::{ThemePack, ThemePatch};

#[test]
fn mixed_history_preserves_map_revision_and_restores_appearance_exactly() {
    let vertex = HeightVertexCoord::new(MapTile::new(3200, 3200), SourcePlane::new(0).unwrap());
    let mut session = EditorSession::new(MapDocument::from_parts([], [(vertex, 0)], []));
    session
        .commit(MapPatch::new([MapOp::SetHeight {
            coord: vertex,
            before: 0,
            after: -8,
        }]))
        .unwrap();
    let npc_patch = osrs_edit::plan_replace_scenario(
        session.scenario(),
        Some(osrs_core::scenario::ScenarioDefinition {
            version: 1,
            build: 240,
            client_fingerprint: "a".repeat(64),
            seed: 7,
            spawns: Vec::new(),
        }),
    )
    .unwrap();
    let (_, checkpoint) = session
        .commit_scenario_checkpointed(npc_patch)
        .unwrap()
        .unwrap();
    session.finalize_checkpoint(checkpoint).unwrap();
    let original = session.appearance().clone();
    let mut changed = original.clone();
    changed.definition.name = "Dusk".into();
    let patch = ThemePatch::plan(&original, changed.clone()).unwrap();
    let (_, checkpoint) = session
        .commit_appearance_checkpointed(patch)
        .unwrap()
        .unwrap();
    session.finalize_checkpoint(checkpoint).unwrap();
    assert_eq!(session.map_revision(), 1);
    assert_eq!(session.appearance(), &changed);
    session.undo().unwrap();
    assert_eq!(session.appearance(), &original);
    assert_eq!(session.document().height_at(vertex), Some(-8));
    assert!(session.scenario().definition().is_some());
    session.undo().unwrap();
    assert!(session.scenario().definition().is_none());
    session.undo().unwrap();
    assert_eq!(session.document().height_at(vertex), Some(0));
    session.redo().unwrap();
    session.redo().unwrap();
    session.redo().unwrap();
    assert!(session.scenario().definition().is_some());
    assert_eq!(session.appearance(), &changed);
    assert_eq!(session.map_revision(), 3);
}
#[test]
fn failed_projection_and_empty_theme_gesture_preserve_redo_and_dirty_regions() {
    let mut session = EditorSession::default();
    let mut changed = ThemePack::default();
    changed.definition.name = "Night".into();
    let patch = ThemePatch::plan(session.appearance(), changed).unwrap();
    let before = session.clone();
    let (_, checkpoint) = session
        .commit_appearance_checkpointed(patch.clone())
        .unwrap()
        .unwrap();
    session.restore_checkpoint(checkpoint).unwrap();
    assert_eq!(session, before);
    let (_, checkpoint) = session
        .commit_appearance_checkpointed(patch)
        .unwrap()
        .unwrap();
    session.finalize_checkpoint(checkpoint).unwrap();
    session.undo().unwrap();
    let empty = ThemePatch::plan(session.appearance(), session.appearance().clone()).unwrap();
    assert!(
        session
            .commit_appearance_checkpointed(empty)
            .unwrap()
            .is_none()
    );
    assert!(session.can_redo());
    assert_eq!(session.dirty_regions().count(), 0);
}
