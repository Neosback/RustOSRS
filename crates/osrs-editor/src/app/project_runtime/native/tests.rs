use super::*;
use crate::app::edit_runtime::EditRuntime;
use crate::session::EditorSession;
use osrs_core::{
    coords::{MapTile, SourcePlane},
    ids::FloorUnderlayId,
};
use osrs_edit::{HeightVertexCoord, MapDocument, MapOp, MapPatch, MapPosition, TerrainTile};

fn plane() -> SourcePlane {
    SourcePlane::new(0).unwrap_or_else(|| unreachable!("plane zero is valid"))
}

fn vertex() -> HeightVertexCoord {
    HeightVertexCoord::new(MapTile::new(3200, 3200), plane())
}

fn position() -> MapPosition {
    MapPosition::new(MapTile::new(3200, 3200), plane())
}

fn terrain(underlay: u32) -> TerrainTile {
    TerrainTile {
        underlay: Some(FloorUnderlayId::new(underlay)),
        ..TerrainTile::default()
    }
}

fn empty_diff(region: RegionCoord) -> RegionDiff {
    RegionDiff {
        region,
        tiles: Vec::new(),
        heights: Vec::new(),
        objects_added: Vec::new(),
        objects_removed: Vec::new(),
    }
}

#[test]
fn source_change_reset_clears_source_bound_project_state() {
    let region = RegionCoord::new(50, 51);
    let mut state = ProjectRuntime::default();
    state.persistence.pending.insert(region, empty_diff(region));
    state.persistence.path = Some(PathBuf::from("old-project.json"));
    state.persistence.saved_edit_revision = 7;
    state.persistence.saving_revision = 6;
    state.persistence.quit_prompt = true;
    state.persistence.close_when_saved = true;
    state.persistence.discard_confirmed = true;
    state.persistence.transition_prompt = true;
    state.persistence.transition_after_save = true;
    state.persistence.transition = Some(LifecycleTransition::Project {
        path: PathBuf::from("next-project.json"),
        diffs: Vec::new(),
    });

    state.reset_for_source_change();

    assert!(state.persistence.pending.is_empty());
    assert!(state.persistence.path.is_none());
    assert!(state.persistence.save.is_none());
    assert_eq!(state.persistence.saved_edit_revision, 0);
    assert_eq!(state.persistence.saving_revision, 0);
    assert!(!state.persistence.quit_prompt);
    assert!(!state.persistence.close_when_saved);
    assert!(!state.persistence.discard_confirmed);
    assert!(state.persistence.transition.is_none());
    assert!(!state.persistence.transition_prompt);
    assert!(!state.persistence.transition_after_save);
}

#[test]
fn project_replacement_discards_existing_session_and_old_pending_before_queueing_new_diffs() {
    let vertex = vertex();
    let mut session = EditRuntime::new(EditorSession::new(MapDocument::from_parts(
        [],
        [(vertex, 0)],
        [],
    )));
    session
        .commit(MapPatch::new([MapOp::SetHeight {
            coord: vertex,
            before: 0,
            after: 64,
        }]))
        .unwrap_or_else(|error| unreachable!("valid edit: {error:?}"));
    assert_eq!(session.edit_revision(), 1);
    assert_eq!(session.document().height_at(vertex), Some(64));

    let old_region = RegionCoord::new(40, 41);
    let new_region = RegionCoord::new(50, 51);
    let mut project = ProjectRuntime {
        persistence: ProjectPersistence {
            path: Some(PathBuf::from("old-project.json")),
            saved_edit_revision: 1,
            ..ProjectPersistence::default()
        },
        ..ProjectRuntime::default()
    };
    project
        .persistence
        .pending
        .insert(old_region, empty_diff(old_region));

    project.prepare_replacement(
        &mut session,
        PathBuf::from("opened-project.json"),
        vec![empty_diff(new_region)],
    );

    assert_eq!(session.revision(), 0);
    assert_eq!(session.edit_revision(), 0);
    assert_eq!(session.document().height_at(vertex), None);
    assert!(!session.can_undo());
    assert!(!session.can_redo());
    assert_eq!(
        project.persistence.path,
        Some(PathBuf::from("opened-project.json"))
    );
    assert_eq!(project.persistence.saved_edit_revision, 0);
    assert!(!project.persistence.pending.contains_key(&old_region));
    assert_eq!(project.persistence.pending.len(), 1);
    assert!(project.persistence.pending.contains_key(&new_region));
}

#[test]
fn failed_project_region_application_keeps_pending_diff_retryable() {
    let region = RegionCoord::new(50, 50);
    let position = position();
    let diff = RegionDiff {
        region,
        // Both operations capture the same original `before` value. The second therefore conflicts
        // with the first during patch validation, giving this test a deterministic apply failure.
        tiles: vec![(position, terrain(1)), (position, terrain(2))],
        heights: Vec::new(),
        objects_added: Vec::new(),
        objects_removed: Vec::new(),
    };
    let mut project = ProjectRuntime::default();
    project.persistence.pending.insert(region, diff);
    let mut session = EditRuntime::default();

    let result = project.apply_pending_semantics(&mut session, region);

    assert!(result.is_err());
    assert!(project.persistence.pending.contains_key(&region));
    assert_eq!(session.revision(), 0);
    assert_eq!(session.edit_revision(), 0);
    assert_eq!(
        session.document().terrain_at(position),
        TerrainTile::default()
    );
}

#[test]
fn unsaved_transition_is_staged_without_replacing_current_project_state() {
    let current_region = RegionCoord::new(40, 41);
    let next_region = RegionCoord::new(50, 51);
    let mut project = ProjectRuntime {
        persistence: ProjectPersistence {
            path: Some(PathBuf::from("current-project.json")),
            saved_edit_revision: 3,
            ..ProjectPersistence::default()
        },
        ..ProjectRuntime::default()
    };
    project
        .persistence
        .pending
        .insert(current_region, empty_diff(current_region));

    let execute_now = project.stage_transition(
        LifecycleTransition::Project {
            path: PathBuf::from("next-project.json"),
            diffs: vec![empty_diff(next_region)],
        },
        true,
    );

    assert!(execute_now.is_none());
    assert_eq!(
        project.persistence.path,
        Some(PathBuf::from("current-project.json"))
    );
    assert!(project.persistence.pending.contains_key(&current_region));
    assert_eq!(project.persistence.pending.len(), 1);
    assert_eq!(project.persistence.saved_edit_revision, 3);
    assert!(project.persistence.transition_prompt);
    match project.persistence.transition.as_ref() {
        Some(LifecycleTransition::Project { path, diffs }) => {
            assert_eq!(path, &PathBuf::from("next-project.json"));
            assert_eq!(diffs.len(), 1);
            assert_eq!(diffs[0].region, next_region);
        }
        _ => unreachable!("dirty project replacement should be staged"),
    }
}

#[test]
fn saved_snapshot_does_not_mark_newer_edits_saved() {
    let mut project = ProjectRuntime::default();
    let (sender, receiver) = std::sync::mpsc::channel();
    project.persistence.saving_revision = 5;
    project.persistence.save = Some(receiver);
    assert!(sender.send(Ok((PathBuf::from("snapshot.json"), 2))).is_ok());
    assert!(matches!(project.poll_save(), Some(Ok((_, 2)))));
    assert!(!project.is_unsaved(5));
    assert!(project.is_unsaved(6));
    assert!(!project.save_running());
    assert!(project.poll_save().is_none());
}

#[test]
fn failed_save_preserves_pending_transition_and_prevents_close() {
    let mut project = ProjectRuntime::default();
    let transition = LifecycleTransition::Project {
        path: PathBuf::from("next.json"),
        diffs: Vec::new(),
    };
    assert!(project.stage_transition(transition, true).is_none());
    project.choose_transition(LifecycleChoice::Save);
    project.wait_transition_save(true);
    project.wait_quit_save(true);
    let (sender, receiver) = std::sync::mpsc::channel();
    project.persistence.save = Some(receiver);
    assert!(sender.send(Err("disk full".to_owned())).is_ok());
    assert_eq!(project.poll_save(), Some(Err("disk full".to_owned())));
    assert!(project.transition_description().is_some());
    assert!(project.ready_transition(true).is_none());
    assert!(!project.ready_to_close(true));
}

#[test]
fn disconnected_save_is_a_retryable_failure() {
    let mut project = ProjectRuntime::default();
    let (sender, receiver) = std::sync::mpsc::channel();
    project.persistence.save = Some(receiver);
    drop(sender);
    assert!(matches!(project.poll_save(), Some(Err(_))));
    assert!(!project.save_running());
    assert_eq!(project.persistence.saved_edit_revision, 0);
}

#[test]
fn save_then_transition_rechecks_edits_and_cancel_discards_only_the_request() {
    let mut project = ProjectRuntime::default();
    let transition = LifecycleTransition::Project {
        path: PathBuf::from("next.json"),
        diffs: Vec::new(),
    };
    assert!(project.stage_transition(transition, true).is_none());
    project.choose_transition(LifecycleChoice::Save);
    project.wait_transition_save(true);
    // A new edit since the saved snapshot keeps the replacement waiting for a fresh decision.
    assert!(project.ready_transition(true).is_none());
    assert!(project.transition_description().is_some());
    project.choose_transition(LifecycleChoice::Cancel);
    assert!(project.transition_description().is_none());
    assert!(project.persistence.transition.is_none());
    assert!(!project.discard_confirmed());
}

#[test]
fn pending_region_is_retained_until_projection_acknowledges_it() {
    let mut project = ProjectRuntime::default();
    let region = RegionCoord::new(50, 50);
    let diff = RegionDiff {
        tiles: vec![(position(), terrain(3))],
        ..empty_diff(region)
    };
    project.persistence.pending.insert(region, diff);
    let mut session = EditRuntime::default();
    assert!(matches!(
        project.apply_pending_semantics(&mut session, region),
        Ok(Some(_))
    ));
    assert!(!project.is_unsaved(session.edit_revision()));
    let revision = session.edit_revision();
    assert!(project.has_pending_exports());
    // Retrying the semantic no-op must still return projection work.
    assert!(matches!(
        project.apply_pending_semantics(&mut session, region),
        Ok(Some(_))
    ));
    assert_eq!(session.edit_revision(), revision);
    project.complete_region_projection(region);
    assert!(!project.has_pending_exports());
    assert!(matches!(
        project.apply_pending_semantics(&mut session, region),
        Ok(None)
    ));
}
