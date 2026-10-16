//! Spline preview planning: the key that identifies a plan, the inputs a fit is made from, and the
//! background worker that makes it. No semantic mutation.
use super::Preview;
use crate::terrain_path::{self, FittedTile};
#[cfg(not(target_arch = "wasm32"))]
use eframe::egui;

const FNV_OFFSET: u64 = 0xCBF2_9CE4_8422_2325;
const FNV_PRIME: u64 = 0x0100_0000_01B3;

/// How the path is fitted: the tool's shape settings.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(in crate::app) struct PathOptions {
    pub width: f32,
    pub smooth_curve: bool,
    pub edge_smoothing: f32,
}

impl PathOptions {
    pub(in crate::app) fn from_settings(settings: &crate::paint::SplineSettings) -> Self {
        Self {
            width: settings.width,
            smooth_curve: settings.smooth_curve,
            edge_smoothing: settings.edge_smoothing,
        }
    }
}

/// The centre line a path is fitted along: the placed points (and the cursor's), smoothed into a
/// curve when asked.
pub(in crate::app) fn path_line(
    points: &[(f32, f32)],
    pending: Option<(f32, f32)>,
    smooth_curve: bool,
) -> Vec<(f64, f64)> {
    let all: Vec<(f64, f64)> = points
        .iter()
        .copied()
        .chain(pending)
        .map(|p| (f64::from(p.0), f64::from(p.1)))
        .collect();
    if smooth_curve {
        terrain_path::smooth(&all)
    } else {
        all
    }
}

/// The overlay tiles of a path.
pub(in crate::app) fn fit_points(
    points: &[(f32, f32)],
    pending: Option<(f32, f32)>,
    options: PathOptions,
) -> (Vec<(f64, f64)>, Vec<FittedTile>) {
    let line = path_line(points, pending, options.smooth_curve);
    let tiles = terrain_path::fit(
        &line,
        f64::from(options.width),
        f64::from(options.edge_smoothing),
    );
    (line, tiles)
}

fn hash_words(words: impl Iterator<Item = u32>) -> u64 {
    words
        .flat_map(u32::to_le_bytes)
        .fold(FNV_OFFSET, |hash, byte| {
            (hash ^ u64::from(byte)).wrapping_mul(FNV_PRIME)
        })
}

/// Identity of what a plan is made from: the placed points, the cursor, the overlay, the plane and
/// the shape settings.
pub(super) fn plan_key(
    points: &[(f32, f32)],
    pending: Option<(f32, f32)>,
    overlay: u16,
    plane: u8,
    options: PathOptions,
) -> u64 {
    let pending = pending.map_or([0, 0, 0], |p| [1, p.0.to_bits(), p.1.to_bits()]);
    hash_words(
        points
            .iter()
            .flat_map(|p| [p.0.to_bits(), p.1.to_bits()])
            .chain(pending)
            .chain([
                u32::from(overlay),
                u32::from(plane),
                options.width.to_bits(),
                u32::from(options.smooth_curve),
                options.edge_smoothing.to_bits(),
                points.len() as u32,
            ]),
    )
}

/// Everything a fit is made from, copied off the editor so another thread can make it.
pub(super) struct PlanInput {
    pub(super) key: u64,
    pub(super) points: Vec<(f32, f32)>,
    pub(super) pending: Option<(f32, f32)>,
    pub(super) overlay_id: u16,
    pub(super) options: PathOptions,
}

impl PlanInput {
    pub(super) fn make(self) -> Preview {
        let (line, paints) = fit_points(&self.points, self.pending, self.options);
        Preview {
            key: self.key,
            overlay_id: self.overlay_id,
            paints,
            line,
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
    /// A short quiet period prevents expensive plans for every pointer sample.
    const DEBOUNCE: std::time::Duration = std::time::Duration::from_millis(24);

    pub(super) fn new(first: PlanInput, repaint: Option<egui::Context>) -> Self {
        let requested = first.key;
        let (request, requests) = std::sync::mpsc::channel::<PlanInput>();
        let (results, result) = std::sync::mpsc::channel::<Preview>();
        let _ = request.send(first);
        std::thread::spawn(move || {
            while let Ok(mut input) = requests.recv() {
                // Wait briefly for the pointer to settle, always replacing the pending input with
                // the newest one. Only one plan is ever running at a time.
                loop {
                    match requests.recv_timeout(Self::DEBOUNCE) {
                        Ok(newer) => input = newer,
                        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => break,
                        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => return,
                    }
                }
                if results.send(input.make()).is_err() {
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
        if input.key == self.requested {
            return None;
        }
        let key = input.key;
        match self.request.send(input) {
            Ok(()) => {
                self.requested = key;
                None
            }
            Err(error) => Some(error.0),
        }
    }
}
