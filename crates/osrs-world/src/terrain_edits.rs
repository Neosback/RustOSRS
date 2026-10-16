//! Editor changes to terrain tiles, applied on top of what the cache decodes.
//!
//! The cache is never written. [`TerrainEdits`] holds the final value of every channel the editor
//! has set on a tile; [`crate::WorldDefinitions::region`] applies them right after decoding, so the
//! scene builder, the picker data and the HD stream all see the edited map.

use osrs_scene::terrain_load::{RegionTerrainTile, RegionTileOverlay};
use std::collections::HashMap;

const REGION_TILES: i32 = 64;

/// The terrain channels of one tile after an edit. Absolute heights project semantic state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TileEdit {
    /// Source plane `0..=3`.
    pub plane: u8,
    /// World tile.
    pub tile: (i32, i32),
    /// Underlay id as stored in the map file (`0` = none).
    pub underlay: u16,
    /// Overlay id as stored in the map file (`0` = none).
    pub overlay: u16,
    pub shape: u8,
    pub rotation: u8,
    /// Tile-settings byte.
    pub flags: u8,
    /// Absolute client-unit corner height (`None` keeps the decoded height). Applied after all
    /// source heights are resolved, before loc placement, lighting, picking and HD extraction.
    pub height: Option<i32>,
}

impl TileEdit {
    /// Region and in-region coordinates of the tile.
    pub fn locate(&self) -> ((i32, i32), (u8, u8)) {
        (
            (
                self.tile.0.div_euclid(REGION_TILES),
                self.tile.1.div_euclid(REGION_TILES),
            ),
            (
                self.tile.0.rem_euclid(REGION_TILES) as u8,
                self.tile.1.rem_euclid(REGION_TILES) as u8,
            ),
        )
    }

    /// Write the edit into a decoded tile; heights are left as decoded.
    pub fn apply_to(&self, tile: &mut RegionTerrainTile) {
        tile.underlay = self.underlay;
        tile.overlay = (self.overlay != 0).then_some(RegionTileOverlay {
            raw_id: self.overlay,
            shape: self.shape.min(11),
            rotation: self.rotation & 3,
        });
        tile.settings = (self.flags != 0).then_some(self.flags);
    }
}

/// Edits of one region by `(plane, local x, local y)`.
type RegionEdits = HashMap<(u8, u8, u8), TileEdit>;

/// Every edit, by region then by tile; a later edit of a tile replaces the earlier one.
#[derive(Debug, Default)]
pub struct TerrainEdits {
    by_region: HashMap<(i32, i32), RegionEdits>,
}

impl TerrainEdits {
    pub fn apply(&mut self, edits: &[TileEdit]) {
        for edit in edits {
            let (region, (x, y)) = edit.locate();
            self.by_region
                .entry(region)
                .or_default()
                .insert((edit.plane, x, y), *edit);
        }
    }

    /// Project authoritative absolute heights after loading and missing-region fill. Applying
    /// decoded height bytes would lose single-unit precision and implicitly move upper planes.
    pub fn project_heights(
        &self,
        grid: &mut osrs_scene::terrain_load::TerrainLoadGrid,
        origin: osrs_core::coords::MapTile,
    ) {
        for edits in self.by_region.values() {
            for edit in edits.values() {
                if let Some(height) = edit.height {
                    let x = edit.tile.0 - origin.x;
                    let y = edit.tile.1 - origin.y;
                    if x >= 0 && y >= 0 {
                        grid.set_height_vertex(
                            usize::from(edit.plane),
                            x as usize,
                            y as usize,
                            height,
                        );
                    }
                }
            }
        }
    }

    pub fn clear(&mut self) {
        self.by_region.clear();
    }

    /// The edits inside one region.
    pub fn region(&self, region: (i32, i32)) -> impl Iterator<Item = &TileEdit> {
        self.by_region
            .get(&region)
            .into_iter()
            .flat_map(|edits| edits.values())
    }

    pub fn is_empty(&self) -> bool {
        self.by_region.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use osrs_core::coords::MapTile;
    use osrs_scene::terrain_load::{RegionTileHeight, TerrainLoadGrid};

    fn tile() -> RegionTerrainTile {
        RegionTerrainTile {
            height: RegionTileHeight::Explicit(9),
            overlay: Some(RegionTileOverlay {
                raw_id: 7,
                shape: 3,
                rotation: 1,
            }),
            settings: Some(1),
            underlay: 5,
        }
    }

    fn edit() -> TileEdit {
        TileEdit {
            plane: 0,
            tile: (3222, 3218),
            underlay: 49,
            overlay: 111,
            shape: 4,
            rotation: 2,
            flags: 0,
            height: None,
        }
    }

    #[test]
    fn an_edit_replaces_the_channels_and_keeps_the_height() {
        let mut tile = tile();
        edit().apply_to(&mut tile);
        assert_eq!(tile.height, RegionTileHeight::Explicit(9));
        assert_eq!(tile.underlay, 49);
        assert_eq!(
            tile.overlay,
            Some(RegionTileOverlay {
                raw_id: 111,
                shape: 4,
                rotation: 2
            })
        );
        assert_eq!(tile.settings, None);
    }

    #[test]
    fn absolute_heights_are_applied_after_decoding_without_moving_upper_planes() {
        let mut edits = TerrainEdits::default();
        edits.apply(&[TileEdit {
            height: Some(-53),
            ..edit()
        }]);
        let mut decoded = tile();
        edits
            .region((50, 50))
            .next()
            .unwrap_or_else(|| unreachable!("valid fixture"))
            .apply_to(&mut decoded);
        assert_eq!(decoded.height, RegionTileHeight::Explicit(9));
        let mut grid = TerrainLoadGrid::new(4, 4).unwrap_or_else(|_| unreachable!("valid fixture"));
        grid.set_height_vertex(1, 2, 2, -240);
        edits.project_heights(&mut grid, MapTile::new(3220, 3216));
        assert_eq!(grid.height(0, 2, 2), -53);
        assert_eq!(grid.height(1, 2, 2), -240);
        edits.apply(&[TileEdit {
            height: Some(-40),
            ..edit()
        }]);
        edits.project_heights(&mut grid, MapTile::new(3220, 3216));
        assert_eq!(grid.height(0, 2, 2), -40);
    }

    #[test]
    fn overlay_zero_clears_the_overlay() {
        let mut tile = tile();
        TileEdit {
            overlay: 0,
            ..edit()
        }
        .apply_to(&mut tile);
        assert_eq!(tile.overlay, None);
    }

    #[test]
    fn edits_are_grouped_by_region_and_replace_earlier_ones() {
        let mut edits = TerrainEdits::default();
        edits.apply(&[
            edit(),
            TileEdit {
                underlay: 3,
                ..edit()
            },
        ]);
        // 3222 / 64 = 50 and 3218 / 64 = 50.
        let found: Vec<_> = edits.region((50, 50)).collect();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].underlay, 3);
        assert_eq!(edits.region((51, 50)).count(), 0);
        assert_eq!(edit().locate(), ((50, 50), (22, 18)));
    }
}
