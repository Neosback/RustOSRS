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
        locs,
        stats,
    } = place_window_locs(definitions, &mut loaded, &mut scene)?;

    apply_terrain(definitions, &loaded.grid, presentation, &mut scene)?;
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
