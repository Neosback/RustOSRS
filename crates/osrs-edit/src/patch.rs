use crate::{HeightVertexCoord, MapPosition, ObjectPlacement, TerrainTile};
use core::ops::{BitOr, BitOrAssign};

/// Semantic effects produced by an edit operation.
///
/// This is deliberately independent from renderer-specific dirty flags. View and
/// infrastructure layers may translate these effects into their own rebuild work.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct ChangeImpact(u8);

impl ChangeImpact {
    pub const NONE: Self = Self(0);
    pub const SCENARIO: Self = Self(1 << 7);
    pub const TERRAIN_GEOMETRY: Self = Self(1 << 0);
    pub const TERRAIN_MATERIAL: Self = Self(1 << 1);
    pub const NORMALS: Self = Self(1 << 2);
    pub const LIGHTING: Self = Self(1 << 3);
    pub const COLLISION: Self = Self(1 << 4);
    pub const LOCATIONS: Self = Self(1 << 5);
    pub const BOUNDS: Self = Self(1 << 6);

    pub const fn bits(self) -> u8 {
        self.0
    }

    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    pub const fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

    pub const fn intersects(self, other: Self) -> bool {
        (self.0 & other.0) != 0
    }
}

impl BitOr for ChangeImpact {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl BitOrAssign for ChangeImpact {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

/// One reversible semantic map mutation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MapOp {
    SetTerrain {
        coord: MapPosition,
        before: TerrainTile,
        after: TerrainTile,
    },
    SetHeight {
        coord: HeightVertexCoord,
        before: i32,
        after: i32,
    },
    AddObject {
        placement: ObjectPlacement,
    },
    RemoveObject {
        placement: ObjectPlacement,
    },
}

impl MapOp {
    /// Return the exact inverse operation.
    pub const fn inverse(self) -> Self {
        match self {
            Self::SetTerrain {
                coord,
                before,
                after,
            } => Self::SetTerrain {
                coord,
                before: after,
                after: before,
            },
            Self::SetHeight {
                coord,
                before,
                after,
            } => Self::SetHeight {
                coord,
                before: after,
                after: before,
            },
            Self::AddObject { placement } => Self::RemoveObject { placement },
            Self::RemoveObject { placement } => Self::AddObject { placement },
        }
    }

    /// Derive conservative semantic invalidation from this operation.
    ///
    /// The first implementation intentionally favors correctness over minimal
    /// rebuild radii. Later view-synchronization work can translate these semantic
    /// effects into tighter chunk/region invalidation without changing patch data.
    pub fn impact(&self) -> ChangeImpact {
        match self {
            Self::SetTerrain { before, after, .. } => {
                let mut impact = ChangeImpact::NONE;

                if before.underlay != after.underlay || before.overlay != after.overlay {
                    impact |= ChangeImpact::TERRAIN_GEOMETRY;
                    impact |= ChangeImpact::TERRAIN_MATERIAL;
                    impact |= ChangeImpact::NORMALS;
                    impact |= ChangeImpact::LIGHTING;
                }

                if before.flags != after.flags {
                    impact |= ChangeImpact::TERRAIN_GEOMETRY;
                    impact |= ChangeImpact::LIGHTING;
                    impact |= ChangeImpact::COLLISION;
                    impact |= ChangeImpact::BOUNDS;
                }

                impact
            }
            Self::SetHeight { before, after, .. } => {
                if before == after {
                    ChangeImpact::NONE
                } else {
                    ChangeImpact::TERRAIN_GEOMETRY
                        | ChangeImpact::NORMALS
                        | ChangeImpact::LIGHTING
                        | ChangeImpact::LOCATIONS
                        | ChangeImpact::BOUNDS
                }
            }
            Self::AddObject { .. } | Self::RemoveObject { .. } => {
                ChangeImpact::LOCATIONS | ChangeImpact::COLLISION | ChangeImpact::BOUNDS
            }
        }
    }
}

/// One logical, reversible editor transaction.
///
/// `impact` is derived from `ops` so callers cannot construct a patch whose
/// invalidation metadata disagrees with its mutations.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct MapPatch {
    ops: Vec<MapOp>,
    impact: ChangeImpact,
}

impl MapPatch {
    pub fn new(ops: impl IntoIterator<Item = MapOp>) -> Self {
        let ops: Vec<_> = ops.into_iter().collect();
        let impact = ops
            .iter()
            .fold(ChangeImpact::NONE, |impact, op| impact | op.impact());

        Self { ops, impact }
    }

    pub fn ops(&self) -> &[MapOp] {
        &self.ops
    }

    pub const fn impact(&self) -> ChangeImpact {
        self.impact
    }

    pub fn is_empty(&self) -> bool {
        self.ops.is_empty()
    }

    pub fn len(&self) -> usize {
        self.ops.len()
    }

    /// Build the transaction that exactly undoes this patch.
    ///
    /// Operations are reversed as well as individually inverted so future
    /// dependent multi-operation patches retain correct transactional ordering.
    pub fn inverse(&self) -> Self {
        Self::new(self.ops.iter().rev().copied().map(MapOp::inverse))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{LocKind, ObjectOrientation};
    use osrs_core::coords::{MapTile, SourcePlane};
    use osrs_core::ids::{FloorUnderlayId, ObjectId};

    fn map_position(x: i32, y: i32) -> MapPosition {
        let plane = SourcePlane::new(0).unwrap_or_else(|| unreachable!("plane zero is valid"));
        MapPosition::new(MapTile::new(x, y), plane)
    }

    fn object_placement() -> ObjectPlacement {
        ObjectPlacement {
            object_id: ObjectId::new(100),
            position: map_position(3200, 3200),
            kind: LocKind::new(10).unwrap_or_else(|| unreachable!("loc kind is valid")),
            orientation: ObjectOrientation::new(2)
                .unwrap_or_else(|| unreachable!("orientation is valid")),
        }
    }

    #[test]
    fn patch_derives_union_of_operation_impacts() {
        let coord = map_position(3200, 3200);
        let terrain = MapOp::SetTerrain {
            coord,
            before: TerrainTile::default(),
            after: TerrainTile {
                underlay: Some(FloorUnderlayId::new(7)),
                ..TerrainTile::default()
            },
        };
        let object = MapOp::AddObject {
            placement: object_placement(),
        };

        let patch = MapPatch::new([terrain, object]);
        let impact = patch.impact();

        assert!(impact.contains(ChangeImpact::TERRAIN_GEOMETRY));
        assert!(impact.contains(ChangeImpact::TERRAIN_MATERIAL));
        assert!(impact.contains(ChangeImpact::NORMALS));
        assert!(impact.contains(ChangeImpact::LIGHTING));
        assert!(impact.contains(ChangeImpact::LOCATIONS));
        assert!(impact.contains(ChangeImpact::COLLISION));
        assert!(impact.contains(ChangeImpact::BOUNDS));
    }

    #[test]
    fn no_op_value_changes_have_no_impact() {
        let position = map_position(3200, 3200);
        let plane = position.plane;
        let corner = HeightVertexCoord::new(MapTile::new(3200, 3200), plane);

        let patch = MapPatch::new([
            MapOp::SetTerrain {
                coord: position,
                before: TerrainTile::default(),
                after: TerrainTile::default(),
            },
            MapOp::SetHeight {
                coord: corner,
                before: 128,
                after: 128,
            },
        ]);

        assert!(patch.impact().is_empty());
    }

    #[test]
    fn inverse_reverses_order_and_round_trips_exactly() {
        let position = map_position(3200, 3200);
        let plane = position.plane;
        let corner = HeightVertexCoord::new(MapTile::new(3201, 3201), plane);
        let original = MapPatch::new([
            MapOp::SetHeight {
                coord: corner,
                before: 0,
                after: 64,
            },
            MapOp::AddObject {
                placement: object_placement(),
            },
        ]);

        let inverse = original.inverse();

        assert_eq!(
            inverse.ops(),
            &[
                MapOp::RemoveObject {
                    placement: object_placement(),
                },
                MapOp::SetHeight {
                    coord: corner,
                    before: 64,
                    after: 0,
                },
            ]
        );
        assert_eq!(inverse.inverse(), original);
    }

    #[test]
    fn terrain_flag_changes_include_collision_and_bounds() {
        let op = MapOp::SetTerrain {
            coord: map_position(3200, 3200),
            before: TerrainTile::default(),
            after: TerrainTile {
                flags: 1,
                ..TerrainTile::default()
            },
        };

        let impact = op.impact();
        assert!(impact.contains(ChangeImpact::COLLISION));
        assert!(impact.contains(ChangeImpact::BOUNDS));
        assert!(!impact.contains(ChangeImpact::TERRAIN_MATERIAL));
    }
}
