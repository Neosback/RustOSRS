//! What the Spline tool reads from and does to the terrain: which tiles a path must not paint over,
//! the real-colour preview of the path, and smoothing the ground along it when laid.
use super::*;
use crate::pick::Infos;
use std::collections::BTreeSet;

/// Tiles of slack around the area a blocked snapshot covers, so small moves do not rebuild it.
const BLOCKED_SLACK: i32 = 12;

/// A vertex height from the picker data of the loaded regions.
fn info_vertex_height(infos: &Infos, plane: u8, vertex: (i32, i32)) -> Option<i32> {
    let region = (vertex.0.div_euclid(64), vertex.1.div_euclid(64));
    let info = infos.get(&region)?;
    Some(info.corner_height(
        usize::from(plane),
        vertex.0.rem_euclid(64) as usize,
        vertex.1.rem_euclid(64) as usize,
    ))
}

/// The centre line as grade points.
fn grade_line(line: &[terrain_path::PathSample]) -> Vec<osrs_edit::GradePoint> {
    line.iter()
        .map(|p| osrs_edit::GradePoint {
            x: p.x,
            y: p.y,
            half: p.half,
        })
        .collect()
}

/// Keep the set of tiles the path must not paint over current: tiles near the path that hold an
/// overlay that is an area (water, floors, ...) rather than a path, other than the overlay being
/// laid. Rebuilt only when the path leaves the area it covers, the terrain changes or the overlay
/// does.
pub(super) fn refresh_blocked(
    app: &mut EditorApp,
    overlay: Option<u16>,
    pending: Option<(f32, f32)>,
) {
    let Some(overlay) = overlay else {
        return;
    };
    let points = app.interaction.spline_points();
    if points.is_empty() {
        return;
    }
    let widest = app
        .interaction
        .spline_attrs()
        .iter()
        .map(|a| a.width)
        .fold(1.0_f32, f32::max);
    let pad = (widest / 2.0).ceil() as i32 + 3;
    let (mut min, mut max) = ((i32::MAX, i32::MAX), (i32::MIN, i32::MIN));
    for point in points.iter().copied().chain(pending) {
        let tile = (point.0.floor() as i32, point.1.floor() as i32);
        min = (min.0.min(tile.0 - pad), min.1.min(tile.1 - pad));
        max = (max.0.max(tile.0 + pad), max.1.max(tile.1 + pad));
    }
    let version = app.world.infos_version();
    let plane = app.interaction.view_plane();
    if app
        .interaction
        .spline_blocked_covers((min, max), version, overlay, plane)
    {
        return;
    }
    let (min, max) = (
        (min.0 - BLOCKED_SLACK, min.1 - BLOCKED_SLACK),
        (max.0 + BLOCKED_SLACK, max.1 + BLOCKED_SLACK),
    );
    let infos = app.world.infos();
    let mut blocked = BTreeSet::new();
    for x in min.0..=max.0 {
        for y in min.1..=max.1 {
            let region = (x.div_euclid(64), y.div_euclid(64));
            let Some(info) = infos
                .get(&region)
                .and_then(|i| i.tile(usize::from(plane), (x, y)))
            else {
                continue;
            };
            let id = info.overlay_raw & 0x7fff;
            if id != 0 && id != overlay && !terrain_path::is_path_overlay(id) {
                blocked.insert((x, y));
            }
        }
    }
    app.interaction
        .set_spline_blocked(blocked, (min, max), version, overlay, plane);
}

/// Rebuild the real-colour appearance (and the ground smoothing it shows) when the plan, the
/// ground option or the terrain under the path changed.
pub(super) fn rebuild_appearance(app: &mut EditorApp) {
    let version = app.world.infos_version();
    let smooth = app.settings.prefs.spline.smooth_ground;
    let grade_key = u64::from(smooth);
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
    let graded = if smooth && !preview.line.is_empty() {
        osrs_edit::plan_path_grade(
            &grade_line(&preview.line),
            osrs_edit::PathGradeSpec::default(),
            &|vertex| info_vertex_height(infos, plane, vertex),
        )
    } else {
        Vec::new()
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

/// The vertices laying `preview` would smooth along the path, from the authoritative document's
/// heights. Empty when smoothing is off, or (with a notice) when the ground is not known.
pub(super) fn commit_grade(
    app: &mut EditorApp,
    preview: &super::super::interaction_runtime::Preview,
) -> Vec<((i32, i32), i32)> {
    use osrs_core::coords::{MapTile, SourcePlane};
    use osrs_edit::HeightVertexCoord;
    let Some(plane) = SourcePlane::new(app.interaction.view_plane()) else {
        return Vec::new();
    };
    if !app.settings.prefs.spline.smooth_ground || preview.line.is_empty() {
        return Vec::new();
    }
    let document = app.session.document();
    let grade = osrs_edit::plan_path_grade(
        &grade_line(&preview.line),
        osrs_edit::PathGradeSpec::default(),
        &|v| document.height_at(HeightVertexCoord::new(MapTile::new(v.0, v.1), plane)),
    );
    if grade.is_empty() {
        app.ui.notify(
            "Ground smoothing was skipped: the ground under the path is not loaded".to_owned(),
        );
    }
    grade
}
