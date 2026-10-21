//! One logical batch ticket; the retained worker bounds physical candidate concurrency.
mod worker;
use super::*;
use std::sync::{
    Arc, Weak,
    atomic::{AtomicBool, Ordering},
    mpsc::{self, Receiver},
};
pub(super) use worker::Worker;
pub(super) struct Job {
    receive: Receiver<Batch>,
    cancel: Arc<AtomicBool>,
    queue: Weak<worker::Shared>,
}
pub(super) struct Batch {
    pub(super) input: Arc<PreviewInput>,
    pub(super) previews: Vec<Preview>,
    pub(super) failures: Vec<FailedAttempt>,
}
impl Job {
    #[cfg(test)]
    pub(super) fn controlled() -> (Self, mpsc::Sender<Batch>) {
        let (send, receive) = mpsc::channel();
        (
            Self {
                receive,
                cancel: Arc::new(AtomicBool::new(false)),
                queue: Weak::new(),
            },
            send,
        )
    }

    pub(super) fn poll(&mut self) -> Result<Option<Batch>, mpsc::TryRecvError> {
        match self.receive.try_recv() {
            Ok(batch) => Ok(Some(batch)),
            Err(mpsc::TryRecvError::Empty) => Ok(None),
            Err(error) => Err(error),
        }
    }
}
impl Drop for Job {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
        if let Some(queue) = self.queue.upgrade() {
            queue.cancel(&self.cancel);
        }
    }
}
