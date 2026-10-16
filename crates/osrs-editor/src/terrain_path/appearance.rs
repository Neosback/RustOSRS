//! The real colour of a planned overlay path: the terrain builder's own output for the tiles the
//! path would paint, so the preview shows slope light, shadow, texture and the underlay faces of
//! shaped tiles instead of a flat guess. Reference colours; the HD profile re-lights terrain.

use super::FittedTile;
use osrs_scene::{
    terrain_appearance::{APPEARANCE_REACH, TileAppearance, window_appearance},
    terrain_build::{FloorLookup, TerrainJitter},
    terrain_load::{ResolvedTile, TerrainLoadGrid},
};
use osrs_world::TileInfo;
use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

/// Longest window edge in tiles. Longer paths fall back to the flat preview.
const MAX_WINDOW_TILES: i32 = 160;

/// Built appearance of the preview identified by `key`, valid for picker data `infos_version`.
#[derive(Debug, Clone, Default)]
pub struct PathAppearance {
    pub key: u64,
    pub infos_version: u64,
    /// Identity of the grade the heights were built with (zero when not grading).
    pub grade_key: u64,
    /// World tile of the build window's origin; vertex positions are relative to it.
    pub origin: (i32, i32),
    /// Empty when the window could not be built (missing data, too long); drawing then falls back.
    pub tiles: Vec<TileAppearance>,
}

/// `Rasterizer3D_colorPalette` at the client brightness the streamer builds terrain with.
pub fn reference_palette() -> &'static [i32] {
    static PALETTE: OnceLock<Vec<i32>> = OnceLock::new();
    PALETTE.get_or_init(|| {
        osrs_core::color_palette::build_color_palette(osrs_world::DEFAULT_BRIGHTNESS)
    })
}

/// Terrain appearance of every tile `paints` would lay overlay `overlay_id` on, on `plane`, and of
/// every tile whose corner `graded` moves (they keep their current floor, at the new heights).
///
/// `tile_at` supplies the current terrain of a world tile. Returns the window origin and the
/// tiles' triangles, or `None` when the window is too large or a painted tile is unknown.
pub fn path_appearance<F: FloorLookup>(
    plane: u8,
    overlay_id: u16,
    paints: &[FittedTile],
    graded: &[((i32, i32), i32)],
    tile_at: impl Fn((i32, i32)) -> Option<TileInfo>,
    floors: &F,
    palette: &[i32],
) -> Option<((i32, i32), Vec<TileAppearance>)> {
    // Tiles whose appearance is returned: painted ones and those a graded vertex touches.
    let mut shown: HashSet<(i32, i32)> = paints.iter().map(|p| (p.tile_x, p.tile_y)).collect();
    for ((x, y), _) in graded {
        for tile in [(*x - 1, *y - 1), (*x, *y - 1), (*x - 1, *y), (*x, *y)] {
            shown.insert(tile);
        }
    }
    let min_x = shown.iter().map(|t| t.0).min()?;
    let max_x = shown.iter().map(|t| t.0).max()?;
    let min_y = shown.iter().map(|t| t.1).min()?;
    let max_y = shown.iter().map(|t| t.1).max()?;
    let origin = (min_x - APPEARANCE_REACH, min_y - APPEARANCE_REACH);
    let size_x = max_x - min_x + 1 + 2 * APPEARANCE_REACH;
    let size_y = max_y - min_y + 1 + 2 * APPEARANCE_REACH;
    if size_x > MAX_WINDOW_TILES || size_y > MAX_WINDOW_TILES {
        return None;
    }
    let by_tile: HashMap<(i32, i32), &FittedTile> =
        paints.iter().map(|p| ((p.tile_x, p.tile_y), p)).collect();
    let graded: HashMap<(i32, i32), i32> = graded.iter().copied().collect();
    let mut grid = TerrainLoadGrid::new(size_x as usize, size_y as usize).ok()?;
    for wx in 0..size_x {
        for wy in 0..size_y {
            let tile = (origin.0 + wx, origin.1 + wy);
            let info = tile_at(tile);
            let mut resolved = match (by_tile.get(&tile), info) {
                (Some(paint), Some(info)) => ResolvedTile {
                    overlay_raw: overlay_id,
                    shape: paint.shape,
                    rotation: paint.orientation,
                    ..resolved(&info)
                },
                (Some(_), None) => return None,
                (None, Some(info)) => resolved(&info),
                (None, None) => continue,
            };
            // Corners `[southwest, southeast, northeast, northwest]` are the vertices at the
            // tile's own and next coordinates; a graded vertex replaces the current height.
            for (corner, (dx, dy)) in [(0, 0), (1, 0), (1, 1), (0, 1)].into_iter().enumerate() {
                if let Some(height) = graded.get(&(tile.0 + dx, tile.1 + dy)) {
                    resolved.heights[corner] = *height;
                }
            }
            grid.set_resolved_tile(usize::from(plane), wx as usize, wy as usize, resolved);
        }
    }
    let x = (min_x - origin.0) as u32..=(max_x - origin.0) as u32;
    let y = (min_y - origin.1) as u32..=(max_y - origin.1) as u32;
    let tiles = window_appearance(
        &grid,
        floors,
        TerrainJitter::default(),
        palette,
        plane,
        x,
        y,
    )
    .ok()?
    .into_iter()
    .filter(|tile| shown.contains(&(origin.0 + tile.tile.x as i32, origin.1 + tile.tile.y as i32)))
    .collect();
    Some((origin, tiles))
}

fn resolved(info: &TileInfo) -> ResolvedTile {
    ResolvedTile {
        heights: info.heights,
        underlay: info.underlay,
        overlay_raw: info.overlay_raw,
        shape: info.shape,
        rotation: info.rotation,
        settings: info.settings,
        shadow: info.shadow,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use osrs_scene::terrain::TerrainColorSource;
    use osrs_world::FloorTable;

    fn flat(_: (i32, i32)) -> Option<TileInfo> {
        Some(TileInfo {
            underlay: 1,
            ..TileInfo::default()
        })
    }

    fn paint(x: i32, y: i32, shape: u8) -> FittedTile {
        FittedTile {
            tile_x: x,
            tile_y: y,
            shape,
            orientation: 0,
        }
    }

    #[test]
    fn only_painted_tiles_are_returned_at_their_world_positions() {
        let paints = [paint(3200, 3210, 0), paint(3201, 3210, 0)];
        let Some((origin, tiles)) = path_appearance(
            0,
            7,
            &paints,
            &[],
            flat,
            &FloorTable::default(),
            reference_palette(),
        ) else {
            unreachable!("a window around two known tiles builds")
        };
        assert_eq!(origin, (3200 - APPEARANCE_REACH, 3210 - APPEARANCE_REACH));
        assert_eq!(tiles.len(), 2);
        for tile in &tiles {
            // A shape-0 overlay tile is the flat paint: two overlay triangles.
            assert_eq!(tile.triangles.len(), 2);
            assert!(
                tile.triangles
                    .iter()
                    .all(|t| t.source == TerrainColorSource::Overlay)
            );
            let world_x = tile.tile.x as i32 * 128; // vertex positions are window-relative
            let positions = tile
                .triangles
                .iter()
                .flat_map(|t| t.vertices.map(|v| v.position[0]));
            assert!(
                positions
                    .clone()
                    .all(|x| (world_x..=world_x + 128).contains(&(x)))
            );
        }
    }

    #[test]
    fn shaped_tiles_keep_underlay_faces_for_the_part_the_path_does_not_cover() {
        let Some((_, tiles)) = path_appearance(
            0,
            7,
            &[paint(3200, 3210, 2)],
            &[],
            flat,
            &FloorTable::default(),
            reference_palette(),
        ) else {
            unreachable!("a window around a known tile builds")
        };
        let sources: Vec<_> = tiles[0].triangles.iter().map(|t| t.source).collect();
        assert!(sources.contains(&TerrainColorSource::Overlay));
        assert!(sources.contains(&TerrainColorSource::Underlay));
    }

    #[test]
    fn unknown_painted_terrain_or_an_oversized_window_yields_none() {
        let paints = [paint(3200, 3210, 0)];
        let missing = |tile: (i32, i32)| (tile != (3200, 3210)).then(TileInfo::default);
        assert!(
            path_appearance(
                0,
                7,
                &paints,
                &[],
                missing,
                &FloorTable::default(),
                reference_palette()
            )
            .is_none()
        );
        let long = [paint(0, 0, 0), paint(MAX_WINDOW_TILES, 0, 0)];
        assert!(
            path_appearance(
                0,
                7,
                &long,
                &[],
                flat,
                &FloorTable::default(),
                reference_palette()
            )
            .is_none()
        );
    }

    #[test]
    fn graded_vertices_move_the_tiles_they_touch_and_add_them_to_the_preview() {
        let paints = [paint(3200, 3210, 0)];
        // Lift the vertex at the south-west corner of an unpainted neighbour to the east.
        let graded = [((3202, 3210), -64)];
        let Some((origin, tiles)) = path_appearance(
            0,
            7,
            &paints,
            &graded,
            flat,
            &FloorTable::default(),
            reference_palette(),
        ) else {
            unreachable!("the window builds")
        };
        // Painted tile plus the four tiles that share the graded vertex.
        let world: std::collections::BTreeSet<_> = tiles
            .iter()
            .map(|t| (origin.0 + t.tile.x as i32, origin.1 + t.tile.y as i32))
            .collect();
        for tile in [
            (3200, 3210),
            (3201, 3209),
            (3202, 3209),
            (3201, 3210),
            (3202, 3210),
        ] {
            assert!(world.contains(&tile), "{tile:?} missing from {world:?}");
        }
        // The tile whose south-west corner is graded has a vertex at the new height.
        let touched = tiles
            .iter()
            .find(|t| (origin.0 + t.tile.x as i32, origin.1 + t.tile.y as i32) == (3202, 3210));
        let Some(touched) = touched else {
            unreachable!("graded tile is shown")
        };
        assert!(
            touched
                .triangles
                .iter()
                .flat_map(|t| t.vertices)
                .any(|v| v.position[1] == -64)
        );
        // Without the grade the same tile has no such vertex.
        let Some((_, plain)) = path_appearance(
            0,
            7,
            &paints,
            &[],
            flat,
            &FloorTable::default(),
            reference_palette(),
        ) else {
            unreachable!("the window builds")
        };
        assert!(plain.iter().all(|t| {
            t.triangles
                .iter()
                .flat_map(|t| t.vertices)
                .all(|v| v.position[1] != -64)
        }));
    }
}
