//! Conversion from fitted preview tiles to headless Path planning.

pub(super) use osrs_edit::TerrainPathIntent;
#[cfg(test)]
pub(super) use osrs_edit::build_terrain_path_patch;

use super::editing::PathTile;

pub(super) fn path_intent(tile: PathTile) -> TerrainPathIntent {
    TerrainPathIntent {
        tile: osrs_core::coords::MapTile::new(tile.tile.0, tile.tile.1),
        overlay: tile.overlay,
        shape: tile.shape,
        rotation: tile.rotation,
    }
}
