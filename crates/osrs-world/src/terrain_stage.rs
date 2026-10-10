//! Terrain stage of scene assembly: regions -> loaded grid -> built colors -> scene tiles.

use crate::{SceneWindow, WorldDefinitions, WorldError};
use osrs_core::color_palette::build_color_palette;
use osrs_scene::{
    SceneGrid,
    terrain_build::{TerrainJitter, build_terrain, link_bridge_tiles},
    terrain_load::{REFERENCE_SCENE_TILES, TerrainLoadGrid},
};

/// Client default brightness used for the palette that feeds tile-level RGB.
pub const DEFAULT_BRIGHTNESS: f64 = 0.8;

/// Presentation inputs that do not affect the four 3D terrain corner colors.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TerrainPresentation {
    pub brightness: f64,
    pub jitter: TerrainJitter,
}

impl Default for TerrainPresentation {
    fn default() -> Self {
        Self {
            brightness: DEFAULT_BRIGHTNESS,
            jitter: TerrainJitter::default(),
        }
    }
}

/// Terrain-only assembly result.
pub struct TerrainScene {
    pub window: SceneWindow,
    pub load: TerrainLoadGrid,
    pub scene: SceneGrid,
}

/// Load every region of `window`, fill regions without land data, and return the loaded grid
/// before any color construction (so callers can add loc-derived shadow input first).
pub fn load_window_terrain(
    definitions: &WorldDefinitions,
    window: SceneWindow,
) -> Result<TerrainLoadGrid, WorldError> {
    let mut grid = TerrainLoadGrid::reference();
    let mut missing = Vec::new();
    for region in window.regions() {
        let origin = window.region_scene_origin(region);
        match definitions.region(region)? {
            Some(map) => grid.load_region(&map.terrain, origin, (window.base_x, window.base_y), 0),
            None => missing.push(origin),
        }
    }
    for (scene_x, scene_y) in missing {
        grid.fill_missing_region(scene_x, scene_y, 64, 64);
    }
    Ok(grid)
}

/// Build terrain tiles for an already-loaded grid and link bridge tiles.
pub fn build_terrain_scene(
    definitions: &WorldDefinitions,
    window: SceneWindow,
    load: TerrainLoadGrid,
    presentation: TerrainPresentation,
) -> Result<TerrainScene, WorldError> {
    let palette = build_color_palette(presentation.brightness);
    let output = build_terrain(&load, definitions.floors(), presentation.jitter, &palette)?;
    let mut scene = SceneGrid::new(
        REFERENCE_SCENE_TILES as u32,
        REFERENCE_SCENE_TILES as u32,
        4,
    )?;
    output.apply_to_scene(&load, &mut scene)?;
    link_bridge_tiles(&load, &mut scene)?;
    Ok(TerrainScene {
        window,
        load,
        scene,
    })
}
