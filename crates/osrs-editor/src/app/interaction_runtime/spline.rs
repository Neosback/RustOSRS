//! Spline (path) points, preview worker and stale-preview admission. No semantic mutation.
use super::InteractionRuntime;
use crate::terrain_path::{self, FittedTile};
use eframe::egui;
mod cancel;
mod extent;
pub(in crate::app) use extent::PathExtent;
mod plan;
#[cfg(not(target_arch = "wasm32"))]
use plan::PlanWorker;
#[cfg(test)]
use plan::path_line;
pub(in crate::app) use plan::{PathOptions, fit_points};
use plan::{PlanInput, plan_key};

#[derive(Debug, Default)]
pub(super) struct SplineState {
    /// The path's points: tile centres in world tile coordinates, in the order they were clicked.
    points: Vec<(f32, f32)>,
    extent: Option<PathExtent>,
    error: Option<&'static str>,
    preview: Option<Preview>,
    /// Real terrain colours of the current preview (see [`terrain_path::PathAppearance`]).
    appearance: Option<std::sync::Arc<terrain_path::PathAppearance>>,
    /// The editor state the preview should represent. The worker may still be catching up.
    desired_key: Option<u64>,
    /// No overlay is armed, so the preview is the flat fill.
    needs_overlay: bool,
    /// A plan for this key was cancelled by the user; it stays cancelled until an edit changes the key.
    cancelled_key: Option<u64>,
    /// One background worker. Rapid changes are coalesced to the newest request.
    #[cfg(not(target_arch = "wasm32"))]
    worker: Option<PlanWorker>,
    ctx: Option<egui::Context>,
}

/// The fitted path for the placed points and the cursor's tile.
#[derive(Debug, Default, Clone)]
pub(in crate::app) struct Preview {
    /// Identity of what this was planned from (points, cursor, overlay, plane, options).
    pub(in crate::app) key: u64,
    /// The overlay (zero: none chosen yet).
    pub(in crate::app) overlay_id: u16,
    pub(in crate::app) paints: Vec<FittedTile>,
    /// The centre line the path was fitted along.
    pub(in crate::app) line: Vec<(f64, f64)>,
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

impl InteractionRuntime {
    pub(in crate::app) fn spline_status(&self) -> SplineStatus {
        if let Some(error) = self.spline.error {
            return SplineStatus::Failed(error);
        }
        if self.spline.points.is_empty() {
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

    /// Whether laying the path can start: there is a point, and an overlay to lay it with. The
    /// path is fitted again when it is laid, so this does not wait for the preview.
    pub(in crate::app) fn spline_can_lay(&self) -> bool {
        !self.spline.points.is_empty() && !self.spline.needs_overlay && self.spline.error.is_none()
    }

    pub(in crate::app) fn spline_points(&self) -> &[(f32, f32)] {
        &self.spline.points
    }

    /// Add a point (a tile centre) unless it is where the last one is.
    pub(in crate::app) fn add_spline_point(&mut self, point: (f32, f32)) {
        let Some(extent) = &self.spline.extent else {
            return;
        };
        if !point.0.is_finite()
            || !point.1.is_finite()
            || !extent.contains(osrs_core::coords::MapTile::new(
                point.0.floor() as i32,
                point.1.floor() as i32,
            ))
        {
            return;
        }
        if self.spline.points.last() != Some(&point) {
            self.spline.points.push(point);
            self.spline.desired_key = None;
        }
    }

    /// Replace the whole path (scripts and tests).
    pub(in crate::app) fn replace_spline_points(&mut self, points: Vec<(f32, f32)>) {
        self.clear_spline_points();
        for point in points {
            self.add_spline_point(point);
        }
    }

    pub(in crate::app) fn pop_spline_point(&mut self) {
        if self.spline.points.pop().is_some() {
            self.spline.desired_key = None;
        }
    }

    pub(in crate::app) fn clear_spline_points(&mut self) {
        self.spline.points.clear();
        self.spline.desired_key = None;
        self.spline.preview = None;
        self.spline.error = None;
        self.spline.appearance = None;
        self.spline.cancelled_key = None;
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.spline.worker = None;
        }
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

    /// Everything the current path is planned from, or `None` when there is nothing to draw: no
    /// point placed and the cursor off the terrain.
    fn plan_input(
        &self,
        overlay_id: Option<u16>,
        pending: Option<(f32, f32)>,
        options: PathOptions,
    ) -> Option<PlanInput> {
        let extent = self.spline.extent.as_ref()?;
        let pending = pending.filter(|p| {
            p.0.is_finite()
                && p.1.is_finite()
                && extent.contains(osrs_core::coords::MapTile::new(
                    p.0.floor() as i32,
                    p.1.floor() as i32,
                ))
        });
        if self.spline.points.is_empty() && pending.is_none() {
            return None;
        }
        let overlay_id = overlay_id.unwrap_or(0);
        Some(PlanInput {
            key: plan_key(
                &self.spline.points,
                pending,
                overlay_id,
                self.view_plane,
                options,
                extent,
            ),
            points: self.spline.points.clone(),
            pending,
            overlay_id,
            options,
            extent: extent.clone(),
        })
    }

    /// Keep the preview in step with the points, the cursor and the options.
    pub(in crate::app) fn update_spline_preview(
        &mut self,
        overlay_id: Option<u16>,
        pending: Option<(f32, f32)>,
        options: PathOptions,
    ) {
        self.spline.needs_overlay = overlay_id.is_none();
        let Some(input) = self.plan_input(overlay_id, pending, options) else {
            self.spline.desired_key = None;
            self.spline.preview = None;
            #[cfg(not(target_arch = "wasm32"))]
            {
                self.spline.worker = None;
            }
            return;
        };
        // Up to date: nothing to do.
        if self.spline.desired_key == Some(input.key) && self.spline.current_preview().is_some() {
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
                            // Results for superseded states are harmless. Keep the previous
                            // preview until the worker catches up, but never promote a stale one.
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
            if self.spline.current_preview().is_some() {
                return;
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
            if self.spline.current_preview().is_none() {
                self.spline.preview = Some(input.make());
            }
        }
    }

    /// Plan now, on this thread.
    #[cfg(test)]
    pub(in crate::app) fn plan_spline_now(
        &mut self,
        overlay_id: Option<u16>,
        pending: Option<(f32, f32)>,
        options: PathOptions,
    ) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.spline.worker = None;
        }
        self.spline.needs_overlay = overlay_id.is_none();
        match self.plan_input(overlay_id, pending, options) {
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
