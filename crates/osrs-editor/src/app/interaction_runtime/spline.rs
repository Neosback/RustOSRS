//! Spline control points, preview worker and stale-preview admission. No semantic mutation.
use super::InteractionRuntime;
use crate::terrain_path::{self, FittedTile};
use eframe::egui;
mod cancel;
const FNV_OFFSET: u64 = 0xCBF2_9CE4_8422_2325;
const FNV_PRIME: u64 = 0x0100_0000_01B3;

#[derive(Debug, Default)]
pub(super) struct SplineState {
    /// Control points in world tile coordinates.
    points: Vec<(f32, f32)>,
    drag: Option<usize>,
    selected: Option<usize>,
    error: Option<&'static str>,
    preview: Option<Preview>,
    /// Real terrain colours of the current preview (see [`terrain_path::PathAppearance`]).
    appearance: Option<std::sync::Arc<terrain_path::PathAppearance>>,
    /// The editor state the preview should represent. The worker may still be catching up.
    desired_key: Option<u64>,
    /// No overlay is armed, so there is nothing to plan.
    needs_overlay: bool,
    /// A plan for this key was cancelled by the user; it stays cancelled until an edit changes the key.
    cancelled_key: Option<u64>,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::app) enum SplineStatus {
    NeedsPoints,
    NeedsOverlay,
    /// A plan for the current points is running.
    Updating,
    /// The user cancelled the running plan; edit or resume to plan again.
    Cancelled,
    Ready {
        tiles: usize,
    },
    Empty,
    Failed(&'static str),
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

fn plan_key(
    points: &[(f32, f32)],
    width: f32,
    edge: f32,
    overlay: u16,
    plane: u8,
    snap: bool,
) -> u64 {
    let words = points
        .iter()
        .flat_map(|point| [point.0.to_bits(), point.1.to_bits()])
        .chain([
            width.to_bits(),
            edge.to_bits(),
            u32::from(overlay),
            u32::from(plane),
            u32::from(snap),
        ]);
    words
        .flat_map(u32::to_le_bytes)
        .fold(FNV_OFFSET, |hash, byte| {
            (hash ^ u64::from(byte)).wrapping_mul(FNV_PRIME)
        })
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
    pub(in crate::app) fn selected_spline_point(&self) -> Option<usize> {
        self.spline.selected
    }
    pub(in crate::app) fn select_spline_point(&mut self, index: usize) {
        if index < self.spline.points.len() {
            self.spline.selected = Some(index);
        }
    }
    pub(in crate::app) fn move_spline_point(&mut self, index: usize, point: (f32, f32)) {
        if !point.0.is_finite() || !point.1.is_finite() {
            return;
        }
        if let Some(current) = self.spline.points.get_mut(index)
            && *current != point
        {
            *current = point;
            self.spline.desired_key = None;
        }
    }
    pub(in crate::app) fn spline_status(&self) -> SplineStatus {
        if let Some(error) = self.spline.error {
            return SplineStatus::Failed(error);
        }
        if self.spline.points.len() < 2 {
            return SplineStatus::NeedsPoints;
        }
        if self.spline.needs_overlay {
            return SplineStatus::NeedsOverlay;
        }
        match self.spline.current_preview() {
            Some(preview) if preview.paints.is_empty() => SplineStatus::Empty,
            Some(preview) => SplineStatus::Ready {
                tiles: preview.paints.len(),
            },
            None if self.spline.cancelled_key.is_some()
                && self.spline.cancelled_key == self.spline.desired_key =>
            {
                SplineStatus::Cancelled
            }
            None => SplineStatus::Updating,
        }
    }
    pub(in crate::app) fn spline_points(&self) -> &[(f32, f32)] {
        &self.spline.points
    }

    pub(in crate::app) fn begin_spline_drag(&mut self, index: usize) {
        if index < self.spline.points.len() {
            self.spline.drag = Some(index);
            self.spline.selected = Some(index);
        }
    }

    pub(in crate::app) fn append_spline_point_and_drag(&mut self, point: (f32, f32)) {
        self.spline.points.push(point);
        self.spline.drag = Some(self.spline.points.len() - 1);
        self.spline.selected = self.spline.drag;
        self.spline.desired_key = None;
    }

    pub(in crate::app) fn insert_spline_point_and_drag(&mut self, index: usize, point: (f32, f32)) {
        let index = index.min(self.spline.points.len());
        self.spline.points.insert(index, point);
        self.spline.drag = Some(index);
        self.spline.selected = Some(index);
        self.spline.desired_key = None;
    }

    pub(in crate::app) fn update_spline_drag(&mut self, point: (f32, f32)) {
        if let Some(index) = self.spline.drag
            && let Some(current) = self.spline.points.get_mut(index)
            && *current != point
        {
            *current = point;
            self.spline.desired_key = None;
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
            self.spline.selected = self.spline.selected.and_then(|selected| {
                if selected == index {
                    None
                } else {
                    Some(selected - usize::from(selected > index))
                }
            });
            self.spline.desired_key = None;
        }
    }

    pub(in crate::app) fn pop_spline_point(&mut self) {
        if let Some(index) = self.spline.points.len().checked_sub(1) {
            self.remove_spline_point(index);
        }
    }

    pub(in crate::app) fn clear_spline_points(&mut self) {
        self.spline.points.clear();
        self.spline.drag = None;
        self.spline.selected = None;
        self.spline.desired_key = None;
        self.spline.preview = None;
        self.spline.error = None;
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
    pub(in crate::app) fn spline_appearance(
        &self,
    ) -> Option<&std::sync::Arc<terrain_path::PathAppearance>> {
        self.spline.appearance.as_ref()
    }
    pub(in crate::app) fn set_spline_appearance(
        &mut self,
        appearance: terrain_path::PathAppearance,
    ) {
        self.spline.appearance = Some(std::sync::Arc::new(appearance));
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
        self.spline.selected = None;
        self.spline.desired_key = None;
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
        let key = plan_key(
            &self.spline.points,
            width,
            edge_weight,
            overlay_id,
            self.view_plane,
            spline.snap_points,
        );
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
        if self
            .spline
            .points
            .iter()
            .any(|point| !point.0.is_finite() || !point.1.is_finite())
        {
            self.spline.error = Some("Control points must have finite coordinates");
            self.spline.desired_key = None;
            self.spline.preview = None;
            return;
        }
        if self.spline.error == Some("Control points must have finite coordinates") {
            self.spline.error = None;
        }
        self.spline.needs_overlay = overlay_id.is_none();
        if let Some(overlay) = overlay_id
            && self.spline.current_preview().is_some()
            && self.spline.desired_key
                == Some(plan_key(
                    &self.spline.points,
                    settings.width.clamp(0.5, 24.0),
                    settings.edge_weight.clamp(0.0, 2.0),
                    overlay,
                    self.view_plane,
                    settings.snap_points,
                ))
        {
            return;
        }
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
        if self.cancelled_plan_stays_cancelled(desired_key) {
            return;
        }

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
                                self.spline.error = None;
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
                self.spline.error = Some("Preview worker stopped; retrying");
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
    #[cfg(test)]
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
