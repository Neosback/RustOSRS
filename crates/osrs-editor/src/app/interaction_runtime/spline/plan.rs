//! Spline preview planning: the key that identifies a plan, the inputs a fit is made from, and the
//! background worker that makes it. No semantic mutation.
use super::Preview;
use crate::paint::PathCurve;
use crate::terrain_path::{self, FittedTile};
#[cfg(not(target_arch = "wasm32"))]
use eframe::egui;

const FNV_OFFSET: u64 = 0xCBF2_9CE4_8422_2325;
const FNV_PRIME: u64 = 0x0100_0000_01B3;

/// The next segment, from the last control point to the cursor, planned alongside the path so the
/// preview shows what the next click will add. It is never laid.
#[derive(Debug, Clone)]
pub(in crate::app) struct Ahead {
    pub(in crate::app) pending: (f32, f32),
    pub(in crate::app) paints: Vec<FittedTile>,
}

/// Identity of the placed path: points, options, overlay and plane. The cursor is not part of it.
pub(super) struct PlanKey<'a> {
    pub(super) points: &'a [(f32, f32)],
    pub(super) width: f32,
    pub(super) edge: f32,
    pub(super) overlay: u16,
    pub(super) plane: u8,
    pub(super) snap: bool,
    pub(super) curve: PathCurve,
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
        key.points
            .iter()
            .flat_map(|point| [point.0.to_bits(), point.1.to_bits()])
            .chain([
                key.width.to_bits(),
                key.edge.to_bits(),
                u32::from(key.overlay),
                u32::from(key.plane),
                u32::from(key.snap),
                key.curve as u32,
            ]),
    )
}

/// A request is the placed path's key plus where the cursor is.
pub(super) fn request_key(key: u64, pending: Option<(f32, f32)>) -> u64 {
    let bytes = key.to_le_bytes();
    let head = [
        u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]),
        u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]),
    ];
    let tail = pending.map_or([0, 0, 0], |p| [1, p.0.to_bits(), p.1.to_bits()]);
    hash_words(head.into_iter().chain(tail))
}

/// Everything a fit is made from, copied off the editor so another thread can make it.
pub(super) struct PlanInput {
    /// Key of the placed path.
    pub(super) key: u64,
    /// Key of the placed path and the cursor.
    pub(super) request_key: u64,
    pub(super) points: Vec<(f32, f32)>,
    pub(super) pending: Option<(f32, f32)>,
    pub(super) width: f32,
    pub(super) edge_weight: f32,
    pub(super) overlay_id: u16,
    pub(super) curve: PathCurve,
}

fn centre_line(points: &[(f32, f32)], curve: PathCurve) -> Vec<(f32, f32)> {
    match curve {
        PathCurve::Smooth => terrain_path::smooth_catmull_rom(points),
        PathCurve::Straight => points.to_vec(),
    }
}

impl PlanInput {
    /// Fit the placed path, reusing `previous` when it was made for the same placed path, then
    /// fit the path extended to the cursor.
    pub(super) fn make(self, previous: Option<&Preview>) -> Preview {
        let reuse = previous.filter(|p| p.key == self.key && p.overlay_id == self.overlay_id);
        let (paints, lines) = match reuse {
            Some(previous) => (previous.paints.clone(), previous.lines.clone()),
            None if self.points.len() >= 2 => {
                let line = centre_line(&self.points, self.curve);
                let paints = terrain_path::fit_path_overlay(&line, self.width, self.edge_weight);
                (paints, vec![line])
            }
            None => (Vec::new(), Vec::new()),
        };
        let ahead = self
            .pending
            .filter(|_| !self.points.is_empty())
            .map(|pending| {
                let mut extended = self.points.clone();
                extended.push(pending);
                let line = centre_line(&extended, self.curve);
                Ahead {
                    pending,
                    paints: terrain_path::fit_path_overlay(&line, self.width, self.edge_weight),
                }
            });
        Preview {
            key: self.key,
            request_key: self.request_key,
            overlay_id: self.overlay_id,
            paints,
            lines,
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
