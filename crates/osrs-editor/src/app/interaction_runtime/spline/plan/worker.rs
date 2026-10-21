//! One spline fitting thread, one replaceable pending input and one completed preview. The lock
//! transfers owned inputs/results only; debounce waits and fitting never hold editor state.
use super::{PlanInput, Preview};
use eframe::egui;
use std::{
    sync::{Arc, Condvar, Mutex, MutexGuard},
    thread::JoinHandle,
    time::Duration,
};

#[derive(Debug, Default)]
struct Slots {
    desired: Option<u64>,
    pending: Option<PlanInput>,
    result: Option<Preview>,
    closed: bool,
}

#[derive(Debug, Default)]
struct Shared {
    slots: Mutex<Slots>,
    changed: Condvar,
}

impl Shared {
    fn lock(&self) -> MutexGuard<'_, Slots> {
        self.slots.lock().unwrap_or_else(|error| error.into_inner())
    }

    fn next(&self) -> Option<PlanInput> {
        let mut slots = self.lock();
        loop {
            if slots.closed {
                return None;
            }
            if slots.pending.is_none() {
                slots = self
                    .changed
                    .wait(slots)
                    .unwrap_or_else(|error| error.into_inner());
                continue;
            }
            // Debounce in the replaceable slot, so pointer samples never form an owned queue.
            let (next, quiet) = self
                .changed
                .wait_timeout(slots, PlanWorker::DEBOUNCE)
                .unwrap_or_else(|error| error.into_inner());
            slots = next;
            if quiet.timed_out() && !slots.closed && slots.pending.is_some() {
                return slots.pending.take();
            }
        }
    }
}

#[derive(Debug)]
pub(in super::super) struct PlanWorker {
    shared: Arc<Shared>,
    thread: JoinHandle<()>,
}

impl PlanWorker {
    const DEBOUNCE: Duration = Duration::from_millis(24);

    pub(in super::super) fn new(first: PlanInput, repaint: Option<egui::Context>) -> Self {
        Self::start(first, repaint, PlanInput::make)
    }

    fn start(
        first: PlanInput,
        repaint: Option<egui::Context>,
        mut make: impl FnMut(PlanInput) -> Preview + Send + 'static,
    ) -> Self {
        let shared = Arc::new(Shared {
            slots: Mutex::new(Slots {
                desired: Some(first.key),
                pending: Some(first),
                ..Default::default()
            }),
            changed: Condvar::new(),
        });
        let worker = Arc::clone(&shared);
        let thread = std::thread::spawn(move || {
            while let Some(input) = worker.next() {
                let preview = make(input);
                let mut slots = worker.lock();
                if slots.closed {
                    return;
                }
                if slots.desired != Some(preview.key) {
                    continue;
                }
                // A key roundtrip can make the active fit current again. Its exact same pending
                // input is redundant; retire it with publication instead of fitting it twice.
                slots.pending = None;
                slots.result = Some(preview);
                drop(slots);
                if let Some(ctx) = &repaint {
                    ctx.request_repaint();
                }
            }
        });
        Self { shared, thread }
    }

    /// Replace queued work immediately. The previous active fit drains on this same thread.
    /// Return the input only after the thread has actually stopped, so retry cannot overlap it.
    pub(in super::super) fn request(&mut self, input: PlanInput) -> Option<PlanInput> {
        if self.thread.is_finished() {
            return Some(input);
        }
        let mut slots = self.shared.lock();
        if slots.desired == Some(input.key) {
            return None;
        }
        slots.desired = Some(input.key);
        slots.pending = Some(input);
        slots.result = None;
        self.shared.changed.notify_one();
        None
    }

    /// Clear retained queued/completed data without spawning another thread on resume or source
    /// replacement. An already running pure fit can finish, but cannot publish while cancelled.
    pub(in super::super) fn cancel(&mut self) {
        let mut slots = self.shared.lock();
        slots.desired = None;
        slots.pending = None;
        slots.result = None;
        self.shared.changed.notify_one();
    }

    pub(in super::super) fn poll(&mut self) -> Result<Option<Preview>, ()> {
        let result = self.shared.lock().result.take();
        if result.is_none() && self.thread.is_finished() {
            Err(())
        } else {
            Ok(result)
        }
    }
}

impl Drop for PlanWorker {
    fn drop(&mut self) {
        let mut slots = self.shared.lock();
        slots.closed = true;
        slots.desired = None;
        slots.pending = None;
        slots.result = None;
        self.shared.changed.notify_one();
        // Never join a fit on the UI thread. It owns only its current immutable input and exits
        // after that fit; ordinary gesture cancellation keeps this owner rather than replacing it.
    }
}

#[cfg(test)]
mod tests;
