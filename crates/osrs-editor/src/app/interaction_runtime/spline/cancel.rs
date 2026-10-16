//! Explicit cancellation of a running spline plan. A cancelled plan stays cancelled for the same
//! points and options; any edit changes the plan key and plans again. No semantic mutation.
use super::{InteractionRuntime, SplineStatus};

impl InteractionRuntime {
    /// Stop planning the current path. Only meaningful while a plan is running.
    pub(in crate::app) fn cancel_spline_plan(&mut self) {
        if self.spline_status() != SplineStatus::Updating {
            return;
        }
        self.spline.cancelled_key = self.spline.desired_key;
        #[cfg(not(target_arch = "wasm32"))]
        {
            // Dropping the worker ends its thread; a result in flight is discarded.
            self.spline.worker = None;
        }
    }

    /// Plan the current path again after a cancel.
    pub(in crate::app) fn resume_spline_plan(&mut self) {
        self.spline.cancelled_key = None;
    }

    /// Whether `key` is the cancelled plan; an edit (a different key) lifts the cancellation.
    pub(super) fn cancelled_plan_stays_cancelled(&mut self, key: u64) -> bool {
        if self.spline.cancelled_key == Some(key) {
            #[cfg(not(target_arch = "wasm32"))]
            {
                self.spline.worker = None;
            }
            return true;
        }
        self.spline.cancelled_key = None;
        false
    }
}
