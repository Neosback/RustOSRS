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
                let names = osrs_edit::census_name(
                    app.names.custom().book(),
                    id.get(),
                    loc.definition.name.as_deref().unwrap_or_default(),
                    app.names
                        .book()
                        .and_then(|b| b.object(id.get()))
                        .unwrap_or_default(),
                );
                placements.push(Placed {
                    id,
                    kind: loc.loc_type,
                    orientation: loc.orientation,
                    tile: loc.tile,
                    plane: loc.plane,
                });
                table.entry(id).or_insert_with(|| ObjectInfo {
                    name: names.0.clone(),
                    symbol: names.1.clone(),
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
    app.prefabs.pending_building = found.seed_near(tile);
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
        "Selected a building: {} room{}, {} floor tiles, {taken} objects. Enter reviews it for saving as a prefab, Esc lets go.",
        found.rooms,
        if found.rooms == 1 { "" } else { "s" },
        found.footprint.len()
    ))
}

/// A plain right-click on a wall (or a piece standing on one) picks the whole building. Shift
/// right-click keeps its context menu and a right-drag still orbits.
pub(super) fn right_click(app: &mut EditorApp) {
    let Some(object) = app.interaction.hover().object.clone() else {
        return;
    };
    let Some(loc) = app
        .world
        .infos()
        .get(&object.region)
        .and_then(|info| info.locs.get(object.index))
    else {
        return;
    };
    // Walls, corners and the decoration on them (loc kinds 0 to 8).
    if loc.loc_type > 8 {
        return;
    }
    let tile = loc.tile;
    let message = select_building(app, tile, false)
        .unwrap_or_else(|| "That wall does not close a building here.".to_owned());
    app.ui.notify(message);
}

/// Enter opens the capture review on the picked building; Escape forgets it.
pub(super) fn keys(app: &mut EditorApp, ctx: &eframe::egui::Context) {
    if app.prefabs.pending_building.is_none() {
        return;
    }
    if app.settings.prefs.tool != crate::paint::Tool::Select {
        return;
    }
    if ctx.input(|i| i.key_pressed(eframe::egui::Key::Escape)) {
        app.prefabs.pending_building = None;
    } else if ctx.input(|i| i.key_pressed(eframe::egui::Key::Enter)) {
        review(app);
    }
}

/// Open the capture review on the picked building.
pub(super) fn review(app: &mut EditorApp) {
    if let Some((x, y)) = app.prefabs.pending_building.take() {
        // The review lives in the Prefabs panel: bring it up.
        app.settings.prefs.properties_panel = true;
        app.settings.prefs.rail_tab = crate::settings::RailTab::Prefabs;
        app.prefabs.page = super::prefab_runtime::Page::Library;
        super::prefab_panel::capture::start(app, osrs_core::coords::MapTile::new(x, y));
    }
}

#[cfg(test)]
mod tests;
