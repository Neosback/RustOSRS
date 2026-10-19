//! Roofs and upper floors in the viewport.
//!
//! The viewed plane decides what can be edited; this decides how much of what lies above it is
//! drawn. Cutaway follows RuneLite's roof removal: every plane is drawn except the roof volumes
//! over the tile under the cursor, the camera and the selection (`RoofRemovalPlugin`,
//! eight-neighbour components of `UNDER_ROOF` tiles, `Scene.buildRoofs`), plus the roof the cursor
//! is looking at from outside, which stays peeled while the cursor is over that building. Volume
//! ids come from [`osrs_scene::roofs`], the same labelling the geometry extraction buckets by.

use super::EditorApp;
use crate::{camera::CameraMode, pick::Infos, settings::RoofChoice};
use osrs_render::gpu::RoofView;
use osrs_scene::roofs::{NO_ROOF, RoofGrid};
use std::collections::HashSet;

/// Tiles around a query tile that are labelled; a roof larger than this is cut at the edge.
const REACH: i32 = 72;
/// Most selected tiles / objects that cut a roof away.
const MAX_FOCUS: usize = 48;
/// The highest plane whose tiles carry a roof volume (geometry on plane N belongs to `N - 1`).
const TOP_ROOF_PLANE: usize = 2;

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
fn cut_roofs(app: &mut EditorApp) -> HashSet<u32> {
    let plane = app.interaction.view_plane();
    let mut focus: Vec<(i32, i32)> = vec![app.camera_tile()];
    if matches!(
        app.interaction.camera().mode,
        CameraMode::Orbit | CameraMode::Game
    ) {
        // The orbit pivot is what the camera looks at.
        let pivot = app.interaction.camera().pivot();
        let origin = app.renderer.render_origin();
        focus.push((
            ((origin.0 as f32 + pivot[0]) / 128.0).floor() as i32,
            ((origin.1 as f32 + pivot[2]) / 128.0).floor() as i32,
        ));
    }
    if let Some(stand) = app.settings.prefs.player.stand {
        focus.push((stand.x, stand.y));
    }
    let hover_tile = app.interaction.hover().tile.map(|pick| pick.tile);
    focus.extend(hover_tile);
    // Sorted, so which tiles make the cap does not depend on hash order.
    let mut selected: Vec<(i32, i32)> = app.interaction.selection_tiles().iter().copied().collect();
    selected.sort_unstable();
    focus.extend(selected.into_iter().take(MAX_FOCUS));
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
    let mut hidden = app.world.roofs_over(plane, &focus);
    let under_hover: HashSet<u32> = hover_tile
        .map(|tile| app.world.roofs_over(plane, &[tile]))
        .unwrap_or_default();
    let peeled = next_peel(
        app.interaction.peeled_roof(),
        app.interaction.hover().roof,
        &under_hover,
    );
    app.interaction.set_peeled_roof(peeled);
    hidden.extend(peeled);
    hidden
}

/// The tile setting byte of world tile `tile` on `plane` (0 where nothing is loaded).
pub(super) fn tile_flags(infos: &Infos, plane: usize, tile: (i32, i32)) -> u8 {
    infos
        .get(&(tile.0.div_euclid(64), tile.1.div_euclid(64)))
        .and_then(|info| info.tile(plane, tile))
        .map_or(0, |t| t.settings)
}

/// The roof that stays peeled: the one the cursor looks at (`looked_at`), kept while the tile
/// under the cursor (`under_cursor`: the volumes over it) is still inside it. Without the latch
/// the peeled roof would reappear the moment it stops being hit, and flicker.
fn next_peel(
    previous: Option<u32>,
    looked_at: Option<u32>,
    under_cursor: &HashSet<u32>,
) -> Option<u32> {
    looked_at.or_else(|| previous.filter(|id| under_cursor.contains(id)))
}

/// The roof volume ids over `focus` tiles when `plane` is viewed: every plane from the viewed one
/// up, because geometry on plane N belongs to the volume of plane N - 1 and all of it is above.
/// `flags(plane, x, y)` gives the tile setting byte at a world tile.
#[cfg(test)]
pub(super) fn roofs_over(
    flags: &dyn Fn(usize, i32, i32) -> u8,
    plane: u8,
    focus: &[(i32, i32)],
) -> HashSet<u32> {
    RoofCache::default().query(0, plane, focus, flags)
}

/// Disposable roof projection, keyed by the accepted info version and viewed plane.
#[derive(Default)]
pub(super) struct RoofCache {
    version: Option<(u64, u8)>,
    grids: Vec<RoofGrid>,
}
impl RoofCache {
    pub(super) fn query(
        &mut self,
        version: u64,
        plane: u8,
        focus: &[(i32, i32)],
        flags: &dyn Fn(usize, i32, i32) -> u8,
    ) -> HashSet<u32> {
        if self.version != Some((version, plane)) {
            self.grids.clear();
            self.version = Some((version, plane));
        }
        let mut hidden = HashSet::new();
        let grids = &mut self.grids;
        let planes = usize::from(plane)..=TOP_ROOF_PLANE;
        for &(x, y) in focus {
            let mut found = false;
            for grid in grids.iter() {
                if grid.covers_world(x, y, REACH / 2) {
                    found = true;
                    hidden.extend(
                        planes
                            .clone()
                            .map(|p| grid.id_at_world(p, x, y))
                            .filter(|id| *id != NO_ROOF),
                    );
                    break;
                }
            }
            if found {
                continue;
            }
            let origin = (x - REACH, y - REACH);
            let size = (2 * REACH + 1) as usize;
            let grid = RoofGrid::build(origin, size, size, |p, lx, ly| {
                if p < usize::from(plane) || p > TOP_ROOF_PLANE {
                    return 0;
                }
                flags(p, origin.0 + lx as i32, origin.1 + ly as i32)
            });
            hidden.extend(
                planes
                    .clone()
                    .map(|p| grid.id_at_world(p, x, y))
                    .filter(|id| *id != NO_ROOF),
            );
            if grids.len() == 16 {
                grids.remove(0);
            }
            grids.push(grid);
        }
        hidden
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use osrs_scene::roofs::{UNDER_ROOF, roof_id};

    /// A two-storey building at x 100..=104, y 200..=204 (roof flag on planes 0 and 1) and a
    /// one-storey one at x 120..=123, y 200..=203 (plane 0 only).
    fn flags(plane: usize, x: i32, y: i32) -> u8 {
        let tall = (100..=104).contains(&x) && (200..=204).contains(&y) && plane <= 1;
        let low = (120..=123).contains(&x) && (200..=203).contains(&y) && plane == 0;
        if tall || low { UNDER_ROOF } else { 0 }
    }

    #[test]
    fn cached_roofs_reuse_only_the_same_version_and_plane() {
        let calls = std::cell::Cell::new(0);
        let query = |p, x, y| {
            calls.set(calls.get() + 1);
            flags(p, x, y)
        };
        let mut cache = RoofCache::default();
        let expected = cache.query(1, 0, &[(102, 202)], &query);
        let count = calls.get();
        assert_eq!(cache.query(1, 0, &[(103, 202)], &query), expected);
        assert_eq!(calls.get(), count);
        cache.query(2, 0, &[(102, 202)], &query);
        assert!(calls.get() > count);
        let count = calls.get();
        cache.query(2, 1, &[(102, 202)], &query);
        assert!(calls.get() > count);
    }

    #[test]
    fn viewing_the_ground_cuts_every_storey_over_the_building() {
        let hidden = roofs_over(&flags, 0, &[(102, 202)]);
        assert_eq!(
            hidden,
            HashSet::from([roof_id(0, 100, 200), roof_id(1, 100, 200)])
        );
    }

    #[test]
    fn viewing_the_upper_floor_leaves_the_lower_roof_alone() {
        let hidden = roofs_over(&flags, 1, &[(102, 202)]);
        assert_eq!(hidden, HashSet::from([roof_id(1, 100, 200)]));
    }

    #[test]
    fn only_the_building_under_the_focus_is_cut() {
        let hidden = roofs_over(&flags, 0, &[(121, 201)]);
        assert_eq!(hidden, HashSet::from([roof_id(0, 120, 200)]));
        assert!(roofs_over(&flags, 0, &[(110, 210)]).is_empty());
    }

    #[test]
    fn the_overhang_tile_cuts_its_roof_and_far_focus_tiles_share_work() {
        let hidden = roofs_over(&flags, 0, &[(99, 199), (102, 202), (121, 201)]);
        assert_eq!(hidden.len(), 3);
    }

    #[test]
    fn a_peeled_roof_stays_while_the_cursor_is_over_it_and_goes_when_it_leaves() {
        let over = HashSet::from([7]);
        let elsewhere = HashSet::new();
        // Looking at a roof peels it.
        assert_eq!(next_peel(None, Some(7), &elsewhere), Some(7));
        // The roof is hidden now, so the ray no longer hits it; the cursor is over its floor.
        assert_eq!(next_peel(Some(7), None, &over), Some(7));
        // The cursor moves off the building: the roof comes back.
        assert_eq!(next_peel(Some(7), None, &elsewhere), None);
        // Looking at another roof switches.
        assert_eq!(next_peel(Some(7), Some(9), &over), Some(9));
    }
}
