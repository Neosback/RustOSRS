//! Terrain stage of scene assembly: regions -> loaded grid -> built colors -> scene tiles.

use crate::{SceneWindow, WorldDefinitions, WorldError};
use osrs_cache::decode::DecodedLocations;
use osrs_core::{color_palette::build_color_palette, coords::RegionCoord};
use osrs_scene::{
    SceneGrid,
    terrain_build::{TerrainJitter, build_terrain},
    terrain_load::TerrainLoadGrid,
};

/// Client default brightness used for the palette that feeds tile-level RGB.
pub const DEFAULT_BRIGHTNESS: f64 = 0.8;

/// Presentation inputs that do not affect the four 3D terrain corner colors.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TerrainPresentation {
    pub brightness: f64,
    pub jitter: TerrainJitter,
    /// Non-reference option: vertical tolerance for wall normal merging (`0` = exact client).
    pub wall_merge_tolerance: i32,
    /// Non-reference option: snap diagonal (`256`) wall-decoration plates flush onto the faces of
    /// the type-9 diagonal wall on their tile instead of using the client's fixed displacement
    /// (see the render review).
    pub flush_diagonal_decorations: bool,
}

impl Default for TerrainPresentation {
    fn default() -> Self {
        Self {
            brightness: DEFAULT_BRIGHTNESS,
            jitter: TerrainJitter::default(),
            wall_merge_tolerance: 0,
            flush_diagonal_decorations: false,
        }
    }
}

/// Terrain loaded for a window, with each region's location stream kept for the loc stage.
pub struct LoadedWindow {
    pub window: SceneWindow,
    pub grid: TerrainLoadGrid,
    pub locations: Vec<(RegionCoord, DecodedLocations)>,
}

/// Load every region of `window`, fill regions without land data, and keep their loc streams.
///
/// Mirrors the client order: all terrain first, then the empty-region fill, with locations
/// applied afterwards by the loc stage.
pub fn load_window(
    definitions: &WorldDefinitions,
    window: SceneWindow,
) -> Result<LoadedWindow, WorldError> {
    let mut grid = TerrainLoadGrid::reference();
    let mut missing = Vec::new();
    let mut locations = Vec::new();
    for region in window.regions() {
        let origin = window.region_scene_origin(region);
        match definitions.region(region)? {
            Some(map) => {
                grid.load_region(&map.terrain, origin, (window.base_x, window.base_y), 0);
                locations.push((region, map.locations));
            }
            None => missing.push(origin),
        }
    }
    for (scene_x, scene_y) in missing {
        grid.fill_missing_region(scene_x, scene_y, 64, 64);
    }
    Ok(LoadedWindow {
        window,
        grid,
        locations,
    })
}

/// Terrain-only convenience: the loaded grid without loc streams.
pub fn load_window_terrain(
    definitions: &WorldDefinitions,
    window: SceneWindow,
) -> Result<TerrainLoadGrid, WorldError> {
    Ok(load_window(definitions, window)?.grid)
}

/// Build terrain tiles from `load` (including any loc-derived shadow input already written to
/// it) and install them, with minimum planes, into `scene`.
pub fn apply_terrain(
    definitions: &WorldDefinitions,
    load: &TerrainLoadGrid,
    presentation: TerrainPresentation,
    scene: &mut SceneGrid,
) -> Result<(), WorldError> {
    let palette = build_color_palette(presentation.brightness);
    let output = build_terrain(load, definitions.floors(), presentation.jitter, &palette)?;
    output.apply_to_scene(load, scene)?;
    Ok(())
}
