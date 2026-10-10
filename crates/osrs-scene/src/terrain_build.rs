//! Terrain color/light builder (`TERRAIN-004`), a port of pinned `class470.method9712`.
//!
//! For each plane the builder (1) derives per-corner slope light from the height field minus the
//! loc-derived shadow grid, (2) accumulates underlay HSL over a radius-5 window with rolling
//! column/row sums, (3) emits one paint or shaped-model surface per tile, and (4) assigns each
//! tile's minimum plane. Operation order, integer division, and rounding follow the Java.
//!
//! Not ported here (separate scene stages): normal finalization of placed models and the
//! `setLinkBelow` relinking, which [`crate::SceneGrid`] already implements.
//!
//! Random hue/lightness jitter ([`TerrainJitter`]) only feeds the tile-level palette RGB values;
//! the four 3D corner colors never depend on it.

use crate::{
    terrain::{
        FlatTerrainSurface, ShapedTerrainInput, ShapedTerrainSurface, TerrainBuildError,
        TerrainCorners, TerrainSurface,
    },
    terrain_load::TerrainLoadGrid,
};
use osrs_core::{
    coords::SceneTile,
    definitions::FloorOverlayDefinition,
    floor_color::{
        UnderlayHsl, adjust_overlay_lightness, adjust_underlay_lightness, pack_terrain_hsl,
    },
};

const FLOOR_ID_MASK: u16 = 32_767;
const BLEND_RADIUS: i32 = 5;
const CORNER_BRIGHTNESS: i32 = 96;
const MAGENTA_RGB: i32 = 16_711_935;
/// Fixed `(int)sqrt(5100) * 768 >> 8` divisor of the slope-light term.
const SLOPE_LIGHT_DIVISOR: i32 = 213;

/// Client-side presentation jitter (`Tiles.rndHue`, `Tiles.rndLightness`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TerrainJitter {
    pub hue: i32,
    pub lightness: i32,
}

/// Overlay definition fields consumed by the builder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OverlayFloor {
    /// Texture id, or `-1`.
    pub texture: i32,
    pub primary_rgb: i32,
    pub hide_underlay: bool,
    pub hue: i32,
    pub saturation: i32,
    pub lightness: i32,
    /// Secondary RGB, or `-1`.
    pub secondary_rgb: i32,
    pub secondary_hue: i32,
    pub secondary_saturation: i32,
    pub secondary_lightness: i32,
}

impl OverlayFloor {
    /// The definition the client builds when the archive has no file for an id
    /// (`FloorOverlayDefinition` defaults followed by `postDecode`).
    pub fn missing() -> Self {
        Self {
            texture: -1,
            primary_rgb: 0,
            hide_underlay: true,
            hue: 0,
            saturation: 0,
            lightness: 0,
            secondary_rgb: -1,
            secondary_hue: 0,
            secondary_saturation: 0,
            secondary_lightness: 0,
        }
    }
}

impl From<&FloorOverlayDefinition> for OverlayFloor {
    fn from(definition: &FloorOverlayDefinition) -> Self {
        let secondary = definition.secondary_hsl;
        Self {
            texture: definition
                .texture
                .map_or(-1, |texture| texture.get() as i32),
            primary_rgb: definition.primary_rgb.get() as i32,
            hide_underlay: definition.hide_underlay,
            hue: definition.primary_hsl.hue,
            saturation: definition.primary_hsl.saturation,
            lightness: definition.primary_hsl.lightness,
            secondary_rgb: definition.secondary_rgb.map_or(-1, |rgb| rgb.get() as i32),
            secondary_hue: secondary.map_or(0, |hsl| hsl.hue),
            secondary_saturation: secondary.map_or(0, |hsl| hsl.saturation),
            secondary_lightness: secondary.map_or(0, |hsl| hsl.lightness),
        }
    }
}

/// Floor-definition and texture inputs.
pub trait FloorLookup {
    /// Underlay HSL for a zero-based definition id (default definition when absent).
    fn underlay(&self, index: u32) -> UnderlayHsl;
    /// Overlay definition for a zero-based id ([`OverlayFloor::missing`] when absent).
    fn overlay(&self, index: u32) -> OverlayFloor;
    /// `TextureLoader.getAverageTextureRGB`.
    fn texture_average_rgb(&self, texture_id: i32) -> i32;
}

/// One emitted terrain tile (`Scene.addTile`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuiltTerrainTile {
    pub plane: u8,
    pub x: u32,
    pub y: u32,
    pub surface: TerrainSurface,
}

/// Complete builder output in emission order.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TerrainBuildOutput {
    pub tiles: Vec<BuiltTerrainTile>,
}

/// Why a terrain build failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerrainColorError {
    /// Shaped-surface construction rejected a shape or rotation.
    Surface(TerrainBuildError),
    /// The palette did not have 65536 entries.
    PaletteSize(usize),
}

impl std::fmt::Display for TerrainColorError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Surface(error) => {
                write!(formatter, "terrain surface construction failed: {error}")
            }
            Self::PaletteSize(len) => {
                write!(formatter, "palette has {len} entries; expected 65536")
            }
        }
    }
}

impl std::error::Error for TerrainColorError {}

impl From<TerrainBuildError> for TerrainColorError {
    fn from(error: TerrainBuildError) -> Self {
        Self::Surface(error)
    }
}

/// Tile minimum plane written by the builder (`Scene.setTileMinPlane` argument).
pub fn tile_min_plane(grid: &TerrainLoadGrid, plane: usize, x: usize, y: usize) -> u8 {
    if grid.settings(plane, x, y) & 8 != 0 {
        0
    } else if plane > 0 && grid.settings(1, x, y) & 2 != 0 {
        (plane - 1) as u8
    } else {
        plane as u8
    }
}

/// Run the terrain builder over the whole loaded grid.
///
/// `palette` is `Rasterizer3D_colorPalette` for the client brightness (see
/// [`osrs_core::color_palette::build_color_palette`]).
pub fn build_terrain<F: FloorLookup>(
    grid: &TerrainLoadGrid,
    floors: &F,
    jitter: TerrainJitter,
    palette: &[i32],
) -> Result<TerrainBuildOutput, TerrainColorError> {
    if palette.len() != osrs_core::color_palette::PALETTE_LEN {
        return Err(TerrainColorError::PaletteSize(palette.len()));
    }
    let size_x = grid.size_x();
    let size_y = grid.size_y();
    let mut output = TerrainBuildOutput::default();
    // `MouseRecorder.field868`: shared across planes; only the interior is ever written.
    let mut light = vec![0_i32; (size_x + 1) * (size_y + 1)];
    let light_at = |light: &[i32], x: usize, y: usize| light[x * (size_y + 1) + y];

    for plane in 0..4_usize {
        compute_slope_light(grid, plane, &mut light);

        let mut hue_sum = vec![0_i32; size_y];
        let mut saturation_sum = vec![0_i32; size_y];
        let mut lightness_sum = vec![0_i32; size_y];
        let mut multiplier_sum = vec![0_i32; size_y];
        let mut count = vec![0_i32; size_y];

        for x in -BLEND_RADIUS..(size_x as i32 + BLEND_RADIUS) {
            for y in 0..size_y {
                let add = x + BLEND_RADIUS;
                if add >= 0 && (add as usize) < size_x {
                    let id = grid.underlay(plane, add as usize, y) & FLOOR_ID_MASK;
                    if id > 0 {
                        let def = floors.underlay(u32::from(id) - 1);
                        hue_sum[y] += def.weighted_hue;
                        saturation_sum[y] += def.saturation;
                        lightness_sum[y] += def.lightness;
                        multiplier_sum[y] += def.hue_multiplier;
                        count[y] += 1;
                    }
                }
                let remove = x - BLEND_RADIUS;
                if remove >= 0 && (remove as usize) < size_x {
                    let id = grid.underlay(plane, remove as usize, y) & FLOOR_ID_MASK;
                    if id > 0 {
                        let def = floors.underlay(u32::from(id) - 1);
                        hue_sum[y] -= def.weighted_hue;
                        saturation_sum[y] -= def.saturation;
                        lightness_sum[y] -= def.lightness;
                        multiplier_sum[y] -= def.hue_multiplier;
                        count[y] -= 1;
                    }
                }
            }

            if x < 1 || x >= size_x as i32 - 1 {
                continue;
            }
            let tile_x = x as usize;

            let (mut hue, mut saturation, mut lightness, mut multiplier, mut contributing) =
                (0_i32, 0_i32, 0_i32, 0_i32, 0_i32);
            for y in -BLEND_RADIUS..(size_y as i32 + BLEND_RADIUS) {
                let add = y + BLEND_RADIUS;
                if add >= 0 && (add as usize) < size_y {
                    hue += hue_sum[add as usize];
                    saturation += saturation_sum[add as usize];
                    lightness += lightness_sum[add as usize];
                    multiplier += multiplier_sum[add as usize];
                    contributing += count[add as usize];
                }
                let remove = y - BLEND_RADIUS;
                if remove >= 0 && (remove as usize) < size_y {
                    hue -= hue_sum[remove as usize];
                    saturation -= saturation_sum[remove as usize];
                    lightness -= lightness_sum[remove as usize];
                    multiplier -= multiplier_sum[remove as usize];
                    contributing -= count[remove as usize];
                }

                if y < 1 || y >= size_y as i32 - 1 {
                    continue;
                }
                let tile_y = y as usize;

                let underlay_id = grid.underlay(plane, tile_x, tile_y) & FLOOR_ID_MASK;
                let overlay_id = grid.overlay_raw(plane, tile_x, tile_y) & FLOOR_ID_MASK;
                if underlay_id == 0 && overlay_id == 0 {
                    continue;
                }

                let heights = TerrainCorners::new(
                    grid.height(plane, tile_x, tile_y),
                    grid.height(plane, tile_x + 1, tile_y),
                    grid.height(plane, tile_x + 1, tile_y + 1),
                    grid.height(plane, tile_x, tile_y + 1),
                );
                let corner_light = TerrainCorners::new(
                    light_at(&light, tile_x, tile_y),
                    light_at(&light, tile_x + 1, tile_y),
                    light_at(&light, tile_x + 1, tile_y + 1),
                    light_at(&light, tile_x, tile_y + 1),
                );

                let mut underlay_hsl = -1;
                let mut underlay_jittered_hsl = -1;
                if underlay_id > 0 {
                    let blended_hue = hue.wrapping_mul(256) / multiplier;
                    let blended_saturation = saturation / contributing;
                    let blended_lightness = lightness / contributing;
                    underlay_hsl =
                        pack_terrain_hsl(blended_hue, blended_saturation, blended_lightness);
                    let jittered_hue = (blended_hue + jitter.hue) & 255;
                    let jittered_lightness = (blended_lightness + jitter.lightness).clamp(0, 255);
                    underlay_jittered_hsl =
                        pack_terrain_hsl(jittered_hue, blended_saturation, jittered_lightness);
                }

                let mut underlay_rgb = 0;
                if underlay_jittered_hsl != -1 {
                    underlay_rgb = palette_rgb(
                        palette,
                        adjust_underlay_lightness(underlay_jittered_hsl, CORNER_BRIGHTNESS),
                    );
                }

                let underlay_colors = TerrainCorners::new(
                    adjust_underlay_lightness(underlay_hsl, corner_light.southwest),
                    adjust_underlay_lightness(underlay_hsl, corner_light.southeast),
                    adjust_underlay_lightness(underlay_hsl, corner_light.northeast),
                    adjust_underlay_lightness(underlay_hsl, corner_light.northwest),
                );

                let tile = SceneTile::new(tile_x as u32, tile_y as u32);
                if overlay_id == 0 {
                    output.tiles.push(BuiltTerrainTile {
                        plane: plane as u8,
                        x: tile.x,
                        y: tile.y,
                        surface: TerrainSurface::Flat(FlatTerrainSurface {
                            heights,
                            colors: underlay_colors,
                            texture_id: None,
                            is_flat: false,
                            rgb: underlay_rgb,
                        }),
                    });
                    continue;
                }

                let overlay = floors.overlay(u32::from(overlay_id) - 1);
                let mut texture = overlay.texture;
                let overlay_hsl;
                let mut overlay_jittered_hsl;
                if texture >= 0 {
                    overlay_jittered_hsl = floors.texture_average_rgb(texture);
                    overlay_hsl = -1;
                } else if overlay.primary_rgb == MAGENTA_RGB {
                    overlay_hsl = -2;
                    texture = -1;
                    overlay_jittered_hsl = -2;
                } else {
                    overlay_hsl =
                        pack_terrain_hsl(overlay.hue, overlay.saturation, overlay.lightness);
                    let jittered_hue = (overlay.hue + jitter.hue) & 255;
                    let jittered_lightness = (overlay.lightness + jitter.lightness).clamp(0, 255);
                    overlay_jittered_hsl =
                        pack_terrain_hsl(jittered_hue, overlay.saturation, jittered_lightness);
                }

                let mut overlay_rgb = 0;
                if overlay_jittered_hsl != -2 {
                    overlay_rgb = palette_rgb(
                        palette,
                        adjust_overlay_lightness(overlay_jittered_hsl, CORNER_BRIGHTNESS),
                    );
                }
                if overlay.secondary_rgb != -1 {
                    let jittered_hue = (overlay.secondary_hue + jitter.hue) & 255;
                    let jittered_lightness =
                        (overlay.secondary_lightness + jitter.lightness).clamp(0, 255);
                    overlay_jittered_hsl = pack_terrain_hsl(
                        jittered_hue,
                        overlay.secondary_saturation,
                        jittered_lightness,
                    );
                    overlay_rgb = palette_rgb(
                        palette,
                        adjust_overlay_lightness(overlay_jittered_hsl, CORNER_BRIGHTNESS),
                    );
                }

                let overlay_colors = TerrainCorners::new(
                    adjust_overlay_lightness(overlay_hsl, corner_light.southwest),
                    adjust_overlay_lightness(overlay_hsl, corner_light.southeast),
                    adjust_overlay_lightness(overlay_hsl, corner_light.northeast),
                    adjust_overlay_lightness(overlay_hsl, corner_light.northwest),
                );

                let add_tile_shape = i32::from(grid.shape(plane, tile_x, tile_y)) + 1;
                let texture_id = (texture >= 0).then_some(texture);
                let surface = if add_tile_shape == 1 {
                    // Flat overlay: `Scene.addTile` builds a paint from the overlay colors.
                    let is_flat = heights.southeast == heights.southwest
                        && heights.southwest == heights.northeast
                        && heights.northwest == heights.southwest;
                    TerrainSurface::Flat(FlatTerrainSurface {
                        heights,
                        colors: overlay_colors,
                        texture_id,
                        is_flat,
                        rgb: overlay_rgb,
                    })
                } else {
                    TerrainSurface::Shaped(ShapedTerrainSurface::build(ShapedTerrainInput {
                        shape: add_tile_shape as u8,
                        rotation: grid.rotation(plane, tile_x, tile_y),
                        texture_id,
                        tile,
                        heights,
                        underlay_colors,
                        overlay_colors,
                        underlay_rgb,
                        // `Scene.addTile` replaces a zero overlay RGB with 1 for shaped tiles.
                        overlay_rgb: if overlay_rgb == 0 { 1 } else { overlay_rgb },
                    })?)
                };
                output.tiles.push(BuiltTerrainTile {
                    plane: plane as u8,
                    x: tile.x,
                    y: tile.y,
                    surface,
                });

                // The client also ORs `2340` into a minimap/occlusion flag word here for flat
                // tiles above ground; no renderer consumes it, so it is not modeled.
            }
        }
    }
    Ok(output)
}

/// Slope-derived per-corner light minus the shadow grid (`field868`), interior tiles only.
fn compute_slope_light(grid: &TerrainLoadGrid, plane: usize, light: &mut [i32]) {
    let size_x = grid.size_x();
    let size_y = grid.size_y();
    for y in 1..size_y - 1 {
        for x in 1..size_x - 1 {
            let gradient_x = grid.height(plane, x + 1, y) - grid.height(plane, x - 1, y);
            let gradient_y = grid.height(plane, x, y + 1) - grid.height(plane, x, y - 1);
            let length =
                f64::from(gradient_x * gradient_x + gradient_y * gradient_y + 65_536).sqrt() as i32;
            let normal_x = (gradient_x << 8) / length;
            let normal_up = 65_536 / length;
            let normal_y = (gradient_y << 8) / length;
            let sun =
                (normal_y * -50 + normal_x * -50 + normal_up * -10) / SLOPE_LIGHT_DIVISOR + 96;
            let shadow = (i32::from(grid.shadow(plane, x, y + 1)) >> 3)
                + (i32::from(grid.shadow(plane, x - 1, y)) >> 2)
                + (i32::from(grid.shadow(plane, x, y - 1)) >> 2)
                + (i32::from(grid.shadow(plane, x + 1, y)) >> 3)
                + (i32::from(grid.shadow(plane, x, y)) >> 1);
            light[x * (size_y + 1) + y] = sun - shadow;
        }
    }
}

fn palette_rgb(palette: &[i32], index: i32) -> i32 {
    usize::try_from(index)
        .ok()
        .and_then(|index| palette.get(index).copied())
        .unwrap_or(0)
}

impl TerrainBuildOutput {
    /// Install the built surfaces into `scene` plane by plane, exactly like the client: each
    /// plane's `addTile` calls are followed by that plane's `setTileMinPlane` pass, so a lower
    /// plane tile created by a later plane's `addTile` keeps the default minimum plane.
    pub fn apply_to_scene(
        &self,
        load: &TerrainLoadGrid,
        scene: &mut crate::SceneGrid,
    ) -> Result<(), crate::SceneGridError> {
        let mut cursor = 0;
        for plane_index in 0..4_u8 {
            let plane = osrs_core::coords::StoragePlane::new(plane_index)
                .ok_or(crate::SceneGridError::InvalidPlaneIndex(plane_index))?;
            while cursor < self.tiles.len() && self.tiles[cursor].plane == plane_index {
                let built = &self.tiles[cursor];
                scene.add_terrain_tile(
                    plane,
                    SceneTile::new(built.x, built.y),
                    built.surface.clone(),
                )?;
                cursor += 1;
            }
            for y in 1..load.size_y() - 1 {
                for x in 1..load.size_x() - 1 {
                    scene.set_tile_min_plane(
                        plane,
                        SceneTile::new(x as u32, y as u32),
                        tile_min_plane(load, usize::from(plane_index), x, y),
                    );
                }
            }
        }
        Ok(())
    }
}

/// Apply `Scene.setLinkBelow` for every tile whose plane-1 settings carry the bridge bit.
///
/// The client runs this after terrain emission and scene normal finalization.
pub fn link_bridge_tiles(
    load: &TerrainLoadGrid,
    scene: &mut crate::SceneGrid,
) -> Result<(), crate::SceneGridError> {
    for x in 0..load.size_x() {
        for y in 0..load.size_y() {
            if load.settings(1, x, y) & 2 == 2 {
                scene.set_link_below(SceneTile::new(x as u32, y as u32))?;
            }
        }
    }
    Ok(())
}
