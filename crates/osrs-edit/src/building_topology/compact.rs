//! Doors for a corridor-free layout.
//!
//! Rooms that share a wall get a door between them only where one is needed to join the building:
//! a spanning tree over the rooms that touch, taking the program's preferred links first. That is
//! what the real map does: multi-room buildings average about 0.64 doors per room, a tree, not a
//! hallway network. One exterior door, on the entrance room when it reaches the outside.
use super::*;
use crate::CompactLayout;

/// One pair of touching rooms and the tiles of the first that border the second.
struct Contact {
    first: usize,
    second: usize,
    shared: Vec<MapTile>,
}

pub(super) fn build(
    request: BuildingRequest,
    program: &BuildingProgram,
    layout: &CompactLayout,
) -> Result<BspBuildingPlan, BspTopologyError> {
    if program.archetype != request.archetype {
        return Err(BspTopologyError::ArchetypeMismatch);
    }
    if layout.site != request.site {
        return Err(BspTopologyError::SiteMismatch);
    }
    let mut rooms = Vec::new();
    let mut room_roles = Vec::new();
    for program_room in program.rooms() {
        let placement =
            layout
                .room(program_room.id)
                .ok_or(BspTopologyError::MissingProgramRoom {
                    room: program_room.id,
                })?;
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

    let mut contacts = Vec::new();
    for first in 0..rooms.len() {
        for second in (first + 1)..rooms.len() {
            let shared: Vec<MapTile> = rect_tiles(rooms[first].rect)?
                .into_iter()
                .filter(|tile| {
                    cardinal_neighbors(*tile)
                        .into_iter()
                        .flatten()
                        .any(|n| rooms[second].rect.contains(n))
                })
                .collect();
            if !shared.is_empty() {
                contacts.push(Contact {
                    first,
                    second,
                    shared,
                });
            }
        }
    }

    // Preferred links first, then the rest; ties broken by the seed so the tree is not always the
    // same one.
    let preferred: BTreeSet<(RoomId, RoomId)> = program
        .connections()
        .iter()
        .map(|c| (c.first.min(c.second), c.first.max(c.second)))
        .collect();
    contacts.sort_by_key(|contact| {
        let (a, b) = (rooms[contact.first].id, rooms[contact.second].id);
        let key = (a.min(b), a.max(b));
        (
            !preferred.contains(&key),
            mixed_index(request.seed, u64::from(a.get()) << 16 | u64::from(b.get())),
        )
    });
    let mut parent: Vec<usize> = (0..rooms.len()).collect();
    fn find(parent: &mut [usize], mut x: usize) -> usize {
        while parent[x] != x {
            parent[x] = parent[parent[x]];
            x = parent[x];
        }
        x
    }
    let mut connections = Vec::new();
    let mut door_tiles = BTreeSet::new();
    for contact in &contacts {
        let (ra, rb) = (
            find(&mut parent, contact.first),
            find(&mut parent, contact.second),
        );
        if ra == rb {
            continue;
        }
        parent[ra] = rb;
        // Away from the ends of the shared wall, where a door would sit in a corner, and off a
        // tile that already has a door.
        let inner: &[MapTile] = if contact.shared.len() >= 3 {
            &contact.shared[1..contact.shared.len() - 1]
        } else {
            &contact.shared
        };
        let free: Vec<MapTile> = inner
            .iter()
            .copied()
            .filter(|tile| !door_tiles.contains(tile))
            .collect();
        let pool = if free.is_empty() { inner } else { &free };
        let doorway = pool[deterministic_index(
            request.seed ^ (contact.first as u64) << 8 ^ contact.second as u64,
            pool.len(),
        )];
        door_tiles.insert(doorway);
        for neighbor in cardinal_neighbors(doorway).into_iter().flatten() {
            if rooms[contact.second].rect.contains(neighbor) {
                door_tiles.insert(neighbor);
            }
        }
        connections.push(RoomConnection {
            from: Some(rooms[contact.first].id),
            to: rooms[contact.second].id,
            doorway,
            kind: PortalKind::Door,
        });
    }
    let root = find(&mut parent, 0);
    if (0..rooms.len()).any(|room| find(&mut parent, room) != root) {
        return Err(BspTopologyError::DisconnectedRooms);
    }

    let (entrance, doorway, to) = choose_entrance(request, program, &rooms, &door_tiles)?;
    connections.push(RoomConnection {
        from: None,
        to,
        doorway,
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
            entrance,
        },
        room_roles,
        preferred_connections: program.connections().to_vec(),
    })
}

/// The outside tile, the doorway tile in the room, and the room, for the one exterior door: on the
/// entrance room when it touches the site's outer ring, off the corners of its wall and away from
/// the doors inside.
fn choose_entrance(
    request: BuildingRequest,
    program: &BuildingProgram,
    rooms: &[RoomSpec],
    door_tiles: &BTreeSet<MapTile>,
) -> Result<(MapTile, MapTile, RoomId), BspTopologyError> {
    let mut all = BTreeSet::new();
    for room in rooms {
        all.extend(rect_tiles(room.rect)?);
    }
    let mut candidates = Vec::new();
    for room in rooms {
        let rect = room.rect;
        for tile in rect_tiles(rect)? {
            for outside in cardinal_neighbors(tile).into_iter().flatten() {
                if all.contains(&outside)
                    || !request.site.contains(outside)
                    || !is_site_boundary(request.site, outside)
                {
                    continue;
                }
                // The wall runs along y when the door faces east or west, else along x.
                let (along, start, length) = if outside.x != tile.x {
                    (tile.y, rect.origin.y, i32::from(rect.height))
                } else {
                    (tile.x, rect.origin.x, i32::from(rect.width))
                };
                let in_corner = length >= 3 && (along == start || along == start + length - 1);
                let near_door = door_tiles
                    .iter()
                    .any(|door| (door.x - tile.x).abs() + (door.y - tile.y).abs() <= 1);
                let wanted_room = room.id != program.entrance_room;
                candidates.push(((wanted_room, in_corner, near_door), outside, tile, room.id));
            }
        }
    }
    candidates.sort();
    let Some(best) = candidates.first().map(|c| c.0) else {
        return Err(BspTopologyError::NoExteriorEntrance);
    };
    let pool: Vec<_> = candidates.iter().filter(|c| c.0 == best).collect();
    let (_, outside, doorway, room) = *pool[deterministic_index(request.seed, pool.len())];
    Ok((outside, doorway, room))
}

fn mixed_index(seed: u64, salt: u64) -> u64 {
    let mut value = (seed ^ salt.rotate_left(17)).wrapping_add(0x9e37_79b9_7f4a_7c15);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}
