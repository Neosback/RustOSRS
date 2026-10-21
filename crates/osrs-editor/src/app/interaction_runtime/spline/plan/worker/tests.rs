//! Real fitting with controlled start/release handshakes, independent of scheduler timing.
use super::super::PathOptions;
use super::*;
use crate::app::interaction_runtime::{InteractionRuntime, SplineStatus};
use crate::camera::FlyCamera;
use osrs_core::coords::RegionCoord;
use std::{sync::mpsc, thread::ThreadId, time::Instant};

const TIMEOUT: Duration = Duration::from_secs(5);
const OPTIONS: PathOptions = PathOptions {
    width: 3.0,
    smooth_curve: true,
    edge_smoothing: 1.5,
};

fn interaction() -> InteractionRuntime {
    let mut state = InteractionRuntime::new(FlyCamera::new(0.0, 0.0, 0.0));
    state.prepare_spline_extent([RegionCoord::new(50, 50)], 1);
    state.replace_spline_points(vec![(3201.5, 3201.5), (3210.5, 3208.5)]);
    state
}

fn input(state: &InteractionRuntime, overlay: u16) -> PlanInput {
    state
        .plan_input(Some(overlay), None, OPTIONS)
        .unwrap_or_else(|| panic!("fixture input"))
}

struct Controlled {
    started: mpsc::Receiver<(u64, ThreadId)>,
    release: mpsc::Sender<()>,
}

fn controlled(first: PlanInput) -> (PlanWorker, Controlled) {
    let (started, starts) = mpsc::channel();
    let (release, releases) = mpsc::channel();
    let worker = PlanWorker::start(first, None, move |input| {
        started
            .send((input.key, std::thread::current().id()))
            .unwrap_or_else(|e| panic!("{e}"));
        releases
            .recv_timeout(TIMEOUT)
            .unwrap_or_else(|e| panic!("release: {e}"));
        input.make()
    });
    (
        worker,
        Controlled {
            started: starts,
            release,
        },
    )
}

fn wait_until(mut ready: impl FnMut() -> bool) {
    let deadline = Instant::now() + TIMEOUT;
    while !ready() {
        assert!(Instant::now() < deadline, "worker did not make progress");
        std::thread::sleep(Duration::from_millis(1));
    }
}

fn complete(worker: &mut PlanWorker) -> Preview {
    let mut preview = None;
    wait_until(|| {
        preview = worker.poll().unwrap_or_else(|()| panic!("worker stopped"));
        preview.is_some()
    });
    preview.unwrap_or_else(|| panic!("preview"))
}

#[test]
fn a_blocked_fit_keeps_only_the_latest_burst_input_and_matches_canonical_fitting() {
    let state = interaction();
    let first = input(&state, 1);
    let first_key = first.key;
    let (mut worker, control) = controlled(first);
    let (started, thread) = control
        .started
        .recv_timeout(TIMEOUT)
        .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(started, first_key);
    for overlay in 2..=256 {
        assert!(worker.request(input(&state, overlay)).is_none());
    }
    let expected = input(&state, 256).make();
    {
        let slots = worker.shared.lock();
        assert_eq!(slots.desired, Some(expected.key));
        assert_eq!(slots.pending.as_ref().map(|i| i.key), Some(expected.key));
        assert!(slots.result.is_none());
    }
    assert!(matches!(
        control.started.try_recv(),
        Err(mpsc::TryRecvError::Empty)
    ));
    control.release.send(()).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(
        control
            .started
            .recv_timeout(TIMEOUT)
            .unwrap_or_else(|e| panic!("{e}")),
        (expected.key, thread)
    );
    assert!(
        worker
            .poll()
            .unwrap_or_else(|()| panic!("worker stopped"))
            .is_none(),
        "superseded first result must not publish"
    );
    control.release.send(()).unwrap_or_else(|e| panic!("{e}"));
    let actual = complete(&mut worker);
    assert_eq!(actual.key, expected.key);
    assert_eq!(actual.line, expected.line);
    assert_eq!(actual.paints, expected.paints);
    assert_eq!(actual.overlay_id, expected.overlay_id);
    assert!(worker.shared.lock().pending.is_none());
}

#[test]
fn an_inactive_consumer_retains_only_the_current_completed_preview() {
    let state = interaction();
    let (mut worker, control) = controlled(input(&state, 1));
    control
        .started
        .recv_timeout(TIMEOUT)
        .unwrap_or_else(|e| panic!("{e}"));
    control.release.send(()).unwrap_or_else(|e| panic!("{e}"));
    wait_until(|| worker.shared.lock().result.is_some());
    for overlay in 2..=4 {
        let key = input(&state, overlay).key;
        assert!(worker.request(input(&state, overlay)).is_none());
        assert!(
            worker.shared.lock().result.is_none(),
            "old completed data retires at replacement"
        );
        control
            .started
            .recv_timeout(TIMEOUT)
            .unwrap_or_else(|e| panic!("{e}"));
        control.release.send(()).unwrap_or_else(|e| panic!("{e}"));
        wait_until(|| {
            worker
                .shared
                .lock()
                .result
                .as_ref()
                .is_some_and(|p| p.key == key)
        });
    }
    assert_eq!(complete(&mut worker).key, input(&state, 4).key);
    assert!(
        worker
            .poll()
            .unwrap_or_else(|()| panic!("worker stopped"))
            .is_none()
    );
}

#[test]
fn cancellation_and_key_roundtrips_reuse_the_one_active_fit() {
    let state = interaction();
    let first = input(&state, 1);
    let key = first.key;
    let (mut worker, control) = controlled(first);
    control
        .started
        .recv_timeout(TIMEOUT)
        .unwrap_or_else(|e| panic!("{e}"));
    assert!(worker.request(input(&state, 2)).is_none());
    worker.cancel();
    {
        let slots = worker.shared.lock();
        assert!(slots.desired.is_none() && slots.pending.is_none() && slots.result.is_none());
    }
    assert!(worker.request(input(&state, 1)).is_none());
    control.release.send(()).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(complete(&mut worker).key, key);
    assert!(
        worker.shared.lock().pending.is_none(),
        "same-key fit is already satisfied"
    );
    assert!(matches!(
        control.started.try_recv(),
        Err(mpsc::TryRecvError::Empty)
    ));
    worker.cancel();
    assert!(worker.shared.lock().result.is_none());
}

#[test]
fn clear_and_source_replacement_cannot_start_an_overlapping_spline_worker() {
    let mut state = interaction();
    let first = input(&state, 1);
    let (worker, control) = controlled(first);
    state.spline.worker = Some(worker);
    let (_, thread) = control
        .started
        .recv_timeout(TIMEOUT)
        .unwrap_or_else(|e| panic!("{e}"));
    for generation in 2..=64 {
        state.clear_spline_points();
        state.prepare_spline_extent([RegionCoord::new(50, 50)], generation);
        state.add_spline_point((3201.5, 3201.5));
        state.update_spline_preview(Some(1), None, OPTIONS);
        let worker = state
            .spline
            .worker
            .as_ref()
            .unwrap_or_else(|| panic!("worker retained"));
        assert_eq!(worker.thread.thread().id(), thread);
    }
    assert!(state.current_spline_preview().is_none());
    control.release.send(()).unwrap_or_else(|e| panic!("{e}"));
    let (latest, latest_thread) = control
        .started
        .recv_timeout(TIMEOUT)
        .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(latest_thread, thread);
    assert_eq!(
        latest,
        state.spline.desired_key.unwrap_or_else(|| panic!("key"))
    );
    assert!(
        state.current_spline_preview().is_none(),
        "old-source completion is never current"
    );
    control.release.send(()).unwrap_or_else(|e| panic!("{e}"));
    wait_until(|| {
        state.update_spline_preview(Some(1), None, OPTIONS);
        matches!(state.spline_status(), SplineStatus::Ready { .. })
    });
    assert_eq!(
        state
            .current_spline_preview()
            .unwrap_or_else(|| panic!("preview"))
            .key,
        latest
    );
}

#[test]
fn dropping_the_owner_discards_pending_output_and_stops_after_the_current_fit() {
    let state = interaction();
    let (mut worker, control) = controlled(input(&state, 1));
    control
        .started
        .recv_timeout(TIMEOUT)
        .unwrap_or_else(|e| panic!("{e}"));
    worker.request(input(&state, 2));
    let shared = Arc::clone(&worker.shared);
    let weak = Arc::downgrade(&shared);
    drop(worker);
    {
        let slots = shared.lock();
        assert!(
            slots.closed
                && slots.desired.is_none()
                && slots.pending.is_none()
                && slots.result.is_none()
        );
    }
    drop(shared);
    control.release.send(()).unwrap_or_else(|e| panic!("{e}"));
    wait_until(|| weak.upgrade().is_none());
    assert!(
        matches!(
            control.started.recv_timeout(TIMEOUT),
            Err(mpsc::RecvTimeoutError::Disconnected)
        ),
        "no pending fit starts after shutdown"
    );
}

#[test]
fn public_cancel_holds_admission_and_resume_reuses_the_draining_thread() {
    let mut state = interaction();
    let first = input(&state, 1);
    let (worker, control) = controlled(first);
    state.spline.worker = Some(worker);
    state.update_spline_preview(Some(1), None, OPTIONS);
    let (_, thread) = control
        .started
        .recv_timeout(TIMEOUT)
        .unwrap_or_else(|e| panic!("{e}"));
    state.cancel_spline_plan();
    for _ in 0..16 {
        state.update_spline_preview(Some(1), None, OPTIONS);
        assert_eq!(state.spline_status(), SplineStatus::Cancelled);
        let worker = state
            .spline
            .worker
            .as_ref()
            .unwrap_or_else(|| panic!("retained worker"));
        assert_eq!(worker.thread.thread().id(), thread);
        let slots = worker.shared.lock();
        assert!(slots.pending.is_none() && slots.result.is_none() && slots.desired.is_none());
    }
    state.resume_spline_plan();
    state.update_spline_preview(Some(2), None, OPTIONS);
    assert_eq!(state.spline_status(), SplineStatus::Updating);
    control.release.send(()).unwrap_or_else(|e| panic!("{e}"));
    let (key, next_thread) = control
        .started
        .recv_timeout(TIMEOUT)
        .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(next_thread, thread);
    assert_eq!(Some(key), state.spline.desired_key);
    assert!(state.current_spline_preview().is_none());
    control.release.send(()).unwrap_or_else(|e| panic!("{e}"));
    wait_until(|| {
        state.update_spline_preview(Some(2), None, OPTIONS);
        matches!(state.spline_status(), SplineStatus::Ready { .. })
    });
}

#[test]
fn a_panicked_worker_returns_the_unaccepted_input_for_terminal_retry() {
    let mut state = interaction();
    let mut worker =
        PlanWorker::start(input(&state, 1), None, |_| panic!("controlled fit failure"));
    wait_until(|| worker.thread.is_finished());
    assert!(worker.poll().is_err());
    let expected = input(&state, 2).key;
    assert_eq!(
        worker.request(input(&state, 2)).map(|i| i.key),
        Some(expected)
    );
    state.spline.worker = Some(worker);
    state.update_spline_preview(Some(2), None, OPTIONS);
    assert!(matches!(state.spline_status(), SplineStatus::Failed(_)));
    wait_until(|| {
        state.update_spline_preview(Some(2), None, OPTIONS);
        matches!(state.spline_status(), SplineStatus::Ready { .. })
    });
    assert_eq!(
        state
            .current_spline_preview()
            .unwrap_or_else(|| panic!("recovered preview"))
            .key,
        expected
    );
}
