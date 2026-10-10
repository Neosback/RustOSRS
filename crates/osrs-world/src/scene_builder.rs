//! Full scene assembly in the reference client's order.
//!
//! terrain streams -> empty-region fill -> loc placement (with shadow input) -> terrain colors
//! and minimum planes -> scene normal reconciliation and final lighting -> bridge link-below.

use crate::{
    LocStageOutput, SceneWindow, TerrainPresentation, WorldDefinitions, WorldError, apply_terrain,
    load_window,
    loc_stage::{LocStageStats, WorldLoc, place_window_locs},
};
use osrs_core::lighting::ReferenceLitModel;
use osrs_scene::{
    SceneGrid, SceneReferenceFinalizer,
    normal_finalization::SceneNormalMergeReport,
    terrain_build::link_bridge_tiles,
    terrain_load::{REFERENCE_SCENE_TILES, TerrainLoadGrid},
};

/// A fully assembled 104x104 scene window.
pub struct WorldScene {
    pub window: SceneWindow,
    /// Loaded height/flag/floor/shadow arrays (the builder's inputs).
    pub load: TerrainLoadGrid,
    pub scene: SceneGrid,
    /// Finalizer with every surviving `ModelData` entity lit.
    pub finalizer: SceneReferenceFinalizer,
    /// Flat-shaded lit models indexed by `LocRenderable::Lit`.
    pub lit: Vec<ReferenceLitModel>,
    pub animated: Vec<std::sync::Arc<crate::AnimatedModel>>,
    pub locs: Vec<WorldLoc>,
    pub loc_stats: LocStageStats,
    pub merge_report: SceneNormalMergeReport,
}

/// Assemble the scene for `window` from the cache.
pub fn build_world_scene(
    definitions: &mut WorldDefinitions,
    window: SceneWindow,
    presentation: TerrainPresentation,
) -> Result<WorldScene, WorldError> {
    let mut loaded = load_window(definitions, window)?;
    let mut scene = SceneGrid::new(
        REFERENCE_SCENE_TILES as u32,
        REFERENCE_SCENE_TILES as u32,
        4,
    )?;

    let LocStageOutput {
        mut finalizer,
        lit,
        animated,
        mut locs,
        stats,
    } = place_window_locs(definitions, &mut loaded, &mut scene)?;

    if presentation.flush_diagonal_decorations {
        flush_diagonal_decorations(&mut locs);
    }
    apply_terrain(definitions, &loaded.grid, presentation, &mut scene)?;
    finalizer.set_vertical_merge_tolerance(presentation.wall_merge_tolerance);
    let merge_report = finalizer
        .reconcile_and_light()
        .map_err(|error| WorldError::Placement(error.to_string()))?;
    link_bridge_tiles(&loaded.grid, &mut scene)?;

    Ok(WorldScene {
        window,
        load: loaded.grid,
        scene,
        finalizer,
        lit,
        animated,
        locs,
        loc_stats: stats,
        merge_report,
    })
}

/// Zero the offsets of diagonal (`256`) wall decorations whose tile holds a type-9 diagonal wall
/// object. The client's `8 * (+-1, +-1)` default displacement assumes a thin diagonal boundary
/// wall; against the type-9 wedge (a game object, so `getBoundaryObjectTag` finds nothing) the
/// visible plate floats 11 units off the wedge face for half of the orientation combinations.
fn flush_diagonal_decorations(locs: &mut [WorldLoc]) {
    use osrs_scene::placement::PlacementKind;
    let hosts: std::collections::HashSet<(u8, u32, u32)> = locs
        .iter()
        .filter(|loc| !loc.renderables.is_empty() && loc.plan.source_loc_type.get() == 9)
        .map(|loc| (loc.plane.index().get(), loc.tile.x, loc.tile.y))
        .collect();
    for loc in locs.iter_mut() {
        let key = (loc.plane.index().get(), loc.tile.x, loc.tile.y);
        if let PlacementKind::WallDecoration(decor) = &mut loc.plan.kind
            && decor.orientation_flag == 256
            && hosts.contains(&key)
        {
            decor.offset_x = 0;
            decor.offset_z = 0;
        }
    }
}
