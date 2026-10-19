//! A sparse, drawn room program becomes the same semantic plan used by BSP and WFC realization.
use super::*;
use crate::{PrefabCell, PrefabSemantics};
#[cfg(test)]
mod tests;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct BuildingSketch {
    pub floors: Vec<PrefabCell>,
    pub semantics: PrefabSemantics,
    pub origin: MapTile,
}

pub fn plan_authored_rooms(
    request: BuildingRequest,
    sketch: &BuildingSketch,
) -> Result<BspBuildingPlan, &'static str> {
    if request.floors != 1 {
        return Err(
            "Drawn room layouts currently generate one floor; existing upper floors remain editable",
        );
    }
    let mut remaining: BTreeSet<_> = sketch
        .floors
        .iter()
        .filter(|c| c.plane == 0)
        .map(|c| (c.x, c.y))
        .collect();
    if remaining.is_empty() {
        return Err("Draw a floor footprint before generating rooms");
    }
    let mut assigned = BTreeSet::new();
    let mut groups = Vec::new();
    for room in &sketch.semantics.rooms {
        let cells: BTreeSet<_> = room
            .cells
            .iter()
            .filter(|c| c.plane == 0)
            .map(|c| (c.x, c.y))
            .collect();
        if cells
            .iter()
            .any(|c| !remaining.contains(c) || !assigned.insert(*c))
        {
            return Err("Room membership overlaps or extends outside the drawn floor");
        }
        if !cells.is_empty() {
            groups.push((room.id, role(&room.role), cells));
        }
    }
    remaining.retain(|c| !assigned.contains(c));
    if !remaining.is_empty() {
        groups.push((u16::MAX, BuildingRoomRole::Living, remaining));
    }
    let mut rooms = Vec::new();
    let mut roles = Vec::new();
    let mut group_of = BTreeMap::new();
    for (group, role, mut cells) in groups {
        while let Some(&(x, y)) = cells.iter().min_by_key(|&&(x, y)| (y, x)) {
            let mut w = 1_u16;
            while x.checked_add(w).is_some_and(|x| cells.contains(&(x, y))) {
                w += 1;
            }
            let mut h = 1_u16;
            while y
                .checked_add(h)
                .is_some_and(|y| (0..w).all(|dx| cells.contains(&(x + dx, y))))
            {
                h += 1;
            }
            for dx in 0..w {
                for dy in 0..h {
                    cells.remove(&(x + dx, y + dy));
                }
            }
            let id = RoomId::new(
                u16::try_from(rooms.len()).map_err(|_| "Too many drawn room sections")?,
            );
            let origin = MapTile::new(
                sketch
                    .origin
                    .x
                    .checked_add(i32::from(x))
                    .ok_or("Room coordinate overflow")?,
                sketch
                    .origin
                    .y
                    .checked_add(i32::from(y))
                    .ok_or("Room coordinate overflow")?,
            );
            rooms.push(RoomSpec {
                id,
                kind: generic_room_kind(role),
                rect: RoomRect {
                    origin,
                    width: w,
                    height: h,
                },
            });
            roles.push(ProgramRoomRole { room: id, role });
            group_of.insert(id, group);
        }
    }
    let mut connections = Vec::new();
    for (i, a) in rooms.iter().enumerate() {
        for b in &rooms[i + 1..] {
            let shared: Vec<_> = rect_tiles(a.rect)
                .map_err(|_| "Room coordinates overflow")?
                .into_iter()
                .filter(|tile| {
                    cardinal_neighbors(*tile)
                        .into_iter()
                        .flatten()
                        .any(|n| b.rect.contains(n))
                })
                .collect();
            if !shared.is_empty() {
                let same = group_of[&a.id] == group_of[&b.id];
                let links: Vec<_> = sketch
                    .semantics
                    .portals
                    .iter()
                    .filter(|p| {
                        p.rooms.contains(&Some(group_of[&a.id]))
                            && p.rooms.contains(&Some(group_of[&b.id]))
                    })
                    .collect();
                let has_internal = sketch
                    .semantics
                    .portals
                    .iter()
                    .any(|p| p.rooms.iter().all(Option::is_some));
                if same {
                    for doorway in shared {
                        connections.push(RoomConnection {
                            from: Some(a.id),
                            to: b.id,
                            doorway,
                            kind: PortalKind::Open,
                        });
                    }
                } else if !has_internal || !links.is_empty() {
                    let requested = links
                        .iter()
                        .flat_map(|p| p.approaches)
                        .map(|c| {
                            MapTile::new(
                                sketch.origin.x + i32::from(c.x),
                                sketch.origin.y + i32::from(c.y),
                            )
                        })
                        .find(|c| shared.contains(c));
                    connections.push(RoomConnection {
                        from: Some(a.id),
                        to: b.id,
                        doorway: requested.unwrap_or(shared[shared.len() / 2]),
                        kind: PortalKind::Door,
                    });
                }
            }
        }
    }
    let all: BTreeSet<_> = rooms
        .iter()
        .flat_map(|r| rect_tiles(r.rect).unwrap_or_default())
        .collect();
    let site = request.site;
    let boundary = |t: MapTile| {
        site.contains(t)
            && (t.x == site.origin().x
                || t.y == site.origin().y
                || t.x == site.origin().x + i32::from(site.width()) - 1
                || t.y == site.origin().y + i32::from(site.height()) - 1)
    };
    let mut entrances = Vec::new();
    for room in &rooms {
        for tile in rect_tiles(room.rect).map_err(|_| "Room coordinates overflow")? {
            for other in cardinal_neighbors(tile).into_iter().flatten() {
                if !all.contains(&other) && boundary(other) {
                    entrances.push((other, tile, room.id));
                }
            }
        }
    }
    entrances.sort();
    let chosen = sketch
        .semantics
        .portals
        .iter()
        .find(|p| p.rooms.contains(&None))
        .and_then(|p| {
            let i = usize::from(p.rooms[0].is_some());
            let c = p.approaches[i];
            entrances
                .iter()
                .find(|(outside, _, _)| {
                    *outside
                        == MapTile::new(
                            sketch.origin.x + i32::from(c.x),
                            sketch.origin.y + i32::from(c.y),
                        )
                })
                .copied()
        })
        .or_else(|| {
            entrances
                .get(request.seed as usize % entrances.len().max(1))
                .copied()
        })
        .ok_or("Leave a one-tile exterior margin beside at least one room for its entrance")?;
    let (entrance, doorway, to) = chosen;
    connections.push(RoomConnection {
        from: None,
        to,
        doorway,
        kind: PortalKind::Door,
    });
    let plan = BuildingPlan {
        request,
        rooms,
        connections,
        entrance,
    };
    let mut reached = BTreeSet::from([to]);
    loop {
        let before = reached.len();
        for c in &plan.connections {
            if let Some(from) = c.from {
                if reached.contains(&from) {
                    reached.insert(c.to);
                }
                if reached.contains(&c.to) {
                    reached.insert(from);
                }
            }
        }
        if before == reached.len() {
            break;
        }
    }
    if reached.len() != plan.rooms.len() {
        return Err(
            "Drawn rooms are disconnected. Add a shared edge/corridor or connect their portals",
        );
    }
    Ok(BspBuildingPlan {
        plan,
        room_roles: roles,
        preferred_connections: vec![],
    })
}
fn role(name: &str) -> BuildingRoomRole {
    use BuildingRoomRole::*;
    match name.to_lowercase().as_str() {
        "kitchen" => Kitchen,
        "bedroom" | "guest_room" => Bedroom,
        "workshop" | "smithy" => Workshop,
        "storage" => Storage,
        "bank" | "lobby" => Lobby,
        "vault" => Vault,
        "bar" => Bar,
        "pub" | "tavern" | "common_room" => CommonRoom,
        "shop" | "sales_floor" => SalesFloor,
        "chapel" | "nave" => Nave,
        "altar" => Altar,
        "office" | "study" => Office,
        "utility" => Utility,
        _ => Living,
    }
}
