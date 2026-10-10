//! Bounded semantic tile storage for M6 scene construction.

use crate::{
    pending_replacement::{
        PendingInsertion, PendingRemoval, PendingSceneCategory, PendingSceneMutation,
    },
    placement::{PlacementKind, PlacementPlan},
    terrain::TerrainSurface,
};
use osrs_core::{
    coords::{SceneTile, SourcePlane, StoragePlane},
    ids::ObjectId,
};
use std::{error::Error, fmt};

const GAME_OBJECT_TAG_TYPE: u8 = 2;
const GAME_OBJECTS_PER_TILE: usize = 5;

/// One fixed-layer semantic location placement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScenePlacedLoc {
    object_id: ObjectId,
    placement: PlacementPlan,
}

impl ScenePlacedLoc {
    pub const fn new(object_id: ObjectId, placement: PlacementPlan) -> Self {
        Self {
            object_id,
            placement,
        }
    }

    pub const fn object_id(self) -> ObjectId {
        self.object_id
    }

    pub const fn placement(self) -> PlacementPlan {
        self.placement
    }
}

/// Semantic game-object occupancy owned by the scene tile stack.
///
/// A production placement receives an internal instance identity so every tile
/// covered by one footprint can retain the same semantic object identity while
/// still preserving the reference per-tile edge mask.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SceneGameObject {
    instance_id: Option<u64>,
    object_id: ObjectId,
    tag_type: u8,
    start: SceneTile,
    end: SceneTile,
    plane: StoragePlane,
    edge_mask: u8,
    placement: Option<PlacementPlan>,
}

impl SceneGameObject {
    /// Construct a minimal game-object record for evidence/tests that do not
    /// originate from the production placement planner.
    pub const fn new(
        object_id: ObjectId,
        tag_type: u8,
        start: SceneTile,
        plane: StoragePlane,
    ) -> Self {
        Self {
            instance_id: None,
            object_id,
            tag_type,
            start,
            end: start,
            plane,
            edge_mask: 0,
            placement: None,
        }
    }

    fn from_placement(
        instance_id: u64,
        object_id: ObjectId,
        plane: StoragePlane,
        start: SceneTile,
        end: SceneTile,
        edge_mask: u8,
        placement: PlacementPlan,
    ) -> Self {
        Self {
            instance_id: Some(instance_id),
            object_id,
            tag_type: GAME_OBJECT_TAG_TYPE,
            start,
            end,
            plane,
            edge_mask,
            placement: Some(placement),
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

    pub const fn end(&self) -> SceneTile {
        self.end
    }

    pub const fn plane(&self) -> StoragePlane {
        self.plane
    }

    pub const fn edge_mask(&self) -> u8 {
        self.edge_mask
    }

    pub const fn placement(&self) -> Option<PlacementPlan> {
        self.placement
    }

    pub const fn instance_id(&self) -> Option<u64> {
        self.instance_id
    }
}

/// Semantic data owned by one scene-storage tile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemanticTile {
    source_plane: Option<SourcePlane>,
    storage_plane: StoragePlane,
    pub terrain: Option<TerrainSurface>,
    floor_decoration: Option<ScenePlacedLoc>,
    boundary: Option<ScenePlacedLoc>,
    wall_decoration: Option<ScenePlacedLoc>,
    game_objects: Vec<SceneGameObject>,
    linked_below: Option<Box<SemanticTile>>,
}

impl SemanticTile {
    pub fn new(source_plane: Option<SourcePlane>, storage_plane: StoragePlane) -> Self {
        Self {
            source_plane,
            storage_plane,
            terrain: None,
            floor_decoration: None,
            boundary: None,
            wall_decoration: None,
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

    pub const fn floor_decoration(&self) -> Option<&ScenePlacedLoc> {
        self.floor_decoration.as_ref()
    }

    pub const fn boundary(&self) -> Option<&ScenePlacedLoc> {
        self.boundary.as_ref()
    }

    pub const fn wall_decoration(&self) -> Option<&ScenePlacedLoc> {
        self.wall_decoration.as_ref()
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
    ) -> Result<Vec<GameObjectPlaneUpdate>, SceneGridError> {
        self.storage_plane = storage_plane;
        let mut updates = Vec::new();
        for object in &mut self.game_objects {
            if object.tag_type == GAME_OBJECT_TAG_TYPE && object.start == anchor {
                let value = object.plane.index().get();
                if value > 0 {
                    object.plane = storage_plane_from_index(value - 1)?;
                    if let Some(instance_id) = object.instance_id {
                        updates.push(GameObjectPlaneUpdate {
                            instance_id,
                            plane: object.plane,
                        });
                    }
                }
            }
        }
        Ok(updates)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct GameObjectPlaneUpdate {
    instance_id: u64,
    plane: StoragePlane,
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
    next_game_object_instance: u64,
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
            next_game_object_instance: 1,
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

    pub fn floor_decoration(
        &self,
        plane: StoragePlane,
        tile: SceneTile,
    ) -> Option<&ScenePlacedLoc> {
        self.tile(plane, tile)
            .and_then(SemanticTile::floor_decoration)
    }

    pub fn boundary(&self, plane: StoragePlane, tile: SceneTile) -> Option<&ScenePlacedLoc> {
        self.tile(plane, tile).and_then(SemanticTile::boundary)
    }

    pub fn wall_decoration(&self, plane: StoragePlane, tile: SceneTile) -> Option<&ScenePlacedLoc> {
        self.tile(plane, tile)
            .and_then(SemanticTile::wall_decoration)
    }

    /// Query the game object anchored at one tile, matching the reference
    /// `getGameObject` identity rule rather than returning an arbitrary overlap.
    pub fn game_object(&self, plane: StoragePlane, tile: SceneTile) -> Option<&SceneGameObject> {
        self.tile(plane, tile).and_then(|semantic_tile| {
            semantic_tile
                .game_objects
                .iter()
                .find(|object| object.tag_type == GAME_OBJECT_TAG_TYPE && object.start == tile)
        })
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
        self.ensure_tile(plane, tile)?.terrain = Some(terrain);
        Ok(())
    }

    /// Insert one already-planned initial semantic location.
    ///
    /// Fixed layers replace their one tile slot. Game objects first preflight
    /// the complete footprint, then insert atomically across every covered tile
    /// with the exact reference five-object capacity and per-tile edge masks.
    pub fn insert_placement(
        &mut self,
        plane: StoragePlane,
        object_id: ObjectId,
        placement: PlacementPlan,
    ) -> Result<bool, SceneGridError> {
        match placement.kind {
            PlacementKind::FloorDecoration(_) => {
                self.ensure_tile(plane, placement_anchor(placement))?
                    .floor_decoration = Some(ScenePlacedLoc::new(object_id, placement));
                Ok(true)
            }
            PlacementKind::Boundary(_) => {
                self.ensure_tile(plane, placement_anchor(placement))?
                    .boundary = Some(ScenePlacedLoc::new(object_id, placement));
                Ok(true)
            }
            PlacementKind::WallDecoration(_) => {
                self.ensure_tile(plane, placement_anchor(placement))?
                    .wall_decoration = Some(ScenePlacedLoc::new(object_id, placement));
                Ok(true)
            }
            PlacementKind::GameObject(game) => {
                self.insert_game_object(plane, object_id, placement, game.storage_footprint)
            }
        }
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

    fn remove_pending_placement(
        &mut self,
        removal: PendingRemoval,
    ) -> Result<bool, SceneGridError> {
        if removal.category == PendingSceneCategory::GameObject {
            return self.remove_game_object(removal.plane, removal.tile);
        }

        let index = self.required_index(removal.plane, removal.tile)?;
        let Some(tile) = self.tiles[index].as_mut() else {
            return Ok(false);
        };
        let removed = match removal.category {
            PendingSceneCategory::Boundary => tile.boundary.take().is_some(),
            PendingSceneCategory::WallDecoration => tile.wall_decoration.take().is_some(),
            PendingSceneCategory::FloorDecoration => tile.floor_decoration.take().is_some(),
            PendingSceneCategory::GameObject => unreachable!(),
        };
        Ok(removed)
    }

    fn remove_game_object(
        &mut self,
        plane: StoragePlane,
        anchor: SceneTile,
    ) -> Result<bool, SceneGridError> {
        self.required_index(plane, anchor)?;
        let Some(instance_id) = self
            .game_object(plane, anchor)
            .map(SceneGameObject::instance_id)
        else {
            return Ok(false);
        };

        if let Some(instance_id) = instance_id {
            for tile in self.tiles.iter_mut().flatten() {
                tile.game_objects
                    .retain(|object| object.instance_id != Some(instance_id));
            }
            return Ok(true);
        }

        let index = self.required_index(plane, anchor)?;
        let Some(tile) = self.tiles[index].as_mut() else {
            return Ok(false);
        };
        let Some(position) = tile.game_objects.iter().position(|object| {
            object.instance_id.is_none()
                && object.tag_type == GAME_OBJECT_TAG_TYPE
                && object.start == anchor
        }) else {
            return Ok(false);
        };
        tile.game_objects.remove(position);
        Ok(true)
    }

    fn insert_game_object(
        &mut self,
        plane: StoragePlane,
        object_id: ObjectId,
        placement: PlacementPlan,
        footprint: crate::placement::Footprint,
    ) -> Result<bool, SceneGridError> {
        let start = placement_anchor(placement);
        let width = u32::from(footprint.width);
        let depth = u32::from(footprint.depth);
        if width == 0 || depth == 0 {
            return Ok(false);
        }
        let Some(end_x) = start.x.checked_add(width - 1) else {
            return Ok(false);
        };
        let Some(end_y) = start.y.checked_add(depth - 1) else {
            return Ok(false);
        };
        if end_x >= self.width || end_y >= self.height {
            return Ok(false);
        }
        let end = SceneTile::new(end_x, end_y);

        for x in start.x..=end.x {
            for y in start.y..=end.y {
                let tile = SceneTile::new(x, y);
                if self
                    .tile(plane, tile)
                    .is_some_and(|value| value.game_objects.len() >= GAME_OBJECTS_PER_TILE)
                {
                    return Ok(false);
                }
            }
        }

        let instance_id = self.next_game_object_instance;
        self.next_game_object_instance = self
            .next_game_object_instance
            .checked_add(1)
            .ok_or(SceneGridError::CapacityOverflow)?;

        for x in start.x..=end.x {
            for y in start.y..=end.y {
                let tile = SceneTile::new(x, y);
                let edge_mask = game_object_edge_mask(tile, start, end);
                let object = SceneGameObject::from_placement(
                    instance_id,
                    object_id,
                    plane,
                    start,
                    end,
                    edge_mask,
                    placement,
                );
                self.ensure_tile(plane, tile)?.push_game_object(object);
            }
        }
        Ok(true)
    }

    fn ensure_tile(
        &mut self,
        plane: StoragePlane,
        tile: SceneTile,
    ) -> Result<&mut SemanticTile, SceneGridError> {
        let index = self.required_index(plane, tile)?;
        let source_plane = SourcePlane::new(plane.index().get())
            .ok_or(SceneGridError::InvalidPlaneIndex(plane.index().get()))?;
        Ok(self.tiles[index].get_or_insert_with(|| SemanticTile::new(Some(source_plane), plane)))
    }

    fn move_tile_down(
        &mut self,
        source_index: usize,
        target_index: usize,
        target_plane: StoragePlane,
        anchor: SceneTile,
    ) -> Result<(), SceneGridError> {
        let mut moved = self.tiles[source_index].take();
        let updates = match moved.as_mut() {
            Some(tile) => tile.relocate_down(target_plane, anchor)?,
            None => Vec::new(),
        };
        self.tiles[target_index] = moved;
        for update in updates {
            self.propagate_game_object_plane(update);
        }
        Ok(())
    }

    fn propagate_game_object_plane(&mut self, update: GameObjectPlaneUpdate) {
        for tile in self.tiles.iter_mut().flatten() {
            for object in &mut tile.game_objects {
                if object.instance_id == Some(update.instance_id) {
                    object.plane = update.plane;
                }
            }
        }
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

impl PendingSceneMutation for SceneGrid {
    type Error = SceneGridError;

    fn remove_pending(&mut self, removal: PendingRemoval) -> Result<bool, Self::Error> {
        self.remove_pending_placement(removal)
    }

    fn insert_pending(&mut self, insertion: PendingInsertion) -> Result<bool, Self::Error> {
        self.insert_placement(insertion.plane, insertion.object_id, insertion.placement)
    }
}

fn placement_anchor(placement: PlacementPlan) -> SceneTile {
    let footprint = placement.kind.storage_footprint();
    let center_x = placement.storage_center.x.units();
    let center_z = placement.storage_center.z.units();
    let offset_x = i32::from(footprint.width) * 64;
    let offset_z = i32::from(footprint.depth) * 64;
    let x = (center_x - offset_x).div_euclid(128);
    let y = (center_z - offset_z).div_euclid(128);
    SceneTile::new(x as u32, y as u32)
}

const fn game_object_edge_mask(tile: SceneTile, start: SceneTile, end: SceneTile) -> u8 {
    let mut mask = 0;
    if tile.x > start.x {
        mask |= 1;
    }
    if tile.x < end.x {
        mask |= 4;
    }
    if tile.y > start.y {
        mask |= 8;
    }
    if tile.y < end.y {
        mask |= 2;
    }
    mask
}

fn storage_plane_from_index(value: u8) -> Result<StoragePlane, SceneGridError> {
    StoragePlane::new(value).ok_or(SceneGridError::InvalidPlaneIndex(value))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        placement::{PlacementInput, plan_placement},
        terrain::{FlatTerrainSurface, TerrainCorners},
    };
    use osrs_core::definitions::LocType;

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
    fn game_object_footprint_is_atomic_and_uses_reference_edge_masks() -> Result<(), Box<dyn Error>>
    {
        let mut grid = SceneGrid::new(64, 64, 1)?;
        let plane = storage_plane_from_index(0)?;
        let placement = plan_placement(PlacementInput {
            loc_type: LocType::new(10),
            orientation: 0,
            tile: SceneTile::new(50, 60),
            size_x: 2,
            size_y: 1,
            sampled_height: 100,
            existing_wall_displacement: None,
        })?;
        assert!(grid.insert_placement(plane, ObjectId::new(1), placement)?);

        let left = grid
            .tile(plane, SceneTile::new(50, 60))
            .and_then(|tile| tile.game_objects().first());
        let right = grid
            .tile(plane, SceneTile::new(51, 60))
            .and_then(|tile| tile.game_objects().first());
        assert_eq!(left.map(SceneGameObject::edge_mask), Some(4));
        assert_eq!(right.map(SceneGameObject::edge_mask), Some(1));
        assert_eq!(
            left.and_then(SceneGameObject::instance_id),
            right.and_then(SceneGameObject::instance_id)
        );
        assert_eq!(
            left.map(SceneGameObject::start),
            Some(SceneTile::new(50, 60))
        );
        assert_eq!(
            right.map(SceneGameObject::end),
            Some(SceneTile::new(51, 60))
        );
        Ok(())
    }

    #[test]
    fn game_object_capacity_rejects_sixth_without_partial_insertion() -> Result<(), Box<dyn Error>>
    {
        let mut grid = SceneGrid::new(2, 1, 1)?;
        let plane = storage_plane_from_index(0)?;
        for object_id in 0..5 {
            let placement = plan_placement(PlacementInput {
                loc_type: LocType::new(10),
                orientation: 0,
                tile: SceneTile::new(0, 0),
                size_x: 1,
                size_y: 1,
                sampled_height: 0,
                existing_wall_displacement: None,
            })?;
            assert!(grid.insert_placement(plane, ObjectId::new(object_id), placement)?);
        }
        let sixth = plan_placement(PlacementInput {
            loc_type: LocType::new(10),
            orientation: 0,
            tile: SceneTile::new(0, 0),
            size_x: 2,
            size_y: 1,
            sampled_height: 0,
            existing_wall_displacement: None,
        })?;
        assert!(!grid.insert_placement(plane, ObjectId::new(99), sixth)?);
        assert_eq!(
            grid.tile(plane, SceneTile::new(0, 0))
                .map(|tile| tile.game_objects().len()),
            Some(5)
        );
        assert!(grid.tile(plane, SceneTile::new(1, 0)).is_none());
        Ok(())
    }

    #[test]
    fn game_object_out_of_bounds_is_atomic() -> Result<(), Box<dyn Error>> {
        let mut grid = SceneGrid::new(2, 2, 1)?;
        let plane = storage_plane_from_index(0)?;
        let placement = plan_placement(PlacementInput {
            loc_type: LocType::new(10),
            orientation: 0,
            tile: SceneTile::new(1, 1),
            size_x: 2,
            size_y: 1,
            sampled_height: 0,
            existing_wall_displacement: None,
        })?;
        assert!(!grid.insert_placement(plane, ObjectId::new(1), placement)?);
        assert!(grid.tile(plane, SceneTile::new(1, 1)).is_none());
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
    fn inserted_multi_tile_game_object_keeps_one_plane_identity_after_link_below()
    -> Result<(), Box<dyn Error>> {
        let mut grid = SceneGrid::new(2, 1, 4)?;
        let plane1 = storage_plane_from_index(1)?;
        let placement = plan_placement(PlacementInput {
            loc_type: LocType::new(10),
            orientation: 0,
            tile: SceneTile::new(0, 0),
            size_x: 2,
            size_y: 1,
            sampled_height: 0,
            existing_wall_displacement: None,
        })?;
        assert!(grid.insert_placement(plane1, ObjectId::new(1), placement)?);
        grid.set_link_below(SceneTile::new(0, 0))?;

        let plane0 = storage_plane_from_index(0)?;
        let moved_anchor = grid
            .tile(plane0, SceneTile::new(0, 0))
            .and_then(|tile| tile.game_objects().first());
        let overlap = grid
            .tile(plane1, SceneTile::new(1, 0))
            .and_then(|tile| tile.game_objects().first());
        assert_eq!(
            moved_anchor.map(|object| object.plane().index().get()),
            Some(0)
        );
        assert_eq!(overlap.map(|object| object.plane().index().get()), Some(0));
        assert_eq!(
            moved_anchor.and_then(SceneGameObject::instance_id),
            overlap.and_then(SceneGameObject::instance_id)
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
