//! Semantic building architecture and RuneScape map-edit realization.
//!
//! The planner describes rooms and connectivity. This module turns that plan into a canonical
//! directional edge graph, then resolves semantic floor/wall/door roles through a palette into
//! ordinary [`MapPatch`] operations. No procedural path bypasses normal map validation or history.

use crate::building::{BuildingPlan, PortalKind, RoomId, RoomRect};
use crate::{
    BuildingValidationReport, GenerationSnapshot, LocKind, MapOp, MapPatch, MapPosition,
    ObjectOrientation, ObjectPlacement, TerrainOverlay, TerrainTile, validate_building_plan,
};
use osrs_core::coords::{MapTile, SourcePlane};
use osrs_core::ids::{FloorUnderlayId, ObjectId};
use std::collections::{BTreeMap, BTreeSet, HashSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CardinalDirection {
    North,
    East,
    South,
    West,
}

impl CardinalDirection {
    pub const ALL: [Self; 4] = [Self::North, Self::East, Self::South, Self::West];

    pub const fn index(self) -> usize {
        match self {
            Self::North => 0,
            Self::East => 1,
            Self::South => 2,
            Self::West => 3,
        }
    }

    pub fn neighbor(self, tile: MapTile) -> Option<MapTile> {
        match self {
            Self::North => Some(MapTile::new(tile.x, tile.y.checked_add(1)?)),
            Self::East => Some(MapTile::new(tile.x.checked_add(1)?, tile.y)),
            Self::South => Some(MapTile::new(tile.x, tile.y.checked_sub(1)?)),
            Self::West => Some(MapTile::new(tile.x.checked_sub(1)?, tile.y)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EdgeFeature {
    Open,
    ExteriorWall,
    InteriorWall,
    Door,
    Window,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SemanticEdge {
    pub tile: MapTile,
    pub direction: CardinalDirection,
    pub feature: EdgeFeature,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildingArchitecture {
    floor_tiles: Vec<MapTile>,
    edges: Vec<SemanticEdge>,
}

impl BuildingArchitecture {
    pub fn floor_tiles(&self) -> &[MapTile] {
        &self.floor_tiles
    }

    pub fn edges(&self) -> &[SemanticEdge] {
        &self.edges
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildingArchitectureError {
    CoordinateOverflow {
        tile: MapTile,
        direction: CardinalDirection,
    },
    RoomTileOverflow {
        room: RoomId,
    },
    DuplicateRoomTile {
        tile: MapTile,
        first: RoomId,
        second: RoomId,
    },
    InvalidInternalPortal {
        from: RoomId,
        to: RoomId,
        doorway: MapTile,
    },
    InvalidExteriorPortal {
        entrance: MapTile,
        doorway: MapTile,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct EdgeKey {
    tile: MapTile,
    direction: CardinalDirection,
}

pub fn derive_building_architecture(
    plan: &BuildingPlan,
) -> Result<BuildingArchitecture, BuildingArchitectureError> {
    let mut room_by_tile = BTreeMap::<MapTile, RoomId>::new();
    for room in &plan.rooms {
        for local_y in 0..room.rect.height {
            for local_x in 0..room.rect.width {
                let Some(tile) = rect_tile(room.rect, local_x, local_y) else {
                    return Err(BuildingArchitectureError::RoomTileOverflow { room: room.id });
                };
                if let Some(first) = room_by_tile.insert(tile, room.id)
                    && first != room.id
                {
                    return Err(BuildingArchitectureError::DuplicateRoomTile {
                        tile,
                        first,
                        second: room.id,
                    });
                }
            }
        }
    }

    let floor_tiles = room_by_tile.keys().copied().collect::<Vec<_>>();
    let mut edges = BTreeMap::<EdgeKey, EdgeFeature>::new();

    for (&tile, &room) in &room_by_tile {
        for direction in CardinalDirection::ALL {
            let Some(neighbor) = direction.neighbor(tile) else {
                return Err(BuildingArchitectureError::CoordinateOverflow { tile, direction });
            };
            match room_by_tile.get(&neighbor).copied() {
                None => {
                    edges.insert(EdgeKey { tile, direction }, EdgeFeature::ExteriorWall);
                }
                Some(other_room) if other_room != room => {
                    let key = canonical_edge_between(tile, neighbor)
                        .unwrap_or(EdgeKey { tile, direction });
                    edges.entry(key).or_insert(EdgeFeature::InteriorWall);
                }
                Some(_) => {}
            }
        }
    }

    for connection in &plan.connections {
        let key = match connection.from {
            None => exterior_portal_edge(plan, connection.doorway)?,
            Some(from) => {
                internal_portal_edge(&room_by_tile, from, connection.to, connection.doorway)?
            }
        };
        let feature = match connection.kind {
            PortalKind::Open => EdgeFeature::Open,
            PortalKind::Door => EdgeFeature::Door,
        };
        edges.insert(key, feature);
    }

    let edges = edges
        .into_iter()
        .map(|(key, feature)| SemanticEdge {
            tile: key.tile,
            direction: key.direction,
            feature,
        })
        .collect();

    Ok(BuildingArchitecture { floor_tiles, edges })
}

fn exterior_portal_edge(
    plan: &BuildingPlan,
    doorway: MapTile,
) -> Result<EdgeKey, BuildingArchitectureError> {
    let Some(direction) = direction_between(doorway, plan.entrance) else {
        return Err(BuildingArchitectureError::InvalidExteriorPortal {
            entrance: plan.entrance,
            doorway,
        });
    };
    Ok(EdgeKey {
        tile: doorway,
        direction,
    })
}

fn internal_portal_edge(
    room_by_tile: &BTreeMap<MapTile, RoomId>,
    from: RoomId,
    to: RoomId,
    doorway: MapTile,
) -> Result<EdgeKey, BuildingArchitectureError> {
    let here = room_by_tile.get(&doorway).copied();
    for direction in CardinalDirection::ALL {
        let Some(neighbor) = direction.neighbor(doorway) else {
            return Err(BuildingArchitectureError::CoordinateOverflow {
                tile: doorway,
                direction,
            });
        };
        let there = room_by_tile.get(&neighbor).copied();
        if matches!((here, there), (Some(a), Some(b)) if (a == from && b == to) || (a == to && b == from))
        {
            if let Some(edge) = canonical_edge_between(doorway, neighbor) {
                return Ok(edge);
            }
        }
    }

    Err(BuildingArchitectureError::InvalidInternalPortal { from, to, doorway })
}

fn canonical_edge_between(first: MapTile, second: MapTile) -> Option<EdgeKey> {
    match (second.x - first.x, second.y - first.y) {
        (1, 0) => Some(EdgeKey {
            tile: first,
            direction: CardinalDirection::East,
        }),
        (-1, 0) => Some(EdgeKey {
            tile: second,
            direction: CardinalDirection::East,
        }),
        (0, 1) => Some(EdgeKey {
            tile: first,
            direction: CardinalDirection::North,
        }),
        (0, -1) => Some(EdgeKey {
            tile: second,
            direction: CardinalDirection::North,
        }),
        _ => None,
    }
}

fn direction_between(from: MapTile, to: MapTile) -> Option<CardinalDirection> {
    match (to.x - from.x, to.y - from.y) {
        (0, 1) => Some(CardinalDirection::North),
        (1, 0) => Some(CardinalDirection::East),
        (0, -1) => Some(CardinalDirection::South),
        (-1, 0) => Some(CardinalDirection::West),
        _ => None,
    }
}

fn rect_tile(rect: RoomRect, local_x: u16, local_y: u16) -> Option<MapTile> {
    Some(MapTile::new(
        rect.origin.x.checked_add(i32::from(local_x))?,
        rect.origin.y.checked_add(i32::from(local_y))?,
    ))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FloorMaterial {
    pub underlay: Option<FloorUnderlayId>,
    pub overlay: Option<TerrainOverlay>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EdgeAsset {
    pub object_id: ObjectId,
    pub kind: LocKind,
    pub orientations: [ObjectOrientation; 4],
}

impl EdgeAsset {
    pub const fn orientation_for(self, direction: CardinalDirection) -> ObjectOrientation {
        self.orientations[direction.index()]
    }

    pub fn placement(self, edge: SemanticEdge, plane: SourcePlane) -> ObjectPlacement {
        ObjectPlacement {
            object_id: self.object_id,
            position: MapPosition::new(edge.tile, plane),
            kind: self.kind,
            orientation: self.orientation_for(edge.direction),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BuildingPalette {
    pub floor: FloorMaterial,
    pub exterior_wall: EdgeAsset,
    pub interior_wall: EdgeAsset,
    pub door: EdgeAsset,
    pub window: Option<EdgeAsset>,
}

impl BuildingPalette {
    fn asset_for(self, feature: EdgeFeature) -> Option<EdgeAsset> {
        match feature {
            EdgeFeature::Open => None,
            EdgeFeature::ExteriorWall => Some(self.exterior_wall),
            EdgeFeature::InteriorWall => Some(self.interior_wall),
            EdgeFeature::Door => Some(self.door),
            EdgeFeature::Window => self.window,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildingRealizationError {
    InvalidPlan { report: BuildingValidationReport },
    Architecture { error: BuildingArchitectureError },
    MissingSnapshotTerrain { tile: MapTile },
    MissingAsset { edge: SemanticEdge },
    DuplicateObjectPlacement { placement: ObjectPlacement },
}

pub fn build_building_patch(
    snapshot: &GenerationSnapshot,
    plan: &BuildingPlan,
    palette: BuildingPalette,
) -> Result<MapPatch, BuildingRealizationError> {
    let report = validate_building_plan(plan, snapshot);
    if !report.is_valid() {
        return Err(BuildingRealizationError::InvalidPlan { report });
    }

    let architecture = derive_building_architecture(plan)
        .map_err(|error| BuildingRealizationError::Architecture { error })?;
    let mut ops = Vec::new();

    for &tile in architecture.floor_tiles() {
        let Some(before) = snapshot.terrain_at(tile) else {
            return Err(BuildingRealizationError::MissingSnapshotTerrain { tile });
        };
        let after = TerrainTile {
            underlay: palette.floor.underlay,
            overlay: palette.floor.overlay,
            flags: before.flags,
        };
        if before != after {
            ops.push(MapOp::SetTerrain {
                coord: MapPosition::new(tile, plan.request.base_plane),
                before,
                after,
            });
        }
    }

    let mut placements = HashSet::new();
    for &edge in architecture.edges() {
        let Some(asset) = palette.asset_for(edge.feature) else {
            if edge.feature == EdgeFeature::Open {
                continue;
            }
            return Err(BuildingRealizationError::MissingAsset { edge });
        };
        let placement = asset.placement(edge, plan.request.base_plane);
        if !placements.insert(placement) {
            return Err(BuildingRealizationError::DuplicateObjectPlacement { placement });
        }
        ops.push(MapOp::AddObject { placement });
    }

    Ok(MapPatch::new(ops))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        BuildingArchetype, BuildingConstraints, BuildingRequest, BuildingStyle, HeightVertexCoord,
        MapDocument, RectSite, capture_generation_snapshot, plan_basic_house,
    };

    fn plane() -> SourcePlane {
        SourcePlane::new(0).unwrap_or_else(|| unreachable!("plane zero is valid"))
    }

    fn request() -> BuildingRequest {
        BuildingRequest {
            seed: 42,
            site: RectSite::new(MapTile::new(3200, 3200), 12, 10)
                .unwrap_or_else(|| unreachable!("test site is valid")),
            base_plane: plane(),
            floors: 1,
            archetype: BuildingArchetype::House,
            style: BuildingStyle::Generic,
            constraints: BuildingConstraints::default(),
        }
    }

    fn heights_for(request: BuildingRequest) -> Vec<(HeightVertexCoord, i32)> {
        let mut heights = Vec::new();
        for local_y in 0..=request.site.height() {
            for local_x in 0..=request.site.width() {
                heights.push((
                    HeightVertexCoord::new(
                        MapTile::new(
                            request.site.origin().x + i32::from(local_x),
                            request.site.origin().y + i32::from(local_y),
                        ),
                        request.base_plane,
                    ),
                    -64,
                ));
            }
        }
        heights
    }

    fn orientations() -> [ObjectOrientation; 4] {
        [0, 1, 2, 3].map(|value| {
            ObjectOrientation::new(value)
                .unwrap_or_else(|| unreachable!("test orientation is valid"))
        })
    }

    fn asset(object_id: u32) -> EdgeAsset {
        EdgeAsset {
            object_id: ObjectId::new(object_id),
            kind: LocKind::new(0).unwrap_or_else(|| unreachable!("test loc kind is valid")),
            orientations: orientations(),
        }
    }

    fn palette() -> BuildingPalette {
        BuildingPalette {
            floor: FloorMaterial {
                underlay: Some(FloorUnderlayId::new(7)),
                overlay: None,
            },
            exterior_wall: asset(100),
            interior_wall: asset(101),
            door: asset(102),
            window: None,
        }
    }

    #[test]
    fn architecture_contains_one_door_per_door_connection() {
        let plan = plan_basic_house(request())
            .unwrap_or_else(|error| unreachable!("valid test plan: {error:?}"));
        let architecture = derive_building_architecture(&plan)
            .unwrap_or_else(|error| unreachable!("valid architecture: {error:?}"));
        let door_count = architecture
            .edges()
            .iter()
            .filter(|edge| edge.feature == EdgeFeature::Door)
            .count();
        let expected = plan
            .connections
            .iter()
            .filter(|connection| connection.kind == PortalKind::Door)
            .count();

        assert_eq!(door_count, expected);
    }

    #[test]
    fn semantic_edges_are_unique_and_stably_sorted() {
        let plan = plan_basic_house(request())
            .unwrap_or_else(|error| unreachable!("valid test plan: {error:?}"));
        let architecture = derive_building_architecture(&plan)
            .unwrap_or_else(|error| unreachable!("valid architecture: {error:?}"));
        let edges = architecture.edges();
        let unique = edges.iter().copied().collect::<BTreeSet<_>>();

        assert_eq!(unique.len(), edges.len());
        assert!(edges.windows(2).all(|pair| pair[0] < pair[1]));
    }

    #[test]
    fn realization_preserves_flags_and_round_trips_exactly() {
        let request = request();
        let plan = plan_basic_house(request)
            .unwrap_or_else(|error| unreachable!("valid test plan: {error:?}"));
        let existing_tile = plan.rooms[0].rect.origin;
        let existing_terrain = TerrainTile {
            underlay: Some(FloorUnderlayId::new(3)),
            overlay: None,
            flags: 5,
        };
        let mut document = MapDocument::from_parts(
            [(
                MapPosition::new(existing_tile, request.base_plane),
                existing_terrain,
            )],
            heights_for(request),
            [],
        );
        let snapshot = capture_generation_snapshot(&document, request, []);
        let patch = build_building_patch(&snapshot, &plan, palette())
            .unwrap_or_else(|error| unreachable!("valid realization: {error:?}"));
        let before = document.clone();

        document
            .apply_patch(&patch)
            .unwrap_or_else(|error| unreachable!("valid generated patch: {error:?}"));
        let after = document.clone();
        assert_ne!(after, before);
        assert_eq!(
            document
                .terrain_at(MapPosition::new(existing_tile, request.base_plane))
                .flags,
            5
        );

        document
            .apply_patch(&patch.inverse())
            .unwrap_or_else(|error| unreachable!("valid generated inverse: {error:?}"));
        assert_eq!(document, before);

        document
            .apply_patch(&patch)
            .unwrap_or_else(|error| unreachable!("valid generated redo: {error:?}"));
        assert_eq!(document, after);
    }

    #[test]
    fn invalid_exterior_portal_is_rejected_before_asset_realization() {
        let mut plan = plan_basic_house(request())
            .unwrap_or_else(|error| unreachable!("valid test plan: {error:?}"));
        let exterior = plan
            .connections
            .iter_mut()
            .find(|connection| connection.from.is_none())
            .unwrap_or_else(|| unreachable!("house has exterior connection"));
        exterior.doorway = plan.rooms[0].rect.origin;

        assert!(matches!(
            derive_building_architecture(&plan),
            Err(BuildingArchitectureError::InvalidExteriorPortal { .. })
        ));
    }
}
