//! Spline preview planning: the key that identifies a plan, the inputs a fit is made from, and the
//! background worker that makes it. No semantic mutation.
use super::Preview;
use crate::terrain_path::{
    self, ControlPoint, EDGE_WEIGHT, FitOptions, FittedTile, PRIOR_WEIGHT, PathSample,
};
#[cfg(not(target_arch = "wasm32"))]
use eframe::egui;
use std::collections::BTreeSet;
use std::sync::Arc;

const FNV_OFFSET: u64 = 0xCBF2_9CE4_8422_2325;
const FNV_PRIME: u64 = 0x0100_0000_01B3;

/// The tiles a path must not paint over (other overlays that are areas, not paths), with a hash of
/// the set so a change replans.
#[derive(Debug, Clone, Default)]
pub(in crate::app) struct BlockedTiles {
    pub(in crate::app) tiles: Arc<BTreeSet<(i32, i32)>>,
    pub(in crate::app) key: u64,
}

/// The next segment, from the last control point to the cursor, planned alongside the path so the
/// preview shows what the next click will add. It is never laid.
#[derive(Debug, Clone)]
pub(in crate::app) struct Ahead {
    pub(in crate::app) pending: (f32, f32),
    pub(in crate::app) paints: Vec<FittedTile>,
}

/// Identity of the placed path: control points (with widths and corners), overlay, plane and the
/// blocked tiles. The cursor is not part of it.
pub(super) struct PlanKey<'a> {
    pub(super) controls: &'a [ControlPoint],
    pub(super) overlay: u16,
    pub(super) plane: u8,
    pub(super) blocked: u64,
}

fn hash_words(words: impl Iterator<Item = u32>) -> u64 {
    words
        .flat_map(u32::to_le_bytes)
        .fold(FNV_OFFSET, |hash, byte| {
            (hash ^ u64::from(byte)).wrapping_mul(FNV_PRIME)
        })
}

pub(super) fn plan_key(key: &PlanKey<'_>) -> u64 {
    hash_words(
        key.controls
            .iter()
            .flat_map(|c| {
                [
                    c.x.to_bits(),
                    c.y.to_bits(),
                    c.width.to_bits(),
                    u32::from(c.corner),
                    c.wander as u32,
                    c.seed,
                    u32::from(c.smooth_ground),
                ]
            })
            .chain([
                u32::from(key.overlay),
                u32::from(key.plane),
                key.blocked as u32,
                (key.blocked >> 32) as u32,
            ]),
    )
}

/// A request is the placed path's key plus where the cursor is.
pub(super) fn request_key(key: u64, pending: Option<(f32, f32)>) -> u64 {
    let head = [key as u32, (key >> 32) as u32];
    let tail = pending.map_or([0, 0, 0], |p| [1, p.0.to_bits(), p.1.to_bits()]);
    hash_words(head.into_iter().chain(tail))
}

/// A stable hash of a blocked set.
pub(in crate::app) fn blocked_key(tiles: &BTreeSet<(i32, i32)>) -> u64 {
    hash_words(
        tiles
            .iter()
            .flat_map(|t| [t.0 as u32, t.1 as u32])
            .chain([tiles.len() as u32]),
    )
}

/// Everything a fit is made from, copied off the editor so another thread can make it.
pub(super) struct PlanInput {
    /// Key of the placed path.
    pub(super) key: u64,
    /// Key of the placed path and the cursor.
    pub(super) request_key: u64,
    pub(super) controls: Vec<ControlPoint>,
    pub(super) pending: Option<(f32, f32)>,
    pub(super) overlay_id: u16,
    pub(super) blocked: Arc<BTreeSet<(i32, i32)>>,
}

impl PlanInput {
    fn fit(&self, line: &[PathSample]) -> terrain_path::Fit {
        let blocked = |tile: (i32, i32)| self.blocked.contains(&tile);
        terrain_path::fit_path_overlay(
            line,
            &FitOptions {
                edge_weight: EDGE_WEIGHT,
                prior_weight: PRIOR_WEIGHT,
                blocked: &blocked,
            },
        )
    }

    /// Fit the placed path, reusing `previous` when it was made for the same placed path, then
    /// fit the path extended to the cursor.
    pub(super) fn make(self, previous: Option<&Preview>) -> Preview {
        let reuse = previous.filter(|p| p.key == self.key && p.overlay_id == self.overlay_id);
        let (paints, clipped, line) = match reuse {
            Some(previous) => (
                previous.paints.clone(),
                previous.clipped.clone(),
                previous.line.clone(),
            ),
            None if self.controls.len() >= 2 => {
                let line = terrain_path::smooth_path(&self.controls);
                let fit = self.fit(&line);
                (fit.tiles, fit.clipped, line)
            }
            None => (Vec::new(), Vec::new(), Vec::new()),
        };
        let ahead = self
            .pending
            .zip(self.controls.last())
            .map(|(pending, last)| {
                let mut extended = self.controls.clone();
                extended.push(ControlPoint::new(pending.0, pending.1, last.width));
                Ahead {
                    pending,
                    paints: self.fit(&terrain_path::smooth_path(&extended)).tiles,
                }
            });
        Preview {
            key: self.key,
            request_key: self.request_key,
            overlay_id: self.overlay_id,
            paints,
            clipped,
            line,
            ahead,
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug)]
pub(super) struct PlanWorker {
    request: std::sync::mpsc::Sender<PlanInput>,
    pub(super) result: std::sync::mpsc::Receiver<Preview>,
    requested: u64,
}

#[cfg(not(target_arch = "wasm32"))]
impl PlanWorker {
    /// A short quiet period prevents expensive plans for every pointer sample while dragging.
    const DEBOUNCE: std::time::Duration = std::time::Duration::from_millis(24);

    pub(super) fn new(first: PlanInput, repaint: Option<egui::Context>) -> Self {
        let requested = first.request_key;
        let (request, requests) = std::sync::mpsc::channel::<PlanInput>();
        let (results, result) = std::sync::mpsc::channel::<Preview>();
        let _ = request.send(first);
        std::thread::spawn(move || {
            let mut last: Option<Preview> = None;
            while let Ok(mut input) = requests.recv() {
                // Wait briefly for the drag stream to settle, always replacing the pending input
                // with the newest one. Only one expensive plan is ever running at a time.
                loop {
                    match requests.recv_timeout(Self::DEBOUNCE) {
                        Ok(newer) => input = newer,
                        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => break,
                        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => return,
                    }
                }
                let preview = input.make(last.as_ref());
                last = Some(preview.clone());
                if results.send(preview).is_err() {
                    return;
                }
                if let Some(ctx) = &repaint {
                    ctx.request_repaint();
                }
            }
        });
        Self {
            request,
            result,
            requested,
        }
    }

    /// Ask for a plan; hands the input back when the worker is gone.
    pub(super) fn request(&mut self, input: PlanInput) -> Option<PlanInput> {
        if input.request_key == self.requested {
            return None;
        }
        let key = input.request_key;
        match self.request.send(input) {
            Ok(()) => {
                self.requested = key;
                None
            }
            Err(error) => Some(error.0),
        }
    }
}
