//! Per-point attributes of a spline path: each control point has its own width and may be a
//! corner. Widths and corners are part of the plan key, so editing them replans. No semantic
//! mutation.
use super::InteractionRuntime;
use super::plan::{BlockedTiles, blocked_key};
use crate::terrain_path::{ControlPoint, Wander};
use std::collections::BTreeSet;
use std::sync::Arc;

/// Narrowest and widest a point may be, in tiles.
pub(in crate::app) const MIN_WIDTH: f32 = 1.0;
pub(in crate::app) const MAX_WIDTH: f32 = 24.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(in crate::app) struct PointAttr {
    pub(in crate::app) width: f32,
    pub(in crate::app) corner: bool,
    /// How the stretch after this point wanders.
    pub(in crate::app) wander: Wander,
    /// Whether the ground is smoothed along the stretch after this point.
    pub(in crate::app) smooth_ground: bool,
    /// Identity of this point's wander, kept while other points move.
    pub(in crate::app) seed: u32,
}

fn clamp_width(width: f32) -> f32 {
    if width.is_finite() {
        width.clamp(MIN_WIDTH, MAX_WIDTH)
    } else {
        MIN_WIDTH
    }
}

impl InteractionRuntime {
    /// The width new control points start with (the Trail / Path / Road preset).
    pub(in crate::app) fn set_spline_default_width(&mut self, width: f32) {
        self.spline.default_width = Some(clamp_width(width));
    }

    pub(in crate::app) fn spline_attrs(&self) -> &[PointAttr] {
        &self.spline.attrs
    }

    /// The control points with their widths and corners.
    pub(in crate::app) fn spline_controls(&self) -> Vec<ControlPoint> {
        self.spline
            .points
            .iter()
            .zip(&self.spline.attrs)
            .map(|(point, attr)| ControlPoint {
                x: point.0,
                y: point.1,
                width: attr.width,
                corner: attr.corner,
                wander: attr.wander,
                seed: attr.seed,
                smooth_ground: attr.smooth_ground,
            })
            .collect()
    }

    pub(in crate::app) fn set_spline_point_width(&mut self, index: usize, width: f32) {
        let width = clamp_width(width);
        if let Some(attr) = self.spline.attrs.get_mut(index)
            && attr.width != width
        {
            attr.width = width;
            self.spline.desired_key = None;
        }
    }

    /// Set a width for the selected point, or for every point when none is selected.
    pub(in crate::app) fn apply_spline_width(&mut self, width: f32) {
        self.spline.default_width = Some(clamp_width(width));
        match self.spline.selected {
            Some(index) => self.set_spline_point_width(index, width),
            None => {
                for index in 0..self.spline.attrs.len() {
                    self.set_spline_point_width(index, width);
                }
            }
        }
    }

    /// Make the selected point (or every point) wider or narrower by `delta` tiles.
    pub(in crate::app) fn nudge_spline_width(&mut self, delta: f32) {
        match self.spline.selected {
            Some(index) => {
                if let Some(attr) = self.spline.attrs.get(index).copied() {
                    self.set_spline_point_width(index, attr.width + delta);
                }
            }
            None => {
                for index in 0..self.spline.attrs.len() {
                    let width = self.spline.attrs[index].width;
                    self.set_spline_point_width(index, width + delta);
                }
            }
        }
        self.spline.default_width = Some(clamp_width(self.spline.default_width() + delta));
    }

    pub(in crate::app) fn toggle_spline_corner(&mut self, index: usize) {
        if let Some(attr) = self.spline.attrs.get_mut(index) {
            attr.corner = !attr.corner;
            self.spline.desired_key = None;
        }
    }

    /// Follow the tool's toggles: when one changed, apply it to the selected point's stretch (or
    /// all of them) and to new points. Points edited by hand keep their own setting until a
    /// toggle changes again.
    pub(in crate::app) fn sync_spline_defaults(&mut self, wander: Wander, smooth_ground: bool) {
        if wander != self.spline.default_wander {
            self.apply_spline_wander(wander);
        }
        if smooth_ground != self.spline.default_smooth {
            self.apply_spline_smooth_ground(smooth_ground);
        }
    }

    /// Set how the stretch after the selected point wanders, or after every point when none is
    /// selected. The setting also becomes the default for new points.
    pub(in crate::app) fn apply_spline_wander(&mut self, wander: Wander) {
        self.spline.default_wander = wander;
        let range = match self.spline.selected {
            Some(index) => index..index + 1,
            None => 0..self.spline.attrs.len(),
        };
        for attr in self.spline.attrs.get_mut(range).into_iter().flatten() {
            if attr.wander != wander {
                attr.wander = wander;
                self.spline.desired_key = None;
            }
        }
    }

    /// As [`Self::apply_spline_wander`], for smoothing the ground.
    pub(in crate::app) fn apply_spline_smooth_ground(&mut self, smooth: bool) {
        self.spline.default_smooth = smooth;
        let range = match self.spline.selected {
            Some(index) => index..index + 1,
            None => 0..self.spline.attrs.len(),
        };
        for attr in self.spline.attrs.get_mut(range).into_iter().flatten() {
            if attr.smooth_ground != smooth {
                attr.smooth_ground = smooth;
                self.spline.desired_key = None;
            }
        }
    }

    /// Give a new wander to the stretch after one point (the points list).
    pub(in crate::app) fn set_spline_point_wander(&mut self, index: usize, wander: Wander) {
        if let Some(attr) = self.spline.attrs.get_mut(index)
            && attr.wander != wander
        {
            attr.wander = wander;
            self.spline.desired_key = None;
        }
    }

    pub(in crate::app) fn toggle_spline_point_ground(&mut self, index: usize) {
        if let Some(attr) = self.spline.attrs.get_mut(index) {
            attr.smooth_ground = !attr.smooth_ground;
            self.spline.desired_key = None;
        }
    }

    /// A new point's attributes: the tool's defaults, and a seed that is not shared with any
    /// other point so each stretch wanders differently.
    pub(in crate::app) fn new_spline_attr(&mut self, width: f32) -> PointAttr {
        self.spline.next_seed = self.spline.next_seed.wrapping_add(1);
        PointAttr {
            width,
            corner: false,
            wander: self.spline.default_wander,
            smooth_ground: self.spline.default_smooth,
            seed: self.spline.next_seed,
        }
    }

    /// Replace the whole path (scripts).
    pub(in crate::app) fn replace_spline_controls(&mut self, controls: Vec<ControlPoint>) {
        self.spline.points = controls.iter().map(|c| (c.x, c.y)).collect();
        let mut attrs = Vec::with_capacity(controls.len());
        for c in &controls {
            // Seed 0 means "none given": hand out the next free one.
            let seed = if c.seed == 0 {
                self.new_spline_attr(c.width).seed
            } else {
                c.seed
            };
            attrs.push(PointAttr {
                width: clamp_width(c.width),
                corner: c.corner,
                wander: c.wander,
                smooth_ground: c.smooth_ground,
                seed,
            });
        }
        self.spline.attrs = attrs;
        self.spline.drag = None;
        self.spline.selected = None;
        self.spline.desired_key = None;
    }

    /// Whether the blocked tiles already cover `need` (inclusive tile bounds) for this terrain
    /// version, overlay and plane.
    pub(in crate::app) fn spline_blocked_covers(
        &self,
        need: ((i32, i32), (i32, i32)),
        version: u64,
        overlay: u16,
        plane: u8,
    ) -> bool {
        self.spline.blocked_scope.is_some_and(|scope| {
            scope.version == version
                && scope.overlay == overlay
                && scope.plane == plane
                && scope.min.0 <= need.0.0
                && scope.min.1 <= need.0.1
                && scope.max.0 >= need.1.0
                && scope.max.1 >= need.1.1
        })
    }

    /// Replace the blocked tiles (overlays the path must not paint over) covering `bounds`.
    pub(in crate::app) fn set_spline_blocked(
        &mut self,
        tiles: BTreeSet<(i32, i32)>,
        bounds: ((i32, i32), (i32, i32)),
        version: u64,
        overlay: u16,
        plane: u8,
    ) {
        self.spline.blocked_scope = Some(BlockedScope {
            min: bounds.0,
            max: bounds.1,
            version,
            overlay,
            plane,
        });
        let key = blocked_key(&tiles);
        if key == self.spline.blocked.key {
            return;
        }
        self.spline.blocked = BlockedTiles {
            tiles: Arc::new(tiles),
            key,
        };
        self.spline.desired_key = None;
    }
}

/// The area, terrain version, overlay and plane a blocked-tile set was built for.
#[derive(Debug, Clone, Copy)]
pub(super) struct BlockedScope {
    min: (i32, i32),
    max: (i32, i32),
    version: u64,
    overlay: u16,
    plane: u8,
}
