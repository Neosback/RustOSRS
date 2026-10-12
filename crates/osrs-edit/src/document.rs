use crate::{
    ChangeImpact, HeightVertexCoord, MapOp, MapPatch, MapPosition, ObjectPlacement, TerrainTile,
};
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
            Self::TerrainMismatch { op_index, .. }
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
        ObjectPlacement {
            object_id: ObjectId::new(100),
            position: position(3200, 3200),
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
