//! The Spline tool: click control points, preview a fitted OSRS overlay band, press Enter.
//!
//! There is deliberately no obstacle routing or wall mode here. The control line is smoothed with
//! deterministic Catmull-Rom sampling and the resulting band is fitted against the exact OSRS
//! shaped-terrain geometry. The fitted tiles are committed as one semantic patch / one undo step.

mod input;
mod overlay;
pub(in crate::app) mod panel;
pub(in crate::app) mod sections;
mod terrain;

use super::EditorApp;
use super::editing::PathTile;
use crate::interact;
use crate::paint::Tool;
use crate::style::ColorKey;
use crate::terrain_path;
use eframe::egui::{self, Color32, Pos2, Rect, Stroke};

/// A click this close (in tiles) to a control point grabs it instead of adding a new one.
const GRAB_DISTANCE: f32 = 0.9;
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

/// Where the next click would put a control point, when the preview follows the cursor: the
/// hovered point (tile centre when snapping) in quarter-tile steps, so small pointer movements do
/// not each replan. `None` while dragging, with no point placed yet, or over an existing point.
fn pending_point(app: &EditorApp) -> Option<(f32, f32)> {
    if app.settings.prefs.tool != Tool::Spline
        || app.interaction.spline_drag().is_some()
        || app.interaction.spline_points().is_empty()
    {
        return None;
    }
    let hit = app.interaction.hover().tile?;
    let at = hit.at;
    if app
        .interaction
        .spline_points()
        .iter()
        .any(|point| distance(*point, at) <= GRAB_DISTANCE)
    {
        return None;
    }
    Some(((at.0 * 4.0).round() / 4.0, (at.1 * 4.0).round() / 4.0))
}

impl EditorApp {
    fn laying_overlay(&self) -> Option<u16> {
        let brush = &self.settings.prefs.brush;
        match brush.overlay.fill {
            crate::paint::Fill::Set(id) if brush.overlay.armed => Some(id),
            _ => None,
        }
    }
    pub(super) fn spline_update(&mut self) {
        let overlay = self.laying_overlay();
        let pending = pending_point(self);
        self.interaction
            .set_spline_default_width(self.settings.prefs.spline.width);
        terrain::refresh_blocked(self, overlay, pending);
        self.interaction.update_spline_preview(overlay, pending);
        terrain::rebuild_appearance(self);
    }
    /// Everything about the current fitted path in one copyable block.
    pub(super) fn spline_report(&self) -> String {
        let controls: Vec<String> = self
            .interaction
            .spline_controls()
            .iter()
            .map(|c| {
                format!(
                    "{:.3},{:.3}:{:.1}{}",
                    c.x,
                    c.y,
                    c.width,
                    if c.corner { "!" } else { "" }
                )
            })
            .collect();
        let mut text = format!(
            "PATH plane={} overlay={:?}\npoints={}\nsmooth_ground={}\n",
            self.interaction.view_plane(),
            self.settings.prefs.brush.overlay.fill,
            controls.join(" "),
            self.settings.prefs.spline.smooth_ground,
        );
        if let Some(preview) = &self.interaction.spline_preview() {
            text.push_str(&format!("fitted {} tiles\n", preview.paints.len()));
        }
        text
    }
}
