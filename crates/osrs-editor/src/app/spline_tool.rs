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
    let at = if app.settings.prefs.spline.snap_points {
        (hit.tile.0 as f32 + 0.5, hit.tile.1 as f32 + 0.5)
    } else {
        hit.at
    };
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

/// A vertex height from the picker data of the loaded regions.
fn info_vertex_height(infos: &crate::pick::Infos, plane: u8, vertex: (i32, i32)) -> Option<i32> {
    let region = (vertex.0.div_euclid(64), vertex.1.div_euclid(64));
    let info = infos.get(&region)?;
    Some(info.corner_height(
        usize::from(plane),
        vertex.0.rem_euclid(64) as usize,
        vertex.1.rem_euclid(64) as usize,
    ))
}

/// Rebuild the real-colour appearance (and the grade it shows) when the plan, the grading options
/// or the terrain under the path changed.
fn rebuild_appearance(app: &mut EditorApp) {
    let version = app.world.infos_version();
    let spline = &app.settings.prefs.spline;
    let spec = spline.grade_spec();
    let grade_key = spec.map_or(0, |spec| {
        [
            spec.smoothing_tiles,
            spec.max_slope,
            spec.blend_tiles,
            spline.width,
        ]
        .iter()
        .fold(0xCBF2_9CE4_8422_2325_u64, |hash, value| {
            (hash ^ u64::from(value.to_bits())).wrapping_mul(0x0100_0000_01B3)
        }) | 1
    });
    let (Some(preview), Some(floors)) =
        (app.interaction.current_spline_preview(), app.world.floors())
    else {
        return;
    };
    let fresh = app.interaction.spline_appearance().is_some_and(|a| {
        a.key == preview.key && a.infos_version == version && a.grade_key == grade_key
    });
    if fresh {
        return;
    }
    let plane = app.interaction.view_plane();
    let infos = app.world.infos();
    let graded = match (spec, preview.lines.first()) {
        (Some(spec), Some(line)) => {
            osrs_edit::plan_path_grade(line, spline.width.clamp(0.5, 24.0), spec, &|vertex| {
                info_vertex_height(infos, plane, vertex)
            })
        }
        _ => Vec::new(),
    };
    let appearance = terrain_path::path_appearance(
        plane,
        preview.overlay_id,
        &preview.paints,
        &graded,
        |tile| {
            let region = (tile.0.div_euclid(64), tile.1.div_euclid(64));
            infos.get(&region)?.tile(usize::from(plane), tile).copied()
        },
        floors.as_ref(),
        terrain_path::reference_palette(),
    );
    let built = terrain_path::PathAppearance {
        key: preview.key,
        infos_version: version,
        grade_key,
        origin: appearance.as_ref().map_or((0, 0), |(origin, _)| *origin),
        tiles: appearance.map(|(_, tiles)| tiles).unwrap_or_default(),
    };
    app.interaction.set_spline_appearance(built);
}

/// The vertices laying `preview` would grade, from the authoritative document's heights. Empty when
/// grading is off, or (with a notice) when the ground under the path is not known.
fn commit_grade(
    app: &mut EditorApp,
    preview: &super::interaction_runtime::Preview,
) -> Vec<((i32, i32), i32)> {
    use osrs_core::coords::{MapTile, SourcePlane};
    use osrs_edit::HeightVertexCoord;
    let settings = &app.settings.prefs.spline;
    let (Some(spec), Some(line), Some(plane)) = (
        settings.grade_spec(),
        preview.lines.first(),
        SourcePlane::new(app.interaction.view_plane()),
    ) else {
        return Vec::new();
    };
    let document = app.session.document();
    let grade = osrs_edit::plan_path_grade(line, settings.width.clamp(0.5, 24.0), spec, &|v| {
        document.height_at(HeightVertexCoord::new(MapTile::new(v.0, v.1), plane))
    });
    if grade.is_empty() {
        app.ui.notify(
            "Terrain grading was skipped: the ground under the path is not loaded".to_owned(),
        );
    }
    grade
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
            .update_spline_preview(&self.settings.prefs.spline, overlay, pending);
        rebuild_appearance(self);
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
