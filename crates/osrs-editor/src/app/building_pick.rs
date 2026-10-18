//! Picking a whole building: click inside it and everything that belongs to it is selected: the
//! floor under the rooms, the walls (including the ones standing on the outside tile of an edge),
//! doors, windows, furniture, decoration, and the roof over it.
//!
//! The building is found the way the census finds real ones: the rooms are the tile sets a flood
//! fill cannot leave without crossing a wall, rooms that share a wall are one building. Nothing
//! here changes the map.

use super::EditorApp;
use osrs_core::ids::ObjectId;
use osrs_scene::{
    building_census::{Building, CensusArea, ObjectInfo, ObjectTable, analyze},
    object_semantics::Placed,
};
use std::collections::HashSet;

type Tile = (i32, i32);

/// Which building a click landed in, and what belongs to it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Found {
    pub(super) plane: u8,
    pub(super) rooms: usize,
    /// The floor: every tile of every room.
    pub(super) footprint: HashSet<Tile>,
    /// The tiles around the floor where the building's walls, decoration and roof overhang stand.
    #[cfg_attr(not(test), allow(dead_code))]
    ring: HashSet<Tile>,
    members: HashSet<Placed>,
}

impl Found {
    /// Whether an object belongs to the building: anything on the floor, boundary pieces and
    /// wall decoration on the ring, and roof pieces on the plane above, over the floor or ring.
    #[cfg(test)]
    pub(super) fn owns(&self, tile: Tile, plane: u8, loc_type: u8) -> bool {
        let on_floor = self.footprint.contains(&tile);
        let on_ring = self.ring.contains(&tile);
        if plane == self.plane {
            on_floor || (on_ring && loc_type <= 8)
        } else if plane == self.plane + 1 {
            (on_floor || on_ring) && (12..=21).contains(&loc_type)
        } else {
            false
        }
    }
}

fn found_from(
    building: &Building,
    description: osrs_scene::building_census::description::BuildingDescription,
) -> Found {
    let footprint: HashSet<Tile> = description
        .floors
        .get(&building.plane)
        .into_iter()
        .flatten()
        .map(|t| (t.x, t.y))
        .collect();
    let mut ring = HashSet::new();
    for &(x, y) in &footprint {
        for dx in -1..=1 {
            for dy in -1..=1 {
                let near = (x + dx, y + dy);
                if !footprint.contains(&near) {
                    ring.insert(near);
                }
            }
        }
    }
    Found {
        plane: building.plane,
        rooms: building.rooms.len(),
        footprint,
        ring,
        members: description
            .members
            .into_iter()
            .map(|m| m.placement)
            .collect(),
    }
}

/// The building at `tile` on `plane` (or the one below it, when the click is on a roof).
pub(super) fn find(
    placements: &[Placed],
    objects: &ObjectTable,
    origin: Tile,
    size: usize,
    tile: Tile,
    plane: u8,
) -> Option<Found> {
    let area = CensusArea::new(origin, size, size);
    let buildings = analyze(&area, placements, objects);
    let candidates = [Some(plane), plane.checked_sub(1)];
    // Inside a room first, then on the ring of a building (its outer wall, its roof overhang).
    for exact in [true, false] {
        for candidate in candidates.into_iter().flatten() {
            for building in buildings.iter().filter(|b| b.plane == candidate) {
                let hit = if exact {
                    building.tiles().any(|t| t == tile)
                } else {
                    building
                        .tiles()
                        .any(|t| (t.0 - tile.0).abs() <= 1 && (t.1 - tile.1).abs() <= 1)
                };
                if hit {
                    let seed = building
                        .tiles()
                        .min_by_key(|t| ((t.0 - tile.0).abs() + (t.1 - tile.1).abs(), *t))?;
                    let description = osrs_scene::building_census::description::describe(
                        &area,
                        placements,
                        objects,
                        osrs_core::coords::MapTile::new(seed.0, seed.1),
                        candidate,
                    )?;
                    return Some(found_from(building, description));
                }
            }
        }
    }
    None
}

/// Select the building under `tile`; returns a line saying what was taken, or `None` when the
/// tile is not in or beside a building.
pub(super) fn select_building(app: &mut EditorApp, tile: Tile, additive: bool) -> Option<String> {
    let plane = app.interaction.view_plane();
    let (rx, ry) = (tile.0.div_euclid(64), tile.1.div_euclid(64));
    let mut placements = Vec::new();
    let mut table = ObjectTable::new();
    for gx in rx - 1..=rx + 1 {
        for gy in ry - 1..=ry + 1 {
            let Some(info) = app.world.infos().get(&(gx, gy)) else {
                continue;
            };
            for loc in &info.locs {
                let id = ObjectId::new(loc.definition.identity.id.get());
                placements.push(Placed {
                    id,
                    kind: loc.loc_type,
                    orientation: loc.orientation,
                    tile: loc.tile,
                    plane: loc.plane,
                });
                table.entry(id).or_insert_with(|| ObjectInfo {
                    name: loc
                        .definition
                        .name
                        .clone()
                        .unwrap_or_default()
                        .to_lowercase(),
                    symbol: String::new(),
                    actions: loc
                        .definition
                        .actions
                        .iter()
                        .flatten()
                        .map(|a| a.to_lowercase())
                        .collect(),
                    size: (
                        u8::try_from(loc.definition.size_x).unwrap_or(1),
                        u8::try_from(loc.definition.size_y).unwrap_or(1),
                    ),
                });
            }
        }
    }
    let found = find(
        &placements,
        &table,
        ((rx - 1) * 64, (ry - 1) * 64),
        192,
        tile,
        plane,
    )?;
    if !additive {
        app.interaction.clear_objects();
        app.interaction.clear_tiles();
    }
    for &floor in &found.footprint {
        app.interaction.add_tile(floor);
    }
    let mut taken = 0;
    for gx in rx - 1..=rx + 1 {
        for gy in ry - 1..=ry + 1 {
            let Some(info) = app.world.infos().get(&(gx, gy)) else {
                continue;
            };
            let picks: Vec<_> = info
                .locs
                .iter()
                .enumerate()
                .filter(|(_, loc)| {
                    found.members.contains(&Placed {
                        id: loc.definition.identity.id,
                        kind: loc.loc_type,
                        orientation: loc.orientation,
                        tile: loc.tile,
                        plane: loc.plane,
                    })
                })
                .map(|(index, _)| crate::pick::ObjectPick {
                    region: (gx, gy),
                    index,
                })
                .collect();
            for pick in picks {
                app.interaction.add_object(pick, None);
                taken += 1;
            }
        }
    }
    Some(format!(
        "Selected a building: {} room{}, {} floor tiles, {taken} objects",
        found.rooms,
        if found.rooms == 1 { "" } else { "s" },
        found.footprint.len()
    ))
}

#[cfg(test)]
mod tests;
