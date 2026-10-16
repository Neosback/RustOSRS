//! The Spline tool: click control points, preview a fitted OSRS overlay band, press Enter.
//!
//! There is deliberately no obstacle routing or wall mode here. The control line is smoothed with
//! deterministic Catmull-Rom sampling and the resulting band is fitted against the exact OSRS
//! shaped-terrain geometry. The fitted tiles are committed as one semantic patch / one undo step.

mod input;
mod overlay;
pub(in crate::app) mod panel;
pub(in crate::app) mod sections;

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
        self.interaction
            .update_spline_preview(&self.settings.prefs.spline, overlay);
        // Rebuild the real-colour appearance when the plan or the terrain under it changed.
        let version = self.world.infos_version();
        let built = match (
            self.interaction.current_spline_preview(),
            self.world.floors(),
        ) {
            (Some(preview), Some(floors))
                if self
                    .interaction
                    .spline_appearance()
                    .is_none_or(|a| a.key != preview.key || a.infos_version != version) =>
            {
                let plane = self.interaction.view_plane();
                let infos = self.world.infos();
                let appearance = terrain_path::path_appearance(
                    plane,
                    preview.overlay_id,
                    &preview.paints,
                    |tile| {
                        let region = (tile.0.div_euclid(64), tile.1.div_euclid(64));
                        infos.get(&region)?.tile(usize::from(plane), tile).copied()
                    },
                    floors.as_ref(),
                    terrain_path::reference_palette(),
                );
                Some(terrain_path::PathAppearance {
                    key: preview.key,
                    infos_version: version,
                    origin: appearance.as_ref().map_or((0, 0), |(origin, _)| *origin),
                    tiles: appearance.map(|(_, tiles)| tiles).unwrap_or_default(),
                })
            }
            _ => None,
        };
        if let Some(appearance) = built {
            self.interaction.set_spline_appearance(appearance);
        }
    }
    /// Everything about the current fitted path in one copyable block.
    pub(super) fn spline_report(&self) -> String {
        let spline = &self.settings.prefs.spline;
        let raw: Vec<String> = self
            .interaction
            .spline_points()
            .iter()
            .map(|p| format!("{:.3},{:.3}", p.0, p.1))
            .collect();
        let mut text = format!(
            "PATH plane={} points={}\nwidth={:.2} edge_weight={:.2} overlay={:?}\n",
            self.interaction.view_plane(),
            raw.join(";"),
            spline.width,
            spline.edge_weight,
            self.settings.prefs.brush.overlay.fill,
        );
        if let Some(preview) = &self.interaction.spline_preview() {
            text.push_str(&format!("fitted {} tiles\n", preview.paints.len()));
        }
        text
    }
}
