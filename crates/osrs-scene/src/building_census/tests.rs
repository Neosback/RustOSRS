use super::*;
use crate::object_semantics::Placed;
use osrs_core::ids::ObjectId;

const WALL: u32 = 10;
const DOOR: u32 = 11;
const WINDOW: u32 = 12;
const BOOTH: u32 = 20;
const TABLE: u32 = 21;
const ROOF: u32 = 30;
const PICTURE: u32 = 40;

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
        (ObjectId::new(WINDOW), info("window", &[])),
        (ObjectId::new(BOOTH), info("bank booth", &["bank"])),
        (ObjectId::new(TABLE), info("table", &[])),
        (ObjectId::new(ROOF), info("roof", &[])),
        (ObjectId::new(PICTURE), info("picture", &[])),
    ])
}

fn put(id: u32, kind: u8, orientation: u8, tile: (i32, i32), plane: u8) -> Placed {
    Placed {
        id: ObjectId::new(id),
        kind,
        orientation,
        tile,
        plane,
    }
}

/// Walls around the tiles `x0..=x1` by `y0..=y1` (west 0, north 1, east 2, south 3).
fn box_walls(x0: i32, y0: i32, x1: i32, y1: i32) -> Vec<Placed> {
    let mut walls = Vec::new();
    for x in x0..=x1 {
        walls.push(put(WALL, 0, 3, (x, y0), 0));
        walls.push(put(WALL, 0, 1, (x, y1), 0));
    }
    for y in y0..=y1 {
        walls.push(put(WALL, 0, 0, (x0, y), 0));
        walls.push(put(WALL, 0, 2, (x1, y), 0));
    }
    walls
}

fn area() -> CensusArea {
    let mut area = CensusArea::new((100, 100), 20, 16);
    for x in 100..120 {
        for y in 100..116 {
            let inside = (103..=108).contains(&x) && (103..=107).contains(&y);
            area.set(
                0,
                (x, y),
                Ground {
                    underlay: if inside { 0 } else { 5 },
                    overlay: if inside { 12 } else { 0 },
                    shape: 0,
                    settings: 0,
                    height_known: true,
                    height: if inside { -240 } else { -240 + (x - 100) * 8 },
                },
            );
        }
    }
    area
}

#[test]
fn a_walled_room_is_a_building_with_its_walls_door_window_roof_and_ring() {
    let mut placements = box_walls(103, 103, 108, 107);
    // A door replaces the south wall at x 105; a window the north wall at x 105.
    placements.retain(|p| !(p.tile == (105, 103) && p.orientation == 3));
    placements.retain(|p| !(p.tile == (105, 107) && p.orientation == 1));
    placements.push(put(DOOR, 0, 3, (105, 103), 0));
    placements.push(put(WINDOW, 0, 1, (105, 107), 0));
    for x in 103..=108 {
        for y in 103..=107 {
            placements.push(put(ROOF, 12, 1, (x, y), 1));
        }
    }
    placements.push(put(BOOTH, 10, 3, (104, 103), 0));
    placements.push(put(BOOTH, 10, 3, (106, 103), 0));
    placements.push(put(TABLE, 10, 0, (106, 105), 0));
    placements.push(put(PICTURE, 4, 1, (104, 107), 0));

    let buildings = analyze(&area(), &placements, &table());
    assert_eq!(buildings.len(), 1);
    let building = &buildings[0];
    assert_eq!(building.size(), (6, 5));
    assert_eq!(building.rooms.len(), 1);
    let room = &building.rooms[0];
    assert_eq!(room.tiles.len(), 30);
    assert_eq!(room.tag, RoomTag::Bank, "two booths make it a bank");
    assert_eq!(room.floor.len(), 1);
    assert_eq!(room.roofed, 1.0);

    // 22 edges on the outside, none inside; one door, one window, the rest plain wall.
    assert!(building.walls.iter().all(|w| w.exterior));
    assert_eq!(building.walls.len(), 22);
    let count = |feature| {
        building
            .walls
            .iter()
            .filter(|w| w.piece.feature == feature)
            .count()
    };
    assert_eq!(
        (count(EdgeFeature::Door), count(EdgeFeature::Window)),
        (1, 1)
    );
    assert_eq!(count(EdgeFeature::Wall), 20);

    // Booths stand against the south wall; the table is in the middle.
    let against: Vec<_> = room
        .furniture
        .iter()
        .filter(|f| f.object == ObjectId::new(BOOTH))
        .map(|f| (f.wall_distance, f.against))
        .collect();
    assert_eq!(against, vec![(0, Some(Dir::South)), (0, Some(Dir::South))]);
    let table_piece = room
        .furniture
        .iter()
        .find(|f| f.object == ObjectId::new(TABLE))
        .unwrap_or_else(|| unreachable!("the table is there"));
    assert!(table_piece.wall_distance >= 1);

    // The picture hangs inside, on the north wall.
    assert_eq!(room.decorations.len(), 1);
    assert!(room.decorations[0].inside);
    assert_eq!(room.decorations[0].on, Some(ObjectId::new(WALL)));
    assert_eq!(building.roof.get(&(ObjectId::new(ROOF), 12)), Some(&30));

    // The ring is the 22 ground tiles outside the walls; the ground there rises, the floor is flat.
    assert_eq!(building.ring.values().sum::<u32>(), 22);
    assert_eq!(building.spread_inside, 0);
    assert!(building.spread_ring > 0);
}

#[test]
fn two_rooms_sharing_a_wall_are_one_building_with_an_interior_wall() {
    let mut placements = box_walls(103, 103, 108, 107);
    // A dividing wall between x 105 and x 106, with a door in it.
    for y in 103..=107 {
        placements.push(put(WALL, 0, 2, (105, y), 0));
    }
    placements.retain(|p| !(p.tile == (105, 105) && p.orientation == 2));
    placements.push(put(DOOR, 0, 2, (105, 105), 0));
    let buildings = analyze(&area(), &placements, &table());
    assert_eq!(buildings.len(), 1);
    let building = &buildings[0];
    assert_eq!(building.rooms.len(), 2);
    let interior: Vec<_> = building.walls.iter().filter(|w| !w.exterior).collect();
    assert_eq!(interior.len(), 5, "one dividing edge per row, counted once");
    assert_eq!(
        interior
            .iter()
            .filter(|w| w.piece.feature == EdgeFeature::Door)
            .count(),
        1
    );
    assert_eq!(building.walls.iter().filter(|w| w.exterior).count(), 22);
}

#[test]
fn an_open_fence_is_not_a_building_and_a_corner_piece_closes_two_sides() {
    // Three sides only: the room leaks, so it is outside.
    let mut open = box_walls(103, 103, 108, 107);
    open.retain(|p| !(p.orientation == 1 && p.tile.1 == 107));
    assert!(analyze(&area(), &open, &table()).is_empty());

    // Corner pieces (type 2) on all four corners with straight walls between.
    let mut closed = Vec::new();
    for x in 104..=107 {
        closed.push(put(WALL, 0, 3, (x, 103), 0));
        closed.push(put(WALL, 0, 1, (x, 107), 0));
    }
    for y in 104..=106 {
        closed.push(put(WALL, 0, 0, (103, y), 0));
        closed.push(put(WALL, 0, 2, (108, y), 0));
    }
    // An L piece at orientation o owns edges o and o + 1: south-west corner is south (3) and
    // west (0), so its orientation is 3; the others follow around.
    closed.push(put(WALL, 2, 3, (103, 103), 0));
    closed.push(put(WALL, 2, 2, (108, 103), 0));
    closed.push(put(WALL, 2, 1, (108, 107), 0));
    closed.push(put(WALL, 2, 0, (103, 107), 0));
    let buildings = analyze(&area(), &closed, &table());
    assert_eq!(buildings.len(), 1);
    assert_eq!(buildings[0].size(), (6, 5));
}

#[test]
fn rooms_are_tagged_by_what_stands_in_them() {
    let furniture = |id: u32| Furniture {
        object: ObjectId::new(id),
        kind: 10,
        tile: (0, 0),
        orientation: 0,
        wall_distance: 0,
        against: None,
    };
    let mut objects = table();
    objects.insert(
        ObjectId::new(50),
        ObjectInfo {
            name: "range".into(),
            ..ObjectInfo::default()
        },
    );
    assert_eq!(tag_room(&[furniture(50)], &objects), RoomTag::Kitchen);
    assert_eq!(tag_room(&[furniture(21)], &objects), RoomTag::Living);
    assert_eq!(tag_room(&[furniture(PICTURE)], &objects), RoomTag::General);
    assert_eq!(tag_room(&[], &objects), RoomTag::General);
    // Strong evidence outweighs a stray table.
    assert_eq!(
        tag_room(&[furniture(21), furniture(BOOTH), furniture(21)], &objects),
        RoomTag::Bank
    );
}

#[test]
fn an_open_doorway_in_a_wall_run_keeps_the_room_closed() {
    let mut placements = box_walls(103, 103, 108, 107);
    // Remove two wall pieces from the south wall: a wide doorway with no door in it.
    placements
        .retain(|p| !(p.orientation == 3 && (105..=106).contains(&p.tile.0) && p.tile.1 == 103));
    let buildings = analyze(&area(), &placements, &table());
    assert_eq!(
        buildings.len(),
        1,
        "a gap between two walls is a doorway, not a leak"
    );
    let openings = buildings[0]
        .walls
        .iter()
        .filter(|w| w.piece.feature == EdgeFeature::Opening)
        .count();
    assert_eq!(openings, 2);
    // A gap at the end of a run (no wall beyond it) is a real leak.
    let mut leak = box_walls(103, 103, 108, 107);
    leak.retain(|p| !(p.orientation == 3 && p.tile.0 >= 106 && p.tile.1 == 103));
    assert!(analyze(&area(), &leak, &table()).is_empty());
}
