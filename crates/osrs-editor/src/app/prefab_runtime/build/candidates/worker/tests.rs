//! Controlled candidate boundaries exercise the actual generator and ordinary BuildState admission.
use super::*;
use std::{
    thread::ThreadId,
    time::{Duration, Instant},
};

const TIMEOUT: Duration = Duration::from_secs(10);

fn fixture() -> (BuildState, MapDocument) {
    let plane = SourcePlane::new(0).unwrap_or_else(|| panic!("plane"));
    let heights = (-1..=11)
        .flat_map(|y| {
            (-1..=15).map(move |x| {
                (
                    osrs_edit::HeightVertexCoord::new(MapTile::new(3300 + x, 3300 + y), plane),
                    -64,
                )
            })
        })
        .collect::<Vec<_>>();
    let document = MapDocument::from_parts([], heights, []);
    let mut state = BuildState::default();
    state.form.width = 14;
    state.form.height = 10;
    state.centre_site_on((3307, 3305), 0);
    state.refresh_for_script(&document, 7, 9, &|_, _| Some((1, 1)));
    assert!(state.preview.is_some(), "{:?}", state.error);
    (state, document)
}

struct Controlled {
    started: mpsc::Receiver<(u64, u64, ThreadId)>,
    release: mpsc::Sender<()>,
}

fn controlled() -> (Worker, Controlled) {
    let (started, starts) = mpsc::channel();
    let (release, releases) = mpsc::channel();
    let worker = Worker::spawn(move |input, form, kit, document| {
        started
            .send((input.source, form.seed, std::thread::current().id()))
            .unwrap_or_else(|e| panic!("{e}"));
        releases
            .recv_timeout(TIMEOUT)
            .unwrap_or_else(|e| panic!("release: {e}"));
        plan::plan(input, form, kit, document)
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
        assert!(Instant::now() < deadline, "candidate worker stalled");
        std::thread::sleep(Duration::from_millis(1));
    }
}

fn finish(state: &mut BuildState, document: &MapDocument, source: u64, revision: u64) {
    wait_until(|| {
        state.refresh_source(document, source, revision, &|_, _| Some((1, 1)));
        state.pending.is_none()
    });
    assert!(
        state
            .current_preview(source, revision, &|_, _| Some((1, 1)))
            .is_some(),
        "{:?}",
        state.error
    );
}

#[test]
fn rapid_rerolls_sources_and_revisions_replace_queued_snapshots_on_one_thread() {
    let (mut state, document) = fixture();
    let (worker, control) = controlled();
    state.candidate_worker = Some(worker);
    state.refresh_source(&document, 8, 10, &|_, _| Some((1, 1)));
    let (_, _, thread) = control
        .started
        .recv_timeout(TIMEOUT)
        .unwrap_or_else(|e| panic!("{e}"));
    let active = Arc::downgrade(state.made_from.as_ref().unwrap_or_else(|| panic!("input")));
    let mut obsolete = Vec::new();
    for version in 9..=72 {
        state.reroll();
        state.refresh_source(&document, version, version + 2, &|_, _| Some((1, 1)));
        assert!(
            state
                .current_preview(version, version + 2, &|_, _| Some((1, 1)))
                .is_none()
        );
        assert_eq!(
            state
                .candidate_worker
                .as_ref()
                .unwrap_or_else(|| panic!("worker"))
                .thread
                .thread()
                .id(),
            thread
        );
        assert!(
            obsolete
                .iter()
                .all(|old: &Weak<PreviewInput>| old.upgrade().is_none()),
            "replaced queued snapshots must retire immediately"
        );
        if version != 72 {
            obsolete.push(Arc::downgrade(
                state.made_from.as_ref().unwrap_or_else(|| panic!("input")),
            ));
        }
    }
    assert!(
        active.upgrade().is_some(),
        "only the active candidate must drain"
    );
    assert!(matches!(
        control.started.try_recv(),
        Err(mpsc::TryRecvError::Empty)
    ));
    let base = state.form.seed;
    control.release.send(()).unwrap_or_else(|e| panic!("{e}"));
    for offset in 0..8 {
        assert_eq!(
            control
                .started
                .recv_timeout(TIMEOUT)
                .unwrap_or_else(|e| panic!("{e}")),
            (72, base.wrapping_add(offset), thread)
        );
        assert!(
            active.upgrade().is_none(),
            "old batch retires after its first candidate"
        );
        control.release.send(()).unwrap_or_else(|e| panic!("{e}"));
    }
    finish(&mut state, &document, 72, 74);
    assert!(state.candidates(72, 74, &|_, _| Some((1, 1))).len() <= 8);
    let preview = state
        .current_preview(72, 74, &|_, _| Some((1, 1)))
        .unwrap_or_else(|| panic!("preview"));
    let mut applied = document.clone();
    applied
        .apply_patch(&preview.apply)
        .unwrap_or_else(|e| panic!("{e:?}"));
    applied
        .apply_patch(&preview.apply.inverse())
        .unwrap_or_else(|e| panic!("{e:?}"));
    assert_eq!(applied, document);
}

#[test]
fn invalid_input_cancels_active_work_and_immediately_releases_the_queued_document() {
    let (mut state, document) = fixture();
    let (worker, control) = controlled();
    state.candidate_worker = Some(worker);
    state.refresh_source(&document, 8, 10, &|_, _| Some((1, 1)));
    let (_, _, thread) = control
        .started
        .recv_timeout(TIMEOUT)
        .unwrap_or_else(|e| panic!("{e}"));
    state.refresh_source(&document, 9, 11, &|_, _| Some((1, 1)));
    let queued = Arc::downgrade(state.made_from.as_ref().unwrap_or_else(|| panic!("input")));
    state.site_plane = 4;
    state.refresh_source(&document, 9, 11, &|_, _| Some((1, 1)));
    assert!(queued.upgrade().is_none());
    assert!(state.pending.is_none() && state.preview.is_none() && state.made_from.is_none());
    assert!(state.error.is_some());
    let worker = state
        .candidate_worker
        .as_ref()
        .unwrap_or_else(|| panic!("worker"));
    assert!(worker.shared.lock().pending.is_none());
    assert_eq!(worker.thread.thread().id(), thread);
    control.release.send(()).unwrap_or_else(|e| panic!("{e}"));
    wait_until(|| worker.shared.lock().active.is_none());
    assert!(
        matches!(control.started.try_recv(), Err(mpsc::TryRecvError::Empty)),
        "no second seed or cancelled queued batch may start"
    );
}

#[test]
fn replacing_an_unpolled_completed_batch_retires_its_metadata_immediately() {
    let (mut state, document) = fixture();
    let (worker, control) = controlled();
    state.candidate_worker = Some(worker);
    state.refresh_source(&document, 8, 10, &|_, _| Some((1, 1)));
    let old = Arc::downgrade(state.made_from.as_ref().unwrap_or_else(|| panic!("input")));
    for _ in 0..8 {
        control
            .started
            .recv_timeout(TIMEOUT)
            .unwrap_or_else(|e| panic!("{e}"));
        control.release.send(()).unwrap_or_else(|e| panic!("{e}"));
    }
    wait_until(|| {
        state
            .pending
            .as_ref()
            .is_some_and(|job| Arc::strong_count(&job.cancel) == 1)
    });
    assert!(
        old.upgrade().is_some(),
        "unpolled result/current input still owns metadata"
    );
    state.refresh_source(&document, 9, 11, &|_, _| Some((1, 1)));
    assert!(old.upgrade().is_none());
    assert!(state.preview.is_none());
    // Finish the active controlled candidate before dropping its physical owner.
    control
        .started
        .recv_timeout(TIMEOUT)
        .unwrap_or_else(|e| panic!("{e}"));
    state.site = None;
    state.refresh_source(&document, 9, 11, &|_, _| Some((1, 1)));
    control.release.send(()).unwrap_or_else(|e| panic!("{e}"));
    wait_until(|| {
        state
            .candidate_worker
            .as_ref()
            .unwrap_or_else(|| panic!("worker"))
            .shared
            .lock()
            .active
            .is_none()
    });
}

#[test]
fn dropping_a_stale_ticket_cannot_cancel_a_newer_queued_request_and_shutdown_does_not_join() {
    let (state, document) = fixture();
    let (mut worker, control) = controlled();
    let input = Arc::clone(state.made_from.as_ref().unwrap_or_else(|| panic!("input")));
    let active = worker.start(document.clone(), Arc::clone(&input), state.kits[0].clone());
    control
        .started
        .recv_timeout(TIMEOUT)
        .unwrap_or_else(|e| panic!("{e}"));
    let stale = worker.start(document.clone(), Arc::clone(&input), state.kits[0].clone());
    let mut current = worker.start(document, input, state.kits[0].clone());
    let current_flag = Arc::clone(&current.cancel);
    drop(stale);
    assert!(
        worker
            .shared
            .lock()
            .pending
            .as_ref()
            .is_some_and(|r| Arc::ptr_eq(&r.cancel, &current_flag))
    );
    let weak = Arc::downgrade(&worker.shared);
    drop(worker); // Must return while the candidate remains blocked on its release handshake.
    assert!(
        current.poll().is_err(),
        "queued response closes immediately on owner shutdown"
    );
    drop(current);
    drop(active);
    control.release.send(()).unwrap_or_else(|e| panic!("{e}"));
    wait_until(|| weak.upgrade().is_none());
    assert!(matches!(
        control.started.recv_timeout(TIMEOUT),
        Err(mpsc::RecvTimeoutError::Disconnected)
    ));
}

#[test]
fn a_panic_disconnects_the_queued_ticket_and_reroll_replaces_only_a_terminal_worker() {
    let (mut state, document) = fixture();
    let (started, starts) = mpsc::channel();
    let (release, releases) = mpsc::channel();
    state.candidate_worker = Some(Worker::spawn(move |_, _, _, _| {
        started
            .send(std::thread::current().id())
            .unwrap_or_else(|e| panic!("{e}"));
        releases
            .recv_timeout(TIMEOUT)
            .unwrap_or_else(|e| panic!("{e}"));
        panic!("controlled candidate failure");
    }));
    state.refresh_source(&document, 8, 10, &|_, _| Some((1, 1)));
    let old_thread = starts
        .recv_timeout(TIMEOUT)
        .unwrap_or_else(|e| panic!("{e}"));
    state.refresh_source(&document, 9, 11, &|_, _| Some((1, 1)));
    release.send(()).unwrap_or_else(|e| panic!("{e}"));
    wait_until(|| {
        state
            .candidate_worker
            .as_ref()
            .is_some_and(Worker::is_finished)
    });
    state.refresh_source(&document, 9, 11, &|_, _| Some((1, 1)));
    assert!(state.pending.is_none() && state.error.is_some());
    state.refresh_source(&document, 9, 11, &|_, _| Some((1, 1)));
    assert!(
        state.pending.is_none(),
        "no per-frame retry loop after terminal failure"
    );
    state.reroll();
    state.refresh_source(&document, 9, 11, &|_, _| Some((1, 1)));
    assert_ne!(
        state
            .candidate_worker
            .as_ref()
            .unwrap_or_else(|| panic!("worker"))
            .thread
            .thread()
            .id(),
        old_thread
    );
    finish(&mut state, &document, 9, 11);
}

#[test]
fn wrapping_seeds_failures_ranking_and_patches_match_the_existing_planner() {
    let (mut state, document) = fixture();
    state.form.seed = u64::MAX - 3;
    state.refresh_for_script(&document, 7, 9, &|_, _| Some((1, 1)));
    let input = Arc::clone(state.made_from.as_ref().unwrap_or_else(|| panic!("input")));
    let kit = state.kits[0].clone();
    let mut expected = Vec::new();
    let mut failures = Vec::new();
    let planner = |input, form: &BuildForm, kit: &Kit, document: &MapDocument| {
        if form.seed & 1 == 1 {
            Err(plan::Failure::request("controlled failure"))
        } else {
            plan::plan(input, form, kit, document)
        }
    };
    for offset in 0..8 {
        let mut form = input.form.clone();
        form.seed = form.seed.wrapping_add(offset);
        match planner(Arc::clone(&input), &form, &kit, &document) {
            Ok(p) => expected.push(p),
            Err(failure) => failures.push(FailedAttempt {
                input: Arc::clone(&input),
                form,
                failure,
            }),
        }
    }
    assert!(
        !expected.is_empty(),
        "fixture must exercise successful real generation"
    );
    assert!(
        !failures.is_empty(),
        "fixture must exercise failure formatting"
    );
    assert_eq!(expected.len() + failures.len(), 8);
    expected.sort_by_key(|p| (std::cmp::Reverse(p.verdict.rooms_reachable), p.form.seed));
    let mut worker = Worker::spawn(planner);
    let mut job = worker.start(document, Arc::clone(&input), kit);
    let mut batch = None;
    wait_until(|| {
        batch = job.poll().unwrap_or_else(|e| panic!("{e}"));
        batch.is_some()
    });
    let batch = batch.unwrap_or_else(|| panic!("batch"));
    assert!(Arc::ptr_eq(&batch.input, &input));
    assert_eq!(batch.failures, failures);
    assert_eq!(batch.previews.len(), expected.len());
    for (actual, expected) in batch.previews.iter().zip(expected) {
        assert_eq!(actual.form, expected.form);
        assert_eq!(actual.apply, expected.apply);
        assert_eq!(actual.building, expected.building);
    }
}

#[test]
fn synchronous_planning_retires_native_output_without_waiting_for_its_active_candidate() {
    let (mut state, document) = fixture();
    let (worker, control) = controlled();
    state.candidate_worker = Some(worker);
    state.refresh_source(&document, 8, 10, &|_, _| Some((1, 1)));
    let (_, _, thread) = control
        .started
        .recv_timeout(TIMEOUT)
        .unwrap_or_else(|e| panic!("{e}"));
    // The script path remains synchronous on the UI thread; queued native work must retire and
    // its single draining candidate cannot replace the synchronous result, even for an equal key.
    state.refresh_for_script(&document, 8, 10, &|_, _| Some((1, 1)));
    assert!(state.pending.is_none());
    let applied = state
        .current_preview(8, 10, &|_, _| Some((1, 1)))
        .unwrap_or_else(|| panic!("script preview"))
        .apply
        .clone();
    assert_eq!(state.candidates(8, 10, &|_, _| Some((1, 1))).len(), 1);
    let worker = state
        .candidate_worker
        .as_ref()
        .unwrap_or_else(|| panic!("worker"));
    assert_eq!(worker.thread.thread().id(), thread);
    assert!(
        worker
            .shared
            .lock()
            .active
            .as_ref()
            .is_some_and(|flag| flag.load(Ordering::Relaxed))
    );
    control.release.send(()).unwrap_or_else(|e| panic!("{e}"));
    wait_until(|| worker.shared.lock().active.is_none());
    state.refresh_source(&document, 8, 10, &|_, _| Some((1, 1)));
    assert!(state.pending.is_none());
    assert_eq!(
        state
            .current_preview(8, 10, &|_, _| Some((1, 1)))
            .unwrap_or_else(|| panic!("current script preview"))
            .apply,
        applied
    );
    assert!(matches!(
        control.started.try_recv(),
        Err(mpsc::TryRecvError::Empty)
    ));
}
