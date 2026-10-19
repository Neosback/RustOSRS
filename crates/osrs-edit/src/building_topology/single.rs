//! A one-room program needs an entrance, not a manufactured corridor room.
use super::*;

pub(super) fn build(
    request: BuildingRequest,
    program: &BuildingProgram,
    layout: &BspLayout,
) -> Result<BspBuildingPlan, BspTopologyError> {
    let room = program
        .rooms()
        .first()
        .ok_or(BspTopologyError::NoExteriorEntrance)?;
    let placement = layout
        .room(room.id)
        .ok_or(BspTopologyError::MissingProgramRoom { room: room.id })?;
    let doorway = MapTile::new(
        placement.rect.origin.x + i32::from(placement.rect.width) / 2,
        placement.rect.origin.y,
    );
    let entrance = MapTile::new(doorway.x, doorway.y - 1);
    Ok(BspBuildingPlan {
        plan: BuildingPlan {
            request,
            rooms: vec![RoomSpec {
                id: room.id,
                kind: generic_room_kind(room.role),
                rect: placement.rect,
            }],
            connections: vec![RoomConnection {
                from: None,
                to: room.id,
                doorway,
                kind: PortalKind::Door,
            }],
            entrance,
        },
        room_roles: vec![ProgramRoomRole {
            room: room.id,
            role: room.role,
        }],
        preferred_connections: vec![],
    })
}
