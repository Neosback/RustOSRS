//! Teaching a path model in the background so the window keeps drawing its progress bar.
//! Native only (the browser build has no threads here).

use osrs_lab::{PathLesson, PathModel, PathProgress, PathTrainRequest, train_paths};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc::{Receiver, TryRecvError, channel},
};

enum Event {
    Step(u8, String),
    Done(Result<Box<PathModel>, String>),
}

pub(super) struct Inputs {
    pub(super) id: String,
    pub(super) name: String,
    pub(super) created: String,
    pub(super) lessons: Vec<PathLesson>,
    pub(super) hidden: usize,
    pub(super) epochs: usize,
}

pub(super) struct Job {
    receiver: Receiver<Event>,
    cancel: Arc<AtomicBool>,
    pub(super) percent: u8,
    pub(super) message: String,
    pub(super) log: Vec<String>,
}

impl Job {
    #[cfg(not(target_arch = "wasm32"))]
    pub(super) fn start(inputs: Inputs) -> Self {
        let (sender, receiver) = channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&cancel);
        std::thread::spawn(move || {
            let started = std::time::Instant::now();
            let steps = sender.clone();
            let result = train_paths(
                &PathTrainRequest {
                    id: &inputs.id,
                    name: &inputs.name,
                    created: &inputs.created,
                    lessons: &inputs.lessons,
                    hidden: inputs.hidden,
                    epochs: inputs.epochs,
                    seed: 1,
                    cancelled: &|| flag.load(Ordering::Relaxed),
                },
                &|| started.elapsed().as_secs_f32(),
                &mut |PathProgress::Step { percent, message }| {
                    let _ = steps.send(Event::Step(percent, message));
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
        }
    }

    pub(super) fn stop(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }

    /// Take everything that arrived; `Some` when the job has ended.
    pub(super) fn poll(&mut self) -> Option<Result<Box<PathModel>, String>> {
        loop {
            match self.receiver.try_recv() {
                Ok(Event::Step(percent, message)) => {
                    self.percent = percent.max(self.percent);
                    if self.log.last() != Some(&message) && !message.starts_with("Learning: round")
                    {
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
