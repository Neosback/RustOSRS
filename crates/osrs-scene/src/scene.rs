//! Bounded semantic tile storage for M6 scene construction.

use crate::terrain::TerrainSurface;
use osrs_core::{
    coords::{SceneTile, SourcePlane, StoragePlane},
    ids::ObjectId,
};
use std::{error::Error, fmt};

const GAME_OBJECT_TAG_TYPE: u8 = 2;

/// Semantic game-object state owned by the scene tile stack.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SceneGameObject {
    object_id: ObjectId,
    tag_type: u8,
    start: SceneTile,
    plane: StoragePlane,
}

impl SceneGameObject {
    pub const fn new(
        object_id: ObjectId,
        tag_type: u8,
        start: SceneTile,
        plane: StoragePlane,
    ) -> Self {
        Self {
            object_id,
            tag_type,
            start,
            plane,
        }
    }

    pub const fn object_id(&self) -> ObjectId {
        self.object_id
    }

    pub const fn tag_type(&self) -> u8 {
        self.tag_type
    }

    pub const fn start(&self) -> SceneTile {
        self.start
    }

    pub const fn plane(&self) -> StoragePlane {
        self.plane
    }
}

/// Semantic data owned by one scene-storage tile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemanticTile {
    source_plane: Option<SourcePlane>,
    storage_plane: StoragePlane,
    pub terrain: Option<TerrainSurface>,
    game_objects: Vec<SceneGameObject>,
    linked_below: Option<Box<SemanticTile>>,
}

impl SemanticTile {
    pub fn new(source_plane: Option<SourcePlane>, storage_plane: StoragePlane) -> Self {
        Self {
            source_plane,
            storage_plane,
            terrain: None,
            game_objects: Vec::new(),
            linked_below: None,
        }
    }

    pub const fn source_plane(&self) -> Option<SourcePlane> {
        self.source_plane
    }

    pub const fn storage_plane(&self) -> StoragePlane {
        self.storage_plane
    }

    pub fn game_objects(&self) -> &[SceneGameObject] {
        &self.game_objects
    }

    pub fn push_game_object(&mut self, object: SceneGameObject) {
        self.game_objects.push(object);
    }

    pub fn linked_below(&self) -> Option<&SemanticTile> {
        self.linked_below.as_deref()
    }

    fn synthetic(storage_plane: StoragePlane) -> Self {
        Self::new(None, storage_plane)
    }

    fn relocate_down(
        &mut self,
        storage_plane: StoragePlane,
        anchor: SceneTile,
    ) -> Result<(), SceneGridError> {
        self.storage_plane = storage_plane;
        for object in &mut self.game_objects {
            if object.tag_type == GAME_OBJECT_TAG_TYPE && object.start == anchor {
                let value = object.plane.index().get();
                if value > 0 {
                    object.plane = storage_plane_from_index(value - 1)?;
                }
            }
        }
        Ok(())
    }
}

/// Dense scene-storage grid with explicit plane count and bounds.
///
/// Slots are nullable because `Scene.setLinkBelow` moves tile references and
/// clears the top slot structurally.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SceneGrid {
    width: u32,
    height: u32,
    plane_count: u8,
    plane_len: usize,
    tiles: Vec<Option<SemanticTile>>,
}

/// Grid construction or access failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SceneGridError {
    InvalidPlaneCount(u8),
    InvalidPlaneIndex(u8),
    CapacityOverflow,
    OutOfBounds { plane: u8, x: u32, y: u32 },
    LinkBelowRequiresFourPlanes(u8),
}

impl fmt::Display for SceneGridError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPlaneCount(count) => {
                write!(formatter, "scene plane count {count} is outside 1..=4")
            }
            Self::InvalidPlaneIndex(plane) => {
                write!(formatter, "scene plane index {plane} is outside 0..=3")
            }
            Self::CapacityOverflow => formatter.write_str("scene grid capacity overflow"),
            Self::OutOfBounds { plane, x, y } => write!(
                formatter,
                "scene tile ({x},{y}) on storage plane {plane} is out of bounds"
            ),
            Self::LinkBelowRequiresFourPlanes(count) => write!(
                formatter,
                "link-below processing requires four scene planes, found {count}"
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
            tiles: vec![None; len],
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
        self.index(plane, tile)
            .and_then(|index| self.tiles[index].as_ref())
    }

    pub fn tile_mut(&mut self, plane: StoragePlane, tile: SceneTile) -> Option<&mut SemanticTile> {
        self.index(plane, tile)
            .and_then(|index| self.tiles[index].as_mut())
    }

    pub fn set_tile(
        &mut self,
        plane: StoragePlane,
        tile: SceneTile,
        mut semantic_tile: SemanticTile,
    ) -> Result<(), SceneGridError> {
        let Some(index) = self.index(plane, tile) else {
            return Err(SceneGridError::OutOfBounds {
                plane: plane.index().get(),
                x: tile.x,
                y: tile.y,
            });
        };
        semantic_tile.storage_plane = plane;
        self.tiles[index] = Some(semantic_tile);
        Ok(())
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
        let source_plane = SourcePlane::new(plane.index().get())
            .ok_or(SceneGridError::InvalidPlaneIndex(plane.index().get()))?;
        let semantic_tile = self.tiles[index]
            .get_or_insert_with(|| SemanticTile::new(Some(source_plane), plane));
        semantic_tile.terrain = Some(terrain);
        Ok(())
    }

    /// Apply the exact four-plane structural relinking from `Scene.setLinkBelow`.
    pub fn set_link_below(&mut self, tile: SceneTile) -> Result<(), SceneGridError> {
        if self.plane_count != 4 {
            return Err(SceneGridError::LinkBelowRequiresFourPlanes(
                self.plane_count,
            ));
        }
        if tile.x >= self.width || tile.y >= self.height {
            return Err(SceneGridError::OutOfBounds {
                plane: 0,
                x: tile.x,
                y: tile.y,
            });
        }

        let plane0 = storage_plane_from_index(0)?;
        let plane1 = storage_plane_from_index(1)?;
        let plane2 = storage_plane_from_index(2)?;
        let plane3 = storage_plane_from_index(3)?;
        let index0 = self.required_index(plane0, tile)?;
        let index1 = self.required_index(plane1, tile)?;
        let index2 = self.required_index(plane2, tile)?;
        let index3 = self.required_index(plane3, tile)?;

        let old_plane0 = self.tiles[index0].take();
        self.move_tile_down(index1, index0, plane0, tile)?;
        self.move_tile_down(index2, index1, plane1, tile)?;
        self.move_tile_down(index3, index2, plane2, tile)?;

        let plane0_tile = self.tiles[index0].get_or_insert_with(|| SemanticTile::synthetic(plane0));
        plane0_tile.linked_below = old_plane0.map(Box::new);
        Ok(())
    }

    fn move_tile_down(
        &mut self,
        source_index: usize,
        target_index: usize,
        target_plane: StoragePlane,
        anchor: SceneTile,
    ) -> Result<(), SceneGridError> {
        let mut moved = self.tiles[source_index].take();
        if let Some(tile) = moved.as_mut() {
            tile.relocate_down(target_plane, anchor)?;
        }
        self.tiles[target_index] = moved;
        Ok(())
    }

    fn required_index(
        &self,
        plane: StoragePlane,
        tile: SceneTile,
    ) -> Result<usize, SceneGridError> {
        self.index(plane, tile).ok_or(SceneGridError::OutOfBounds {
            plane: plane.index().get(),
            x: tile.x,
            y: tile.y,
        })
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

fn storage_plane_from_index(value: u8) -> Result<StoragePlane, SceneGridError> {
    StoragePlane::new(value).ok_or(SceneGridError::InvalidPlaneIndex(value))
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
    fn terrain_storage_is_plane_and_tile_exact() -> Result<(), SceneGridError> {
        let mut grid = SceneGrid::new(2, 3, 4)?;
        let plane = storage_plane_from_index(2)?;
        let tile = SceneTile::new(1, 2);
        let surface = TerrainSurface::Flat(FlatTerrainSurface::new(
            TerrainCorners::new(1, 2, 3, 4),
            TerrainCorners::new(10, 11, 12, 13),
            Some(7),
        ));
        grid.set_terrain(plane, tile, surface.clone())?;
        assert_eq!(
            grid.tile(plane, tile)
                .and_then(|value| value.terrain.as_ref()),
            Some(&surface)
        );
        let other_plane = storage_plane_from_index(1)?;
        assert!(
            grid.tile(other_plane, tile)
                .and_then(|value| value.terrain.as_ref())
                .is_none()
        );
        Ok(())
    }

    #[test]
    fn out_of_bounds_access_does_not_alias_storage() -> Result<(), SceneGridError> {
        let mut grid = SceneGrid::new(2, 2, 1)?;
        let plane = storage_plane_from_index(0)?;
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
        Ok(())
    }

    #[test]
    fn link_below_shifts_tiles_and_only_qualifying_game_object_planes() -> Result<(), SceneGridError>
    {
        let mut grid = SceneGrid::new(2, 1, 4)?;
        let anchor = SceneTile::new(0, 0);

        for value in 0..=3 {
            let source = SourcePlane::new(value).ok_or(SceneGridError::InvalidPlaneIndex(value))?;
            let storage = storage_plane_from_index(value)?;
            let mut tile = SemanticTile::new(Some(source), storage);
            tile.push_game_object(SceneGameObject::new(
                ObjectId::new(u32::from(value)),
                GAME_OBJECT_TAG_TYPE,
                anchor,
                storage,
            ));
            if value == 1 {
                tile.push_game_object(SceneGameObject::new(
                    ObjectId::new(10),
                    GAME_OBJECT_TAG_TYPE,
                    SceneTile::new(1, 0),
                    storage,
                ));
                tile.push_game_object(SceneGameObject::new(ObjectId::new(11), 1, anchor, storage));
            }
            grid.set_tile(storage, anchor, tile)?;
        }

        grid.set_link_below(anchor)?;

        for value in 0..=2 {
            let tile = grid.tile(storage_plane_from_index(value)?, anchor);
            assert_eq!(
                tile.map(|value| value.storage_plane().index().get()),
                Some(value)
            );
            assert_eq!(
                tile.and_then(|value| value.source_plane())
                    .map(|plane| plane.index().get()),
                Some(value + 1)
            );
            assert_eq!(
                tile.and_then(|value| value.game_objects().first())
                    .map(|object| object.plane().index().get()),
                Some(value)
            );
        }
        assert!(grid.tile(storage_plane_from_index(3)?, anchor).is_none());

        let plane0 = grid.tile(storage_plane_from_index(0)?, anchor);
        assert_eq!(
            plane0
                .and_then(|tile| tile.game_objects().get(1))
                .map(|object| object.plane().index().get()),
            Some(1)
        );
        assert_eq!(
            plane0
                .and_then(|tile| tile.game_objects().get(2))
                .map(|object| object.plane().index().get()),
            Some(1)
        );
        let linked = plane0.and_then(SemanticTile::linked_below);
        assert_eq!(
            linked
                .and_then(SemanticTile::source_plane)
                .map(|plane| plane.index().get()),
            Some(0)
        );
        assert_eq!(
            linked
                .and_then(|tile| tile.game_objects().first())
                .map(|object| object.plane().index().get()),
            Some(0)
        );
        Ok(())
    }

    #[test]
    fn link_below_requires_exact_four_plane_scene() -> Result<(), SceneGridError> {
        let mut grid = SceneGrid::new(1, 1, 3)?;
        assert_eq!(
            grid.set_link_below(SceneTile::new(0, 0)),
            Err(SceneGridError::LinkBelowRequiresFourPlanes(3))
        );
        Ok(())
    }
}
