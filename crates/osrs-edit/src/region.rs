use crate::{HeightVertexCoord, MapPosition, ObjectPlacement, TerrainTile};
use osrs_core::coords::RegionCoord;
use std::collections::{BTreeMap, HashSet};

/// A semantic value was assigned to a region other than the one declared by the snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegionSnapshotError {
    TerrainOutsideRegion {
        region: RegionCoord,
        coord: MapPosition,
    },
    HeightOutsideRegion {
        region: RegionCoord,
        coord: HeightVertexCoord,
    },
    ObjectOutsideRegion {
        region: RegionCoord,
        placement: ObjectPlacement,
    },
}

/// Canonical semantic state owned by one 64x64 source-map region.
///
/// This type deliberately contains only editor-domain values. Cache opcodes, scene-local arrays,
/// renderer geometry, picker data, and GPU resources are adapted before construction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MapRegionSnapshot {
    region: RegionCoord,
    pub(crate) terrain: BTreeMap<MapPosition, TerrainTile>,
    pub(crate) heights: BTreeMap<HeightVertexCoord, i32>,
    pub(crate) objects: HashSet<ObjectPlacement>,
}

impl MapRegionSnapshot {
    pub fn from_parts(
        region: RegionCoord,
        terrain: impl IntoIterator<Item = (MapPosition, TerrainTile)>,
        heights: impl IntoIterator<Item = (HeightVertexCoord, i32)>,
        objects: impl IntoIterator<Item = ObjectPlacement>,
    ) -> Result<Self, RegionSnapshotError> {
        let mut terrain_map = BTreeMap::new();
        for (coord, tile) in terrain {
            if coord.tile.split_region().0 != region {
                return Err(RegionSnapshotError::TerrainOutsideRegion { region, coord });
            }
            if tile == TerrainTile::default() {
                terrain_map.remove(&coord);
            } else {
                terrain_map.insert(coord, tile);
            }
        }

        let mut height_map = BTreeMap::new();
        for (coord, height) in heights {
            if coord.corner.split_region().0 != region {
                return Err(RegionSnapshotError::HeightOutsideRegion { region, coord });
            }
            height_map.insert(coord, height);
        }

        let mut object_set = HashSet::new();
        for placement in objects {
            if placement.position.tile.split_region().0 != region {
                return Err(RegionSnapshotError::ObjectOutsideRegion { region, placement });
            }
            object_set.insert(placement);
        }

        Ok(Self {
            region,
            terrain: terrain_map,
            heights: height_map,
            objects: object_set,
        })
    }

    pub const fn region(&self) -> RegionCoord {
        self.region
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{LocKind, ObjectOrientation};
    use osrs_core::{
        coords::{MapTile, SourcePlane},
        ids::{FloorUnderlayId, ObjectId},
    };

    fn plane() -> SourcePlane {
        SourcePlane::new(0).unwrap_or_else(|| unreachable!("plane zero is valid"))
    }

    fn position(x: i32, y: i32) -> MapPosition {
        MapPosition::new(MapTile::new(x, y), plane())
    }

    fn vertex(x: i32, y: i32) -> HeightVertexCoord {
        HeightVertexCoord::new(MapTile::new(x, y), plane())
    }

    fn object(x: i32, y: i32) -> ObjectPlacement {
        ObjectPlacement {
            object_id: ObjectId::new(100),
            position: position(x, y),
            kind: LocKind::new(10).unwrap_or_else(|| unreachable!("loc kind is valid")),
            orientation: ObjectOrientation::new(2)
                .unwrap_or_else(|| unreachable!("orientation is valid")),
        }
    }

    #[test]
    fn snapshot_rejects_values_from_another_region() {
        let region = RegionCoord::new(50, 50);
        assert_eq!(
            MapRegionSnapshot::from_parts(
                region,
                [
                    (
                        position(3200, 3200),
                        TerrainTile {
                            underlay: Some(FloorUnderlayId::new(7)),
                            ..TerrainTile::default()
                        },
                    ),
                    (
                        position(3264, 3200),
                        TerrainTile {
                            underlay: Some(FloorUnderlayId::new(8)),
                            ..TerrainTile::default()
                        },
                    ),
                ],
                [],
                [],
            ),
            Err(RegionSnapshotError::TerrainOutsideRegion {
                region,
                coord: position(3264, 3200),
            })
        );
    }

    #[test]
    fn default_terrain_remains_sparse_but_heights_and_objects_are_retained() {
        let region = RegionCoord::new(50, 50);
        let snapshot = MapRegionSnapshot::from_parts(
            region,
            [
                (position(3200, 3200), TerrainTile::default()),
                (
                    position(3201, 3200),
                    TerrainTile {
                        underlay: Some(FloorUnderlayId::new(7)),
                        ..TerrainTile::default()
                    },
                ),
            ],
            [(vertex(3200, 3200), -64)],
            [object(3200, 3200)],
        )
        .unwrap_or_else(|error| unreachable!("valid snapshot: {error:?}"));

        assert_eq!(snapshot.region(), region);
        assert_eq!(snapshot.terrain_entry_count(), 1);
        assert_eq!(snapshot.height_vertex_count(), 1);
        assert_eq!(snapshot.object_count(), 1);
    }
}
