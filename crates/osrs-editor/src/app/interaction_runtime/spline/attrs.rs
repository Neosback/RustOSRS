//! Per-point attributes of a spline path: each control point has its own width and may be a
//! corner. Widths and corners are part of the plan key, so editing them replans. No semantic
//! mutation.
use super::InteractionRuntime;
use super::plan::{BlockedTiles, blocked_key};
use crate::terrain_path::ControlPoint;
use std::collections::BTreeSet;
use std::sync::Arc;

/// Narrowest and widest a point may be, in tiles.
pub(in crate::app) const MIN_WIDTH: f32 = 1.0;
pub(in crate::app) const MAX_WIDTH: f32 = 24.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(in crate::app) struct PointAttr {
    pub(in crate::app) width: f32,
    pub(in crate::app) corner: bool,
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

    /// Replace the whole path (scripts).
    pub(in crate::app) fn replace_spline_controls(&mut self, controls: Vec<ControlPoint>) {
        self.spline.points = controls.iter().map(|c| (c.x, c.y)).collect();
        self.spline.attrs = controls
            .iter()
            .map(|c| PointAttr {
                width: clamp_width(c.width),
                corner: c.corner,
            })
            .collect();
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
