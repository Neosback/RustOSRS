//! Editor changes to terrain tiles, applied on top of what the cache decodes.
//!
//! The cache is never written. [`TerrainEdits`] holds the final value of every channel the editor
//! has set on a tile; [`crate::WorldDefinitions::region`] applies them right after decoding, so the
//! scene builder, the picker data and the HD stream all see the edited map.

use osrs_scene::terrain_load::{RegionTerrainTile, RegionTileHeight, RegionTileOverlay};
use std::collections::HashMap;

const REGION_TILES: i32 = 64;

/// The terrain channels of one tile after an edit (heights are not edited yet).
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
    /// The stored height byte (`None` keeps the height as decoded). On plane 0 the height is
    /// `-8 * byte`; above it, the height of the plane below minus `8 * byte`; a stored 1 reads as 0.
    pub height: Option<u8>,
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
        if let Some(byte) = self.height {
            tile.height = RegionTileHeight::Explicit(byte);
        }
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
    fn a_stored_height_replaces_the_decoded_one() {
        let mut tile = tile();
        TileEdit {
            height: Some(30),
            ..edit()
        }
        .apply_to(&mut tile);
        assert_eq!(tile.height, RegionTileHeight::Explicit(30));
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
