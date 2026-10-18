//! Generation-side map snapshots and deterministic building-plan validation.
//!
//! Procedural generation consumes a bounded semantic snapshot rather than reaching into cache,
//! renderer, or editor state while solving. Object footprints are supplied by an adapter so this
//! layer remains independent of cache object definitions and catalog infrastructure.

use crate::building::{
    BuildingPlan, BuildingRequest, OccupancyCell, OccupancyGrid, RectSite, RoomId, RoomRect,
    RoomSpec,
};
use crate::{HeightVertexCoord, MapPosition, MapQuery, TerrainTile};
use osrs_core::coords::{MapTile, SourcePlane};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OccupiedFootprint {
    pub origin: MapTile,
    pub plane: SourcePlane,
    pub width: u16,
    pub height: u16,
}

impl OccupiedFootprint {
    pub fn new(origin: MapTile, plane: SourcePlane, width: u16, height: u16) -> Option<Self> {
        if width == 0 || height == 0 {
            return None;
        }
        origin.x.checked_add(i32::from(width))?;
        origin.y.checked_add(i32::from(height))?;
        Some(Self {
            origin,
            plane,
            width,
            height,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenerationSnapshot {
    request: BuildingRequest,
    terrain: Vec<TerrainTile>,
    upper_terrain: Vec<(SourcePlane, Vec<TerrainTile>)>,
    heights: Vec<Option<i32>>,
    occupancy: OccupancyGrid,
}

impl GenerationSnapshot {
    pub const fn request(&self) -> BuildingRequest {
        self.request
    }

    pub(crate) fn with_request(&self, request: BuildingRequest) -> Self {
        Self {
            request,
            terrain: self.terrain.clone(),
            upper_terrain: self.upper_terrain.clone(),
            heights: self.heights.clone(),
            occupancy: self.occupancy.clone(),
        }
    }

    pub const fn site(&self) -> RectSite {
        self.request.site
    }

    pub fn terrain_at(&self, tile: MapTile) -> Option<TerrainTile> {
        self.terrain_index(tile).map(|index| self.terrain[index])
    }

    pub fn terrain_at_plane(&self, tile: MapTile, plane: SourcePlane) -> Option<TerrainTile> {
        if plane == self.request.base_plane {
            return self.terrain_at(tile);
        }
        let index = self.terrain_index(tile)?;
        self.upper_terrain
            .iter()
            .find(|(p, _)| *p == plane)
            .map(|(_, tiles)| tiles[index])
    }

    pub fn height_at(&self, coord: HeightVertexCoord) -> Option<i32> {
        if coord.plane != self.request.base_plane {
            return None;
        }
        self.height_index(coord.corner)
            .and_then(|index| self.heights[index])
    }

    pub const fn occupancy(&self) -> &OccupancyGrid {
        &self.occupancy
    }

    pub fn missing_height_vertices(&self) -> Vec<HeightVertexCoord> {
        let site = self.request.site;
        let mut missing = Vec::new();
        for local_y in 0..=u32::from(site.height()) {
            for local_x in 0..=u32::from(site.width()) {
                let corner = offset_tile(site.origin(), local_x, local_y);
                let coord = HeightVertexCoord::new(corner, self.request.base_plane);
                if self.height_at(coord).is_none() {
                    missing.push(coord);
                }
            }
        }
        missing
    }

    pub fn blocked_tiles(&self) -> Vec<MapTile> {
        let site = self.request.site;
        let mut blocked = Vec::new();
        for local_y in 0..u32::from(site.height()) {
            for local_x in 0..u32::from(site.width()) {
                let tile = offset_tile(site.origin(), local_x, local_y);
                if self.occupancy.get(tile) == Some(OccupancyCell::Blocked) {
                    blocked.push(tile);
                }
            }
        }
        blocked
    }

    fn terrain_index(&self, tile: MapTile) -> Option<usize> {
        let site = self.request.site;
        if !site.contains(tile) {
            return None;
        }
        let local_x = usize::try_from(tile.x - site.origin().x).ok()?;
        let local_y = usize::try_from(tile.y - site.origin().y).ok()?;
        Some(local_y * usize::from(site.width()) + local_x)
    }

    fn height_index(&self, corner: MapTile) -> Option<usize> {
        let site = self.request.site;
        let local_x = corner.x.checked_sub(site.origin().x)?;
        let local_y = corner.y.checked_sub(site.origin().y)?;
        if local_x < 0
            || local_y < 0
            || local_x > i32::from(site.width())
            || local_y > i32::from(site.height())
        {
            return None;
        }
        let row_width = usize::from(site.width()) + 1;
        Some(usize::try_from(local_y).ok()? * row_width + usize::try_from(local_x).ok()?)
    }
}

pub fn capture_generation_snapshot(
    query: &impl MapQuery,
    request: BuildingRequest,
    occupied_footprints: impl IntoIterator<Item = OccupiedFootprint>,
) -> GenerationSnapshot {
    let site = request.site;
    let mut terrain = Vec::with_capacity(usize::from(site.width()) * usize::from(site.height()));
    for local_y in 0..u32::from(site.height()) {
        for local_x in 0..u32::from(site.width()) {
            let tile = offset_tile(site.origin(), local_x, local_y);
            terrain.push(query.terrain_at(MapPosition::new(tile, request.base_plane)));
        }
    }

    let mut upper_terrain = Vec::new();
    for offset in 1..request.floors {
        if let Some(plane) =
            SourcePlane::new(request.base_plane.index().get().saturating_add(offset))
        {
            let mut tiles =
                Vec::with_capacity(usize::from(site.width()) * usize::from(site.height()));
            for local_y in 0..u32::from(site.height()) {
                for local_x in 0..u32::from(site.width()) {
                    let tile = offset_tile(site.origin(), local_x, local_y);
                    tiles.push(query.terrain_at(MapPosition::new(tile, plane)));
                }
            }
            upper_terrain.push((plane, tiles));
        }
    }

    let height_width = usize::from(site.width()) + 1;
    let height_height = usize::from(site.height()) + 1;
    let mut heights = Vec::with_capacity(height_width * height_height);
    for local_y in 0..=u32::from(site.height()) {
        for local_x in 0..=u32::from(site.width()) {
            let corner = offset_tile(site.origin(), local_x, local_y);
            heights.push(query.height_at(HeightVertexCoord::new(corner, request.base_plane)));
        }
    }

    let mut occupancy = OccupancyGrid::new(site);
    for footprint in occupied_footprints {
        if footprint.plane != request.base_plane {
            continue;
        }
        for local_y in 0..u32::from(footprint.height) {
            for local_x in 0..u32::from(footprint.width) {
                let tile = offset_tile(footprint.origin, local_x, local_y);
                if site.contains(tile) {
                    occupancy.set(tile, OccupancyCell::Blocked);
                }
            }
        }
    }

    GenerationSnapshot {
        request,
        terrain,
        upper_terrain,
        heights,
        occupancy,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildingDiagnostic {
    SnapshotRequestMismatch,
    MissingHeight {
        coord: HeightVertexCoord,
    },
    OccupiedTile {
        tile: MapTile,
    },
    DuplicateRoomId {
        room: RoomId,
    },
    RoomHasZeroArea {
        room: RoomId,
    },
    RoomOutsideSite {
        room: RoomId,
    },
    RoomOverlap {
        first: RoomId,
        second: RoomId,
    },
    EntranceOutsideSite {
        tile: MapTile,
    },
    EntranceNotOnBoundary {
        tile: MapTile,
    },
    ExteriorConnectionCount {
        count: usize,
    },
    UnknownConnectionRoom {
        room: RoomId,
    },
    DoorwayOutsideSite {
        doorway: MapTile,
    },
    ExteriorDoorwayOutsideRoom {
        room: RoomId,
        doorway: MapTile,
    },
    DoorwayNotBetweenRooms {
        from: RoomId,
        to: RoomId,
        doorway: MapTile,
    },
    UnreachableRoom {
        room: RoomId,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BuildingValidationReport {
    diagnostics: Vec<BuildingDiagnostic>,
}

impl BuildingValidationReport {
    pub fn diagnostics(&self) -> &[BuildingDiagnostic] {
        &self.diagnostics
    }

    pub fn is_valid(&self) -> bool {
        self.diagnostics.is_empty()
    }
}

pub fn validate_building_plan(
    plan: &BuildingPlan,
    snapshot: &GenerationSnapshot,
) -> BuildingValidationReport {
    let mut diagnostics = Vec::new();
    if plan.request != snapshot.request {
        diagnostics.push(BuildingDiagnostic::SnapshotRequestMismatch);
    }

    diagnostics.extend(
        snapshot
            .missing_height_vertices()
            .into_iter()
            .map(|coord| BuildingDiagnostic::MissingHeight { coord }),
    );
    diagnostics.extend(
        snapshot
            .blocked_tiles()
            .into_iter()
            .map(|tile| BuildingDiagnostic::OccupiedTile { tile }),
    );

    let site = plan.request.site;
    let mut rooms_by_id = BTreeMap::<RoomId, &RoomSpec>::new();
    for room in &plan.rooms {
        if rooms_by_id.insert(room.id, room).is_some() {
            diagnostics.push(BuildingDiagnostic::DuplicateRoomId { room: room.id });
        }
        if room.rect.width == 0 || room.rect.height == 0 {
            diagnostics.push(BuildingDiagnostic::RoomHasZeroArea { room: room.id });
        } else if !rect_inside_site(site, room.rect) {
            diagnostics.push(BuildingDiagnostic::RoomOutsideSite { room: room.id });
        }
    }

    for first_index in 0..plan.rooms.len() {
        for second_index in (first_index + 1)..plan.rooms.len() {
            let first = &plan.rooms[first_index];
            let second = &plan.rooms[second_index];
            if rects_overlap(first.rect, second.rect) {
                diagnostics.push(BuildingDiagnostic::RoomOverlap {
                    first: first.id,
                    second: second.id,
                });
            }
        }
    }

    if !site.contains(plan.entrance) {
        diagnostics.push(BuildingDiagnostic::EntranceOutsideSite {
            tile: plan.entrance,
        });
    } else if !is_site_boundary(site, plan.entrance) {
        diagnostics.push(BuildingDiagnostic::EntranceNotOnBoundary {
            tile: plan.entrance,
        });
    }

    let exterior_count = plan
        .connections
        .iter()
        .filter(|connection| connection.from.is_none())
        .count();
    if exterior_count != 1 {
        diagnostics.push(BuildingDiagnostic::ExteriorConnectionCount {
            count: exterior_count,
        });
    }

    for connection in &plan.connections {
        let to_room = rooms_by_id.get(&connection.to).copied();
        if to_room.is_none() {
            diagnostics.push(BuildingDiagnostic::UnknownConnectionRoom {
                room: connection.to,
            });
        }
        let from_room = connection.from.and_then(|room| {
            let resolved = rooms_by_id.get(&room).copied();
            if resolved.is_none() {
                diagnostics.push(BuildingDiagnostic::UnknownConnectionRoom { room });
            }
            resolved
        });

        if !site.contains(connection.doorway) {
            diagnostics.push(BuildingDiagnostic::DoorwayOutsideSite {
                doorway: connection.doorway,
            });
            continue;
        }

        match (connection.from, from_room, to_room) {
            (None, _, Some(to_room)) => {
                if !to_room.rect.contains(connection.doorway) {
                    diagnostics.push(BuildingDiagnostic::ExteriorDoorwayOutsideRoom {
                        room: to_room.id,
                        doorway: connection.doorway,
                    });
                }
            }
            (Some(from), Some(from_room), Some(to_room))
                if !doorway_bridges_rooms(connection.doorway, from_room.rect, to_room.rect) =>
            {
                diagnostics.push(BuildingDiagnostic::DoorwayNotBetweenRooms {
                    from,
                    to: to_room.id,
                    doorway: connection.doorway,
                });
            }
            _ => {}
        }
    }

    let reachable = reachable_rooms(plan, &rooms_by_id);
    for room in rooms_by_id.keys().copied() {
        if !reachable.contains(&room) {
            diagnostics.push(BuildingDiagnostic::UnreachableRoom { room });
        }
    }

    BuildingValidationReport { diagnostics }
}

fn reachable_rooms(
    plan: &BuildingPlan,
    rooms_by_id: &BTreeMap<RoomId, &RoomSpec>,
) -> BTreeSet<RoomId> {
    let mut adjacency = BTreeMap::<RoomId, BTreeSet<RoomId>>::new();
    for room in rooms_by_id.keys().copied() {
        adjacency.entry(room).or_default();
    }

    let mut roots = BTreeSet::new();
    for connection in &plan.connections {
        if !rooms_by_id.contains_key(&connection.to) {
            continue;
        }
        match connection.from {
            None => {
                roots.insert(connection.to);
            }
            Some(from) if rooms_by_id.contains_key(&from) => {
                adjacency.entry(from).or_default().insert(connection.to);
                adjacency.entry(connection.to).or_default().insert(from);
            }
            Some(_) => {}
        }
    }

    let mut visited = BTreeSet::new();
    let mut queue: VecDeque<_> = roots.into_iter().collect();
    while let Some(room) = queue.pop_front() {
        if !visited.insert(room) {
            continue;
        }
        if let Some(neighbors) = adjacency.get(&room) {
            for neighbor in neighbors {
                if !visited.contains(neighbor) {
                    queue.push_back(*neighbor);
                }
            }
        }
    }
    visited
}

fn rect_inside_site(site: RectSite, rect: RoomRect) -> bool {
    if rect.width == 0 || rect.height == 0 || !site.contains(rect.origin) {
        return false;
    }
    let Some(far_x) = rect
        .origin
        .x
        .checked_add(i32::from(rect.width).saturating_sub(1))
    else {
        return false;
    };
    let Some(far_y) = rect
        .origin
        .y
        .checked_add(i32::from(rect.height).saturating_sub(1))
    else {
        return false;
    };
    site.contains(MapTile::new(far_x, far_y))
}

fn rects_overlap(first: RoomRect, second: RoomRect) -> bool {
    if first.width == 0 || first.height == 0 || second.width == 0 || second.height == 0 {
        return false;
    }
    let first_left = i64::from(first.origin.x);
    let first_bottom = i64::from(first.origin.y);
    let first_right = first_left + i64::from(first.width);
    let first_top = first_bottom + i64::from(first.height);
    let second_left = i64::from(second.origin.x);
    let second_bottom = i64::from(second.origin.y);
    let second_right = second_left + i64::from(second.width);
    let second_top = second_bottom + i64::from(second.height);

    first_left < second_right
        && second_left < first_right
        && first_bottom < second_top
        && second_bottom < first_top
}

fn is_site_boundary(site: RectSite, tile: MapTile) -> bool {
    if !site.contains(tile) {
        return false;
    }
    let max_x = site.origin().x + i32::from(site.width()) - 1;
    let max_y = site.origin().y + i32::from(site.height()) - 1;
    tile.x == site.origin().x || tile.x == max_x || tile.y == site.origin().y || tile.y == max_y
}

fn doorway_bridges_rooms(doorway: MapTile, first: RoomRect, second: RoomRect) -> bool {
    (first.contains(doorway) && cardinal_neighbors(doorway).any(|tile| second.contains(tile)))
        || (second.contains(doorway)
            && cardinal_neighbors(doorway).any(|tile| first.contains(tile)))
}

fn cardinal_neighbors(tile: MapTile) -> impl Iterator<Item = MapTile> {
    [
        MapTile::new(tile.x, tile.y - 1),
        MapTile::new(tile.x + 1, tile.y),
        MapTile::new(tile.x, tile.y + 1),
        MapTile::new(tile.x - 1, tile.y),
    ]
    .into_iter()
}

fn offset_tile(origin: MapTile, local_x: u32, local_y: u32) -> MapTile {
    MapTile::new(
        origin.x + i32::try_from(local_x).unwrap_or(i32::MAX),
        origin.y + i32::try_from(local_y).unwrap_or(i32::MAX),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        BuildingArchetype, BuildingConstraints, BuildingStyle, MapDocument, plan_basic_house,
    };
    use osrs_core::ids::FloorUnderlayId;

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
        for local_y in 0..=u32::from(request.site.height()) {
            for local_x in 0..=u32::from(request.site.width()) {
                heights.push((
                    HeightVertexCoord::new(
                        offset_tile(request.site.origin(), local_x, local_y),
                        request.base_plane,
                    ),
                    -64,
                ));
            }
        }
        heights
    }

    fn loaded_document(request: BuildingRequest) -> MapDocument {
        MapDocument::from_parts([], heights_for(request), [])
    }

    #[test]
    fn snapshot_captures_terrain_heights_and_adapter_occupancy() {
        let request = request();
        let terrain = TerrainTile {
            underlay: Some(FloorUnderlayId::new(7)),
            ..TerrainTile::default()
        };
        let terrain_position = MapPosition::new(request.site.origin(), request.base_plane);
        let document =
            MapDocument::from_parts([(terrain_position, terrain)], heights_for(request), []);
        let blocked_origin = offset_tile(request.site.origin(), 2, 3);
        let footprint = OccupiedFootprint::new(blocked_origin, request.base_plane, 2, 1)
            .unwrap_or_else(|| unreachable!("test footprint is valid"));

        let snapshot = capture_generation_snapshot(&document, request, [footprint]);

        assert_eq!(snapshot.terrain_at(request.site.origin()), Some(terrain));
        assert_eq!(
            snapshot.height_at(HeightVertexCoord::new(
                request.site.origin(),
                request.base_plane
            )),
            Some(-64)
        );
        assert_eq!(
            snapshot.occupancy().get(blocked_origin),
            Some(OccupancyCell::Blocked)
        );
        assert_eq!(
            snapshot
                .occupancy()
                .get(offset_tile(request.site.origin(), 3, 3)),
            Some(OccupancyCell::Blocked)
        );
    }

    #[test]
    fn complete_basic_house_validates_against_clear_loaded_site() {
        let request = request();
        let document = loaded_document(request);
        let snapshot = capture_generation_snapshot(&document, request, []);
        let plan = plan_basic_house(request)
            .unwrap_or_else(|error| unreachable!("valid test plan: {error:?}"));

        assert!(validate_building_plan(&plan, &snapshot).is_valid());
    }

    #[test]
    fn missing_height_is_reported_with_exact_vertex() {
        let request = request();
        let missing =
            HeightVertexCoord::new(offset_tile(request.site.origin(), 4, 5), request.base_plane);
        let heights = heights_for(request)
            .into_iter()
            .filter(|(coord, _)| *coord != missing)
            .collect::<Vec<_>>();
        let document = MapDocument::from_parts([], heights, []);
        let snapshot = capture_generation_snapshot(&document, request, []);
        let plan = plan_basic_house(request)
            .unwrap_or_else(|error| unreachable!("valid test plan: {error:?}"));
        let report = validate_building_plan(&plan, &snapshot);

        assert!(
            report
                .diagnostics()
                .contains(&BuildingDiagnostic::MissingHeight { coord: missing })
        );
    }

    #[test]
    fn occupied_footprint_is_reported_in_stable_tile_order() {
        let request = request();
        let document = loaded_document(request);
        let footprint = OccupiedFootprint::new(
            offset_tile(request.site.origin(), 2, 2),
            request.base_plane,
            2,
            1,
        )
        .unwrap_or_else(|| unreachable!("test footprint is valid"));
        let snapshot = capture_generation_snapshot(&document, request, [footprint]);
        let plan = plan_basic_house(request)
            .unwrap_or_else(|error| unreachable!("valid test plan: {error:?}"));
        let report = validate_building_plan(&plan, &snapshot);

        assert_eq!(
            report.diagnostics().first(),
            Some(&BuildingDiagnostic::OccupiedTile {
                tile: offset_tile(request.site.origin(), 2, 2),
            })
        );
    }

    #[test]
    fn disconnected_room_is_found_by_bfs() {
        let request = request();
        let document = loaded_document(request);
        let snapshot = capture_generation_snapshot(&document, request, []);
        let mut plan = plan_basic_house(request)
            .unwrap_or_else(|error| unreachable!("valid test plan: {error:?}"));
        let disconnected = plan
            .rooms
            .iter()
            .find(|room| room.id != RoomId::new(0))
            .map(|room| room.id)
            .unwrap_or_else(|| unreachable!("house has non-hall rooms"));
        plan.connections.retain(|connection| {
            connection.from != Some(disconnected) && connection.to != disconnected
        });
        let report = validate_building_plan(&plan, &snapshot);

        assert!(
            report
                .diagnostics()
                .contains(&BuildingDiagnostic::UnreachableRoom { room: disconnected })
        );
    }

    #[test]
    fn overlapping_rooms_are_rejected() {
        let request = request();
        let document = loaded_document(request);
        let snapshot = capture_generation_snapshot(&document, request, []);
        let mut plan = plan_basic_house(request)
            .unwrap_or_else(|error| unreachable!("valid test plan: {error:?}"));
        let first = plan.rooms[0].id;
        let second = plan.rooms[1].id;
        plan.rooms[1].rect = plan.rooms[0].rect;
        let report = validate_building_plan(&plan, &snapshot);

        assert!(
            report
                .diagnostics()
                .contains(&BuildingDiagnostic::RoomOverlap { first, second })
        );
    }

    #[test]
    fn doorway_must_touch_both_connected_rooms() {
        let request = request();
        let document = loaded_document(request);
        let snapshot = capture_generation_snapshot(&document, request, []);
        let mut plan = plan_basic_house(request)
            .unwrap_or_else(|error| unreachable!("valid test plan: {error:?}"));
        let connection = plan
            .connections
            .iter_mut()
            .find(|connection| connection.from.is_some())
            .unwrap_or_else(|| unreachable!("house has internal connections"));
        let from = connection
            .from
            .unwrap_or_else(|| unreachable!("selected connection is internal"));
        let to = connection.to;
        connection.doorway = request.site.origin();
        let report = validate_building_plan(&plan, &snapshot);

        assert!(
            report
                .diagnostics()
                .contains(&BuildingDiagnostic::DoorwayNotBetweenRooms {
                    from,
                    to,
                    doorway: request.site.origin(),
                })
        );
    }
}
