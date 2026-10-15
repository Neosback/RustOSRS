//! Spline control points, preview worker and stale-preview admission. No semantic mutation.
use super::InteractionRuntime;
use crate::terrain_path::{self, FittedTile};
use eframe::egui;
const FNV_OFFSET: u64 = 0xCBF2_9CE4_8422_2325;
const FNV_PRIME: u64 = 0x0100_0000_01B3;

#[derive(Debug, Default)]
pub(super) struct SplineState {
    /// Control points in world tile coordinates.
    points: Vec<(f32, f32)>,
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
pub(in crate::app) struct Preview {
    pub(in crate::app) key: u64,
    pub(in crate::app) overlay_id: u16,
    pub(in crate::app) paints: Vec<FittedTile>,
    /// The exact smoothed centre line used by the fitter.
    pub(in crate::app) lines: Vec<Vec<(f32, f32)>>,
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

impl InteractionRuntime {
    pub(in crate::app) fn spline_points(&self) -> &[(f32, f32)] {
        &self.spline.points
    }

    pub(in crate::app) fn begin_spline_drag(&mut self, index: usize) {
        if index < self.spline.points.len() {
            self.spline.drag = Some(index);
        }
    }

    pub(in crate::app) fn append_spline_point_and_drag(&mut self, point: (f32, f32)) {
        self.spline.points.push(point);
        self.spline.drag = Some(self.spline.points.len() - 1);
    }

    pub(in crate::app) fn insert_spline_point_and_drag(&mut self, index: usize, point: (f32, f32)) {
        let index = index.min(self.spline.points.len());
        self.spline.points.insert(index, point);
        self.spline.drag = Some(index);
    }

    pub(in crate::app) fn update_spline_drag(&mut self, point: (f32, f32)) {
        if let Some(index) = self.spline.drag
            && let Some(current) = self.spline.points.get_mut(index)
        {
            *current = point;
        }
    }

    pub(in crate::app) fn finish_spline_drag(&mut self) {
        self.spline.drag = None;
    }

    pub(in crate::app) fn cancel_spline_drag(&mut self) {
        self.spline.drag = None;
    }

    pub(in crate::app) fn remove_spline_point(&mut self, index: usize) {
        if index < self.spline.points.len() {
            self.spline.points.remove(index);
            self.spline.drag = None;
        }
    }

    pub(in crate::app) fn pop_spline_point(&mut self) {
        self.spline.points.pop();
        self.spline.drag = None;
    }

    pub(in crate::app) fn clear_spline_points(&mut self) {
        self.spline.points.clear();
        self.spline.drag = None;
    }
}

impl InteractionRuntime {
    pub(in crate::app) fn spline_drag(&self) -> Option<usize> {
        self.spline.drag
    }
    pub(in crate::app) fn spline_preview(&self) -> Option<&Preview> {
        self.spline.preview.as_ref()
    }
    pub(in crate::app) fn current_spline_preview(&self) -> Option<&Preview> {
        self.spline.current_preview()
    }
    pub(in crate::app) fn spline_preview_is_current(&self) -> bool {
        self.spline.current_preview().is_some()
    }
    pub(in crate::app) fn register_spline_repaint(&mut self, ctx: &egui::Context) {
        self.spline.ctx.get_or_insert_with(|| ctx.clone());
    }
    pub(in crate::app) fn replace_spline_points(&mut self, points: Vec<(f32, f32)>) {
        self.spline.points = points;
        self.spline.drag = None;
    }
    /// Everything the current path is planned from, or `None` when there is nothing to plan.
    fn plan_input(
        &self,
        spline: &crate::paint::SplineSettings,
        overlay_id: Option<u16>,
    ) -> Option<PlanInput> {
        if self.spline.points.len() < 2 {
            return None;
        }
        let overlay_id = overlay_id?;
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
    pub(in crate::app) fn update_spline_preview(
        &mut self,
        settings: &crate::paint::SplineSettings,
        overlay_id: Option<u16>,
    ) {
        let Some(input) = self.plan_input(settings, overlay_id) else {
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
    pub(in crate::app) fn plan_spline_now(
        &mut self,
        settings: &crate::paint::SplineSettings,
        overlay_id: Option<u16>,
    ) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.spline.worker = None;
        }
        match self.plan_input(settings, overlay_id) {
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
}

#[cfg(test)]
mod tests;
