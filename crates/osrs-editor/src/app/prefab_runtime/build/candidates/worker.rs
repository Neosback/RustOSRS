//! One background candidate at a time, one replaceable queued snapshot and one bounded result
//! per logical job. Cancellation drops queued data immediately and drains the active candidate.
use super::*;
use std::{
    sync::{Condvar, Mutex, MutexGuard, mpsc::SyncSender},
    thread::JoinHandle,
};

struct Request {
    document: MapDocument,
    input: Arc<PreviewInput>,
    kit: Kit,
    send: SyncSender<Batch>,
    cancel: Arc<AtomicBool>,
}

#[derive(Default)]
struct Slots {
    pending: Option<Request>,
    active: Option<Arc<AtomicBool>>,
    closed: bool,
}

#[derive(Default)]
pub(super) struct Shared {
    slots: Mutex<Slots>,
    changed: Condvar,
}

impl Shared {
    fn lock(&self) -> MutexGuard<'_, Slots> {
        self.slots.lock().unwrap_or_else(|error| error.into_inner())
    }

    fn next(&self) -> Option<Request> {
        let mut slots = self.lock();
        loop {
            if slots.closed {
                return None;
            }
            if let Some(request) = slots.pending.take() {
                slots.active = Some(Arc::clone(&request.cancel));
                return Some(request);
            }
            slots = self
                .changed
                .wait(slots)
                .unwrap_or_else(|error| error.into_inner());
        }
    }

    pub(super) fn cancel(&self, flag: &Arc<AtomicBool>) {
        let mut slots = self.lock();
        let removed = if slots
            .pending
            .as_ref()
            .is_some_and(|r| Arc::ptr_eq(&r.cancel, flag))
        {
            slots.pending.take()
        } else {
            None
        };
        drop(slots);
        drop(removed);
    }

    fn close(&self) {
        let mut slots = self.lock();
        slots.closed = true;
        if let Some(active) = slots.active.take() {
            active.store(true, Ordering::Relaxed);
        }
        let removed = slots.pending.take();
        self.changed.notify_one();
        drop(slots);
        drop(removed);
    }
}

/// Also closes a queued response sender if planning panics, so polling cannot wait forever.
struct CloseOnExit(Arc<Shared>);
impl Drop for CloseOnExit {
    fn drop(&mut self) {
        self.0.close();
    }
}

pub(in super::super) struct Worker {
    shared: Arc<Shared>,
    thread: JoinHandle<()>,
}

impl Worker {
    pub(in super::super) fn new() -> Self {
        Self::spawn(plan::plan)
    }

    fn spawn(
        mut plan: impl FnMut(
            Arc<PreviewInput>,
            &BuildForm,
            &Kit,
            &MapDocument,
        ) -> Result<Preview, plan::Failure>
        + Send
        + 'static,
    ) -> Self {
        let shared = Arc::new(Shared::default());
        let queue = Arc::clone(&shared);
        let thread = std::thread::spawn(move || {
            let _close = CloseOnExit(Arc::clone(&queue));
            while let Some(request) = queue.next() {
                run(request, &mut plan);
                queue.lock().active = None;
            }
        });
        Self { shared, thread }
    }

    pub(in super::super) fn is_finished(&self) -> bool {
        self.thread.is_finished()
    }

    pub(in super::super) fn start(
        &mut self,
        document: MapDocument,
        input: Arc<PreviewInput>,
        kit: Kit,
    ) -> Job {
        let (send, receive) = mpsc::sync_channel(1);
        let cancel = Arc::new(AtomicBool::new(false));
        let request = Request {
            document,
            input,
            kit,
            send,
            cancel: Arc::clone(&cancel),
        };
        let mut slots = self.shared.lock();
        let removed = if slots.closed {
            Some(request)
        } else {
            if let Some(active) = &slots.active {
                active.store(true, Ordering::Relaxed);
            }
            let removed = slots.pending.replace(request);
            self.shared.changed.notify_one();
            removed
        };
        drop(slots);
        if let Some(request) = &removed {
            request.cancel.store(true, Ordering::Relaxed);
        }
        drop(removed);
        Job {
            receive,
            cancel,
            queue: Arc::downgrade(&self.shared),
        }
    }
}

fn run(
    request: Request,
    plan: &mut impl FnMut(
        Arc<PreviewInput>,
        &BuildForm,
        &Kit,
        &MapDocument,
    ) -> Result<Preview, plan::Failure>,
) {
    let mut previews = Vec::new();
    let mut failures = Vec::new();
    for offset in 0..8 {
        if request.cancel.load(Ordering::Relaxed) {
            return;
        }
        let mut attempt = request.input.form.clone();
        attempt.seed = attempt.seed.wrapping_add(offset);
        let result = plan(
            Arc::clone(&request.input),
            &attempt,
            &request.kit,
            &request.document,
        );
        if request.cancel.load(Ordering::Relaxed) {
            return;
        }
        match result {
            Ok(p) => previews.push(p),
            Err(failure) => failures.push(FailedAttempt {
                input: Arc::clone(&request.input),
                form: attempt,
                failure,
            }),
        }
    }
    previews.sort_by_key(|p| (std::cmp::Reverse(p.verdict.rooms_reachable), p.form.seed));
    let _ = request.send.try_send(Batch {
        input: request.input,
        previews,
        failures,
    });
}

impl Drop for Worker {
    fn drop(&mut self) {
        // Do not join pure generation on the UI thread. Close cancels the current candidate,
        // removes queued snapshots, and causes exit as soon as that candidate returns.
        self.shared.close();
    }
}

#[cfg(test)]
mod tests;
