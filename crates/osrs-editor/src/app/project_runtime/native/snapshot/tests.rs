use super::*;
use crate::session::{EditorSession, authored_snapshot};
use std::time::{Duration, Instant};

fn finish(state: &mut ProjectRuntime) -> Result<PathBuf, String> {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(result) = state.poll_authored_snapshot() {
            return result;
        }
        assert!(Instant::now() < deadline, "snapshot worker stalled");
        std::thread::yield_now();
    }
}

#[test]
fn authored_snapshot_export_is_bounded_and_survives_project_replacement_without_marking_edits_saved()
 {
    let mut session = EditRuntime::new(authored_snapshot::tests::fixture());
    let expected = session
        .freeze_authored_snapshot()
        .encode()
        .unwrap_or_else(|e| panic!("{e}"));
    let mut state = ProjectRuntime::default();
    state.persistence.saved_edit_revision = 77;
    let dir = crate::export_flow::test_support::temp("authored-snapshot");
    let out = dir.join("snapshot.json");
    state
        .start_authored_snapshot(out.clone(), &session)
        .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(state.persistence.saved_edit_revision, 77);
    assert!(
        state
            .start_authored_snapshot(dir.join("second.json"), &session)
            .is_err()
    );
    state.replace_source(
        CacheSource::Dir(PathBuf::from("replacement-cache")),
        PathBuf::from("replacement-cache"),
    );
    state.prepare_replacement(
        &mut session,
        PathBuf::from("replacement-project.json"),
        vec![],
    );
    assert!(state.authored_snapshot_running());
    assert!(
        state
            .start_authored_snapshot(dir.join("third.json"), &session)
            .is_err()
    );
    assert_eq!(finish(&mut state).unwrap_or_else(|e| panic!("{e}")), out);
    let text = std::fs::read_to_string(&out).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(text, expected);
    assert_eq!(state.persistence.saved_edit_revision, 0);
    let restored =
        EditorSession::from_authored_snapshot_json_v1(&text).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(
        restored
            .scenario()
            .definition()
            .unwrap_or_else(|| panic!("scenario"))
            .spawns
            .len(),
        3
    );
    assert!(authored_snapshot::verify(&text).is_ok());
    assert!(session.scenario().definition().is_none());
    state
        .start_authored_snapshot(out.clone(), &session)
        .unwrap_or_else(|e| panic!("{e}"));
    assert!(finish(&mut state).is_err());
    assert_eq!(
        std::fs::read_to_string(&out).unwrap_or_else(|e| panic!("{e}")),
        expected
    );
    assert_eq!(
        std::fs::read_dir(&dir)
            .unwrap_or_else(|e| panic!("{e}"))
            .count(),
        1
    );
    std::fs::remove_dir_all(dir).unwrap_or_else(|e| panic!("{e}"));
}

#[test]
fn disconnected_authored_snapshot_slot_reports_once_then_allows_explicit_retry() {
    let mut state = ProjectRuntime::default();
    let (send, receive) = mpsc::sync_channel(1);
    state.snapshot = Some(SnapshotJob { receive });
    drop(send);
    assert!(
        state
            .poll_authored_snapshot()
            .unwrap_or_else(|| panic!("result"))
            .is_err()
    );
    assert!(!state.authored_snapshot_running());
    assert!(state.poll_authored_snapshot().is_none());
    let session = EditRuntime::new(EditorSession::default());
    let dir = crate::export_flow::test_support::temp("authored-snapshot-retry");
    let out = dir.join("retry.json");
    state
        .start_authored_snapshot(out, &session)
        .unwrap_or_else(|e| panic!("{e}"));
    assert!(finish(&mut state).is_ok());
    std::fs::remove_dir_all(dir).unwrap_or_else(|e| panic!("{e}"));
}
