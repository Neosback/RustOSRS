//! At most eight bounded candidates per job. Dropping a job cancels between candidates.
use super::*;
#[cfg(not(target_arch = "wasm32"))]
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc::{self, Receiver},
};
#[cfg(not(target_arch = "wasm32"))]
pub(super) struct Job {
    receive: Receiver<(Vec<Preview>, Vec<String>)>,
    cancel: Arc<AtomicBool>,
}
#[cfg(not(target_arch = "wasm32"))]
impl Job {
    pub(super) fn start(
        document: MapDocument,
        form: BuildForm,
        kit: Kit,
        origin: MapTile,
        plane: SourcePlane,
        sizes: std::collections::BTreeMap<ObjectId, (u8, u8)>,
    ) -> Self {
        let (send, receive) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&cancel);
        std::thread::spawn(move || {
            let table = osrs_lab::kit_objects(&kit.file);
            let size_of = |id: ObjectId, _| {
                sizes
                    .get(&id)
                    .copied()
                    .or_else(|| table.get(&id).map(|i| i.size))
                    .unwrap_or((1, 1))
            };
            let mut previews = Vec::new();
            let mut failures = Vec::new();
            for offset in 0..8 {
                if flag.load(Ordering::Relaxed) {
                    return;
                }
                let mut attempt = form.clone();
                attempt.seed = attempt.seed.wrapping_add(offset);
                match plan::plan(&attempt, &kit, &document, origin, plane, &size_of) {
                    Ok(p) => previews.push(p),
                    Err(e) => failures.push(format!("Seed {}: {e}", attempt.seed)),
                }
            }
            previews.sort_by_key(|p| (std::cmp::Reverse(p.verdict.rooms_reachable), p.form.seed));
            let _ = send.send((previews, failures));
        });
        Self { receive, cancel }
    }
    pub(super) fn poll(&mut self) -> Option<(Vec<Preview>, Vec<String>)> {
        self.receive.try_recv().ok()
    }
}
#[cfg(not(target_arch = "wasm32"))]
impl Drop for Job {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}
