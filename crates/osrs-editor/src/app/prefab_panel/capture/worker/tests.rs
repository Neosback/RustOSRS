//! Controlled physical work, snapshot lifetime and real membership results through capture tickets.
use super::super::{CaptureReview, membership, tests::fixture};
use super::*;
use std::{
    sync::mpsc::RecvTimeoutError,
    time::{Duration, Instant},
};
const WAIT: Duration = Duration::from_secs(5);

fn recv<T>(rx: &Receiver<T>) -> T {
    rx.recv_timeout(WAIT)
        .unwrap_or_else(|e| panic!("capture handshake: {e}"))
}
fn until(mut predicate: impl FnMut() -> bool) {
    let deadline = Instant::now() + WAIT;
    while !predicate() {
        assert!(Instant::now() < deadline, "capture state did not settle");
        std::thread::yield_now();
    }
}
fn review(revision: u64, source: u64) -> Review {
    let (_, mut r) = fixture();
    r.revision = revision;
    r.source = source;
    membership::refresh(&mut r);
    r
}
fn poll(state: &mut CaptureReview, revision: u64, source: u64) -> Result<Review, String> {
    let mut result = None;
    until(|| {
        result = state.poll_response(revision, source);
        result.is_some()
    });
    result.unwrap_or_else(|| unreachable!())
}

#[test]
fn replacement_burst_retains_only_latest_snapshot_and_one_physical_analysis() {
    let mut state = CaptureReview::default();
    let (started, starts) = mpsc::channel();
    let (release, blocked) = mpsc::channel();
    let first = started.clone();
    state.start(1, 2, move || {
        first
            .send((0, std::thread::current().id()))
            .unwrap_or_else(|e| panic!("{e}"));
        recv(&blocked);
        Ok(review(1, 2))
    });
    let (_, thread) = recv(&starts);
    let mut weak = Vec::new();
    for i in 1..=64 {
        let snapshot = Arc::new(());
        weak.push(Arc::downgrade(&snapshot));
        let started = started.clone();
        state.start(i, 2, move || {
            let _snapshot = snapshot;
            started
                .send((i, std::thread::current().id()))
                .unwrap_or_else(|e| panic!("{e}"));
            Ok(review(i, 2))
        });
        assert_eq!(weak.iter().filter(|w| w.upgrade().is_some()).count(), 1);
    }
    assert!(matches!(starts.try_recv(), Err(mpsc::TryRecvError::Empty)));
    release.send(()).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(recv(&starts), (64, thread));
    let result = poll(&mut state, 64, 2).unwrap_or_else(|e| panic!("{e}"));
    let expected = review(64, 2);
    assert_eq!(result.description, expected.description);
    assert_eq!(result.objects, expected.objects);
    assert_eq!(result.included, expected.included);
    until(|| weak.iter().all(|w| w.upgrade().is_none()));
    assert!(state.pending.is_none());
}

#[test]
fn clear_and_semantic_or_source_replacement_reuse_draining_worker() {
    let mut state = CaptureReview {
        refining: true,
        open: true,
        ..Default::default()
    };
    let (started, starts) = mpsc::channel();
    let (release, blocked) = mpsc::channel();
    let first = started.clone();
    state.start(4, 7, move || {
        first
            .send(std::thread::current().id())
            .unwrap_or_else(|e| panic!("{e}"));
        recv(&blocked);
        Ok(review(4, 7))
    });
    let thread = recv(&starts);
    state.clear();
    assert!(!state.refining && !state.open && state.pending.is_none() && state.review.is_none());
    for (revision, source) in [(4, 7), (5, 7), (5, 8)] {
        let snapshot = Arc::new(());
        let weak = Arc::downgrade(&snapshot);
        state.start(revision, source, move || {
            let _snapshot = snapshot;
            panic!("superseded queued capture must never start")
        });
        assert!(!state.is_current(revision + 1, source));
        assert!(poll(&mut state, revision + 1, source).is_err());
        assert!(weak.upgrade().is_none());
        assert!(state.pending.is_none());
    }
    state.start(5, 9, move || {
        started
            .send(std::thread::current().id())
            .unwrap_or_else(|e| panic!("{e}"));
        Ok(review(5, 9))
    });
    assert!(matches!(starts.try_recv(), Err(mpsc::TryRecvError::Empty)));
    release.send(()).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(recv(&starts), thread);
    assert!(poll(&mut state, 5, 9).is_ok());
}

#[test]
fn unread_completed_review_is_released_by_clear_and_owner_stays_idle() {
    let mut state = CaptureReview::default();
    let result = review(3, 4);
    let weak = Arc::downgrade(&result.inputs);
    state.start(3, 4, move || Ok(result));
    let worker = state.worker.as_ref().unwrap_or_else(|| unreachable!());
    until(|| {
        let slots = worker.shared.lock();
        slots.active.is_none() && slots.pending.is_none()
    });
    assert!(
        weak.upgrade().is_some(),
        "current unread response retains exactly its review"
    );
    state.clear();
    assert!(weak.upgrade().is_none());
    assert!(state.worker.as_ref().is_some_and(|w| !w.is_finished()));
    assert!(state.poll_response(3, 4).is_none());
}

#[test]
fn obsolete_ticket_cannot_remove_new_equal_input_work() {
    let mut worker = Worker::new();
    let (started, starts) = mpsc::channel();
    let (release, blocked) = mpsc::channel();
    let active = worker.start(1, 2, move || {
        started.send(()).unwrap_or_else(|e| panic!("{e}"));
        recv(&blocked);
        Ok(review(1, 2))
    });
    recv(&starts);
    let obsolete = worker.start(1, 2, || panic!("replaced queue ran"));
    let current = worker.start(1, 2, || Ok(review(1, 2)));
    drop(obsolete);
    drop(active);
    release.send(()).unwrap_or_else(|e| panic!("{e}"));
    let result = recv(&current.receive).unwrap_or_else(|e| panic!("{e}"));
    assert!(result.is_current(1, 2));
}

#[test]
fn owner_shutdown_drops_queued_snapshots_without_joining_active_work() {
    let mut worker = Worker::new();
    let shared = Arc::downgrade(&worker.shared);
    let (started, starts) = mpsc::channel();
    let (release, blocked) = mpsc::channel();
    let active = worker.start(1, 2, move || {
        started.send(()).unwrap_or_else(|e| panic!("{e}"));
        recv(&blocked);
        Ok(review(1, 2))
    });
    recv(&starts);
    let snapshot = Arc::new(());
    let weak = Arc::downgrade(&snapshot);
    let pending = worker.start(1, 2, move || {
        let _snapshot = snapshot;
        panic!("closed queue ran")
    });
    drop(worker);
    assert!(weak.upgrade().is_none());
    assert!(matches!(
        pending.receive.recv_timeout(WAIT),
        Err(RecvTimeoutError::Disconnected)
    ));
    release.send(()).unwrap_or_else(|e| panic!("{e}"));
    assert!(matches!(
        active.receive.recv_timeout(WAIT),
        Err(RecvTimeoutError::Disconnected)
    ));
    until(|| shared.upgrade().is_none());
}

#[test]
fn panic_closes_queued_ticket_and_explicit_reselection_retries_only_terminal_worker() {
    let mut state = CaptureReview {
        refining: true,
        open: true,
        ..Default::default()
    };
    let (started, starts) = mpsc::channel();
    let (release, blocked) = mpsc::channel();
    state.start(1, 2, move || {
        started.send(()).unwrap_or_else(|e| panic!("{e}"));
        recv(&blocked);
        panic!("controlled capture panic")
    });
    recv(&starts);
    let snapshot = Arc::new(());
    let weak = Arc::downgrade(&snapshot);
    state.start(1, 2, move || {
        let _snapshot = snapshot;
        panic!("panicked worker queue ran")
    });
    release.send(()).unwrap_or_else(|e| panic!("{e}"));
    assert!(poll(&mut state, 1, 2).is_err());
    until(|| state.worker.as_ref().is_some_and(Worker::is_finished));
    assert!(weak.upgrade().is_none());
    assert!(!state.refining && !state.open && state.pending.is_none());
    assert!(
        state.poll_response(1, 2).is_none(),
        "failure does not restart per frame"
    );
    state.start(1, 2, || Ok(review(1, 2)));
    assert!(poll(&mut state, 1, 2).is_ok());
}

#[test]
fn semantic_source_and_mismatched_payload_failures_retire_logical_selection() {
    for (revision, source) in [(8, 7), (7, 8), (7, 7)] {
        let mut state = CaptureReview {
            refining: true,
            open: true,
            ..Default::default()
        };
        state.start(7, 7, || Ok(review(6, 7)));
        assert!(poll(&mut state, revision, source).is_err());
        assert!(
            state.pending.is_none() && state.review.is_none() && !state.refining && !state.open
        );
        state.start(revision, source, || Err("definition unavailable".into()));
        assert!(
            matches!(poll(&mut state, revision, source), Err(e) if e == "definition unavailable")
        );
        assert!(!state.refining && state.pending.is_none());
    }
}
