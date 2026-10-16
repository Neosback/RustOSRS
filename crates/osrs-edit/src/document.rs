use crate::{
    ChangeImpact, HeightVertexCoord, MapOp, MapPatch, MapPosition, MapRegionSnapshot,
    ObjectPlacement, TerrainTile,
};
use osrs_core::coords::RegionCoord;
use std::collections::{BTreeMap, HashMap, HashSet};

/// Authoritative, headless editable map state.
///
/// Terrain storage is sparse: an absent entry is semantically
/// [`TerrainTile::default()`]. Height vertices are not implicit because a missing
/// source height is different from a real height of zero. Object placements are
/// stored by their exact semantic identity.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MapDocument {
    terrain: BTreeMap<MapPosition, TerrainTile>,
    heights: BTreeMap<HeightVertexCoord, i32>,
    objects: HashSet<ObjectPlacement>,
}

/// Why a patch could not be applied to the current authoritative state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PatchApplyError {
    ScenarioMismatch {
        op_index: usize,
    },
    InvalidScenario {
        op_index: usize,
    },
    TerrainMismatch {
        op_index: usize,
        coord: MapPosition,
        expected: TerrainTile,
        actual: TerrainTile,
    },
    HeightMissing {
        op_index: usize,
        coord: HeightVertexCoord,
        expected: i32,
    },
    HeightMismatch {
        op_index: usize,
        coord: HeightVertexCoord,
        expected: i32,
        actual: i32,
    },
    ObjectAlreadyPresent {
        op_index: usize,
        placement: ObjectPlacement,
    },
    ObjectMissing {
        op_index: usize,
        placement: ObjectPlacement,
    },
}

impl PatchApplyError {
    pub const fn op_index(self) -> usize {
        match self {
            Self::ScenarioMismatch { op_index }
            | Self::InvalidScenario { op_index }
            | Self::TerrainMismatch { op_index, .. }
            | Self::HeightMissing { op_index, .. }
            | Self::HeightMismatch { op_index, .. }
            | Self::ObjectAlreadyPresent { op_index, .. }
            | Self::ObjectMissing { op_index, .. } => op_index,
        }
    }
}

impl MapDocument {
    pub fn new() -> Self {
        Self::default()
    }

    /// Build initial authoritative state from decoded/adapted semantic values.
    ///
    /// This constructor is the intended infrastructure boundary for loading a map;
    /// interactive edits should use [`Self::apply_patch`] instead.
    pub fn from_parts(
        terrain: impl IntoIterator<Item = (MapPosition, TerrainTile)>,
        heights: impl IntoIterator<Item = (HeightVertexCoord, i32)>,
        objects: impl IntoIterator<Item = ObjectPlacement>,
    ) -> Self {
        let terrain = terrain
            .into_iter()
            .filter(|(_, tile)| *tile != TerrainTile::default())
            .collect();

        Self {
            terrain,
            heights: heights.into_iter().collect(),
            objects: objects.into_iter().collect(),
        }
    }

    /// Replace one region's currently loaded semantic state without creating edit history.
    ///
    /// This is an infrastructure hydration boundary, not an interactive edit path. Values outside
    /// `snapshot.region()` are preserved exactly. Replacing a region with identical semantic data
    /// is a true no-op and returns `false`.
    pub fn replace_region(&mut self, snapshot: &MapRegionSnapshot) -> bool {
        let region = snapshot.region();
        let current_terrain: BTreeMap<_, _> = self
            .terrain
            .iter()
            .filter(|(coord, _)| coord.tile.split_region().0 == region)
            .map(|(coord, tile)| (*coord, *tile))
            .collect();
        let current_heights: BTreeMap<_, _> = self
            .heights
            .iter()
            .filter(|(coord, _)| coord.corner.split_region().0 == region)
            .map(|(coord, height)| (*coord, *height))
            .collect();
        let current_objects: HashSet<_> = self
            .objects
            .iter()
            .filter(|placement| placement.position.tile.split_region().0 == region)
            .copied()
            .collect();

        if current_terrain == snapshot.terrain
            && current_heights == snapshot.heights
            && current_objects == snapshot.objects
        {
            return false;
        }

        self.remove_region_entries(region);
        self.terrain
            .extend(snapshot.terrain.iter().map(|(coord, tile)| (*coord, *tile)));
        self.heights.extend(
            snapshot
                .heights
                .iter()
                .map(|(coord, height)| (*coord, *height)),
        );
        self.objects.extend(snapshot.objects.iter().copied());
        true
    }

    /// Remove all semantic values whose source ownership is `region`.
    ///
    /// Returns whether the document changed. Session policy decides whether an edited region may
    /// actually be evicted; this low-level operation intentionally has no history knowledge.
    pub fn remove_region(&mut self, region: RegionCoord) -> bool {
        let before = (self.terrain.len(), self.heights.len(), self.objects.len());
        self.remove_region_entries(region);
        before != (self.terrain.len(), self.heights.len(), self.objects.len())
    }

    fn remove_region_entries(&mut self, region: RegionCoord) {
        self.terrain
            .retain(|coord, _| coord.tile.split_region().0 != region);
        self.heights
            .retain(|coord, _| coord.corner.split_region().0 != region);
        self.objects
            .retain(|placement| placement.position.tile.split_region().0 != region);
    }

    /// Every object placement whose source ownership is `region`, in no particular order.
    pub fn objects_in_region(&self, region: RegionCoord) -> Vec<ObjectPlacement> {
        self.objects
            .iter()
            .filter(|placement| placement.position.tile.split_region().0 == region)
            .copied()
            .collect()
    }

    /// Query one terrain tile. Missing sparse entries are semantic defaults.
    pub fn terrain_at(&self, coord: MapPosition) -> TerrainTile {
        self.terrain.get(&coord).copied().unwrap_or_default()
    }

    /// Query one explicit terrain-height vertex.
    pub fn height_at(&self, coord: HeightVertexCoord) -> Option<i32> {
        self.heights.get(&coord).copied()
    }

    pub fn contains_object(&self, placement: ObjectPlacement) -> bool {
        self.objects.contains(&placement)
    }

    pub fn terrain_entry_count(&self) -> usize {
        self.terrain.len()
    }

    pub fn height_vertex_count(&self) -> usize {
        self.heights.len()
    }

    pub fn object_count(&self) -> usize {
        self.objects.len()
    }

    /// Validate every operation against current state, including the effects of
    /// earlier operations in the same patch, without mutating this document.
    pub fn validate_patch(&self, patch: &MapPatch) -> Result<(), PatchApplyError> {
        let mut terrain = BTreeMap::<MapPosition, TerrainTile>::new();
        let mut heights = BTreeMap::<HeightVertexCoord, i32>::new();
        let mut objects = HashMap::<ObjectPlacement, bool>::new();

        for (op_index, op) in patch.ops().iter().copied().enumerate() {
            match op {
                MapOp::SetTerrain {
                    coord,
                    before,
                    after,
                } => {
                    let actual = terrain
                        .get(&coord)
                        .copied()
                        .unwrap_or_else(|| self.terrain_at(coord));
                    if actual != before {
                        return Err(PatchApplyError::TerrainMismatch {
                            op_index,
                            coord,
                            expected: before,
                            actual,
                        });
                    }
                    terrain.insert(coord, after);
                }
                MapOp::SetHeight {
                    coord,
                    before,
                    after,
                } => {
                    let actual = heights
                        .get(&coord)
                        .copied()
                        .or_else(|| self.height_at(coord));
                    match actual {
                        None => {
                            return Err(PatchApplyError::HeightMissing {
                                op_index,
                                coord,
                                expected: before,
                            });
                        }
                        Some(actual) if actual != before => {
                            return Err(PatchApplyError::HeightMismatch {
                                op_index,
                                coord,
                                expected: before,
                                actual,
                            });
                        }
                        Some(_) => {
                            heights.insert(coord, after);
                        }
                    }
                }
                MapOp::AddObject { placement } => {
                    let present = objects
                        .get(&placement)
                        .copied()
                        .unwrap_or_else(|| self.contains_object(placement));
                    if present {
                        return Err(PatchApplyError::ObjectAlreadyPresent {
                            op_index,
                            placement,
                        });
                    }
                    objects.insert(placement, true);
                }
                MapOp::RemoveObject { placement } => {
                    let present = objects
                        .get(&placement)
                        .copied()
                        .unwrap_or_else(|| self.contains_object(placement));
                    if !present {
                        return Err(PatchApplyError::ObjectMissing {
                            op_index,
                            placement,
                        });
                    }
                    objects.insert(placement, false);
                }
            }
        }

        Ok(())
    }

    /// Atomically apply a fully validated semantic patch.
    ///
    /// No authoritative state is changed unless every operation validates first.
    pub fn apply_patch(&mut self, patch: &MapPatch) -> Result<ChangeImpact, PatchApplyError> {
        self.validate_patch(patch)?;

        for op in patch.ops().iter().copied() {
            self.apply_validated_op(op);
        }

        Ok(patch.impact())
    }

    fn apply_validated_op(&mut self, op: MapOp) {
        match op {
            MapOp::SetTerrain { coord, after, .. } => {
                if after == TerrainTile::default() {
                    self.terrain.remove(&coord);
                } else {
                    self.terrain.insert(coord, after);
                }
            }
            MapOp::SetHeight { coord, after, .. } => {
                self.heights.insert(coord, after);
            }
            MapOp::AddObject { placement } => {
                self.objects.insert(placement);
            }
            MapOp::RemoveObject { placement } => {
                self.objects.remove(&placement);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{LocKind, ObjectOrientation};
    use osrs_core::coords::{MapTile, SourcePlane};
    use osrs_core::ids::{FloorUnderlayId, ObjectId};

    fn plane() -> SourcePlane {
        SourcePlane::new(0).unwrap_or_else(|| unreachable!("plane zero is valid"))
    }

    fn position(x: i32, y: i32) -> MapPosition {
        MapPosition::new(MapTile::new(x, y), plane())
    }

    fn height(x: i32, y: i32) -> HeightVertexCoord {
        HeightVertexCoord::new(MapTile::new(x, y), plane())
    }

    fn object() -> ObjectPlacement {
        object_at(3200, 3200)
    }

    fn object_at(x: i32, y: i32) -> ObjectPlacement {
        ObjectPlacement {
            object_id: ObjectId::new(100),
            position: position(x, y),
            kind: LocKind::new(10).unwrap_or_else(|| unreachable!("loc kind is valid")),
            orientation: ObjectOrientation::new(2)
                .unwrap_or_else(|| unreachable!("orientation is valid")),
        }
    }

    #[test]
    fn valid_patch_updates_authoritative_queries() {
        let tile = position(3200, 3200);
        let vertex = height(3200, 3200);
        let placement = object();
        let mut document = MapDocument::from_parts([], [(vertex, 32)], []);
        let terrain_after = TerrainTile {
            underlay: Some(FloorUnderlayId::new(7)),
            ..TerrainTile::default()
        };
        let patch = MapPatch::new([
            MapOp::SetTerrain {
                coord: tile,
                before: TerrainTile::default(),
                after: terrain_after,
            },
            MapOp::SetHeight {
                coord: vertex,
                before: 32,
                after: 64,
            },
            MapOp::AddObject { placement },
        ]);

        assert_eq!(document.apply_patch(&patch), Ok(patch.impact()));
        assert_eq!(document.terrain_at(tile), terrain_after);
        assert_eq!(document.height_at(vertex), Some(64));
        assert!(document.contains_object(placement));
    }

    #[test]
    fn region_replacement_is_idempotent_and_removes_stale_values() {
        let region = RegionCoord::new(50, 50);
        let outside = position(3264, 3200);
        let outside_height = height(3264, 3200);
        let inside = position(3200, 3200);
        let inside_height = height(3200, 3200);
        let placement = object();
        let terrain = TerrainTile {
            underlay: Some(FloorUnderlayId::new(7)),
            ..TerrainTile::default()
        };
        let mut document = MapDocument::from_parts(
            [(outside, terrain)],
            [(outside_height, -8)],
            [object_at(3264, 3200)],
        );
        let first = MapRegionSnapshot::from_parts(
            region,
            [(inside, terrain)],
            [(inside_height, -64)],
            [placement],
        )
        .unwrap_or_else(|error| unreachable!("valid snapshot: {error:?}"));

        assert!(document.replace_region(&first));
        assert!(!document.replace_region(&first));
        assert_eq!(document.terrain_at(inside), terrain);
        assert_eq!(document.height_at(inside_height), Some(-64));
        assert!(document.contains_object(placement));

        let replacement = MapRegionSnapshot::from_parts(
            region,
            [(inside, TerrainTile::default())],
            [(inside_height, -128)],
            [],
        )
        .unwrap_or_else(|error| unreachable!("valid replacement: {error:?}"));
        assert!(document.replace_region(&replacement));
        assert_eq!(document.terrain_at(inside), TerrainTile::default());
        assert_eq!(document.height_at(inside_height), Some(-128));
        assert!(!document.contains_object(placement));

        assert_eq!(document.terrain_at(outside), terrain);
        assert_eq!(document.height_at(outside_height), Some(-8));
        assert!(document.contains_object(object_at(3264, 3200)));
    }

    #[test]
    fn region_removal_preserves_other_regions() {
        let region = RegionCoord::new(50, 50);
        let inside = position(3200, 3200);
        let outside = position(3264, 3200);
        let terrain = TerrainTile {
            underlay: Some(FloorUnderlayId::new(7)),
            ..TerrainTile::default()
        };
        let mut document = MapDocument::from_parts(
            [(inside, terrain), (outside, terrain)],
            [(height(3200, 3200), -64), (height(3264, 3200), -8)],
            [object(), object_at(3264, 3200)],
        );

        assert!(document.remove_region(region));
        assert!(!document.remove_region(region));
        assert_eq!(document.terrain_at(inside), TerrainTile::default());
        assert_eq!(document.height_at(height(3200, 3200)), None);
        assert!(!document.contains_object(object()));
        assert_eq!(document.terrain_at(outside), terrain);
        assert_eq!(document.height_at(height(3264, 3200)), Some(-8));
        assert!(document.contains_object(object_at(3264, 3200)));
    }

    #[test]
    fn rejected_late_operation_leaves_document_unchanged() {
        let tile = position(3200, 3200);
        let vertex = height(3200, 3200);
        let mut document = MapDocument::from_parts([], [(vertex, 32)], []);
        let original = document.clone();
        let patch = MapPatch::new([
            MapOp::SetTerrain {
                coord: tile,
                before: TerrainTile::default(),
                after: TerrainTile {
                    underlay: Some(FloorUnderlayId::new(7)),
                    ..TerrainTile::default()
                },
            },
            MapOp::SetHeight {
                coord: vertex,
                before: 999,
                after: 64,
            },
        ]);

        assert_eq!(
            document.apply_patch(&patch),
            Err(PatchApplyError::HeightMismatch {
                op_index: 1,
                coord: vertex,
                expected: 999,
                actual: 32,
            })
        );
        assert_eq!(document, original);
    }

    #[test]
    fn validation_tracks_prior_operations_in_same_patch() {
        let vertex = height(3200, 3200);
        let mut document = MapDocument::from_parts([], [(vertex, 0)], []);
        let patch = MapPatch::new([
            MapOp::SetHeight {
                coord: vertex,
                before: 0,
                after: 64,
            },
            MapOp::SetHeight {
                coord: vertex,
                before: 64,
                after: 128,
            },
        ]);

        assert_eq!(document.apply_patch(&patch), Ok(patch.impact()));
        assert_eq!(document.height_at(vertex), Some(128));
    }

    #[test]
    fn inverse_patch_restores_exact_document_state() {
        let tile = position(3200, 3200);
        let vertex = height(3200, 3200);
        let placement = object();
        let mut document = MapDocument::from_parts([], [(vertex, 32)], []);
        let original = document.clone();
        let patch = MapPatch::new([
            MapOp::SetTerrain {
                coord: tile,
                before: TerrainTile::default(),
                after: TerrainTile {
                    underlay: Some(FloorUnderlayId::new(7)),
                    ..TerrainTile::default()
                },
            },
            MapOp::SetHeight {
                coord: vertex,
                before: 32,
                after: 64,
            },
            MapOp::AddObject { placement },
        ]);

        assert_eq!(document.apply_patch(&patch), Ok(patch.impact()));
        assert_eq!(document.apply_patch(&patch.inverse()), Ok(patch.impact()));
        assert_eq!(document, original);
    }

    #[test]
    fn object_presence_is_validated_before_mutation() {
        let placement = object();
        let mut document = MapDocument::from_parts([], [], [placement]);
        let original = document.clone();
        let patch = MapPatch::new([MapOp::AddObject { placement }]);

        assert_eq!(
            document.apply_patch(&patch),
            Err(PatchApplyError::ObjectAlreadyPresent {
                op_index: 0,
                placement,
            })
        );
        assert_eq!(document, original);
    }

    #[test]
    fn set_height_requires_an_existing_vertex() {
        let vertex = height(3200, 3200);
        let mut document = MapDocument::new();
        let patch = MapPatch::new([MapOp::SetHeight {
            coord: vertex,
            before: 0,
            after: 64,
        }]);

        assert_eq!(
            document.apply_patch(&patch),
            Err(PatchApplyError::HeightMissing {
                op_index: 0,
                coord: vertex,
                expected: 0,
            })
        );
        assert_eq!(document.height_at(vertex), None);
    }
}
