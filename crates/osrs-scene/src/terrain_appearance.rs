//! The terrain tile appearance contract: the constants, shape rows and windowed build that every
//! consumer of "what will this tile look like" shares with the full terrain builder.
//!
//! `build_terrain` (`TERRAIN-004`) stays the single implementation of the client's blend, slope
//! light and shape emission. This module names the reach of that computation so callers (world
//! invalidation, the editor's path preview) do not each hard-code it, and exposes
//! [`window_appearance`], which runs the same builder over a small cropped grid and returns
//! palette-resolved triangles. For a tile at least [`APPEARANCE_REACH`] tiles inside the grid the
//! result equals the full-scene build, because nothing farther away can influence it: the
//! underlay blend sums a radius-[`BLEND_RADIUS`] window, slope light reads one tile around, and
//! the shadow grid is an input like any other.

use crate::{
    terrain::{TerrainColorSource, TerrainCorner, TerrainSurface},
    terrain_build::{FloorLookup, TerrainColorError, TerrainJitter, build_terrain, palette_rgb},
    terrain_contract::REFERENCE_TERRAIN_SKIP_COLOR,
    terrain_load::TerrainLoadGrid,
};
use osrs_core::coords::SceneTile;
use std::ops::RangeInclusive;

/// Underlay blending reaches this many tiles from a tile (`class470.method9712`).
pub const BLEND_RADIUS: i32 = 5;
/// Slope light reads the height field and shadow grid one tile around a tile.
pub const SLOPE_LIGHT_REACH: i32 = 1;
/// Tiles of context a window needs around a tile for its appearance to be exact: also how far an
/// edit can change a neighbour's colour.
pub const APPEARANCE_REACH: i32 = BLEND_RADIUS + SLOPE_LIGHT_REACH;
/// Lightness the tile-level palette RGB is rendered at (`Scene.addTile` brightness).
pub const CORNER_BRIGHTNESS: i32 = 96;

/// Row of the builder's 13-row shape tables for a file overlay shape (`0..=11`). Rows 0 and 1 are
/// "all underlay" and "all overlay"; row 1 is the flat paint.
pub const fn shape_row(file_shape: u8) -> i32 {
    file_shape as i32 + 1
}

/// One vertex of a built terrain triangle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TileVertex {
    /// Scene-local `[x, height, z]`, 128 units per tile, relative to the build grid's origin.
    pub position: [i32; 3],
    /// Palette colour (`0xRRGGBB`) of the vertex after slope light and shadow.
    pub rgb: i32,
}

/// One visible triangle of a built tile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TileTriangle {
    pub vertices: [TileVertex; 3],
    /// Texture of an overlay face, whose colour modulates the texture rather than replacing it.
    pub texture_id: Option<i32>,
    /// Average texture colour (`0xRRGGBB`) for a textured face.
    pub texture_rgb: Option<i32>,
    pub source: TerrainColorSource,
}

/// Everything the client would draw for one tile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TileAppearance {
    pub plane: u8,
    pub tile: SceneTile,
    pub triangles: Vec<TileTriangle>,
}

/// Why a window could not be built exactly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowError {
    /// A requested tile is closer than [`APPEARANCE_REACH`] to a grid edge, so tiles outside the
    /// grid could have changed its appearance.
    InsufficientMargin,
    Build(TerrainColorError),
}

impl std::fmt::Display for WindowError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InsufficientMargin => write!(
                formatter,
                "window needs {APPEARANCE_REACH} tiles of context around every requested tile"
            ),
            Self::Build(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for WindowError {}

/// Palette-resolved triangles of one built surface, skipping what the client skips (a flat paint
/// whose north-east colour is the sentinel; a shaped face whose first colour is).
pub fn surface_triangles(
    tile: SceneTile,
    surface: &TerrainSurface,
    palette: &[i32],
    texture_rgb: impl Fn(i32) -> i32,
) -> Vec<TileTriangle> {
    let base_x = tile.x as i32 * 128;
    let base_z = tile.y as i32 * 128;
    let vertex = |position: [i32; 3], color: i32| TileVertex {
        position,
        rgb: palette_rgb(palette, color),
    };
    match surface {
        TerrainSurface::Flat(flat) => {
            if flat.colors.northeast == REFERENCE_TERRAIN_SKIP_COLOR {
                return Vec::new();
            }
            let corner = |corner: TerrainCorner| match corner {
                TerrainCorner::Southwest => (
                    [base_x, flat.heights.southwest, base_z],
                    flat.colors.southwest,
                ),
                TerrainCorner::Southeast => (
                    [base_x + 128, flat.heights.southeast, base_z],
                    flat.colors.southeast,
                ),
                TerrainCorner::Northeast => (
                    [base_x + 128, flat.heights.northeast, base_z + 128],
                    flat.colors.northeast,
                ),
                TerrainCorner::Northwest => (
                    [base_x, flat.heights.northwest, base_z + 128],
                    flat.colors.northwest,
                ),
            };
            crate::terrain::FlatTerrainSurface::REFERENCE_TRIANGLES
                .iter()
                .map(|triangle| {
                    let [a, b, c] = triangle.map(corner);
                    TileTriangle {
                        vertices: [vertex(a.0, a.1), vertex(b.0, b.1), vertex(c.0, c.1)],
                        texture_id: flat.texture_id,
                        texture_rgb: flat.texture_id.map(&texture_rgb),
                        source: TerrainColorSource::Overlay,
                    }
                })
                .collect()
        }
        TerrainSurface::Shaped(shaped) => shaped
            .faces
            .iter()
            .filter(|face| face.colors[0] != REFERENCE_TERRAIN_SKIP_COLOR)
            .map(|face| {
                let at = |slot: usize| {
                    let position = shaped.vertices[face.indices[slot]].position;
                    vertex(
                        [position.x.units(), position.y.units(), position.z.units()],
                        face.colors[slot],
                    )
                };
                TileTriangle {
                    vertices: [at(0), at(1), at(2)],
                    texture_id: face.texture_id,
                    texture_rgb: face.texture_id.map(&texture_rgb),
                    source: face.color_source,
                }
            })
            .collect(),
    }
}

/// Appearance of the tiles `x` by `y` on `plane`, built by the real terrain builder over `grid`.
///
/// `grid` is a window assembled with [`TerrainLoadGrid::set_resolved_tile`]; every requested tile
/// must lie at least [`APPEARANCE_REACH`] tiles inside it.
pub fn window_appearance<F: FloorLookup>(
    grid: &TerrainLoadGrid,
    floors: &F,
    jitter: TerrainJitter,
    palette: &[i32],
    plane: u8,
    x: RangeInclusive<u32>,
    y: RangeInclusive<u32>,
) -> Result<Vec<TileAppearance>, WindowError> {
    let reach = APPEARANCE_REACH as u32;
    let inside = |range: &RangeInclusive<u32>, size: usize| {
        *range.start() >= reach && (*range.end() as usize) + (reach as usize) < size
    };
    if !inside(&x, grid.size_x()) || !inside(&y, grid.size_y()) {
        return Err(WindowError::InsufficientMargin);
    }
    let built = build_terrain(grid, floors, jitter, palette).map_err(WindowError::Build)?;
    Ok(built
        .tiles
        .iter()
        .filter(|tile| tile.plane == plane && x.contains(&tile.x) && y.contains(&tile.y))
        .map(|tile| {
            let scene_tile = SceneTile::new(tile.x, tile.y);
            TileAppearance {
                plane,
                tile: scene_tile,
                triangles: surface_triangles(scene_tile, &tile.surface, palette, |texture| {
                    floors.texture_average_rgb(texture)
                }),
            }
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{terrain_build::OverlayFloor, terrain_load::ResolvedTile};
    use osrs_core::floor_color::UnderlayHsl;

    struct Floors;

    impl FloorLookup for Floors {
        fn underlay(&self, index: u32) -> UnderlayHsl {
            let id = index as i32;
            UnderlayHsl {
                weighted_hue: (id * 37) % 200 + 20,
                saturation: (id * 53) % 120 + 30,
                lightness: (id * 29) % 150 + 40,
                hue_multiplier: 200 + id,
            }
        }

        fn overlay(&self, index: u32) -> OverlayFloor {
            let id = index as i32;
            OverlayFloor {
                texture: if id == 2 { 5 } else { -1 },
                primary_rgb: if id == 3 { 16_711_935 } else { 0x40_80_20 + id },
                hide_underlay: id % 2 == 0,
                hue: (id * 41) % 60,
                saturation: (id * 17) % 7,
                lightness: (id * 23) % 100 + 20,
                secondary_rgb: if id == 1 { 0x20_20_90 } else { -1 },
                secondary_hue: 12,
                secondary_saturation: 3,
                secondary_lightness: 70,
            }
        }

        fn texture_average_rgb(&self, texture_id: i32) -> i32 {
            0x10_20_30 + texture_id
        }
    }

    /// Deterministic pseudo-random terrain field over absolute tile coordinates.
    fn tile_at(x: i32, y: i32) -> ResolvedTile {
        let mix = |a: i32, b: i32, salt: i32| {
            let mut value = (a.wrapping_mul(73_856_093) ^ b.wrapping_mul(19_349_663) ^ salt)
                .wrapping_mul(0x9e37_79b1_u32 as i32);
            value ^= value >> 15;
            value.rem_euclid(1000)
        };
        let corner = |cx: i32, cy: i32| -(mix(cx, cy, 11) / 20) * 8;
        let overlay = if mix(x, y, 3) < 350 {
            mix(x, y, 4) % 4 + 1
        } else {
            0
        };
        ResolvedTile {
            heights: [
                corner(x, y),
                corner(x + 1, y),
                corner(x + 1, y + 1),
                corner(x, y + 1),
            ],
            underlay: (mix(x / 3, y / 3, 5) % 6) as u16,
            overlay_raw: overlay as u16,
            shape: (mix(x, y, 6) % 12) as u8,
            rotation: (mix(x, y, 7) % 4) as u8,
            settings: 0,
            shadow: (mix(x, y, 8) % 60) as u8,
        }
    }

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    fn grid(origin: (i32, i32), size: usize) -> TerrainLoadGrid {
        let Ok(mut grid) = TerrainLoadGrid::new(size, size) else {
            unreachable!("the test grids are at least 3x3")
        };
        for plane in 0..4 {
            for x in 0..size {
                for y in 0..size {
                    grid.set_resolved_tile(
                        plane,
                        x,
                        y,
                        tile_at(origin.0 + x as i32, origin.1 + y as i32),
                    );
                }
            }
        }
        grid
    }

    fn palette() -> Vec<i32> {
        (0..65_536).map(|index| index * 3 + 1).collect()
    }

    #[test]
    fn named_reach_matches_the_builder_and_shape_rows_skip_the_two_flat_rows() {
        assert_eq!(BLEND_RADIUS, 5);
        assert_eq!(APPEARANCE_REACH, 6);
        assert_eq!(shape_row(0), 1);
        assert_eq!(shape_row(11), 12);
    }

    #[test]
    fn window_matches_the_full_scene_build_for_interior_tiles() -> TestResult {
        let palette = palette();
        let jitter = TerrainJitter::default();
        let full = grid((0, 0), 48);
        let built = build_terrain(&full, &Floors, jitter, &palette)?;
        // Requested tiles 20..=27 by 18..=24 with the required context around them.
        let (origin, size) = ((14, 12), 8 + 12 + 1);
        let window = grid(origin, size);
        let (x_range, y_range) = (6..=13_u32, 6..=12_u32);
        let result = window_appearance(&window, &Floors, jitter, &palette, 0, x_range, y_range)?;
        assert!(result.iter().any(|tile| !tile.triangles.is_empty()));
        let mut compared = 0;
        for tile in result {
            let absolute =
                SceneTile::new(tile.tile.x + origin.0 as u32, tile.tile.y + origin.1 as u32);
            let reference = built
                .tiles
                .iter()
                .find(|b| b.plane == 0 && b.x == absolute.x && b.y == absolute.y)
                .ok_or("full build emits the tile")?;
            let mut expected = surface_triangles(absolute, &reference.surface, &palette, |t| {
                Floors.texture_average_rgb(t)
            });
            for triangle in &mut expected {
                for vertex in &mut triangle.vertices {
                    vertex.position[0] -= origin.0 * 128;
                    vertex.position[2] -= origin.1 * 128;
                }
            }
            assert_eq!(tile.triangles, expected, "tile {:?}", absolute);
            compared += 1;
        }
        assert!(compared > 40, "compared {compared} tiles");
        Ok(())
    }

    #[test]
    fn windows_without_context_are_rejected_instead_of_guessed() {
        let window = grid((0, 0), 20);
        let palette = palette();
        let near_edge = window_appearance(
            &window,
            &Floors,
            TerrainJitter::default(),
            &palette,
            0,
            5..=8,
            8..=10,
        );
        assert_eq!(near_edge.err(), Some(WindowError::InsufficientMargin));
        let far_edge = window_appearance(
            &window,
            &Floors,
            TerrainJitter::default(),
            &palette,
            0,
            8..=10,
            8..=14,
        );
        assert_eq!(far_edge.err(), Some(WindowError::InsufficientMargin));
    }

    /// Rewrite a whole column of tiles so the effect survives HSL quantisation.
    fn recolor_column(grid: &mut TerrainLoadGrid, x: i32, underlay: u16, height: i32) {
        for y in 0..grid.size_y() {
            let mut tile = tile_at(x, y as i32);
            tile.underlay = underlay;
            tile.heights = [height; 4];
            grid.set_resolved_tile(0, x as usize, y, tile);
        }
    }

    #[test]
    fn changing_tiles_beyond_the_reach_does_not_change_the_result_but_within_it_does() {
        let palette = palette();
        let jitter = TerrainJitter::default();
        let ask = |window: &TerrainLoadGrid| {
            window_appearance(window, &Floors, jitter, &palette, 0, 9..=9, 9..=9)
                .unwrap_or_default()
        };
        let mut base = grid((0, 0), 19);
        // The probed tile must exist (the builder emits nothing for a tile with no floor).
        base.set_resolved_tile(
            0,
            9,
            9,
            ResolvedTile {
                underlay: 2,
                overlay_raw: 0,
                heights: [-80; 4],
                ..tile_at(9, 9)
            },
        );
        assert_eq!(ask(&base).len(), 1, "the probed tile is built");
        assert!(!ask(&base)[0].triangles.is_empty());
        // One tile past the underlay blend reach and past the slope-light reach: no effect.
        let mut far = base.clone();
        recolor_column(&mut far, 9 + BLEND_RADIUS + 1, 6, -800);
        assert_eq!(ask(&base), ask(&far));
        // The outermost tile the blend reads: the average moves.
        let mut blend_edge = base.clone();
        recolor_column(
            &mut blend_edge,
            9 + BLEND_RADIUS,
            6,
            tile_at(9 + BLEND_RADIUS, 9).heights[0],
        );
        assert_ne!(
            ask(&base),
            ask(&blend_edge),
            "an underlay at the blend radius must matter"
        );
        // A neighbouring height: slope light moves.
        let mut slope = base.clone();
        recolor_column(&mut slope, 10, tile_at(10, 9).underlay, -1600);
        assert_ne!(ask(&base), ask(&slope), "a neighbouring height must matter");
    }
}
