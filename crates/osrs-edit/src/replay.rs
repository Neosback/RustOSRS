use crate::{HeightVertexCoord, MapOp, MapPatch, MapPosition, ObjectPlacement};
use core::ops::{BitOr, BitOrAssign};
use std::collections::{BTreeMap, BTreeSet};

/// Semantic terrain channels that must be refreshed after replaying a map patch.
///
/// Overlay ID, shape, and rotation intentionally form one channel because they are one semantic
/// `TerrainOverlay` value. Renderer- or worker-specific dirty flags do not belong here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct TerrainChannels(u8);

impl TerrainChannels {
    pub const NONE: Self = Self(0);
    pub const UNDERLAY: Self = Self(1 << 0);
    pub const OVERLAY: Self = Self(1 << 1);
    pub const FLAGS: Self = Self(1 << 2);

    pub const fn bits(self) -> u8 {
        self.0
    }

    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    pub const fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }
}

impl BitOr for TerrainChannels {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl BitOrAssign for TerrainChannels {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

/// One source-map tile and the semantic terrain channels changed there by a patch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TerrainReplayChange {
    pub coord: MapPosition,
    pub channels: TerrainChannels,
}

/// Immutable semantic projection requirements derived from one exact `MapPatch`.
///
/// Terrain coordinates and height vertices are coalesced and sorted deterministically. Object
/// placement order follows transaction order so inverse patches naturally reverse both operation
/// order and mutation polarity.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct MapReplayFootprint {
    terrain: Vec<TerrainReplayChange>,
    heights: Vec<HeightVertexCoord>,
    removed_objects: Vec<ObjectPlacement>,
    added_objects: Vec<ObjectPlacement>,
}

impl MapReplayFootprint {
    pub fn terrain(&self) -> &[TerrainReplayChange] {
        &self.terrain
    }

    pub fn heights(&self) -> &[HeightVertexCoord] {
        &self.heights
    }

    pub fn removed_objects(&self) -> &[ObjectPlacement] {
        &self.removed_objects
    }

    pub fn added_objects(&self) -> &[ObjectPlacement] {
        &self.added_objects
    }

    pub fn is_empty(&self) -> bool {
        self.terrain.is_empty()
            && self.heights.is_empty()
            && self.removed_objects.is_empty()
            && self.added_objects.is_empty()
    }
}

impl MapPatch {
    /// Derive the semantic projection footprint for replaying this exact patch.
    ///
    /// This records what semantic state must be refreshed, not how any renderer or worker performs
    /// that refresh. No-op value operations contribute nothing. Repeated terrain/height coordinates
    /// are coalesced deterministically.
    pub fn replay_footprint(&self) -> MapReplayFootprint {
        let mut terrain = BTreeMap::<MapPosition, TerrainChannels>::new();
        let mut heights = BTreeSet::<HeightVertexCoord>::new();
        let mut removed_objects = Vec::new();
        let mut added_objects = Vec::new();

        for op in self.ops() {
            match *op {
                MapOp::SetTerrain {
                    coord,
                    before,
                    after,
                } => {
                    let mut channels = TerrainChannels::NONE;
                    if before.underlay != after.underlay {
                        channels |= TerrainChannels::UNDERLAY;
                    }
                    if before.overlay != after.overlay {
                        channels |= TerrainChannels::OVERLAY;
                    }
                    if before.flags != after.flags {
                        channels |= TerrainChannels::FLAGS;
                    }
                    if !channels.is_empty() {
                        terrain
                            .entry(coord)
                            .and_modify(|existing| *existing |= channels)
                            .or_insert(channels);
                    }
                }
                MapOp::SetHeight {
                    coord,
                    before,
                    after,
                } => {
                    if before != after {
                        heights.insert(coord);
                    }
                }
                MapOp::AddObject { placement } => added_objects.push(placement),
                MapOp::RemoveObject { placement } => removed_objects.push(placement),
            }
        }

        MapReplayFootprint {
            terrain: terrain
                .into_iter()
                .map(|(coord, channels)| TerrainReplayChange { coord, channels })
                .collect(),
            heights: heights.into_iter().collect(),
            removed_objects,
            added_objects,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        LocKind, ObjectOrientation, OverlayRotation, OverlayShape, TerrainOverlay, TerrainTile,
    };
    use osrs_core::coords::{MapTile, SourcePlane};
    use osrs_core::ids::{FloorOverlayId, FloorUnderlayId, ObjectId};

    fn plane() -> SourcePlane {
        SourcePlane::new(0).unwrap_or_else(|| unreachable!("plane zero is valid"))
    }

    fn position(x: i32, y: i32) -> MapPosition {
        MapPosition::new(MapTile::new(x, y), plane())
    }

    fn height(x: i32, y: i32) -> HeightVertexCoord {
        HeightVertexCoord::new(MapTile::new(x, y), plane())
    }

    fn object(id: u32, x: i32, y: i32) -> ObjectPlacement {
        ObjectPlacement {
            object_id: ObjectId::new(id),
            position: position(x, y),
            kind: LocKind::new(10).unwrap_or_else(|| unreachable!("loc kind is valid")),
            orientation: ObjectOrientation::new(1)
                .unwrap_or_else(|| unreachable!("orientation is valid")),
        }
    }

    fn overlay(id: u32, shape: u8, rotation: u8) -> TerrainOverlay {
        TerrainOverlay {
            id: FloorOverlayId::new(id),
            shape: OverlayShape::new(shape).unwrap_or_else(|| unreachable!("shape is valid")),
            rotation: OverlayRotation::new(rotation)
                .unwrap_or_else(|| unreachable!("rotation is valid")),
        }
    }

    #[test]
    fn terrain_channels_match_exact_semantic_differences() {
        let coord = position(3200, 3200);
        let patch = MapPatch::new([MapOp::SetTerrain {
            coord,
            before: TerrainTile::default(),
            after: TerrainTile {
                underlay: Some(FloorUnderlayId::new(7)),
                flags: 1,
                ..TerrainTile::default()
            },
        }]);

        let footprint = patch.replay_footprint();
        assert_eq!(footprint.terrain().len(), 1);
        let change = footprint.terrain()[0];
        assert_eq!(change.coord, coord);
        assert!(change.channels.contains(TerrainChannels::UNDERLAY));
        assert!(change.channels.contains(TerrainChannels::FLAGS));
        assert!(!change.channels.contains(TerrainChannels::OVERLAY));
    }

    #[test]
    fn overlay_identity_shape_and_rotation_are_one_channel() {
        let coord = position(3200, 3200);
        for after in [overlay(2, 0, 0), overlay(1, 1, 0), overlay(1, 0, 1)] {
            let patch = MapPatch::new([MapOp::SetTerrain {
                coord,
                before: TerrainTile {
                    overlay: Some(overlay(1, 0, 0)),
                    ..TerrainTile::default()
                },
                after: TerrainTile {
                    overlay: Some(after),
                    ..TerrainTile::default()
                },
            }]);
            assert_eq!(
                patch.replay_footprint().terrain(),
                &[TerrainReplayChange {
                    coord,
                    channels: TerrainChannels::OVERLAY,
                }]
            );
        }
    }

    #[test]
    fn no_op_value_operations_create_no_footprint_entries() {
        let coord = position(3200, 3200);
        let corner = height(3201, 3201);
        let patch = MapPatch::new([
            MapOp::SetTerrain {
                coord,
                before: TerrainTile::default(),
                after: TerrainTile::default(),
            },
            MapOp::SetHeight {
                coord: corner,
                before: 64,
                after: 64,
            },
        ]);

        assert!(patch.replay_footprint().is_empty());
    }

    #[test]
    fn repeated_coordinates_coalesce_in_deterministic_order() {
        let later = position(3201, 3200);
        let earlier = position(3200, 3200);
        let corner = height(3200, 3200);
        let patch = MapPatch::new([
            MapOp::SetTerrain {
                coord: later,
                before: TerrainTile::default(),
                after: TerrainTile {
                    flags: 1,
                    ..TerrainTile::default()
                },
            },
            MapOp::SetTerrain {
                coord: earlier,
                before: TerrainTile::default(),
                after: TerrainTile {
                    underlay: Some(FloorUnderlayId::new(4)),
                    ..TerrainTile::default()
                },
            },
            MapOp::SetTerrain {
                coord: earlier,
                before: TerrainTile::default(),
                after: TerrainTile {
                    overlay: Some(overlay(3, 1, 2)),
                    ..TerrainTile::default()
                },
            },
            MapOp::SetHeight {
                coord: corner,
                before: 0,
                after: 32,
            },
            MapOp::SetHeight {
                coord: corner,
                before: 32,
                after: 64,
            },
        ]);

        let footprint = patch.replay_footprint();
        assert_eq!(
            footprint.terrain(),
            &[
                TerrainReplayChange {
                    coord: earlier,
                    channels: TerrainChannels::UNDERLAY | TerrainChannels::OVERLAY,
                },
                TerrainReplayChange {
                    coord: later,
                    channels: TerrainChannels::FLAGS,
                },
            ]
        );
        assert_eq!(footprint.heights(), &[corner]);
    }

    #[test]
    fn inverse_preserves_terrain_and_height_coordinate_footprint() {
        let coord = position(3200, 3200);
        let corner = height(3201, 3201);
        let patch = MapPatch::new([
            MapOp::SetTerrain {
                coord,
                before: TerrainTile::default(),
                after: TerrainTile {
                    overlay: Some(overlay(3, 1, 2)),
                    flags: 1,
                    ..TerrainTile::default()
                },
            },
            MapOp::SetHeight {
                coord: corner,
                before: 0,
                after: 64,
            },
        ]);

        let forward = patch.replay_footprint();
        let inverse = patch.inverse().replay_footprint();
        assert_eq!(inverse.terrain(), forward.terrain());
        assert_eq!(inverse.heights(), forward.heights());
    }

    #[test]
    fn inverse_reverses_object_mutation_polarity() {
        let added = object(100, 3200, 3200);
        let removed = object(200, 3201, 3200);
        let patch = MapPatch::new([
            MapOp::AddObject { placement: added },
            MapOp::RemoveObject { placement: removed },
        ]);

        let forward = patch.replay_footprint();
        assert_eq!(forward.added_objects(), &[added]);
        assert_eq!(forward.removed_objects(), &[removed]);

        let inverse = patch.inverse().replay_footprint();
        assert_eq!(inverse.added_objects(), &[removed]);
        assert_eq!(inverse.removed_objects(), &[added]);
    }
}
