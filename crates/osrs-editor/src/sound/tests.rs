use super::*;
use osrs_core::ids::SoundId;

fn sound(looping: Option<u32>, random: &[u32], distance: u8, retain: u8) -> ObjectSound {
    ObjectSound {
        looping: looping.map(SoundId::new),
        random: random.iter().copied().map(SoundId::new).collect(),
        distance,
        retain,
        ..ObjectSound::empty()
    }
}

fn row(name: &str, object: u32, tile: (i32, i32), size: (u8, u8), s: ObjectSound) -> AreaSound {
    AreaSound {
        object,
        name: name.to_owned(),
        tile,
        plane: 0,
        size,
        region: (50, 50),
        index: 0,
        sound: s,
    }
}

#[test]
fn names_come_from_the_runelite_table() {
    assert_eq!(name(62), Some("open door"));
    assert_eq!(name(2739), Some("item drop"));
    assert_eq!(name(1), None);
    assert!(NAMES.lines().count() > 50);
}

#[test]
fn footprints_swap_on_odd_turns_only() {
    assert_eq!(footprint(2, 3, 0), (2, 3));
    assert_eq!(footprint(2, 3, 1), (3, 2));
    assert_eq!(footprint(2, 3, 2), (2, 3));
    assert_eq!(footprint(2, 3, 3), (3, 2));
}

#[test]
fn reach_follows_the_client_radii() {
    // distance 10, retain 4: full volume to 3.5 tiles out, silent past 10.5.
    assert_eq!(reach(&sound(Some(1), &[], 10, 4)), (3.5, 10.5));
    // No retain byte (retain 0) still has the half-tile of full volume.
    assert_eq!(reach(&sound(Some(1), &[], 6, 0)), (0.5, 6.5));
    // The inner radius never exceeds the outer one.
    assert_eq!(reach(&sound(Some(1), &[], 2, 9)), (2.5, 2.5));
}

#[test]
fn a_ring_is_exactly_the_distance_from_the_rectangle() {
    let (min, max) = ((10.0, 20.0), (12.0, 21.0));
    for (x, y) in ring(min, max, 3.0) {
        // Euclidean distance from the point to the rectangle.
        let dx = (min.0 - x).max(x - max.0).max(0.0);
        let dy = (min.1 - y).max(y - max.1).max(0.0);
        let distance = (dx * dx + dy * dy).sqrt();
        assert!(
            (distance - 3.0).abs() < 1e-3,
            "({x}, {y}) is {distance} away"
        );
    }
    // It encloses the rectangle: extreme points sit `distance` beyond each side.
    let points = ring(min, max, 3.0);
    let xs: Vec<f32> = points.iter().map(|p| p.0).collect();
    assert!((xs.iter().copied().fold(f32::MIN, f32::max) - 15.0).abs() < 1e-3);
    assert!((xs.iter().copied().fold(f32::MAX, f32::min) - 7.0).abs() < 1e-3);
}

#[test]
fn rows_match_by_object_name_sound_name_and_ids() {
    let door = row(
        "Large door",
        1521,
        (3200, 3200),
        (1, 1),
        sound(Some(62), &[], 5, 1),
    );
    assert!(door.matches(""));
    assert!(door.matches("door"), "object name");
    assert!(door.matches("open"), "sound name");
    assert!(door.matches("62"), "sound id");
    assert!(door.matches("1521"), "object id");
    assert!(!door.matches("fire"));
    let random = row("Fire", 9, (0, 0), (1, 1), sound(None, &[2739, 5], 4, 1));
    assert!(random.matches("item drop"), "a random sound's name");
    assert_eq!(
        random.main_sound(),
        Some(2739),
        "no looping sound: the first random one"
    );
}

#[test]
fn sounds_are_counted_most_used_first() {
    let rows = [
        row("a", 1, (0, 0), (1, 1), sound(Some(7), &[], 3, 0)),
        row("b", 2, (1, 0), (1, 1), sound(Some(7), &[9], 3, 0)),
        row("c", 3, (2, 0), (1, 1), sound(Some(8), &[], 3, 0)),
    ];
    assert_eq!(by_sound(&rows), [(7, 2), (8, 1), (9, 1)]);
}

#[test]
fn a_rectangle_is_the_footprint_in_tile_corners() {
    let r = row("x", 1, (100, 200), (2, 3), sound(Some(1), &[], 1, 0));
    assert_eq!(r.rect(), ((100.0, 200.0), (102.0, 203.0)));
}
