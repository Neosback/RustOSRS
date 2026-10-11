//! Background animation playback: owns the [`AnimationSystem`] on a worker thread so posing and
//! geometry building never stall the render thread.
//!
//! The owner sends region changes and rebuild requests; at most one rebuild is in flight, and the
//! cycles that elapse while it runs are accumulated into the next request.

use crate::{AnimatedInstance, AnimationSystem};
use osrs_render::SceneGeometry;
use std::{
    sync::mpsc::{Receiver, Sender, channel},
    thread::JoinHandle,
};

enum Request {
    Insert((i32, i32), Vec<AnimatedInstance>),
    Remove((i32, i32)),
    Rebuild {
        cycles: i32,
        center_tile: (i32, i32),
        radius_tiles: i32,
        /// Build even when no instance changed frame (regions or camera moved).
        force: bool,
    },
    Shutdown,
}

/// Result of one rebuild request.
pub struct AnimationUpdate {
    /// `None` when nothing changed and no rebuild was forced.
    pub geometry: Option<SceneGeometry>,
}

pub struct AnimationWorker {
    requests: Sender<Request>,
    updates: Receiver<AnimationUpdate>,
    worker: Option<JoinHandle<()>>,
    in_flight: bool,
    /// Rebuild forced by changes made while a rebuild was in flight.
    pending_force: bool,
}

impl AnimationWorker {
    pub fn spawn() -> Self {
        let (request_tx, request_rx) = channel::<Request>();
        let (update_tx, update_rx) = channel::<AnimationUpdate>();
        let worker = std::thread::Builder::new()
            .name("animation".to_owned())
            .spawn(move || {
                let mut system = AnimationSystem::new();
                while let Ok(request) = request_rx.recv() {
                    match request {
                        Request::Shutdown => break,
                        Request::Insert(key, instances) => system.insert_region(key, instances),
                        Request::Remove(key) => system.remove_region(key),
                        Request::Rebuild {
                            cycles,
                            center_tile,
                            radius_tiles,
                            force,
                        } => {
                            let advanced = system.advance(cycles);
                            let geometry = (advanced || force)
                                .then(|| system.build_geometry(center_tile, radius_tiles));
                            if update_tx.send(AnimationUpdate { geometry }).is_err() {
                                break;
                            }
                        }
                    }
                }
            })
            .ok();
        Self {
            requests: request_tx,
            updates: update_rx,
            worker,
            in_flight: false,
            pending_force: false,
        }
    }

    pub fn insert_region(&mut self, key: (i32, i32), instances: Vec<AnimatedInstance>) {
        let _ = self.requests.send(Request::Insert(key, instances));
        self.pending_force = true;
    }

    pub fn remove_region(&mut self, key: (i32, i32)) {
        let _ = self.requests.send(Request::Remove(key));
        self.pending_force = true;
    }

    /// Mark the geometry stale (the camera moved far enough to change the drawn set).
    pub fn force_rebuild(&mut self) {
        self.pending_force = true;
    }

    /// Whether a new rebuild can be requested.
    pub fn is_idle(&self) -> bool {
        !self.in_flight
    }

    /// Ask for a rebuild advanced by `cycles` game cycles. Ignored while one is in flight.
    pub fn request(&mut self, cycles: i32, center_tile: (i32, i32), radius_tiles: i32) -> bool {
        if self.in_flight {
            return false;
        }
        self.in_flight = true;
        let force = std::mem::take(&mut self.pending_force);
        let _ = self.requests.send(Request::Rebuild {
            cycles,
            center_tile,
            radius_tiles,
            force,
        });
        true
    }

    /// Finished rebuild, if any.
    pub fn poll(&mut self) -> Option<AnimationUpdate> {
        let update = self.updates.try_recv().ok()?;
        self.in_flight = false;
        Some(update)
    }
}

impl Drop for AnimationWorker {
    fn drop(&mut self) {
        let _ = self.requests.send(Request::Shutdown);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
