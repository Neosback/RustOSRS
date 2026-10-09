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

    fn relocate_down(&mut self, storage_plane: StoragePlane, anchor: SceneTile) {
        self.storage_plane = storage_plane;
        for object in &mut self.game_objects {
            if object.tag_type == GAME_OBJECT_TAG_TYPE && object.start == anchor {
                let value = object.plane.index().get();
                if value > 0 {
                    object.plane = StoragePlane::new(value - 1)
                        .expect("decremented game-object plane must remain valid");
                }
            }
        }
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
        if self.tiles[index].is_none() {
            let source_plane = SourcePlane::new(plane.index().get())
                .expect("validated storage plane must map to a valid source plane");
            self.tiles[index] = Some(SemanticTile::new(Some(source_plane), plane));
        }
        self.tiles[index]
            .as_mut()
            .expect("scene tile was just created")
            .terrain = Some(terrain);
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

        let plane0 = storage_plane(0);
        let plane1 = storage_plane(1);
        let plane2 = storage_plane(2);
        let plane3 = storage_plane(3);
        let index0 = self
            .index(plane0, tile)
            .expect("validated four-plane tile must have plane-0 storage");
        let index1 = self
            .index(plane1, tile)
            .expect("validated four-plane tile must have plane-1 storage");
        let index2 = self
            .index(plane2, tile)
            .expect("validated four-plane tile must have plane-2 storage");
        let index3 = self
            .index(plane3, tile)
            .expect("validated four-plane tile must have plane-3 storage");

        let old_plane0 = self.tiles[index0].take();
        self.move_tile_down(index1, index0, plane0, tile);
        self.move_tile_down(index2, index1, plane1, tile);
        self.move_tile_down(index3, index2, plane2, tile);

        if self.tiles[index0].is_none() {
            self.tiles[index0] = Some(SemanticTile::synthetic(plane0));
        }
        self.tiles[index0]
            .as_mut()
            .expect("plane-0 tile exists after link-below shift")
            .linked_below = old_plane0.map(Box::new);
        Ok(())
    }

    fn move_tile_down(
        &mut self,
        source_index: usize,
        target_index: usize,
        target_plane: StoragePlane,
        anchor: SceneTile,
    ) {
        let mut moved = self.tiles[source_index].take();
        if let Some(tile) = moved.as_mut() {
            tile.relocate_down(target_plane, anchor);
        }
        self.tiles[target_index] = moved;
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

fn storage_plane(value: u8) -> StoragePlane {
    StoragePlane::new(value).expect("0..=3 storage plane must be valid")
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
        let plane = storage_plane(2);
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
        let other_plane = storage_plane(1);
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
        let plane = storage_plane(0);
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
            let source = SourcePlane::new(value).expect("test source plane is valid");
            let storage = storage_plane(value);
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
            let tile = grid
                .tile(storage_plane(value), anchor)
                .expect("shifted tile must exist");
            assert_eq!(tile.storage_plane().index().get(), value);
            assert_eq!(
                tile.source_plane().map(|plane| plane.index().get()),
                Some(value + 1)
            );
            assert_eq!(tile.game_objects()[0].plane().index().get(), value);
        }
        assert!(grid.tile(storage_plane(3), anchor).is_none());

        let plane0 = grid
            .tile(storage_plane(0), anchor)
            .expect("new plane-0 tile must exist");
        assert_eq!(plane0.game_objects()[1].plane().index().get(), 1);
        assert_eq!(plane0.game_objects()[2].plane().index().get(), 1);
        let linked = plane0
            .linked_below()
            .expect("old plane-0 tile must be linked below");
        assert_eq!(
            linked.source_plane().map(|plane| plane.index().get()),
            Some(0)
        );
        assert_eq!(linked.game_objects()[0].plane().index().get(), 0);
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
