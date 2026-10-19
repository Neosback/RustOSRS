//! Picking a whole building: click inside it and everything that belongs to it is selected: the
//! floor under the rooms, the walls (including the ones standing on the outside tile of an edge),
//! doors, windows, furniture, decoration, and the roof over it.
//!
//! The building is found the way the census finds real ones: the rooms are the tile sets a flood
//! fill cannot leave without crossing a wall, rooms that share a wall are one building. Nothing
//! here changes the map.

use super::EditorApp;
#[cfg(test)]
use osrs_core::ids::ObjectId;
#[cfg(test)]
use osrs_scene::{
    building_census::{Building, CensusArea, ObjectInfo, ObjectTable, analyze},
    object_semantics::Placed,
};
#[cfg(test)]
use std::collections::HashSet;

type Tile = (i32, i32);

/// Which building a click landed in, and what belongs to it.
#[cfg(test)]
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

#[cfg(test)]
impl Found {
    /// The floor tile of the building nearest to `tile` (where a click on its wall points to).
    pub(super) fn seed_near(&self, tile: Tile) -> Option<Tile> {
        self.footprint
            .iter()
            .min_by_key(|t| ((t.0 - tile.0).abs() + (t.1 - tile.1).abs(), **t))
            .copied()
    }

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

#[cfg(test)]
fn found_from(
    building: &Building,
    description: osrs_scene::building_census::description::BuildingDescription,
) -> Found {
    let footprint: HashSet<Tile> = description
        .floors
        .values()
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
#[cfg(test)]
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

/// Building selection and capture consume one immutable document analysis.
pub(super) fn select_building(app: &mut EditorApp, tile: Tile, additive: bool) -> Option<String> {
    super::prefab_panel::capture::start_selection(
        app,
        osrs_core::coords::MapTile::new(tile.0, tile.1),
        additive,
        false,
    );
    Some("Analysing the edited building…".into())
}

pub(super) fn right_click(app: &mut EditorApp) {
    if app.settings.prefs.select_mode != crate::paint::SelectMode::Building {
        return;
    }
    let Some(p) = super::prefab_panel::capture::refine::hover_placement(app) else {
        return;
    };
    if p.kind.get() <= 8 {
        select_building(app, (p.position.tile.x, p.position.tile.y), false);
    }
}

pub(super) fn review(app: &mut EditorApp) {
    super::prefab_panel::capture::refine::open(app);
}

#[cfg(test)]
mod tests;
