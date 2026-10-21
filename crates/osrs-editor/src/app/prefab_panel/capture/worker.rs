//! One physical capture worker across analysis, membership rebuild, cancellation and reselection.
use super::Review;
use std::{
    sync::{
        Arc, Condvar, Mutex, MutexGuard, Weak,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender},
    },
    thread::JoinHandle,
};

type Work = Box<dyn FnOnce() -> Result<Review, String> + Send>;
struct Request {
    work: Work,
    send: SyncSender<Result<Review, String>>,
    cancelled: Arc<AtomicBool>,
}
#[derive(Default)]
struct Slots {
    pending: Option<Request>,
    active: Option<Arc<AtomicBool>>,
    closed: bool,
}
#[derive(Default)]
struct Shared {
    slots: Mutex<Slots>,
    changed: Condvar,
}
impl Shared {
    fn lock(&self) -> MutexGuard<'_, Slots> {
        self.slots.lock().unwrap_or_else(|e| e.into_inner())
    }
    fn next(&self) -> Option<Request> {
        let mut slots = self.lock();
        loop {
            if slots.closed {
                return None;
            }
            if let Some(request) = slots.pending.take() {
                slots.active = Some(Arc::clone(&request.cancelled));
                return Some(request);
            }
            slots = self.changed.wait(slots).unwrap_or_else(|e| e.into_inner());
        }
    }
    fn cancel(&self, flag: &Arc<AtomicBool>) {
        let mut slots = self.lock();
        let removed = if slots
            .pending
            .as_ref()
            .is_some_and(|r| Arc::ptr_eq(&r.cancelled, flag))
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
// A panic also closes queued senders; a current ticket must never wait on a dead worker.
struct CloseOnExit(Arc<Shared>);
impl Drop for CloseOnExit {
    fn drop(&mut self) {
        self.0.close();
    }
}

pub(super) struct Worker {
    shared: Arc<Shared>,
    thread: JoinHandle<()>,
}
impl Worker {
    pub(super) fn new() -> Self {
        let shared = Arc::new(Shared::default());
        let queue = Arc::clone(&shared);
        let thread = std::thread::spawn(move || {
            let _close = CloseOnExit(Arc::clone(&queue));
            while let Some(request) = queue.next() {
                if !request.cancelled.load(Ordering::Relaxed) {
                    let result = (request.work)();
                    if !request.cancelled.load(Ordering::Relaxed) {
                        let _ = request.send.try_send(result);
                    }
                }
                queue.lock().active = None;
            }
        });
        Self { shared, thread }
    }
    pub(super) fn is_finished(&self) -> bool {
        self.thread.is_finished()
    }
    pub(super) fn start(
        &mut self,
        revision: u64,
        source: u64,
        work: impl FnOnce() -> Result<Review, String> + Send + 'static,
    ) -> Job {
        let (send, receive) = mpsc::sync_channel(1);
        let cancelled = Arc::new(AtomicBool::new(false));
        let request = Request {
            work: Box::new(work),
            send,
            cancelled: Arc::clone(&cancelled),
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
            request.cancelled.store(true, Ordering::Relaxed);
        }
        drop(removed);
        Job {
            revision,
            source,
            receive,
            cancelled,
            queue: Arc::downgrade(&self.shared),
        }
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        // The active read/analysis drains before exit; never join it on the UI thread.
        self.shared.close();
    }
}

pub(super) struct Job {
    revision: u64,
    source: u64,
    pub(super) receive: Receiver<Result<Review, String>>,
    cancelled: Arc<AtomicBool>,
    queue: Weak<Shared>,
}
impl Job {
    pub(super) fn is_current(&self, revision: u64, source: u64) -> bool {
        (self.revision, self.source) == (revision, source)
    }
}
impl Drop for Job {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Relaxed);
        if let Some(queue) = self.queue.upgrade() {
            queue.cancel(&self.cancelled);
        }
    }
}

#[cfg(test)]
mod tests;
