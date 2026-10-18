use super::*;

const WALL: u32 = 10;
const DOOR: u32 = 11;
const TABLE: u32 = 21;
const ROOF: u32 = 30;

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
        (ObjectId::new(ROOF), info("roof", &[])),
    ])
}

fn put(id: u32, kind: u8, orientation: u8, tile: Tile, plane: u8) -> Placed {
    Placed {
        id: ObjectId::new(id),
        kind,
        orientation,
        tile,
        plane,
    }
}

/// Walls round `x0..=x1` by `y0..=y1` (west 0, north 1, east 2, south 3).
fn ring(x0: i32, y0: i32, x1: i32, y1: i32, into: &mut Vec<Placed>) {
    for x in x0..=x1 {
        into.push(put(WALL, 0, 3, (x, y0), 0));
        into.push(put(WALL, 0, 1, (x, y1), 0));
    }
    for y in y0..=y1 {
        into.push(put(WALL, 0, 0, (x0, y), 0));
        into.push(put(WALL, 0, 2, (x1, y), 0));
    }
}

/// A house of two rooms (x 103..=108 split at x 105/106 by a door) with a roof and a table.
fn house() -> Vec<Placed> {
    let mut p = Vec::new();
    ring(103, 103, 108, 107, &mut p);
    for y in 103..=107 {
        p.push(put(WALL, 0, 2, (105, y), 0));
    }
    p.retain(|w| !(w.tile == (105, 105) && w.orientation == 2));
    p.push(put(DOOR, 0, 2, (105, 105), 0));
    for x in 102..=109 {
        for y in 102..=108 {
            p.push(put(ROOF, 12, 1, (x, y), 1));
        }
    }
    p.push(put(TABLE, 10, 0, (104, 105), 0));
    // A tree just outside, and a wall decoration on the outside of the north wall.
    p.push(put(TABLE, 10, 0, (110, 105), 0));
    p.push(put(WALL, 4, 3, (104, 108), 0));
    p
}

fn find_at(tile: Tile, plane: u8) -> Option<Found> {
    find(&house(), &table(), (100, 100), 20, tile, plane)
}

#[test]
fn a_click_inside_either_room_finds_the_whole_two_room_building() {
    let a = find_at((104, 104), 0).unwrap_or_else(|| unreachable!("room one"));
    let b = find_at((107, 106), 0).unwrap_or_else(|| unreachable!("room two"));
    assert_eq!(a, b, "both rooms are the same building");
    assert_eq!(a.rooms, 2);
    assert_eq!(a.footprint.len(), 6 * 5);
}

#[test]
fn clicking_the_outer_wall_or_the_roof_still_finds_it_and_open_ground_does_not() {
    // On the tile just outside the wall.
    assert!(find_at((102, 105), 0).is_some());
    // From plane 1 (viewing the roof) over the building.
    let from_roof = find_at((106, 105), 1).unwrap_or_else(|| unreachable!("roof click"));
    assert_eq!(from_roof.plane, 0);
    // Far from any wall.
    assert!(find_at((115, 115), 0).is_none());
    assert!(find_at((115, 115), 1).is_none());
}

#[test]
fn what_belongs_to_the_building_is_floor_objects_boundary_pieces_decoration_and_roof() {
    let found = find_at((104, 104), 0).unwrap_or_else(|| unreachable!());
    // Furniture on the floor, walls on the floor and on the outside tile, roof over it all.
    assert!(found.owns((104, 105), 0, 10));
    assert!(found.owns((103, 103), 0, 0));
    assert!(
        found.owns((102, 105), 0, 0),
        "a wall owned by the outside tile of its edge"
    );
    assert!(
        found.owns((104, 108), 0, 4),
        "decoration on the outside of the wall"
    );
    assert!(found.owns((106, 105), 1, 12));
    assert!(found.owns((109, 108), 1, 17), "roof overhang");
    // Not the tree beside it, not other planes, not roof kinds on the ground plane.
    assert!(
        !found.owns((110, 105), 0, 10),
        "scenery outside the walls is not the building"
    );
    assert!(!found.owns((106, 105), 2, 12));
    assert!(
        !found.owns((106, 105), 1, 10),
        "only roof pieces count on the plane above"
    );
}

#[test]
fn every_object_of_the_house_is_taken_and_nothing_else() {
    let found = find_at((104, 104), 0).unwrap_or_else(|| unreachable!());
    let all = house();
    let taken: Vec<_> = all
        .iter()
        .filter(|p| found.owns(p.tile, p.plane, p.kind))
        .collect();
    // The tree at (110,105) is the only thing left behind.
    assert_eq!(taken.len(), all.len() - 1);
    assert!(taken.iter().all(|p| p.tile != (110, 105)));
}
