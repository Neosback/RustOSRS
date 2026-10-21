//! Conversion from archetype BSP layouts into connected semantic building plans.
//!
//! BSP owns macro geometry. This layer turns that geometry into rooms, corridor rooms, explicit
//! open/door portals, and one deterministic exterior entrance while retaining the exact archetype
//! room roles for later furnishing and WFC refinement.

mod authored;
mod compact;
mod single;
use crate::{
    BspLayout, BuildingPlan, BuildingProgram, BuildingRequest, BuildingRoomRole, PortalKind,
    ProgramConnection, RoomConnection, RoomId, RoomKind, RoomRect, RoomSpec,
};
pub use authored::{BuildingSketch, plan_authored_rooms};
use osrs_core::coords::MapTile;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ProgramRoomRole {
    pub room: RoomId,
    pub role: BuildingRoomRole,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BspBuildingPlan {
    pub plan: BuildingPlan,
    room_roles: Vec<ProgramRoomRole>,
    preferred_connections: Vec<ProgramConnection>,
}

impl BspBuildingPlan {
    pub fn room_roles(&self) -> &[ProgramRoomRole] {
        &self.room_roles
    }

    pub fn preferred_connections(&self) -> &[ProgramConnection] {
        &self.preferred_connections
    }

    pub fn role_for(&self, room: RoomId) -> Option<BuildingRoomRole> {
        self.room_roles
            .iter()
            .find(|entry| entry.room == room)
            .map(|entry| entry.role)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BspTopologyError {
    ArchetypeMismatch,
    SiteMismatch,
    NoCorridors,
    DisconnectedRooms,
    MissingProgramRoom { room: RoomId },
    RoomIdOverflow,
    RoomNotAdjacentToCorridor { room: RoomId },
    DisconnectedCorridor { corridor: RoomId },
    NoExteriorEntrance,
    CoordinateOverflow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct EntranceCandidate {
    doorway: MapTile,
    entrance: MapTile,
    corridor: RoomId,
}

/// The plan for a corridor-free layout: a door tree over the rooms that touch, one entrance.
pub fn build_compact_building_plan(
    request: BuildingRequest,
    program: &BuildingProgram,
    layout: &crate::CompactLayout,
) -> Result<BspBuildingPlan, BspTopologyError> {
    compact::build(request, program, layout)
}

pub fn build_bsp_building_plan(
    request: BuildingRequest,
    program: &BuildingProgram,
    layout: &BspLayout,
) -> Result<BspBuildingPlan, BspTopologyError> {
    if program.archetype != request.archetype {
        return Err(BspTopologyError::ArchetypeMismatch);
    }
    if layout.site != request.site {
        return Err(BspTopologyError::SiteMismatch);
    }
    if program.rooms().len() == 1 {
        return single::build(request, program, layout);
    }
    if layout.corridors().is_empty() {
        return Err(BspTopologyError::NoCorridors);
    }

    let mut rooms = Vec::new();
    let mut room_roles = Vec::new();
    for program_room in program.rooms() {
        let Some(placement) = layout.room(program_room.id) else {
            return Err(BspTopologyError::MissingProgramRoom {
                room: program_room.id,
            });
        };
        rooms.push(RoomSpec {
            id: placement.room,
            kind: generic_room_kind(program_room.role),
            rect: placement.rect,
        });
        room_roles.push(ProgramRoomRole {
            room: program_room.id,
            role: program_room.role,
        });
    }

    let mut next_id = program
        .rooms()
        .iter()
        .map(|room| room.id.get())
        .max()
        .unwrap_or(0)
        .checked_add(1)
        .ok_or(BspTopologyError::RoomIdOverflow)?;
    let mut corridor_rooms = Vec::with_capacity(layout.corridors().len());
    for corridor in layout.corridors() {
        let id = RoomId::new(next_id);
        next_id = next_id
            .checked_add(1)
            .ok_or(BspTopologyError::RoomIdOverflow)?;
        let room = RoomSpec {
            id,
            kind: RoomKind::Hallway,
            rect: corridor.rect,
        };
        rooms.push(room);
        corridor_rooms.push(room);
    }

    let mut connections = Vec::new();
    for program_room in program.rooms() {
        let room = rooms.iter().find(|room| room.id == program_room.id).ok_or(
            BspTopologyError::MissingProgramRoom {
                room: program_room.id,
            },
        )?;
        let mut candidates = corridor_rooms
            .iter()
            .filter_map(|corridor| {
                adjacent_portal(room.rect, corridor.rect).map(|doorway| (corridor.id, doorway))
            })
            .collect::<Vec<_>>();
        candidates.sort_by_key(|(corridor, doorway)| (*doorway, *corridor));
        let Some((corridor, doorway)) = candidates.first().copied() else {
            return Err(BspTopologyError::RoomNotAdjacentToCorridor {
                room: program_room.id,
            });
        };
        connections.push(RoomConnection {
            from: Some(program_room.id),
            to: corridor,
            doorway,
            kind: PortalKind::Door,
        });
    }

    for first_index in 0..corridor_rooms.len() {
        for second_index in (first_index + 1)..corridor_rooms.len() {
            let first = corridor_rooms[first_index];
            let second = corridor_rooms[second_index];
            if let Some(doorway) = adjacent_portal(first.rect, second.rect) {
                connections.push(RoomConnection {
                    from: Some(first.id),
                    to: second.id,
                    doorway,
                    kind: PortalKind::Open,
                });
            }
        }
    }

    ensure_corridor_connectivity(&corridor_rooms, &connections)?;

    let mut entrance_candidates = Vec::new();
    for corridor in &corridor_rooms {
        for tile in rect_tiles(corridor.rect)? {
            for neighbor in cardinal_neighbors(tile).into_iter().flatten() {
                if request.site.contains(neighbor) && is_site_boundary(request.site, neighbor) {
                    entrance_candidates.push(EntranceCandidate {
                        doorway: tile,
                        entrance: neighbor,
                        corridor: corridor.id,
                    });
                }
            }
        }
    }
    entrance_candidates.sort();
    entrance_candidates.dedup();
    if entrance_candidates.is_empty() {
        return Err(BspTopologyError::NoExteriorEntrance);
    }
    let entrance_index = deterministic_index(request.seed, entrance_candidates.len());
    let entrance = entrance_candidates[entrance_index];
    connections.push(RoomConnection {
        from: None,
        to: entrance.corridor,
        doorway: entrance.doorway,
        kind: PortalKind::Door,
    });

    connections.sort_by_key(|connection| {
        (
            connection.from.map(RoomId::get),
            connection.to.get(),
            connection.doorway,
            portal_order(connection.kind),
        )
    });
    room_roles.sort_by_key(|entry| entry.room);

    Ok(BspBuildingPlan {
        plan: BuildingPlan {
            request,
            rooms,
            connections,
            entrance: entrance.entrance,
        },
        room_roles,
        preferred_connections: program.connections().to_vec(),
    })
}

fn generic_room_kind(role: BuildingRoomRole) -> RoomKind {
    match role {
        BuildingRoomRole::Living
        | BuildingRoomRole::SalesFloor
        | BuildingRoomRole::CommonRoom
        | BuildingRoomRole::Lobby
        | BuildingRoomRole::Nave
        | BuildingRoomRole::GreatHall => RoomKind::Living,
        BuildingRoomRole::Kitchen | BuildingRoomRole::Bar => RoomKind::Kitchen,
        BuildingRoomRole::Bedroom | BuildingRoomRole::GuestRoom | BuildingRoomRole::Cell => {
            RoomKind::Bedroom
        }
        BuildingRoomRole::Bathroom
        | BuildingRoomRole::Utility
        | BuildingRoomRole::Storage
        | BuildingRoomRole::Office
        | BuildingRoomRole::Workshop
        | BuildingRoomRole::Vault
        | BuildingRoomRole::Security
        | BuildingRoomRole::Altar
        | BuildingRoomRole::Vestry
        | BuildingRoomRole::Loading
        | BuildingRoomRole::Armory
        | BuildingRoomRole::GuardRoom => RoomKind::Utility,
    }
}

fn ensure_corridor_connectivity(
    corridors: &[RoomSpec],
    connections: &[RoomConnection],
) -> Result<(), BspTopologyError> {
    let corridor_ids = corridors
        .iter()
        .map(|room| room.id)
        .collect::<BTreeSet<_>>();
    let Some(start) = corridor_ids.iter().next().copied() else {
        return Err(BspTopologyError::NoCorridors);
    };
    let mut adjacency = BTreeMap::<RoomId, BTreeSet<RoomId>>::new();
    for id in &corridor_ids {
        adjacency.entry(*id).or_default();
    }
    for connection in connections {
        let Some(from) = connection.from else {
            continue;
        };
        if connection.kind != PortalKind::Open
            || !corridor_ids.contains(&from)
            || !corridor_ids.contains(&connection.to)
        {
            continue;
        }
        adjacency.entry(from).or_default().insert(connection.to);
        adjacency.entry(connection.to).or_default().insert(from);
    }

    let mut visited = BTreeSet::new();
    let mut queue = VecDeque::from([start]);
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

    for corridor in corridor_ids {
        if !visited.contains(&corridor) {
            return Err(BspTopologyError::DisconnectedCorridor { corridor });
        }
    }
    Ok(())
}

fn adjacent_portal(first: RoomRect, second: RoomRect) -> Option<MapTile> {
    for local_y in 0..first.height {
        for local_x in 0..first.width {
            let tile = MapTile::new(
                first.origin.x.checked_add(i32::from(local_x))?,
                first.origin.y.checked_add(i32::from(local_y))?,
            );
            if cardinal_neighbors(tile)
                .into_iter()
                .flatten()
                .any(|neighbor| second.contains(neighbor))
            {
                return Some(tile);
            }
        }
    }
    None
}

fn rect_tiles(rect: RoomRect) -> Result<Vec<MapTile>, BspTopologyError> {
    let mut tiles = Vec::with_capacity(usize::from(rect.width) * usize::from(rect.height));
    for local_y in 0..rect.height {
        for local_x in 0..rect.width {
            let x = rect
                .origin
                .x
                .checked_add(i32::from(local_x))
                .ok_or(BspTopologyError::CoordinateOverflow)?;
            let y = rect
                .origin
                .y
                .checked_add(i32::from(local_y))
                .ok_or(BspTopologyError::CoordinateOverflow)?;
            tiles.push(MapTile::new(x, y));
        }
    }
    Ok(tiles)
}

fn cardinal_neighbors(tile: MapTile) -> [Option<MapTile>; 4] {
    [
        tile.y.checked_add(1).map(|y| MapTile::new(tile.x, y)),
        tile.x.checked_add(1).map(|x| MapTile::new(x, tile.y)),
        tile.y.checked_sub(1).map(|y| MapTile::new(tile.x, y)),
        tile.x.checked_sub(1).map(|x| MapTile::new(x, tile.y)),
    ]
}

fn is_site_boundary(site: crate::RectSite, tile: MapTile) -> bool {
    let max_x = site.origin().x + i32::from(site.width()) - 1;
    let max_y = site.origin().y + i32::from(site.height()) - 1;
    tile.x == site.origin().x || tile.x == max_x || tile.y == site.origin().y || tile.y == max_y
}

const fn portal_order(kind: PortalKind) -> u8 {
    match kind {
        PortalKind::Open => 0,
        PortalKind::Door => 1,
    }
}

fn deterministic_index(seed: u64, len: usize) -> usize {
    debug_assert!(len > 0);
    let mut value = seed.wrapping_add(0x9e37_79b9_7f4a_7c15);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^= value >> 31;
    usize::try_from(value % u64::try_from(len).unwrap_or(u64::MAX)).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        BuildingArchetype, BuildingConstraints, BuildingStyle, HeightVertexCoord, MapDocument,
        RectSite, building_program, capture_generation_snapshot, derive_building_architecture,
        pack_building_program, validate_building_plan,
    };
    use osrs_core::coords::SourcePlane;

    fn plane() -> SourcePlane {
        SourcePlane::new(0).unwrap_or_else(|| unreachable!("plane zero is valid"))
    }

    fn request(archetype: BuildingArchetype, seed: u64) -> BuildingRequest {
        BuildingRequest {
            seed,
            site: RectSite::new(MapTile::new(3200, 3200), 28, 22)
                .unwrap_or_else(|| unreachable!("test site is valid")),
            base_plane: plane(),
            floors: 1,
            archetype,
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

    #[test]
    fn every_archetype_bsp_layout_becomes_a_valid_connected_plan() {
        for archetype in [
            BuildingArchetype::House,
            BuildingArchetype::Shop,
            BuildingArchetype::Tavern,
            BuildingArchetype::Bank,
            BuildingArchetype::Church,
            BuildingArchetype::Warehouse,
            BuildingArchetype::Castle,
            BuildingArchetype::Dungeon,
        ] {
            let request = request(archetype, 41);
            let program = building_program(archetype);
            let layout = pack_building_program(&program, request.site, request.seed)
                .unwrap_or_else(|error| unreachable!("valid BSP layout: {error:?}"));
            let generated = build_bsp_building_plan(request, &program, &layout)
                .unwrap_or_else(|error| unreachable!("valid BSP topology: {error:?}"));
            let document = MapDocument::from_parts([], heights_for(request), []);
            let snapshot = capture_generation_snapshot(&document, request, []);
            let report = validate_building_plan(&generated.plan, &snapshot);

            assert!(
                report.is_valid(),
                "{archetype:?}: {:?}",
                report.diagnostics()
            );
            assert_eq!(
                generated
                    .plan
                    .connections
                    .iter()
                    .filter(|connection| connection.from.is_none())
                    .count(),
                1
            );
        }
    }

    fn compact_plan(
        archetype: BuildingArchetype,
        rooms: usize,
        seed: u64,
    ) -> (BuildingRequest, BspBuildingPlan) {
        let request = request(archetype, seed);
        let program = building_program(archetype).limited_to(rooms);
        let layout = crate::pack_compact_program(&program, request.site, seed)
            .unwrap_or_else(|error| unreachable!("valid compact layout: {error:?}"));
        let plan = build_compact_building_plan(request, &program, &layout)
            .unwrap_or_else(|error| unreachable!("valid compact topology: {error:?}"));
        (request, plan)
    }

    #[test]
    fn compact_plans_validate_and_join_the_rooms_with_a_door_tree() {
        for archetype in [
            BuildingArchetype::House,
            BuildingArchetype::Tavern,
            BuildingArchetype::Church,
            BuildingArchetype::Castle,
        ] {
            for rooms in 1..=5 {
                for seed in 0..40 {
                    let (request, generated) = compact_plan(archetype, rooms, seed);
                    let document = MapDocument::from_parts([], heights_for(request), []);
                    let snapshot = capture_generation_snapshot(&document, request, []);
                    let report = validate_building_plan(&generated.plan, &snapshot);
                    assert!(
                        report.is_valid(),
                        "{archetype:?} {rooms} rooms #{seed}: {:?}",
                        report.diagnostics()
                    );
                    // A tree: one door fewer than rooms between them, plus the way in.
                    let inside = generated
                        .plan
                        .connections
                        .iter()
                        .filter(|c| c.from.is_some())
                        .count();
                    assert_eq!(inside, rooms - 1, "{archetype:?} {rooms} rooms #{seed}");
                    assert!(
                        generated
                            .plan
                            .connections
                            .iter()
                            .all(|c| c.kind == PortalKind::Door)
                    );
                    derive_building_architecture(&generated.plan)
                        .unwrap_or_else(|e| panic!("{archetype:?} #{seed}: {e:?}"));
                }
            }
        }
    }

    #[test]
    fn the_way_in_is_on_the_entrance_room_when_it_reaches_the_outside() {
        let mut on_entrance_room = 0;
        let mut total = 0;
        for seed in 0..60 {
            let (_, generated) = compact_plan(BuildingArchetype::House, 3, seed);
            let exterior = generated
                .plan
                .connections
                .iter()
                .find(|c| c.from.is_none())
                .unwrap_or_else(|| unreachable!("every plan has a way in"));
            total += 1;
            if exterior.to == RoomId::new(0) {
                on_entrance_room += 1;
            }
        }
        assert!(
            on_entrance_room * 10 >= total * 9,
            "{on_entrance_room}/{total}"
        );
    }

    #[test]
    fn no_door_stands_in_a_corner_of_a_wall_it_could_avoid() {
        for seed in 0..60 {
            let (_, generated) = compact_plan(BuildingArchetype::Tavern, 4, seed);
            for connection in generated
                .plan
                .connections
                .iter()
                .filter(|c| c.from.is_some())
            {
                let from = generated
                    .plan
                    .rooms
                    .iter()
                    .find(|r| Some(r.id) == connection.from)
                    .unwrap_or_else(|| unreachable!("known room"));
                let to = generated
                    .plan
                    .rooms
                    .iter()
                    .find(|r| r.id == connection.to)
                    .unwrap_or_else(|| unreachable!("known room"));
                let shared = rect_tiles(from.rect)
                    .unwrap_or_default()
                    .into_iter()
                    .filter(|t| {
                        cardinal_neighbors(*t)
                            .into_iter()
                            .flatten()
                            .any(|n| to.rect.contains(n))
                    })
                    .count();
                if shared >= 3 {
                    let d = connection.doorway;
                    let r = from.rect;
                    let corner_x = d.x == r.origin.x || d.x == r.origin.x + i32::from(r.width) - 1;
                    let corner_y = d.y == r.origin.y || d.y == r.origin.y + i32::from(r.height) - 1;
                    // A door at a room's corner would need the neighbour's wall to continue past it.
                    assert!(
                        !(corner_x && corner_y),
                        "seed {seed}: door {d:?} in a corner of {r:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn corridor_to_corridor_portals_are_open_architectural_edges() {
        let request = request(BuildingArchetype::Castle, 71);
        let program = building_program(request.archetype);
        let layout = pack_building_program(&program, request.site, request.seed)
            .unwrap_or_else(|error| unreachable!("valid BSP layout: {error:?}"));
        let generated = build_bsp_building_plan(request, &program, &layout)
            .unwrap_or_else(|error| unreachable!("valid BSP topology: {error:?}"));
        let architecture = derive_building_architecture(&generated.plan)
            .unwrap_or_else(|error| unreachable!("valid architecture: {error:?}"));

        assert!(generated.plan.connections.iter().any(|connection| {
            connection.from.is_some() && connection.kind == PortalKind::Open
        }));
        assert!(
            architecture
                .edges()
                .iter()
                .any(|edge| edge.feature == crate::EdgeFeature::Open)
        );
    }

    #[test]
    fn exact_program_roles_survive_generic_room_kind_conversion() {
        let request = request(BuildingArchetype::Bank, 99);
        let program = building_program(request.archetype);
        let layout = pack_building_program(&program, request.site, request.seed)
            .unwrap_or_else(|error| unreachable!("valid BSP layout: {error:?}"));
        let generated = build_bsp_building_plan(request, &program, &layout)
            .unwrap_or_else(|error| unreachable!("valid BSP topology: {error:?}"));

        for room in program.rooms() {
            assert_eq!(generated.role_for(room.id), Some(room.role));
        }
    }

    #[test]
    fn same_request_program_and_layout_produce_identical_topology() {
        let request = request(BuildingArchetype::Tavern, 555);
        let program = building_program(request.archetype);
        let layout = pack_building_program(&program, request.site, request.seed)
            .unwrap_or_else(|error| unreachable!("valid BSP layout: {error:?}"));
        let first = build_bsp_building_plan(request, &program, &layout)
            .unwrap_or_else(|error| unreachable!("valid BSP topology: {error:?}"));
        let second = build_bsp_building_plan(request, &program, &layout)
            .unwrap_or_else(|error| unreachable!("valid BSP topology: {error:?}"));

        assert_eq!(first, second);
    }
}
