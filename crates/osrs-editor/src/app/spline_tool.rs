//! The Spline (path) tool, the same as the EikenB00m map editor's: click points, preview the
//! fitted OSRS overlay band, press Enter to lay it, Esc to cancel.
//!
//! There is deliberately no obstacle routing, per-point setting or wall mode here. The points
//! are smoothed into a Catmull-Rom curve (when asked) and the real overlay shapes are fitted to a
//! band of the chosen width (`terrain_path`). The fitted tiles are laid as one semantic patch / one
//! undo step; smoothing the terrain under the path is a second one.

mod input;
mod overlay;
pub(in crate::app) mod panel;
pub(in crate::app) mod sections;
mod terrain;

use super::EditorApp;
use super::editing::PathTile;
use super::interaction_runtime::PathOptions;
use crate::interact;
use crate::paint::Tool;
use crate::style::ColorKey;
use crate::terrain_path;
use eframe::egui::{self, Color32, Pos2, Rect, Stroke};

/// Where the next click would put a point: the centre of the tile under the cursor.
fn pending_point(app: &EditorApp) -> Option<(f32, f32)> {
    if app.settings.prefs.tool != Tool::Spline {
        return None;
    }
    let tile = app.interaction.hover().tile?.tile;
    Some((tile.0 as f32 + 0.5, tile.1 as f32 + 0.5))
}

impl EditorApp {
    fn laying_overlay(&self) -> Option<u16> {
        let brush = &self.settings.prefs.brush;
        match brush.overlay.fill {
            crate::paint::Fill::Set(id) if brush.overlay.armed && id > 0 => Some(id),
            _ => None,
        }
    }
    pub(super) fn spline_update(&mut self) {
        self.interaction
            .prepare_spline_extent(self.session.available_regions(), self.project.generation());
        let overlay = self.laying_overlay();
        let pending = pending_point(self);
        let options = PathOptions::from_settings(&self.settings.prefs.spline);
        self.interaction
            .update_spline_preview(overlay, pending, options);
        terrain::rebuild_appearance(self);
    }
    /// Everything about the current path in one copyable block.
    pub(super) fn spline_report(&self) -> String {
        let spline = &self.settings.prefs.spline;
        let points: Vec<String> = self
            .interaction
            .spline_points()
            .iter()
            .map(|p| format!("{:.1},{:.1}", p.0, p.1))
            .collect();
        let mut text = format!(
            "PATH plane={} overlay={:?}\npoints={}\nwidth={} curve={} edge={} height={} passes={} blend={}\n",
            self.interaction.view_plane(),
            self.settings.prefs.brush.overlay.fill,
            points.join(" "),
            spline.width,
            spline.smooth_curve,
            spline.edge_smoothing,
            spline.smooth_height,
            spline.height_passes,
            spline.height_blend,
        );
        if let Some(preview) = &self.interaction.spline_preview() {
            text.push_str(&format!("fitted {} tiles\n", preview.paints.len()));
        }
        text
    }
}
