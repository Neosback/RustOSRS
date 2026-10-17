//! Deterministic routing of archetype-level room relationships through BSP circulation.
//!
//! The BSP topology already guarantees that every semantic room reaches the corridor network.
//! This layer preserves the stronger building-program intent by computing one stable shortest
//! corridor path for every preferred room-to-room relationship.

use crate::{BspBuildingPlan, PortalKind, ProgramConnection, RoomId, RoomKind};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramRoute {
    pub connection: ProgramConnection,
    pub from_corridor: RoomId,
    pub to_corridor: RoomId,
    corridor_path: Vec<RoomId>,
}

impl ProgramRoute {
    pub fn corridor_path(&self) -> &[RoomId] {
        &self.corridor_path
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgramRoutingError {
    MissingRoomDoor { room: RoomId },
    AmbiguousRoomDoor { room: RoomId },
    UnknownCorridor { room: RoomId },
    NoCorridorRoute { from: RoomId, to: RoomId },
}

pub fn route_program_connections(
    building: &BspBuildingPlan,
) -> Result<Vec<ProgramRoute>, ProgramRoutingError> {
    let corridor_ids = building
        .plan
        .rooms
        .iter()
        .filter(|room| room.kind == RoomKind::Hallway)
        .map(|room| room.id)
        .collect::<BTreeSet<_>>();

    let mut room_corridor = BTreeMap::<RoomId, RoomId>::new();
    for role in building.room_roles() {
        let mut attached = building
            .plan
            .connections
            .iter()
            .filter_map(|connection| {
                if connection.kind != PortalKind::Door {
                    return None;
                }
                match connection.from {
                    Some(from) if from == role.room && corridor_ids.contains(&connection.to) => {
                        Some(connection.to)
                    }
                    Some(from) if connection.to == role.room && corridor_ids.contains(&from) => {
                        Some(from)
                    }
                    _ => None,
                }
            })
            .collect::<Vec<_>>();
        attached.sort();
        attached.dedup();
        match attached.as_slice() {
            [] => return Err(ProgramRoutingError::MissingRoomDoor { room: role.room }),
            [corridor] => {
                room_corridor.insert(role.room, *corridor);
            }
            _ => return Err(ProgramRoutingError::AmbiguousRoomDoor { room: role.room }),
        }
    }

    let mut adjacency = BTreeMap::<RoomId, BTreeSet<RoomId>>::new();
    for corridor in &corridor_ids {
        adjacency.entry(*corridor).or_default();
    }
    for connection in &building.plan.connections {
        if connection.kind != PortalKind::Open {
            continue;
        }
        let Some(from) = connection.from else {
            continue;
        };
        if !corridor_ids.contains(&from) || !corridor_ids.contains(&connection.to) {
            continue;
        }
        adjacency.entry(from).or_default().insert(connection.to);
        adjacency.entry(connection.to).or_default().insert(from);
    }

    let mut routes = Vec::with_capacity(building.preferred_connections().len());
    for &connection in building.preferred_connections() {
        let Some(&from_corridor) = room_corridor.get(&connection.first) else {
            return Err(ProgramRoutingError::MissingRoomDoor {
                room: connection.first,
            });
        };
        let Some(&to_corridor) = room_corridor.get(&connection.second) else {
            return Err(ProgramRoutingError::MissingRoomDoor {
                room: connection.second,
            });
        };
        if !adjacency.contains_key(&from_corridor) {
            return Err(ProgramRoutingError::UnknownCorridor {
                room: from_corridor,
            });
        }
        if !adjacency.contains_key(&to_corridor) {
            return Err(ProgramRoutingError::UnknownCorridor { room: to_corridor });
        }
        let Some(corridor_path) = shortest_path(&adjacency, from_corridor, to_corridor) else {
            return Err(ProgramRoutingError::NoCorridorRoute {
                from: from_corridor,
                to: to_corridor,
            });
        };
        routes.push(ProgramRoute {
            connection,
            from_corridor,
            to_corridor,
            corridor_path,
        });
    }

    routes.sort_by_key(|route| (route.connection.first, route.connection.second));
    Ok(routes)
}

fn shortest_path(
    adjacency: &BTreeMap<RoomId, BTreeSet<RoomId>>,
    start: RoomId,
    goal: RoomId,
) -> Option<Vec<RoomId>> {
    if start == goal {
        return Some(vec![start]);
    }

    let mut previous = BTreeMap::<RoomId, RoomId>::new();
    let mut visited = BTreeSet::from([start]);
    let mut queue = VecDeque::from([start]);

    while let Some(current) = queue.pop_front() {
        let neighbors = adjacency.get(&current)?;
        for &neighbor in neighbors {
            if !visited.insert(neighbor) {
                continue;
            }
            previous.insert(neighbor, current);
            if neighbor == goal {
                return rebuild_path(start, goal, &previous);
            }
            queue.push_back(neighbor);
        }
    }
    None
}

fn rebuild_path(
    start: RoomId,
    goal: RoomId,
    previous: &BTreeMap<RoomId, RoomId>,
) -> Option<Vec<RoomId>> {
    let mut reversed = vec![goal];
    let mut current = goal;
    while current != start {
        current = *previous.get(&current)?;
        reversed.push(current);
    }
    reversed.reverse();
    Some(reversed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        BuildingArchetype, BuildingConstraints, BuildingRequest, BuildingStyle, RectSite,
        build_bsp_building_plan, building_program, pack_building_program,
    };
    use osrs_core::coords::{MapTile, SourcePlane};

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

    fn building(archetype: BuildingArchetype, seed: u64) -> BspBuildingPlan {
        let request = request(archetype, seed);
        let program = building_program(archetype);
        let layout = pack_building_program(&program, request.site, request.seed)
            .unwrap_or_else(|error| unreachable!("valid BSP layout: {error:?}"));
        build_bsp_building_plan(request, &program, &layout)
            .unwrap_or_else(|error| unreachable!("valid BSP topology: {error:?}"))
    }

    #[test]
    fn every_archetype_preferred_connection_receives_a_route() {
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
            let building = building(archetype, 812);
            let routes = route_program_connections(&building)
                .unwrap_or_else(|error| unreachable!("valid routing: {error:?}"));
            assert_eq!(routes.len(), building.preferred_connections().len());
            assert!(routes.iter().all(|route| !route.corridor_path().is_empty()));
        }
    }

    #[test]
    fn every_route_step_follows_an_open_corridor_connection() {
        let building = building(BuildingArchetype::Castle, 91);
        let routes = route_program_connections(&building)
            .unwrap_or_else(|error| unreachable!("valid routing: {error:?}"));
        let open_edges = building
            .plan
            .connections
            .iter()
            .filter(|connection| connection.kind == PortalKind::Open)
            .filter_map(|connection| connection.from.map(|from| (from, connection.to)))
            .flat_map(|(first, second)| [(first, second), (second, first)])
            .collect::<BTreeSet<_>>();

        for route in routes {
            assert_eq!(route.corridor_path().first(), Some(&route.from_corridor));
            assert_eq!(route.corridor_path().last(), Some(&route.to_corridor));
            for pair in route.corridor_path().windows(2) {
                assert!(open_edges.contains(&(pair[0], pair[1])));
            }
        }
    }

    #[test]
    fn same_topology_produces_identical_program_routes() {
        let building = building(BuildingArchetype::Tavern, 555);
        let first = route_program_connections(&building)
            .unwrap_or_else(|error| unreachable!("valid routing: {error:?}"));
        let second = route_program_connections(&building)
            .unwrap_or_else(|error| unreachable!("valid routing: {error:?}"));
        assert_eq!(first, second);
    }

    #[test]
    fn rooms_attached_to_same_corridor_have_single_node_route() {
        let building = building(BuildingArchetype::House, 42);
        let routes = route_program_connections(&building)
            .unwrap_or_else(|error| unreachable!("valid routing: {error:?}"));
        for route in routes {
            if route.from_corridor == route.to_corridor {
                assert_eq!(route.corridor_path(), [route.from_corridor]);
            }
        }
    }
}
