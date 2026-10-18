//! Training in the background: a worker thread runs the trainer and reports its steps, so the
//! window keeps drawing a progress bar. Native only (the browser build has no threads here).

use osrs_edit::{KitFile, Prefab};
use osrs_lab::{Corpus, Model, Progress, TrainRequest};
use osrs_scene::building_census::ObjectTable;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc::{Receiver, TryRecvError, channel},
};

pub(super) enum Event {
    Step(u8, String),
    Done(Result<Box<Model>, String>),
}

pub(super) struct Inputs {
    pub(super) id: String,
    pub(super) name: String,
    pub(super) created: String,
    pub(super) prefabs: Vec<(String, Prefab)>,
    pub(super) objects: ObjectTable,
    pub(super) base_kit: KitFile,
    pub(super) seed: u64,
}

pub(super) struct Job {
    receiver: Receiver<Event>,
    cancel: Arc<AtomicBool>,
    pub(super) percent: u8,
    pub(super) message: String,
    /// The steps seen so far, without repeats, for the log.
    pub(super) log: Vec<String>,
    pub(super) name: String,
    pub(super) building_count: usize,
}

impl Job {
    #[cfg(not(target_arch = "wasm32"))]
    pub(super) fn start(inputs: Inputs) -> Self {
        let (sender, receiver) = channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&cancel);
        let (name, building_count) = (inputs.name.clone(), inputs.prefabs.len());
        std::thread::spawn(move || {
            let started = std::time::Instant::now();
            let corpus = Corpus::shipped();
            let step_sender = sender.clone();
            let result = osrs_lab::train_model(
                &TrainRequest {
                    id: &inputs.id,
                    name: &inputs.name,
                    created: &inputs.created,
                    prefabs: &inputs.prefabs,
                    objects: &inputs.objects,
                    base_kit: &inputs.base_kit,
                    corpus: &corpus,
                    seed: inputs.seed,
                    cancelled: &|| flag.load(Ordering::Relaxed),
                },
                &|| started.elapsed().as_secs_f32(),
                &mut |Progress::Step { percent, message }| {
                    let _ = step_sender.send(Event::Step(percent, message));
                },
            );
            let _ = sender.send(Event::Done(result.map(Box::new)));
        });
        Self {
            receiver,
            cancel,
            percent: 0,
            message: "Starting".to_owned(),
            log: Vec::new(),
            name,
            building_count,
        }
    }

    #[cfg(target_arch = "wasm32")]
    pub(super) fn start(inputs: Inputs) -> Self {
        let (sender, receiver) = channel();
        let _ = sender.send(Event::Done(Err(
            "Training needs the desktop app.".to_owned()
        )));
        Self {
            receiver,
            cancel: Arc::new(AtomicBool::new(false)),
            percent: 0,
            message: String::new(),
            log: Vec::new(),
            name: inputs.name,
            building_count: inputs.prefabs.len(),
        }
    }

    pub(super) fn stop(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }

    /// Everything that arrived since the last call; the last item may be the end.
    pub(super) fn poll(&mut self) -> Option<Result<Box<Model>, String>> {
        loop {
            match self.receiver.try_recv() {
                Ok(Event::Step(percent, message)) => {
                    self.percent = percent.max(self.percent);
                    if self.log.last() != Some(&message) {
                        self.log.push(message.clone());
                    }
                    self.message = message;
                }
                Ok(Event::Done(result)) => return Some(result),
                Err(TryRecvError::Empty) => return None,
                Err(TryRecvError::Disconnected) => {
                    return Some(Err("The trainer stopped unexpectedly.".to_owned()));
                }
            }
        }
    }
}

/// Today as `YYYY-MM-DD` (UTC), from the system clock.
pub(super) fn today() -> String {
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    civil(i64::try_from(seconds / 86_400).unwrap_or(0))
}

/// The date of day number `days` since 1970-01-01 (Howard Hinnant's algorithm).
fn civil(days: i64) -> String {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    format!("{:04}-{m:02}-{d:02}", if m <= 2 { y + 1 } else { y })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_come_out_right() {
        assert_eq!(civil(0), "1970-01-01");
        assert_eq!(civil(20_014), "2024-10-18");
        assert_eq!(civil(11_016), "2000-02-29");
    }
}
