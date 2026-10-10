//! Terrain stream loading into the scene's height/flag/floor arrays (`TERRAIN-004` inputs).
//!
//! Ports the pinned client's `class264.loadTerrain`, its default-height noise
//! (`class236.method5170` + `class450.method9120`), and `ScriptFrame.method749` (the fill applied
//! to scene regions that have no land data). The arrays mirror the `WorldView` arrays the terrain
//! builder consumes; every operation and its order follow the Java.
//!
//! Inputs are cache-agnostic ([`RegionTerrainTile`]) so `osrs-scene` keeps depending only on
//! `osrs-core`; callers convert decoded cache streams.

use osrs_core::trig::trig_tables;
use std::{error::Error, fmt};

/// Tiles along one region axis.
pub const REGION_TILES: usize = 64;
/// Planes in a scene.
pub const SCENE_PLANES: usize = 4;
/// Reference scene edge length in tiles (13 chunks of 8).
pub const REFERENCE_SCENE_TILES: usize = 104;

const NOISE_X_BIAS: i32 = 932_731;
const NOISE_Z_BIAS: i32 = 556_238;

/// Height opcode state of one terrain tile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegionTileHeight {
    /// Opcode 0: default height (noise on plane 0, `-240` above the plane below otherwise).
    Default,
    /// Opcode 1: explicit height byte.
    Explicit(u8),
}

/// Overlay payload of one tile before the region rotation is applied.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RegionTileOverlay {
    /// Raw 16-bit overlay bits (masked with `0x7fff` by consumers; the client stores a `short`).
    pub raw_id: u16,
    /// Overlay shape `0..=11` (`(opcode - 2) / 4`).
    pub shape: u8,
    /// Overlay rotation `0..=3` (`(opcode - 2) & 3`) before the region rotation.
    pub rotation: u8,
}

/// One decoded terrain tile in stream order (plane, then local X, then local Y).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RegionTerrainTile {
    pub height: RegionTileHeight,
    pub overlay: Option<RegionTileOverlay>,
    /// Tile-settings value (`opcode - 49`), or `None` when the stream had no settings opcode.
    pub settings: Option<u8>,
    /// One-based underlay id (`opcode - 81`), or `0` when absent.
    pub underlay: u16,
}

/// A whole region (`4 * 64 * 64` tiles) in stream order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegionTerrain {
    tiles: Vec<RegionTerrainTile>,
}

/// Region construction failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerrainLoadError {
    /// The tile count was not `4 * 64 * 64`.
    WrongTileCount { actual: usize },
    /// A scene dimension or origin was unusable.
    InvalidSceneSize { size_x: usize, size_y: usize },
}

impl fmt::Display for TerrainLoadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongTileCount { actual } => write!(
                formatter,
                "region terrain has {actual} tiles; expected {}",
                SCENE_PLANES * REGION_TILES * REGION_TILES
            ),
            Self::InvalidSceneSize { size_x, size_y } => {
                write!(formatter, "invalid terrain scene size {size_x}x{size_y}")
            }
        }
    }
}

impl Error for TerrainLoadError {}

impl RegionTerrain {
    pub fn new(tiles: Vec<RegionTerrainTile>) -> Result<Self, TerrainLoadError> {
        let expected = SCENE_PLANES * REGION_TILES * REGION_TILES;
        if tiles.len() != expected {
            return Err(TerrainLoadError::WrongTileCount {
                actual: tiles.len(),
            });
        }
        Ok(Self { tiles })
    }

    fn tile(&self, plane: usize, x: usize, y: usize) -> &RegionTerrainTile {
        &self.tiles[(plane * REGION_TILES + x) * REGION_TILES + y]
    }
}

/// Scene-sized arrays the terrain builder consumes.
///
/// Per-plane tile arrays are `size_x * size_y`; height and shadow arrays are
/// `(size_x + 1) * (size_y + 1)` like the client's `tileHeights` and `Tiles_underlays2`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerrainLoadGrid {
    size_x: usize,
    size_y: usize,
    heights: Vec<i32>,
    settings: Vec<u8>,
    underlays: Vec<u16>,
    overlays: Vec<u16>,
    shapes: Vec<u8>,
    rotations: Vec<u8>,
    shadow: Vec<u8>,
}

impl TerrainLoadGrid {
    pub fn new(size_x: usize, size_y: usize) -> Result<Self, TerrainLoadError> {
        if size_x < 3 || size_y < 3 {
            return Err(TerrainLoadError::InvalidSceneSize { size_x, size_y });
        }
        let tile_len = SCENE_PLANES * size_x * size_y;
        let corner_len = SCENE_PLANES * (size_x + 1) * (size_y + 1);
        Ok(Self {
            size_x,
            size_y,
            heights: vec![0; corner_len],
            settings: vec![0; tile_len],
            underlays: vec![0; tile_len],
            overlays: vec![0; tile_len],
            shapes: vec![0; tile_len],
            rotations: vec![0; tile_len],
            shadow: vec![0; corner_len],
        })
    }

    /// Reference 104x104 scene.
    pub fn reference() -> Self {
        Self {
            size_x: REFERENCE_SCENE_TILES,
            size_y: REFERENCE_SCENE_TILES,
            heights: vec![0; SCENE_PLANES * (REFERENCE_SCENE_TILES + 1).pow(2)],
            settings: vec![0; SCENE_PLANES * REFERENCE_SCENE_TILES.pow(2)],
            underlays: vec![0; SCENE_PLANES * REFERENCE_SCENE_TILES.pow(2)],
            overlays: vec![0; SCENE_PLANES * REFERENCE_SCENE_TILES.pow(2)],
            shapes: vec![0; SCENE_PLANES * REFERENCE_SCENE_TILES.pow(2)],
            rotations: vec![0; SCENE_PLANES * REFERENCE_SCENE_TILES.pow(2)],
            shadow: vec![0; SCENE_PLANES * (REFERENCE_SCENE_TILES + 1).pow(2)],
        }
    }

    pub const fn size_x(&self) -> usize {
        self.size_x
    }

    pub const fn size_y(&self) -> usize {
        self.size_y
    }

    fn tile_index(&self, plane: usize, x: usize, y: usize) -> usize {
        (plane * self.size_x + x) * self.size_y + y
    }

    fn corner_index(&self, plane: usize, x: usize, y: usize) -> usize {
        (plane * (self.size_x + 1) + x) * (self.size_y + 1) + y
    }

    /// `tileHeights[plane][x][y]` (corner height, `x <= size_x`, `y <= size_y`).
    pub fn height(&self, plane: usize, x: usize, y: usize) -> i32 {
        self.heights[self.corner_index(plane, x, y)]
    }

    /// `tileSettings[plane][x][y]`.
    pub fn settings(&self, plane: usize, x: usize, y: usize) -> u8 {
        self.settings[self.tile_index(plane, x, y)]
    }

    /// `Tiles_underlays[plane][x][y]` one-based id, `0` for none.
    pub fn underlay(&self, plane: usize, x: usize, y: usize) -> u16 {
        self.underlays[self.tile_index(plane, x, y)]
    }

    /// `Tiles_overlays[plane][x][y]` raw bits.
    pub fn overlay_raw(&self, plane: usize, x: usize, y: usize) -> u16 {
        self.overlays[self.tile_index(plane, x, y)]
    }

    /// `Tiles_shapes[plane][x][y]` (overlay shape `0..=11`).
    pub fn shape(&self, plane: usize, x: usize, y: usize) -> u8 {
        self.shapes[self.tile_index(plane, x, y)]
    }

    /// Overlay rotation after the region rotation (`field49`).
    pub fn rotation(&self, plane: usize, x: usize, y: usize) -> u8 {
        self.rotations[self.tile_index(plane, x, y)]
    }

    /// `Tiles_underlays2[plane][x][y]` shadow/clipping value.
    pub fn shadow(&self, plane: usize, x: usize, y: usize) -> u8 {
        self.shadow[self.corner_index(plane, x, y)]
    }

    /// Set a shadow cell directly (stand-in for the loc-placement `clipped` writes).
    pub fn set_shadow(&mut self, plane: usize, x: usize, y: usize, value: u8) {
        let index = self.corner_index(plane, x, y);
        self.shadow[index] = value;
    }

    /// Raise a shadow cell to at least `value` (the `clipped` game-object write rule).
    pub fn raise_shadow(&mut self, plane: usize, x: usize, y: usize, value: u8) {
        let index = self.corner_index(plane, x, y);
        if value > self.shadow[index] {
            self.shadow[index] = value;
        }
    }

    /// Port of `class337.method7281`'s terrain loop for one region.
    ///
    /// `scene_origin` is the scene tile of the region's local `(0, 0)` and may be negative or
    /// extend past the scene; out-of-range tiles are skipped exactly like the client skips their
    /// bytes. `noise_origin` is the world tile of scene tile `(0, 0)` and only feeds the default
    /// height noise. `rotation` is the instance rotation (`0` for normal regions).
    pub fn load_region(
        &mut self,
        terrain: &RegionTerrain,
        scene_origin: (i32, i32),
        noise_origin: (i32, i32),
        rotation: u8,
    ) {
        for plane in 0..SCENE_PLANES {
            for local_x in 0..REGION_TILES {
                for local_y in 0..REGION_TILES {
                    let tile_x = scene_origin.0 + local_x as i32;
                    let tile_y = scene_origin.1 + local_y as i32;
                    let tile = terrain.tile(plane, local_x, local_y);
                    self.load_tile(
                        plane,
                        tile_x,
                        tile_y,
                        tile_x.wrapping_add(noise_origin.0),
                        noise_origin.1.wrapping_add(tile_y),
                        rotation,
                        tile,
                    );
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn load_tile(
        &mut self,
        plane: usize,
        tile_x: i32,
        tile_y: i32,
        noise_x: i32,
        noise_y: i32,
        rotation: u8,
        tile: &RegionTerrainTile,
    ) {
        // `loadTerrain` bounds: 0 <= x < heights[0].length - 1, same for y.
        if tile_x < 0
            || tile_y < 0
            || tile_x as usize >= self.size_x
            || tile_y as usize >= self.size_y
        {
            return;
        }
        let (x, y) = (tile_x as usize, tile_y as usize);
        let tile_index = self.tile_index(plane, x, y);
        self.settings[tile_index] = 0;

        if let Some(overlay) = tile.overlay {
            self.overlays[tile_index] = overlay.raw_id;
            self.shapes[tile_index] = overlay.shape;
            self.rotations[tile_index] = (overlay.rotation + rotation) & 3;
        }
        if let Some(settings) = tile.settings {
            self.settings[tile_index] = settings;
        }
        if tile.underlay != 0 {
            self.underlays[tile_index] = tile.underlay;
        }

        let corner = self.corner_index(plane, x, y);
        self.heights[corner] = match (tile.height, plane) {
            (RegionTileHeight::Default, 0) => -default_noise_height(noise_x, noise_y) * 8,
            (RegionTileHeight::Default, _) => {
                self.heights[self.corner_index(plane - 1, x, y)] - 240
            }
            (RegionTileHeight::Explicit(raw), 0) => -i32::from(explicit_height(raw)) * 8,
            (RegionTileHeight::Explicit(raw), _) => {
                self.heights[self.corner_index(plane - 1, x, y)]
                    - i32::from(explicit_height(raw)) * 8
            }
        };
    }

    /// Port of `ScriptFrame.method749`: fill a scene region that has no land data.
    ///
    /// Marks plane-0 shadow cells `127` and copies neighbouring plane-0 heights onto the region
    /// border. Bounds are inclusive on both axes exactly like the client.
    pub fn fill_missing_region(&mut self, scene_x: i32, scene_y: i32, width: i32, height: i32) {
        let last_x = self.size_x as i32 - 1;
        let last_y = self.size_y as i32 - 1;
        for y in scene_y..=scene_y + height {
            for x in scene_x..=scene_x + width {
                if x < 0 || x >= self.size_x as i32 || y < 0 || y >= self.size_y as i32 {
                    continue;
                }
                let (ux, uy) = (x as usize, y as usize);
                let shadow = self.corner_index(0, ux, uy);
                self.shadow[shadow] = 127;
                let target = self.corner_index(0, ux, uy);
                if x == scene_x && x > 0 {
                    self.heights[target] = self.heights[self.corner_index(0, ux - 1, uy)];
                }
                if width + scene_x == x && x < last_x {
                    self.heights[target] = self.heights[self.corner_index(0, ux + 1, uy)];
                }
                if y == scene_y && y > 0 {
                    self.heights[target] = self.heights[self.corner_index(0, ux, uy - 1)];
                }
                if scene_y + height == y && y < last_y {
                    self.heights[target] = self.heights[self.corner_index(0, ux, uy + 1)];
                }
            }
        }
    }
}

const fn explicit_height(raw: u8) -> u8 {
    // The client maps an explicit height byte of 1 to 0.
    if raw == 1 { 0 } else { raw }
}

/// `class236.method5170` composition used for default plane-0 heights (before the `* -8`).
fn default_noise_height(noise_x: i32, noise_y: i32) -> i32 {
    let x = noise_x.wrapping_add(NOISE_X_BIAS);
    let z = noise_y.wrapping_add(NOISE_Z_BIAS);
    let combined = interpolated_noise(45_365_i32.wrapping_add(x), z.wrapping_add(91_923), 4) - 128
        + ((interpolated_noise(x.wrapping_add(10_294), z.wrapping_add(37_821), 2) - 128) >> 1)
        + ((interpolated_noise(x, z, 1) - 128) >> 2);
    let scaled = (f64::from(combined) * 0.3) as i32 + 35;
    scaled.clamp(10, 60)
}

/// `class236.method5170(x, y, scale)`.
fn interpolated_noise(x: i32, y: i32, scale: i32) -> i32 {
    let cosine = |position: i32| trig_tables().cosine((position * 1024 / scale) as usize);
    let cell_x = x / scale;
    let frac_x = x & (scale - 1);
    let cell_y = y / scale;
    let frac_y = y & (scale - 1);

    let a = smoothed_noise(cell_x, cell_y);
    let b = smoothed_noise(cell_x + 1, cell_y);
    let c = smoothed_noise(cell_x, cell_y + 1);
    let d = smoothed_noise(cell_x + 1, cell_y + 1);

    let weight_x = (65_536 - cosine(frac_x)) >> 1;
    let row0 = (((65_536 - weight_x).wrapping_mul(a)) >> 16) + ((weight_x.wrapping_mul(b)) >> 16);
    let row1 = (((65_536 - weight_x).wrapping_mul(c)) >> 16) + ((weight_x.wrapping_mul(d)) >> 16);
    let weight_y = (65_536 - cosine(frac_y)) >> 1;
    (((65_536 - weight_y).wrapping_mul(row0)) >> 16) + ((weight_y.wrapping_mul(row1)) >> 16)
}

/// `Projection.method5894`.
fn smoothed_noise(x: i32, y: i32) -> i32 {
    let corners = hash_noise(x - 1, y - 1)
        + hash_noise(x + 1, y - 1)
        + hash_noise(x - 1, y + 1)
        + hash_noise(x + 1, y + 1);
    let sides =
        hash_noise(x - 1, y) + hash_noise(x + 1, y) + hash_noise(x, y - 1) + hash_noise(x, y + 1);
    let center = hash_noise(x, y);
    corners / 16 + sides / 8 + center / 4
}

/// `class450.method9120`.
fn hash_noise(x: i32, y: i32) -> i32 {
    let mut n = x.wrapping_add(y.wrapping_mul(57));
    n ^= n.wrapping_shl(13);
    let mixed = n
        .wrapping_mul(n.wrapping_mul(n).wrapping_mul(15_731).wrapping_add(789_221))
        .wrapping_add(1_376_312_589)
        & i32::MAX;
    (mixed >> 19) & 255
}
