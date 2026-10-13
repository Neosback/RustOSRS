//! The Spline tool: click control points, preview a fitted OSRS overlay band, press Enter.
//!
//! There is deliberately no obstacle routing or wall mode here. The control line is smoothed with
//! deterministic Catmull-Rom sampling and the resulting band is fitted against the exact OSRS
//! shaped-terrain geometry. The fitted tiles are committed as one semantic patch / one undo step.

mod input;
mod overlay;

use super::EditorApp;
use super::editing::PathTile;
use crate::interact;
use crate::paint::Tool;
use crate::style::ColorKey;
use crate::terrain_path::{self, FittedTile};
use eframe::egui::{self, Color32, Pos2, Rect, Stroke};

/// A click this close (in tiles) to a control point grabs it instead of adding a new one.
const GRAB_DISTANCE: f32 = 0.9;
const FNV_OFFSET: u64 = 0xCBF2_9CE4_8422_2325;
const FNV_PRIME: u64 = 0x0100_0000_01B3;

#[derive(Debug, Default)]
pub struct SplineState {
    /// Control points in world tile coordinates.
    pub points: Vec<(f32, f32)>,
    drag: Option<usize>,
    preview: Option<Preview>,
    /// The editor state the preview should represent. The worker may still be catching up.
    desired_key: Option<u64>,
    /// One background worker. Rapid edits are coalesced to the newest request.
    #[cfg(not(target_arch = "wasm32"))]
    worker: Option<PlanWorker>,
    ctx: Option<egui::Context>,
}

#[derive(Debug, Default, Clone)]
struct Preview {
    key: u64,
    overlay_id: u16,
    paints: Vec<FittedTile>,
    /// The exact smoothed centre line used by the fitter.
    lines: Vec<Vec<(f32, f32)>>,
}

impl SplineState {
    fn current_preview(&self) -> Option<&Preview> {
        self.preview
            .as_ref()
            .filter(|preview| self.desired_key == Some(preview.key))
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug)]
struct PlanWorker {
    request: std::sync::mpsc::Sender<PlanInput>,
    result: std::sync::mpsc::Receiver<Preview>,
    requested_key: u64,
}

fn fnv(text: &str) -> u64 {
    let mut hash = FNV_OFFSET;
    for byte in text.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

fn distance(a: (f32, f32), b: (f32, f32)) -> f32 {
    ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt()
}

/// Distance from `p` to the segment `a`-`b`.
fn segment_distance(p: (f32, f32), a: (f32, f32), b: (f32, f32)) -> f32 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let length_squared = dx * dx + dy * dy;
    let t = if length_squared > 0.0 {
        (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / length_squared).clamp(0.0, 1.0)
    } else {
        0.0
    };
    distance(p, (a.0 + dx * t, a.1 + dy * t))
}

/// Everything a fit is made from, copied off the editor so another thread can make it.
struct PlanInput {
    key: u64,
    points: Vec<(f32, f32)>,
    width: f32,
    edge_weight: f32,
    overlay_id: u16,
}

impl PlanInput {
    fn make(self) -> Preview {
        let line = terrain_path::smooth_catmull_rom(&self.points);
        let paints = terrain_path::fit_path_overlay(&line, self.width, self.edge_weight);
        Preview {
            key: self.key,
            overlay_id: self.overlay_id,
            paints,
            lines: vec![line],
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl PlanWorker {
    /// A short quiet period prevents expensive plans for every pointer sample while dragging.
    const DEBOUNCE: std::time::Duration = std::time::Duration::from_millis(24);

    fn new(first: PlanInput, repaint: Option<egui::Context>) -> Self {
        let requested_key = first.key;
        let (request, requests) = std::sync::mpsc::channel::<PlanInput>();
        let (results, result) = std::sync::mpsc::channel::<Preview>();
        let _ = request.send(first);
        std::thread::spawn(move || {
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
                let preview = input.make();
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
            requested_key,
        }
    }

    fn request(&mut self, input: PlanInput) -> Option<PlanInput> {
        if input.key == self.requested_key {
            return None;
        }
        let key = input.key;
        match self.request.send(input) {
            Ok(()) => {
                self.requested_key = key;
                None
            }
            Err(error) => Some(error.0),
        }
    }
}

impl EditorApp {
    /// Tiles the current path would change.
    pub(super) fn spline_tile_count(&self) -> usize {
        self.spline
            .current_preview()
            .map_or(0, |preview| preview.paints.len())
    }

    /// The overlay id the path lays, when the brush sets one.
    fn laying_overlay(&self) -> Option<u16> {
        let brush = &self.prefs.brush;
        match brush.overlay.fill {
            crate::paint::Fill::Set(id) if brush.overlay.armed => Some(id),
            _ => None,
        }
    }

    /// Everything the current path is planned from, or `None` when there is nothing to plan.
    fn plan_input(&self) -> Option<PlanInput> {
        if self.spline.points.len() < 2 {
            return None;
        }
        let overlay_id = self.laying_overlay()?;
        let spline = &self.prefs.spline;
        let width = spline.width.clamp(0.5, 24.0);
        let edge_weight = spline.edge_weight.clamp(0.0, 2.0);
        let key = fnv(&format!(
            "{:?}|{}|{}|{}|{}|{}",
            self.spline.points,
            width.to_bits(),
            edge_weight.to_bits(),
            overlay_id,
            self.view_plane,
            spline.snap_points,
        ));
        Some(PlanInput {
            key,
            points: self.spline.points.clone(),
            width,
            edge_weight,
            overlay_id,
        })
    }

    /// Keep the preview in step with the points and options.
    pub(super) fn spline_update(&mut self) {
        let Some(input) = self.plan_input() else {
            self.spline.desired_key = None;
            self.spline.preview = None;
            #[cfg(not(target_arch = "wasm32"))]
            {
                self.spline.worker = None;
            }
            return;
        };
        let desired_key = input.key;
        self.spline.desired_key = Some(desired_key);

        #[cfg(not(target_arch = "wasm32"))]
        {
            let mut disconnected = false;
            if let Some(worker) = &mut self.spline.worker {
                loop {
                    match worker.result.try_recv() {
                        Ok(preview) => {
                            // Results for superseded drag states are harmless. Keep the previous
                            // tile preview until the worker catches up, but never promote stale
                            // warnings/counts or a stale planned centre line.
                            if preview.key == desired_key {
                                self.spline.preview = Some(preview);
                            }
                        }
                        Err(std::sync::mpsc::TryRecvError::Empty) => break,
                        Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                            disconnected = true;
                            break;
                        }
                    }
                }
            }
            if disconnected {
                self.spline.worker = None;
            }

            if self.spline.worker.is_none() {
                self.spline.worker = Some(PlanWorker::new(input, self.spline.ctx.clone()));
                return;
            }

            let retry = {
                self.spline
                    .worker
                    .as_mut()
                    .and_then(|worker| worker.request(input))
            };
            if let Some(input) = retry {
                self.spline.worker = Some(PlanWorker::new(input, self.spline.ctx.clone()));
            }
        }

        #[cfg(target_arch = "wasm32")]
        {
            if self.spline.current_preview().is_none() {
                self.spline.preview = Some(input.make());
            }
        }
    }

    /// Plan now, on this thread, so what is laid is what the points and options say.
    fn spline_plan_now(&mut self) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.spline.worker = None;
        }
        match self.plan_input() {
            Some(input) => {
                self.spline.desired_key = Some(input.key);
                if self.spline.current_preview().is_none() {
                    self.spline.preview = Some(input.make());
                }
            }
            None => {
                self.spline.desired_key = None;
                self.spline.preview = None;
            }
        }
    }

    /// Everything about the current fitted path in one copyable block.
    pub(super) fn spline_report(&self) -> String {
        let spline = &self.prefs.spline;
        let raw: Vec<String> = self
            .spline
            .points
            .iter()
            .map(|p| format!("{:.3},{:.3}", p.0, p.1))
            .collect();
        let mut text = format!(
            "PATH plane={} points={}\nwidth={:.2} edge_weight={:.2} overlay={:?}\n",
            self.view_plane,
            raw.join(";"),
            spline.width,
            spline.edge_weight,
            self.prefs.brush.overlay.fill,
        );
        if let Some(preview) = &self.spline.preview {
            text.push_str(&format!("fitted {} tiles\n", preview.paints.len()));
        }
        text
    }
}
