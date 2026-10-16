//! What the Spline tool reads from and does to the terrain: the real-colour preview of the path,
//! and smoothing the ground under it when laid.
use super::*;

/// Rebuild the real-colour appearance when the plan or the terrain under the path changed.
pub(super) fn rebuild_appearance(app: &mut EditorApp) {
    let version = app.world.infos_version();
    let (Some(preview), Some(floors)) =
        (app.interaction.current_spline_preview(), app.world.floors())
    else {
        return;
    };
    // Without an overlay there is nothing to colour: the preview is the flat fill.
    if preview.overlay_id == 0 {
        return;
    }
    let fresh = app
        .interaction
        .spline_appearance()
        .is_some_and(|a| a.key == preview.key && a.infos_version == version);
    if fresh {
        return;
    }
    let plane = app.interaction.view_plane();
    let infos = app.world.infos();
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
    let built = terrain_path::PathAppearance {
        key: preview.key,
        infos_version: version,
        origin: appearance.as_ref().map_or((0, 0), |(origin, _)| *origin),
        tiles: appearance.map(|(_, tiles)| tiles).unwrap_or_default(),
    };
    app.interaction.set_spline_appearance(built);
}

type SmoothedHeights = Vec<((i32, i32), i32)>;

/// Plan source-equivalent smoothing in the fixed local map. Missing interior data is a failure,
/// not a smaller smoothing neighbourhood. Only the true captured boundary excludes neighbours.
pub(super) fn height_plan(
    app: &EditorApp,
    line: &[(f64, f64)],
) -> Result<SmoothedHeights, &'static str> {
    use osrs_core::coords::{MapTile, SourcePlane};
    use osrs_edit::HeightVertexCoord;
    if !app.settings.prefs.spline.smooth_height {
        return Ok(Vec::new());
    }
    let extent = app
        .interaction
        .spline_extent()
        .ok_or("The path has no captured map")?;
    let plane = SourcePlane::new(app.interaction.view_plane()).ok_or("Invalid path plane")?;
    let settings = &app.settings.prefs.spline;
    let spec = osrs_edit::PathHeightSpec {
        width: f64::from(settings.width),
        passes: settings.height_passes,
        blend: f64::from(settings.height_blend),
    };
    let local: Vec<_> = line.iter().map(|&p| extent.local(p)).collect();
    let world = |v: (i32, i32)| MapTile::new(v.0 + extent.origin.x, v.1 + extent.origin.y);
    let document = app.session.document();
    for v in osrs_edit::smoothing_vertices(&local, spec) {
        if !extent.in_bounds(world(v)) {
            continue;
        }
        for dx in -1..=1 {
            for dy in -1..=1 {
                let tile = world((v.0 + dx, v.1 + dy));
                if !extent.in_bounds(tile) {
                    continue;
                }
                if !extent.contains(tile) {
                    return Err("The path reaches terrain missing from its captured map");
                }
                for z in 0..4 {
                    let p = SourcePlane::new(z).ok_or("Invalid path plane")?;
                    if document
                        .height_at(HeightVertexCoord::new(tile, p))
                        .is_none()
                        || !tile_available(app, z, tile)
                    {
                        return Err(
                            "Required path terrain is unavailable; return to the captured map and retry",
                        );
                    }
                }
            }
        }
    }
    Ok(osrs_edit::plan_path_height_smoothing(&local, spec, &|v| {
        let tile = world(v);
        extent
            .in_bounds(tile)
            .then(|| document.height_at(HeightVertexCoord::new(tile, plane)))
            .flatten()
    })
    .into_iter()
    .map(|(v, h)| ((world(v).x, world(v).y), h))
    .collect())
}

/// Projection availability is an admission prerequisite only; values always come from MapDocument.
pub(super) fn tile_available(app: &EditorApp, plane: u8, tile: osrs_core::coords::MapTile) -> bool {
    let Some(source_plane) = osrs_core::coords::SourcePlane::new(plane) else {
        return false;
    };
    app.session
        .document()
        .height_at(osrs_edit::HeightVertexCoord::new(tile, source_plane))
        .is_some()
        && app
            .world
            .infos()
            .get(&(tile.x.div_euclid(64), tile.y.div_euclid(64)))
            .and_then(|info| info.tile(usize::from(plane), (tile.x, tile.y)))
            .is_some()
}
