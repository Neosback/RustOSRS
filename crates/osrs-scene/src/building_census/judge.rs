//! Judging a building without a target to compare it with: is it usable, and how does it read as
//! numbers. The same code looks at a real building found by the census and at a generated one, so
//! a model trained on real buildings can score generated ones.
//!
//! The numbers (see [`FEATURE_NAMES`]) are the vector the realism critic is trained on; the
//! problems are hard checks a building must pass whatever a model thinks of it.

use super::{Building, EdgeFeature, RoomTag};
use std::collections::{HashMap, HashSet};

/// Names of the entries of [`Judgement::features`], in order.
pub const FEATURE_NAMES: [&str; FEATURES] = [
    "floor_tiles",
    "rooms",
    "largest_room_share",
    "outside_wall_edges",
    "window_share",
    "outside_doors",
    "links_per_room",
    "roofed",
    "furniture_per_tile",
    "against_wall_share",
    "decorations_per_outside_edge",
    "floor_kinds",
    "plain_room_share",
    "squareness",
    "height_spread",
    "ring_kinds",
];

/// How many numbers describe a building.
pub const FEATURES: usize = 16;

#[derive(Debug, Clone, PartialEq)]
pub struct Judgement {
    /// Doors and doorways from outside into the building.
    pub entrances: u32,
    pub rooms: usize,
    /// Rooms a person can walk to from an entrance through doors and doorways.
    pub rooms_reachable: usize,
    /// Doors and doorways with furniture on either side of them.
    pub blocked_doorways: u32,
    pub features: [f32; FEATURES],
}

impl Judgement {
    /// What is wrong with the building, in plain words; empty when it is usable.
    pub fn problems(&self) -> Vec<&'static str> {
        let mut problems = Vec::new();
        if self.rooms == 0 {
            problems.push("no enclosed room");
        }
        if self.entrances == 0 {
            problems.push("no way in");
        }
        if self.rooms_reachable < self.rooms {
            problems.push("a room cannot be reached");
        }
        if self.blocked_doorways > 0 {
            problems.push("furniture blocks a doorway");
        }
        problems
    }

    pub fn is_usable(&self) -> bool {
        self.problems().is_empty()
    }
}

fn is_gap(feature: EdgeFeature) -> bool {
    matches!(feature, EdgeFeature::Door | EdgeFeature::Opening)
}

pub fn judge(building: &Building) -> Judgement {
    let room_of: HashMap<(i32, i32), usize> = building
        .rooms
        .iter()
        .enumerate()
        .flat_map(|(index, room)| room.tiles.iter().map(move |tile| (*tile, index)))
        .collect();
    let rooms = building.rooms.len();

    // Rooms joined by interior doors and doorways; the ones with an outside door are the start.
    let mut parent: Vec<usize> = (0..rooms).collect();
    fn find(parent: &mut [usize], mut x: usize) -> usize {
        while parent[x] != x {
            parent[x] = parent[parent[x]];
            x = parent[x];
        }
        x
    }
    let mut entrances = 0_u32;
    let mut entered: HashSet<usize> = HashSet::new();
    let mut interior_gaps = 0_u32;
    let (mut exterior_edges, mut windows) = (0_u32, 0_u32);
    let mut blocked = 0_u32;
    let furniture: HashSet<(i32, i32)> = building
        .rooms
        .iter()
        .flat_map(|room| room.furniture.iter().map(|f| f.tile))
        .collect();
    for wall in &building.walls {
        let other = wall.dir.from(wall.tile);
        if wall.exterior {
            exterior_edges += 1;
            if wall.piece.feature == EdgeFeature::Window {
                windows += 1;
            }
        }
        if !is_gap(wall.piece.feature) {
            continue;
        }
        if furniture.contains(&wall.tile) || furniture.contains(&other) {
            blocked += 1;
        }
        if wall.exterior {
            entrances += 1;
            for tile in [wall.tile, other] {
                if let Some(room) = room_of.get(&tile) {
                    entered.insert(*room);
                }
            }
        } else {
            interior_gaps += 1;
            if let (Some(a), Some(b)) = (room_of.get(&wall.tile), room_of.get(&other)) {
                let (ra, rb) = (find(&mut parent, *a), find(&mut parent, *b));
                parent[ra] = rb;
            }
        }
    }
    let start_roots: HashSet<usize> = entered.iter().map(|r| find(&mut parent, *r)).collect();
    let rooms_reachable = (0..rooms)
        .filter(|room| start_roots.contains(&find(&mut parent, *room)))
        .count();

    let tiles: usize = building.rooms.iter().map(|r| r.tiles.len()).sum();
    let tiles_f = tiles.max(1) as f32;
    let largest = building
        .rooms
        .iter()
        .map(|r| r.tiles.len())
        .max()
        .unwrap_or(0);
    let roofed: f32 = building
        .rooms
        .iter()
        .map(|r| r.roofed * r.tiles.len() as f32)
        .sum::<f32>()
        / tiles_f;
    let furniture_count: usize = building.rooms.iter().map(|r| r.furniture.len()).sum();
    let against = building
        .rooms
        .iter()
        .flat_map(|r| &r.furniture)
        .filter(|f| f.wall_distance == 0)
        .count();
    let decorations: usize = building.rooms.iter().map(|r| r.decorations.len()).sum();
    let floor_kinds: HashSet<_> = building.rooms.iter().flat_map(|r| r.floor.keys()).collect();
    let plain = building
        .rooms
        .iter()
        .filter(|r| r.tag == RoomTag::General)
        .count();
    let (width, height) = building.size();
    let (small, large) = (
        width.min(height).max(1) as f32,
        width.max(height).max(1) as f32,
    );
    let outside = exterior_edges.max(1) as f32;
    let rooms_f = rooms.max(1) as f32;
    let features = [
        tiles as f32,
        rooms as f32,
        largest as f32 / tiles_f,
        exterior_edges as f32,
        windows as f32 / outside,
        entrances as f32,
        interior_gaps as f32 / rooms_f,
        roofed,
        furniture_count as f32 / tiles_f,
        against as f32 / furniture_count.max(1) as f32,
        decorations as f32 / outside,
        floor_kinds.len() as f32,
        plain as f32 / rooms_f,
        small / large,
        building.spread_inside as f32,
        building.ring.len() as f32,
    ];
    Judgement {
        entrances,
        rooms,
        rooms_reachable,
        blocked_doorways: blocked,
        features,
    }
}

#[cfg(test)]
mod tests {
    use super::super::{CensusArea, Ground, ObjectInfo, ObjectTable, analyze};
    use super::*;
    use crate::object_semantics::Placed;
    use osrs_core::ids::ObjectId;

    const WALL: u32 = 10;
    const DOOR: u32 = 11;
    const TABLE: u32 = 21;

    fn table() -> ObjectTable {
        let info = |name: &str, actions: &[&str]| ObjectInfo {
            name: name.to_owned(),
            symbol: String::new(),
            actions: actions.iter().map(|a| (*a).to_owned()).collect(),
            size: (1, 1),
        };
        ObjectTable::from([
            (ObjectId::new(WALL), info("wall", &[])),
            (ObjectId::new(DOOR), info("door", &["open"])),
            (ObjectId::new(TABLE), info("table", &[])),
        ])
    }

    fn put(id: u32, kind: u8, orientation: u8, tile: (i32, i32)) -> Placed {
        Placed {
            id: ObjectId::new(id),
            kind,
            orientation,
            tile,
            plane: 0,
        }
    }

    fn box_walls(x0: i32, y0: i32, x1: i32, y1: i32) -> Vec<Placed> {
        let mut walls = Vec::new();
        for x in x0..=x1 {
            walls.push(put(WALL, 0, 3, (x, y0)));
            walls.push(put(WALL, 0, 1, (x, y1)));
        }
        for y in y0..=y1 {
            walls.push(put(WALL, 0, 0, (x0, y)));
            walls.push(put(WALL, 0, 2, (x1, y)));
        }
        walls
    }

    fn area() -> CensusArea {
        let mut area = CensusArea::new((100, 100), 20, 16);
        for x in 100..120 {
            for y in 100..116 {
                area.set(
                    0,
                    (x, y),
                    Ground {
                        underlay: 5,
                        height_known: true,
                        ..Ground::default()
                    },
                );
            }
        }
        area
    }

    /// Two 3x3 rooms side by side (x 103..=105 and 106..=108) sharing a wall at x 105/106.
    fn two_rooms(outside_door: bool, inside_door: bool) -> Vec<Placed> {
        let mut placements = box_walls(103, 103, 108, 105);
        // The dividing wall: east edges of x 105 and west edges of x 106.
        for y in 103..=105 {
            placements.push(put(WALL, 0, 2, (105, y)));
        }
        if outside_door {
            placements.retain(|p| !(p.tile == (104, 103) && p.orientation == 3));
            placements.push(put(DOOR, 0, 3, (104, 103)));
        }
        if inside_door {
            placements.retain(|p| !(p.tile == (105, 104) && p.orientation == 2));
            placements.push(put(DOOR, 0, 2, (105, 104)));
        }
        placements
    }

    fn judged(placements: &[Placed]) -> Judgement {
        let buildings = analyze(&area(), placements, &table());
        assert_eq!(buildings.len(), 1, "one building");
        judge(&buildings[0])
    }

    #[test]
    fn a_building_with_an_outside_door_and_a_door_between_rooms_is_usable() {
        let judgement = judged(&two_rooms(true, true));
        assert_eq!(judgement.rooms, 2);
        assert_eq!(judgement.entrances, 1);
        assert_eq!(judgement.rooms_reachable, 2);
        assert!(judgement.is_usable(), "{:?}", judgement.problems());
        assert_eq!(judgement.features[1], 2.0);
    }

    #[test]
    fn a_room_with_no_door_to_it_cannot_be_reached() {
        let judgement = judged(&two_rooms(true, false));
        assert_eq!(judgement.rooms_reachable, 1);
        assert_eq!(judgement.problems(), vec!["a room cannot be reached"]);
    }

    #[test]
    fn no_outside_door_means_no_way_in() {
        let judgement = judged(&two_rooms(false, true));
        assert!(judgement.problems().contains(&"no way in"));
    }

    #[test]
    fn furniture_in_front_of_a_door_blocks_it() {
        let mut placements = two_rooms(true, true);
        // Right behind the outside door at (104, 103), inside.
        placements.push(put(TABLE, 10, 0, (104, 103)));
        let judgement = judged(&placements);
        assert_eq!(judgement.blocked_doorways, 1);
        assert!(judgement.problems().contains(&"furniture blocks a doorway"));
    }
}
