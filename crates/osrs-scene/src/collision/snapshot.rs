use super::{CollisionPlanError, CollisionStencil, FLOOR, UNLOADED};
use crate::{CollisionSideEffect, collision_plane, terrain_load::REFERENCE_SCENE_TILES};
use osrs_core::{
    coords::{CollisionPlane, MapTile, PlaneIndex, RegionCoord, SceneTile, SourcePlane},
    definitions::LocType,
    ids::ObjectId,
};

const SIZE: usize = REFERENCE_SCENE_TILES;
const CELLS: usize = SIZE * SIZE;

/// Original loc identity and clipping operation, even when the loc does not draw.
#[derive(Debug, Clone, Copy)]
pub struct CollisionObject {
    pub object_id: ObjectId,
    pub tile: MapTile,
    pub source_plane: SourcePlane,
    pub loc_type: LocType,
    pub orientation: u8,
    pub effect: CollisionSideEffect,
}

#[derive(Debug)]
struct Contribution {
    object: CollisionObject,
    plane: PlaneIndex,
    anchor: SceneTile,
    stencil: CollisionStencil,
}

/// One cell on a collision plane, not a scene storage plane.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CollisionCell {
    pub flags: u32,
    /// False for missing terrain and the reference map's unusable edge padding.
    pub loaded: bool,
    /// Bit N identifies terrain source plane N contributing a floor blocker here.
    pub terrain_source_planes: u8,
}

/// Immutable clipping and provenance for a contextual 104×104 scene window.
#[derive(Debug)]
pub struct CollisionSnapshot {
    origin: MapTile,
    flags: Vec<u32>,
    terrain_sources: Vec<u8>,
    objects: Vec<Contribution>,
    unresolved_definitions: usize,
}

impl CollisionSnapshot {
    fn index(&self, plane: PlaneIndex, tile: MapTile) -> Option<(usize, SceneTile)> {
        let scene = tile.to_scene(self.origin)?;
        (scene.x < SIZE as u32 && scene.y < SIZE as u32).then_some((
            usize::from(plane.get()) * CELLS + scene.x as usize * SIZE + scene.y as usize,
            scene,
        ))
    }

    /// Missing definitions prevent this snapshot from certifying traversability.
    pub const fn unresolved_definitions(&self) -> usize {
        self.unresolved_definitions
    }

    /// Read flags and terrain provenance; outside the contextual window returns `None`.
    pub fn cell(&self, plane: PlaneIndex, tile: MapTile) -> Option<CollisionCell> {
        let (index, scene) = self.index(plane, tile)?;
        let interior =
            scene.x >= 1 && scene.y >= 1 && scene.x < SIZE as u32 - 5 && scene.y < SIZE as u32 - 5;
        Some(CollisionCell {
            flags: self.flags[index],
            loaded: interior && self.flags[index] & UNLOADED == 0,
            terrain_source_planes: self.terrain_sources[index],
        })
    }

    /// All locs whose clipping writes touch this cell, including neighbouring wall anchors.
    pub fn objects_at(
        &self,
        plane: PlaneIndex,
        tile: MapTile,
    ) -> impl Iterator<Item = (&CollisionObject, u32)> {
        let scene = self.index(plane, tile).map(|(_, scene)| scene);
        self.objects.iter().filter_map(move |contribution| {
            let scene = scene?;
            if contribution.plane != plane {
                return None;
            }
            let flags = contribution.stencil.flags_at(contribution.anchor, scene);
            (flags != 0).then_some((&contribution.object, flags))
        })
    }
}

/// Worker-local builder. Rebuilding replaces the whole snapshot; removal never clears shared bits.
pub struct CollisionSnapshotBuilder {
    snapshot: CollisionSnapshot,
}

impl CollisionSnapshotBuilder {
    /// Match `CollisionMap.clear()` with an ordinary (non-expanded) reference window.
    pub fn reference(origin: MapTile) -> Self {
        let mut flags = vec![UNLOADED; 4 * CELLS];
        for plane in 0..4 {
            for x in 0..SIZE {
                for y in 0..SIZE {
                    if x < 1 || y < 1 || x >= SIZE - 5 || y >= SIZE - 5 {
                        flags[plane * CELLS + x * SIZE + y] = 0x00ff_ffff;
                    }
                }
            }
        }
        Self {
            snapshot: CollisionSnapshot {
                origin,
                flags,
                terrain_sources: vec![0; 4 * CELLS],
                objects: Vec::new(),
                unresolved_definitions: 0,
            },
        }
    }

    /// Mark terrain supplied by an actual map square. Missing-region height fill is not loaded land.
    pub fn load_region(&mut self, region: RegionCoord) {
        let Some((x, y)) = region.x.checked_mul(64).zip(region.y.checked_mul(64)) else {
            return;
        };
        let origin = MapTile::new(x, y);
        for plane in 0..4 {
            let Some(plane) = PlaneIndex::new(plane) else {
                continue;
            };
            for x in 0..64 {
                for y in 0..64 {
                    let Some(tile) = origin
                        .x
                        .checked_add(x)
                        .zip(origin.y.checked_add(y))
                        .map(|(x, y)| MapTile::new(x, y))
                    else {
                        continue;
                    };
                    if let Some((index, scene)) = self.snapshot.index(plane, tile)
                        && scene.x >= 1
                        && scene.y >= 1
                        && scene.x < SIZE as u32 - 5
                        && scene.y < SIZE as u32 - 5
                    {
                        self.snapshot.flags[index] &= !UNLOADED;
                    }
                }
            }
        }
    }

    /// Apply a source tile's bit-1 floor clipping using only its bridge collision projection.
    pub fn terrain(&mut self, source: SourcePlane, tile: MapTile, settings: u8, bridge: bool) {
        if settings & 1 == 0 {
            return;
        }
        if let CollisionPlane::Plane(plane) = collision_plane(source, bridge)
            && let Some((index, _)) = self.snapshot.index(plane, tile)
        {
            self.snapshot.flags[index] |= FLOOR;
            self.snapshot.terrain_sources[index] |= 1 << source.index().get();
        }
    }

    /// Apply an initial-placement recipe regardless of model availability or scene capacity.
    ///
    /// # Errors
    /// Rejects invalid boundary recipes before changing any clipping state.
    pub fn object(
        &mut self,
        object: CollisionObject,
        bridge: bool,
    ) -> Result<(), CollisionPlanError> {
        let stencil = CollisionStencil::from_effect(object.effect)?;
        let CollisionPlane::Plane(plane) = collision_plane(object.source_plane, bridge) else {
            return Ok(());
        };
        let Some(anchor) = object.tile.to_scene(self.snapshot.origin) else {
            return Ok(());
        };
        if anchor.x >= SIZE as u32 || anchor.y >= SIZE as u32 {
            return Ok(());
        }
        if matches!(object.effect, CollisionSideEffect::None) {
            return Ok(());
        }
        stencil.visit(anchor, SIZE as u32, SIZE as u32, |tile, flags| {
            self.snapshot.flags
                [usize::from(plane.get()) * CELLS + tile.x as usize * SIZE + tile.y as usize] |=
                flags;
        });
        self.snapshot.objects.push(Contribution {
            object,
            plane,
            anchor,
            stencil,
        });
        Ok(())
    }

    /// Record a placement whose missing definition prevents complete clipping construction.
    pub fn missing_definition(&mut self) {
        self.snapshot.unresolved_definitions += 1;
    }

    /// Freeze the worker result for immutable sharing and freshness-gated installation.
    pub fn build(self) -> CollisionSnapshot {
        self.snapshot
    }
}
