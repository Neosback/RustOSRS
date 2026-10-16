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

/// Smooth the terrain under a laid path, as its own undo step, with the tool's terrain settings.
/// Heights come from the authoritative document.
pub(super) fn smooth_height(app: &mut EditorApp, line: &[(f64, f64)]) {
    use osrs_core::coords::{MapTile, SourcePlane};
    use osrs_edit::HeightVertexCoord;
    let Some(plane) = SourcePlane::new(app.interaction.view_plane()) else {
        return;
    };
    let settings = &app.settings.prefs.spline;
    let spec = osrs_edit::PathHeightSpec {
        width: f64::from(settings.width),
        passes: settings.height_passes,
        blend: f64::from(settings.height_blend),
    };
    let document = app.session.document();
    let smoothed = osrs_edit::plan_path_height_smoothing(line, spec, &|v| {
        document.height_at(HeightVertexCoord::new(MapTile::new(v.0, v.1), plane))
    });
    if smoothed.is_empty() {
        app.ui
            .notify("The terrain under the path is already smooth".to_owned());
        return;
    }
    crate::app::editing::commit_path_height(app, &smoothed);
}
