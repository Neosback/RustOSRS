use super::*;

#[test]
fn a_source_or_project_replacement_rejects_even_a_matching_edit_revision() {
    let mut runtime = ExportRuntime::default();
    let old = runtime.stamp(0);
    runtime.reset_source();
    let current = runtime.stamp(0);
    assert_ne!(old, current);
    assert!(!runtime.accepts(old));
    assert!(runtime.accepts(current));
}

#[test]
fn a_superseded_job_cannot_install_a_result() {
    let mut runtime = ExportRuntime::default();
    let old = runtime.stamp(3);
    let current = runtime.stamp(3);
    let (tx, rx) = std::sync::mpsc::channel();
    tx.send(ExportEvent::Published(Err("stale failure".into())))
        .unwrap_or_else(|e| unreachable!("{e}"));
    runtime.job = Some(ExportJob {
        stamp: old,
        receiver: rx,
    });
    runtime.poll();
    assert!(runtime.failure.is_none());
    assert_eq!(runtime.active, Some(current));
}

#[test]
fn worker_failure_is_persistent_and_does_not_claim_success() {
    let mut runtime = ExportRuntime::default();
    let stamp = runtime.stamp(4);
    let (tx, rx) = std::sync::mpsc::channel();
    tx.send(ExportEvent::Published(Err("disk full".into())))
        .unwrap_or_else(|e| unreachable!("{e}"));
    runtime.job = Some(ExportJob {
        stamp,
        receiver: rx,
    });
    runtime.poll();
    runtime.poll();
    assert_eq!(runtime.failure.as_deref(), Some("disk full"));
    assert!(runtime.report.is_none());
}

#[test]
fn disconnected_worker_has_an_actionable_failure() {
    let mut runtime = ExportRuntime::default();
    let stamp = runtime.stamp(0);
    let (tx, rx) = std::sync::mpsc::channel();
    drop(tx);
    runtime.job = Some(ExportJob {
        stamp,
        receiver: rx,
    });
    runtime.poll();
    assert!(
        runtime
            .failure
            .as_deref()
            .is_some_and(|s| s.contains("retry"))
    );
}
