//! Bounded semantic tile storage for M6 scene construction.

use crate::terrain::TerrainSurface;
use osrs_core::coords::{SceneTile, StoragePlane};
use std::{error::Error, fmt};

/// Semantic data owned by one scene-storage tile.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SemanticTile {
    pub terrain: Option<TerrainSurface>,
}

/// Dense scene-storage grid with explicit plane count and bounds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SceneGrid {
    width: u32,
    height: u32,
    plane_count: u8,
    plane_len: usize,
    tiles: Vec<SemanticTile>,
}

/// Grid construction or access failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SceneGridError {
    InvalidPlaneCount(u8),
    CapacityOverflow,
    OutOfBounds {
        plane: u8,
        x: u32,
        y: u32,
    },
}

impl fmt::Display for SceneGridError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPlaneCount(count) => {
                write!(formatter, "scene plane count {count} is outside 1..=4")
            }
            Self::CapacityOverflow => formatter.write_str("scene grid capacity overflow"),
            Self::OutOfBounds { plane, x, y } => write!(
                formatter,
                "scene tile ({x},{y}) on storage plane {plane} is out of bounds"
            ),
        }
    }
}

impl Error for SceneGridError {}

impl SceneGrid {
    pub fn new(width: u32, height: u32, plane_count: u8) -> Result<Self, SceneGridError> {
        if !(1..=4).contains(&plane_count) {
            return Err(SceneGridError::InvalidPlaneCount(plane_count));
        }
        let width_usize = usize::try_from(width).map_err(|_| SceneGridError::CapacityOverflow)?;
        let height_usize = usize::try_from(height).map_err(|_| SceneGridError::CapacityOverflow)?;
        let plane_len = width_usize
            .checked_mul(height_usize)
            .ok_or(SceneGridError::CapacityOverflow)?;
        let len = plane_len
            .checked_mul(usize::from(plane_count))
            .ok_or(SceneGridError::CapacityOverflow)?;
        Ok(Self {
            width,
            height,
            plane_count,
            plane_len,
            tiles: vec![SemanticTile::default(); len],
        })
    }

    pub const fn width(&self) -> u32 {
        self.width
    }

    pub const fn height(&self) -> u32 {
        self.height
    }

    pub const fn plane_count(&self) -> u8 {
        self.plane_count
    }

    pub fn tile(&self, plane: StoragePlane, tile: SceneTile) -> Option<&SemanticTile> {
        self.index(plane, tile).map(|index| &self.tiles[index])
    }

    pub fn tile_mut(&mut self, plane: StoragePlane, tile: SceneTile) -> Option<&mut SemanticTile> {
        self.index(plane, tile).map(|index| &mut self.tiles[index])
    }

    pub fn set_terrain(
        &mut self,
        plane: StoragePlane,
        tile: SceneTile,
        terrain: TerrainSurface,
    ) -> Result<(), SceneGridError> {
        let Some(index) = self.index(plane, tile) else {
            return Err(SceneGridError::OutOfBounds {
                plane: plane.index().get(),
                x: tile.x,
                y: tile.y,
            });
        };
        self.tiles[index].terrain = Some(terrain);
        Ok(())
    }

    fn index(&self, plane: StoragePlane, tile: SceneTile) -> Option<usize> {
        let plane = plane.index().get();
        if plane >= self.plane_count || tile.x >= self.width || tile.y >= self.height {
            return None;
        }
        let x = usize::try_from(tile.x).ok()?;
        let y = usize::try_from(tile.y).ok()?;
        let width = usize::try_from(self.width).ok()?;
        let tile_index = y.checked_mul(width)?.checked_add(x)?;
        usize::from(plane)
            .checked_mul(self.plane_len)?
            .checked_add(tile_index)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terrain::{FlatTerrainSurface, TerrainCorners};

    #[test]
    fn grid_rejects_invalid_plane_counts() {
        assert_eq!(
            SceneGrid::new(64, 64, 0),
            Err(SceneGridError::InvalidPlaneCount(0))
        );
        assert_eq!(
            SceneGrid::new(64, 64, 5),
            Err(SceneGridError::InvalidPlaneCount(5))
        );
    }

    #[test]
    fn terrain_storage_is_plane_and_tile_exact() {
        let mut grid = SceneGrid::new(2, 3, 4).expect("valid grid");
        let plane = StoragePlane::new(2).expect("valid plane");
        let tile = SceneTile::new(1, 2);
        let surface = TerrainSurface::Flat(FlatTerrainSurface::new(
            TerrainCorners::new(1, 2, 3, 4),
            TerrainCorners::new(10, 11, 12, 13),
            Some(7),
        ));
        grid.set_terrain(plane, tile, surface.clone())
            .expect("in bounds");
        assert_eq!(grid.tile(plane, tile).and_then(|value| value.terrain.as_ref()), Some(&surface));
        assert!(grid
            .tile(StoragePlane::new(1).expect("valid plane"), tile)
            .and_then(|value| value.terrain.as_ref())
            .is_none());
    }

    #[test]
    fn out_of_bounds_access_does_not_alias_storage() {
        let mut grid = SceneGrid::new(2, 2, 1).expect("valid grid");
        let plane = StoragePlane::new(0).expect("valid plane");
        let terrain = TerrainSurface::Flat(FlatTerrainSurface::new(
            TerrainCorners::new(0, 0, 0, 0),
            TerrainCorners::new(0, 0, 0, 0),
            None,
        ));
        assert_eq!(
            grid.set_terrain(plane, SceneTile::new(2, 0), terrain),
            Err(SceneGridError::OutOfBounds {
                plane: 0,
                x: 2,
                y: 0,
            })
        );
    }
}
