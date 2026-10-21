//! Semantic transaction footprints and derived compatibility summaries. Coordinates and exact
//! object mutation polarity survive through commit/replay into invalidation and diagnostics.
use osrs_core::coords::{MapTile, RegionCoord};
use osrs_edit::{
    HeightVertexCoord, MapOp, MapPatch, MapPosition, MapReplayFootprint, ObjectPlacement,
};
use std::{collections::BTreeSet, ops::Deref};

/// Exact semantic coordinates touched by one application transaction.
///
/// This remains renderer-independent. The world projection decides how far each class of source
/// mutation can propagate into disposable derived geometry. `object_tiles` is a derived compatibility
/// summary for conservative projection routing; `object_removed` and `object_added` retain the full
/// canonical semantic Object mutation identities required by later dependency proofs.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(in crate::app) struct EditFootprint {
    pub terrain_tiles: Vec<MapTile>,
    /// Exact source-plane identity; terrain_tiles remains a derived compatibility summary.
    pub terrain_positions: Vec<MapPosition>,
    pub height_vertices: Vec<HeightVertexCoord>,
    pub object_tiles: Vec<MapTile>,
    pub object_removed: Vec<ObjectPlacement>,
    pub object_added: Vec<ObjectPlacement>,
}

impl EditFootprint {
    pub(in crate::app) fn with_terrain_positions(
        mut self,
        positions: impl IntoIterator<Item = MapPosition>,
    ) -> Self {
        self.terrain_positions = positions
            .into_iter()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        self
    }

    #[cfg(test)]
    pub(in crate::app) fn new(
        terrain_tiles: impl IntoIterator<Item = MapTile>,
        height_vertices: impl IntoIterator<Item = HeightVertexCoord>,
        object_tiles: impl IntoIterator<Item = MapTile>,
    ) -> Self {
        Self {
            terrain_tiles: normalize_tiles(terrain_tiles),
            terrain_positions: Vec::new(),
            height_vertices: normalize_height_vertices(height_vertices),
            object_tiles: normalize_tiles(object_tiles),
            object_removed: Vec::new(),
            object_added: Vec::new(),
        }
    }

    /// Build a footprint that retains exact Object mutation identity and derives anchor-tile routing
    /// from those placements. Removed and added sets remain separate so Undo/Redo can reverse
    /// mutation polarity without losing object id, source plane, loc kind, or orientation.
    pub(in crate::app) fn with_object_mutations(
        terrain_tiles: impl IntoIterator<Item = MapTile>,
        height_vertices: impl IntoIterator<Item = HeightVertexCoord>,
        object_removed: impl IntoIterator<Item = ObjectPlacement>,
        object_added: impl IntoIterator<Item = ObjectPlacement>,
    ) -> Self {
        let object_removed = normalize_object_placements(object_removed);
        let object_added = normalize_object_placements(object_added);
        let object_tiles = normalize_tiles(
            object_removed
                .iter()
                .chain(&object_added)
                .map(|placement| placement.position.tile),
        );
        Self {
            terrain_tiles: normalize_tiles(terrain_tiles),
            terrain_positions: Vec::new(),
            height_vertices: normalize_height_vertices(height_vertices),
            object_tiles,
            object_removed,
            object_added,
        }
    }

    pub(super) fn from_patch(patch: &MapPatch) -> Self {
        let mut terrain_tiles = Vec::new();
        let mut positions = Vec::new();
        let mut height_vertices = Vec::new();
        let mut object_removed = Vec::new();
        let mut object_added = Vec::new();
        for op in patch.ops() {
            match op {
                MapOp::SetTerrain { coord, .. } => {
                    terrain_tiles.push(coord.tile);
                    positions.push(*coord);
                }
                MapOp::SetHeight { coord, .. } => height_vertices.push(*coord),
                MapOp::RemoveObject { placement } => object_removed.push(*placement),
                MapOp::AddObject { placement } => object_added.push(*placement),
            }
        }
        Self::with_object_mutations(terrain_tiles, height_vertices, object_removed, object_added)
            .with_terrain_positions(positions)
    }

    /// Adapt the authoritative semantic replay footprint into the application's existing
    /// invalidation/diagnostic summary. This is a one-way projection, not a second reconstruction
    /// of history semantics.
    pub(super) fn from_replay_footprint(footprint: &MapReplayFootprint) -> Self {
        let positions = footprint
            .terrain()
            .iter()
            .map(|change| change.coord)
            .collect::<Vec<_>>();
        Self::with_object_mutations(
            positions.iter().map(|coord| coord.tile),
            footprint.heights().iter().copied(),
            footprint.removed_objects().iter().copied(),
            footprint.added_objects().iter().copied(),
        )
        .with_terrain_positions(positions)
    }

    pub(super) fn into_touches(self) -> EditTouches {
        let mut regions = BTreeSet::new();
        regions.extend(self.terrain_tiles.iter().map(|tile| tile.split_region().0));
        regions.extend(
            self.height_vertices
                .iter()
                .map(|coord| coord.corner.split_region().0),
        );
        regions.extend(self.object_tiles.iter().map(|tile| tile.split_region().0));
        EditTouches {
            regions: regions.into_iter().collect(),
            footprint: self,
        }
    }
}

/// Region summary plus the exact coordinate footprint that produced it.
///
/// Dereferencing preserves the historical `&[RegionCoord]` view for callers that only need region
/// membership, while projection invalidation can inspect the richer footprint without a parallel
/// side channel.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(in crate::app) struct EditTouches {
    pub(super) regions: Vec<RegionCoord>,
    pub footprint: EditFootprint,
}

impl Deref for EditTouches {
    type Target = [RegionCoord];

    fn deref(&self) -> &Self::Target {
        &self.regions
    }
}

impl PartialEq<Vec<RegionCoord>> for EditTouches {
    fn eq(&self, other: &Vec<RegionCoord>) -> bool {
        self.regions == *other
    }
}

fn normalize_tiles(tiles: impl IntoIterator<Item = MapTile>) -> Vec<MapTile> {
    tiles
        .into_iter()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn normalize_height_vertices(
    vertices: impl IntoIterator<Item = HeightVertexCoord>,
) -> Vec<HeightVertexCoord> {
    vertices
        .into_iter()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn normalize_object_placements(
    placements: impl IntoIterator<Item = ObjectPlacement>,
) -> Vec<ObjectPlacement> {
    let mut placements = placements.into_iter().collect::<Vec<_>>();
    placements.sort_unstable_by_key(object_placement_key);
    placements.dedup();
    placements
}

fn object_placement_key(placement: &ObjectPlacement) -> (u8, i32, i32, u32, u8, u8) {
    (
        placement.position.plane.index().get(),
        placement.position.tile.x,
        placement.position.tile.y,
        placement.object_id.get(),
        placement.kind.get(),
        placement.orientation.get(),
    )
}
