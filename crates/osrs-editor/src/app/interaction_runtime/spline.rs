//! Spline control points, preview worker and stale-preview admission. No semantic mutation.
use super::InteractionRuntime;
use crate::terrain_path::{self, FittedTile};
use eframe::egui;
mod attrs;
mod cancel;
mod plan;
#[cfg(test)]
use crate::terrain_path::ControlPoint;
pub(in crate::app) use attrs::PointAttr;
#[cfg(test)]
pub(in crate::app) use attrs::{MAX_WIDTH, MIN_WIDTH};
#[cfg(not(target_arch = "wasm32"))]
use plan::PlanWorker;
pub(in crate::app) use plan::{Ahead, BlockedTiles};
use plan::{PlanInput, PlanKey, plan_key, request_key};

#[derive(Debug, Default)]
pub(super) struct SplineState {
    /// Control points in world tile coordinates.
    points: Vec<(f32, f32)>,
    /// Width and corner flag of each control point (parallel to `points`).
    attrs: Vec<PointAttr>,
    /// The width new points start with (the Path preset until the app says otherwise).
    default_width: Option<f32>,
    /// What new points (and the stretch after them) start with.
    default_wander: crate::terrain_path::Wander,
    default_smooth: bool,
    /// Seeds are handed out in order so no two points share one.
    next_seed: u32,
    /// Tiles the path must not paint over, from the loaded terrain.
    blocked: BlockedTiles,
    blocked_scope: Option<attrs::BlockedScope>,
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
    /// Key of the placed path this preview plans (the cursor is not part of it).
    pub(in crate::app) key: u64,
    /// Key of the placed path and the cursor position it was made for.
    pub(in crate::app) request_key: u64,
    pub(in crate::app) overlay_id: u16,
    pub(in crate::app) paints: Vec<FittedTile>,
    /// Tiles the path covers but left alone because another overlay is there.
    pub(in crate::app) clipped: Vec<(i32, i32)>,
    /// The exact smoothed centre line, with its half-width, used by the fitter.
    pub(in crate::app) line: Vec<terrain_path::PathSample>,
    /// The path extended to the cursor, when following the cursor.
    pub(in crate::app) ahead: Option<Ahead>,
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
        /// Tiles skipped because they are other (area) overlays.
        clipped: usize,
    },
    Empty,
    Failed(&'static str),
}

impl SplineState {
    /// The width new points start with.
    fn default_width(&self) -> f32 {
        self.default_width
            .unwrap_or(crate::paint::PathStyle::Path.width())
    }

    fn current_preview(&self) -> Option<&Preview> {
        self.preview
            .as_ref()
            .filter(|preview| self.desired_key == Some(preview.key))
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
                clipped: preview.clipped.len(),
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
        let attr = self.new_spline_attr(self.spline.default_width());
        self.spline.attrs.push(attr);
        self.spline.drag = Some(self.spline.points.len() - 1);
        self.spline.selected = self.spline.drag;
        self.spline.desired_key = None;
    }

    pub(in crate::app) fn insert_spline_point_and_drag(&mut self, index: usize, point: (f32, f32)) {
        let index = index.min(self.spline.points.len());
        self.spline.points.insert(index, point);
        // An inserted point takes the width of the stretch it splits.
        let width = match (index.checked_sub(1), self.spline.attrs.get(index)) {
            (Some(before), Some(after)) => (self.spline.attrs[before].width + after.width) / 2.0,
            _ => self.spline.default_width(),
        };
        // ...and its wander and ground setting too.
        let mut attr = self.new_spline_attr(width);
        if let Some(before) = index.checked_sub(1).and_then(|i| self.spline.attrs.get(i)) {
            attr.wander = before.wander;
            attr.smooth_ground = before.smooth_ground;
        }
        self.spline.attrs.insert(index, attr);
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
            self.spline.attrs.remove(index);
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
        self.spline.attrs.clear();
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
    /// Replace the path with points that all take the default width.
    #[cfg(test)]
    pub(in crate::app) fn replace_spline_points(&mut self, points: Vec<(f32, f32)>) {
        let width = self.spline.default_width();
        self.replace_spline_controls(
            points
                .into_iter()
                .map(|p| ControlPoint::new(p.0, p.1, width))
                .collect(),
        );
    }
    /// The key of the placed path under `overlay`.
    fn placed_key(&self, overlay: u16) -> u64 {
        plan_key(&PlanKey {
            controls: &self.spline_controls(),
            overlay,
            plane: self.view_plane,
            blocked: self.spline.blocked.key,
        })
    }

    /// Everything the current path is planned from, or `None` when there is nothing to plan.
    ///
    /// Two placed points make a path. One placed point plus the cursor still plans, so the first
    /// segment can be previewed before the second click.
    fn plan_input(
        &self,
        overlay_id: Option<u16>,
        pending: Option<(f32, f32)>,
    ) -> Option<PlanInput> {
        let pending = pending.filter(|p| p.0.is_finite() && p.1.is_finite());
        if self.spline.points.len() < 2 && !(self.spline.points.len() == 1 && pending.is_some()) {
            return None;
        }
        let overlay_id = overlay_id?;
        let key = self.placed_key(overlay_id);
        Some(PlanInput {
            key,
            request_key: request_key(key, pending),
            controls: self.spline_controls(),
            pending,
            overlay_id,
            blocked: self.spline.blocked.tiles.clone(),
        })
    }

    /// Keep the preview in step with the points and (following it) the cursor.
    pub(in crate::app) fn update_spline_preview(
        &mut self,
        overlay_id: Option<u16>,
        pending: Option<(f32, f32)>,
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
        let Some(input) = self.plan_input(overlay_id, pending) else {
            self.spline.desired_key = None;
            self.spline.preview = None;
            #[cfg(not(target_arch = "wasm32"))]
            {
                self.spline.worker = None;
            }
            return;
        };
        // Up to date for the placed path and the cursor: nothing to do.
        if self.spline.desired_key == Some(input.key)
            && self
                .spline
                .current_preview()
                .is_some_and(|preview| preview.request_key == input.request_key)
        {
            return;
        }
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
            // The browser has no worker thread: plan on this thread when the plan changed.
            if self
                .spline
                .current_preview()
                .is_none_or(|preview| preview.request_key != input.request_key)
            {
                self.spline.preview = Some(input.make(None));
            }
        }
    }

    /// Plan now, on this thread, so what is laid is what the points and options say.
    #[cfg(test)]
    pub(in crate::app) fn plan_spline_now(&mut self, overlay_id: Option<u16>) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.spline.worker = None;
        }
        match self.plan_input(overlay_id, None) {
            Some(input) => {
                self.spline.desired_key = Some(input.key);
                if self.spline.current_preview().is_none() {
                    self.spline.preview = Some(input.make(None));
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
