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
    let mut state = ProjectState::default();
    state.pending.insert(region, empty_diff(region));
    state.path = Some(PathBuf::from("old-project.json"));
    state.saved_edit_revision = 7;
    state.saving_revision = 6;
    state.quit_prompt = true;
    state.close_when_saved = true;
    state.discard_confirmed = true;
    state.transition_prompt = true;
    state.transition_after_save = true;
    state.transition = Some(lifecycle::LifecycleTransition::Project {
        path: PathBuf::from("next-project.json"),
        diffs: Vec::new(),
    });

    state.reset_for_source_change();

    assert!(state.pending.is_empty());
    assert!(state.path.is_none());
    assert!(state.save.is_none());
    assert_eq!(state.saved_edit_revision, 0);
    assert_eq!(state.saving_revision, 0);
    assert!(!state.quit_prompt);
    assert!(!state.close_when_saved);
    assert!(!state.discard_confirmed);
    assert!(state.transition.is_none());
    assert!(!state.transition_prompt);
    assert!(!state.transition_after_save);
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
    let mut project = ProjectState {
        path: Some(PathBuf::from("old-project.json")),
        saved_edit_revision: 1,
        ..ProjectState::default()
    };
    project.pending.insert(old_region, empty_diff(old_region));

    prepare_project_replacement(
        &mut project,
        &mut session,
        PathBuf::from("opened-project.json"),
        vec![empty_diff(new_region)],
    );

    assert_eq!(session.revision(), 0);
    assert_eq!(session.edit_revision(), 0);
    assert_eq!(session.document().height_at(vertex), None);
    assert!(!session.can_undo());
    assert!(!session.can_redo());
    assert_eq!(project.path, Some(PathBuf::from("opened-project.json")));
    assert_eq!(project.saved_edit_revision, 0);
    assert!(!project.pending.contains_key(&old_region));
    assert_eq!(project.pending.len(), 1);
    assert!(project.pending.contains_key(&new_region));
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
    let mut project = ProjectState::default();
    project.pending.insert(region, diff);
    let mut session = EditRuntime::default();

    let result = apply_pending_project_semantics(&project, &mut session, region);

    assert!(result.is_err());
    assert!(project.pending.contains_key(&region));
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
    let mut project = ProjectState {
        path: Some(PathBuf::from("current-project.json")),
        saved_edit_revision: 3,
        ..ProjectState::default()
    };
    project
        .pending
        .insert(current_region, empty_diff(current_region));

    let execute_now = project.stage_transition(
        lifecycle::LifecycleTransition::Project {
            path: PathBuf::from("next-project.json"),
            diffs: vec![empty_diff(next_region)],
        },
        true,
    );

    assert!(execute_now.is_none());
    assert_eq!(project.path, Some(PathBuf::from("current-project.json")));
    assert!(project.pending.contains_key(&current_region));
    assert_eq!(project.pending.len(), 1);
    assert_eq!(project.saved_edit_revision, 3);
    assert!(project.transition_prompt);
    match project.transition.as_ref() {
        Some(lifecycle::LifecycleTransition::Project { path, diffs }) => {
            assert_eq!(path, &PathBuf::from("next-project.json"));
            assert_eq!(diffs.len(), 1);
            assert_eq!(diffs[0].region, next_region);
        }
        _ => unreachable!("dirty project replacement should be staged"),
    }
}
