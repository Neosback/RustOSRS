use super::*;
use osrs_core::coords::{MapTile, SourcePlane};
use osrs_edit::{HeightVertexCoord, MapDocument, MapOp, MapPatch};

fn vertex() -> HeightVertexCoord {
    let plane = SourcePlane::new(0).unwrap_or_else(|| unreachable!("plane zero is valid"));
    HeightVertexCoord::new(MapTile::new(3200, 3200), plane)
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
fn project_replacement_discards_existing_semantic_session_before_queueing_diffs() {
    let vertex = vertex();
    let mut session = EditorSession::new(MapDocument::from_parts([], [(vertex, 0)], []));
    session
        .commit(MapPatch::new([MapOp::SetHeight {
            coord: vertex,
            before: 0,
            after: 64,
        }]))
        .unwrap_or_else(|error| unreachable!("valid edit: {error:?}"));
    assert_eq!(session.edit_revision(), 1);
    assert_eq!(session.document().height_at(vertex), Some(64));

    let region = RegionCoord::new(50, 51);
    let mut project = ProjectState {
        path: Some(PathBuf::from("old-project.json")),
        saved_edit_revision: 1,
        ..ProjectState::default()
    };

    prepare_project_replacement(
        &mut project,
        &mut session,
        PathBuf::from("opened-project.json"),
        vec![empty_diff(region)],
    );

    assert_eq!(session.revision(), 0);
    assert_eq!(session.edit_revision(), 0);
    assert_eq!(session.document().height_at(vertex), None);
    assert!(!session.can_undo());
    assert!(!session.can_redo());
    assert_eq!(project.path, Some(PathBuf::from("opened-project.json")));
    assert_eq!(project.saved_edit_revision, 0);
    assert!(project.pending.contains_key(&region));
}
