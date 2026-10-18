//! Roofs and upper floors in the viewport.
//!
//! The viewed plane decides what can be edited; this decides how much of what lies above it is
//! drawn. Cutaway follows RuneLite's roof removal: every plane is drawn except the roof volume over
//! the tile under the cursor, the camera and the selection (`RoofRemovalPlugin`, eight-neighbour
//! components of `UNDER_ROOF` tiles, `Scene.buildRoofs`). Volume ids come from
//! [`osrs_scene::roofs`], the same labelling the geometry extraction buckets by.

use super::EditorApp;
use crate::{camera::CameraMode, pick::Infos, settings::RoofChoice};
use osrs_render::gpu::RoofView;
use osrs_scene::roofs::{NO_ROOF, RoofGrid};
use std::collections::HashSet;

/// Tiles around a query tile that are labelled; a roof larger than this is cut at the edge.
const REACH: i32 = 72;
/// Most selected tiles / objects that cut a roof away.
const MAX_FOCUS: usize = 48;

/// The roof view for this frame, applied to the renderer.
pub(super) fn apply(app: &mut EditorApp) {
    let view = match app.settings.prefs.roofs {
        RoofChoice::Hidden => RoofView::hidden_above(),
        RoofChoice::Visible => RoofView::all_but(HashSet::new()),
        RoofChoice::Cutaway => RoofView::all_but(cut_roofs(app)),
    };
    app.renderer.set_roof_view(view);
}

/// The roof volumes over everything the user is working on.
fn cut_roofs(app: &EditorApp) -> HashSet<u32> {
    let plane = app.interaction.view_plane();
    let mut focus: Vec<(i32, i32)> = vec![app.camera_tile()];
    if app.interaction.camera().mode == CameraMode::Orbit {
        // The orbit pivot is what the camera looks at.
        let pivot = app.interaction.camera().pivot();
        let origin = app.renderer.render_origin();
        focus.push((
            ((origin.0 as f32 + pivot[0]) / 128.0).floor() as i32,
            ((origin.1 as f32 + pivot[2]) / 128.0).floor() as i32,
        ));
    }
    focus.extend(app.interaction.last_hover_tile());
    focus.extend(app.interaction.selection_tiles().iter().take(MAX_FOCUS));
    for pick in app.interaction.selection_objects().iter().take(MAX_FOCUS) {
        if let Some(loc) = app
            .world
            .infos()
            .get(&pick.region)
            .and_then(|info| info.locs.get(pick.index))
        {
            focus.push(loc.tile);
        }
    }
    cut_roofs_at(app.world.infos(), plane, &focus)
}

/// The roof volume ids over `focus` tiles when `plane` is viewed.
pub(super) fn cut_roofs_at(infos: &Infos, plane: u8, focus: &[(i32, i32)]) -> HashSet<u32> {
    let mut hidden = HashSet::new();
    // Tiles near an already labelled one share its grid: label once per cluster.
    let mut grids: Vec<RoofGrid> = Vec::new();
    for &(x, y) in focus {
        let id = grids
            .iter()
            .map(|grid| grid.id_at_world(usize::from(plane), x, y))
            .find(|id| *id != NO_ROOF);
        if let Some(id) = id {
            hidden.insert(id);
            continue;
        }
        // A tile already inside a grid's window but without a roof needs no new grid.
        if grids.iter().any(|grid| grid.covers_world(x, y, REACH / 2)) {
            continue;
        }
        let origin = (x - REACH, y - REACH);
        let size = (2 * REACH + 1) as usize;
        let grid = RoofGrid::build(origin, size, size, |p, lx, ly| {
            if p != usize::from(plane) {
                return 0;
            }
            let tile = (origin.0 + lx as i32, origin.1 + ly as i32);
            infos
                .get(&(tile.0.div_euclid(64), tile.1.div_euclid(64)))
                .and_then(|info| info.tile(p, tile))
                .map_or(0, |t| t.settings)
        });
        let id = grid.id_at_world(usize::from(plane), x, y);
        if id != NO_ROOF {
            hidden.insert(id);
        }
        grids.push(grid);
    }
    hidden
}
