//! How many rooms a building of a given size has.
//!
//! Measured on the 470 roofed buildings (at least 12 floor tiles, at least 30% roofed) of the
//! shipped real-building corpus, `osrs-lab/data/real-buildings.json`: two thirds are one room, and
//! the count grows with the floor area. Small buildings are almost always one room; only large
//! ones have the hallway-and-rooms plan the earlier generator gave every building.
//! `osrs-lab` re-derives this table from the corpus in a test, so it cannot drift from the data.

use crate::BuildingArchetype;

/// Rooms counted as 1, 2, 3, 4, 5 and "6 or more".
pub const ROOM_COUNT_BUCKETS: usize = 6;

/// One row: the floor-area range `[min_tiles, max_tiles)` and how many real buildings in it had
/// each room count.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RoomCountRow {
    pub min_tiles: u32,
    pub max_tiles: u32,
    pub buildings: [u32; ROOM_COUNT_BUCKETS],
}

pub const ROOM_COUNT_TABLE: [RoomCountRow; 6] = [
    RoomCountRow {
        min_tiles: 0,
        max_tiles: 20,
        buildings: [78, 6, 0, 0, 0, 0],
    },
    RoomCountRow {
        min_tiles: 20,
        max_tiles: 40,
        buildings: [136, 21, 10, 0, 0, 0],
    },
    RoomCountRow {
        min_tiles: 40,
        max_tiles: 70,
        buildings: [69, 24, 18, 3, 5, 1],
    },
    RoomCountRow {
        min_tiles: 70,
        max_tiles: 120,
        buildings: [20, 16, 11, 6, 4, 4],
    },
    RoomCountRow {
        min_tiles: 120,
        max_tiles: 200,
        buildings: [6, 9, 2, 0, 4, 2],
    },
    RoomCountRow {
        min_tiles: 200,
        max_tiles: u32::MAX,
        buildings: [2, 2, 0, 1, 1, 9],
    },
];

/// The fewest rooms a kind of building plausibly has. This is a design choice, not a measurement
/// (the corpus rows do not say what kind of building they are): a bank without a vault or a
/// church without an altar is not one.
pub const fn archetype_min_rooms(archetype: BuildingArchetype) -> usize {
    match archetype {
        BuildingArchetype::House | BuildingArchetype::Shop | BuildingArchetype::Warehouse => 1,
        BuildingArchetype::Tavern
        | BuildingArchetype::Bank
        | BuildingArchetype::Church
        | BuildingArchetype::Dungeon => 2,
        BuildingArchetype::Castle => 3,
    }
}

/// A room count for a building of `area` floor tiles, drawn from the real distribution for that
/// size. The same `seed` gives the same count. "6 or more" is read as 6; the caller limits the
/// count to the rooms its program has.
pub fn sampled_room_count(area: u32, archetype: BuildingArchetype, seed: u64) -> usize {
    let row = ROOM_COUNT_TABLE
        .iter()
        .find(|row| (row.min_tiles..row.max_tiles).contains(&area))
        .unwrap_or(&ROOM_COUNT_TABLE[ROOM_COUNT_TABLE.len() - 1]);
    let total: u32 = row.buildings.iter().sum();
    let mut pick =
        u32::try_from(mix(seed ^ 0x600d_5eed_0000_0002) % u64::from(total.max(1))).unwrap_or(0);
    let mut count = 1;
    for (index, weight) in row.buildings.iter().enumerate() {
        if pick < *weight {
            count = index + 1;
            break;
        }
        pick -= weight;
    }
    count
        .max(archetype_min_rooms(archetype))
        .max(rooms_to_keep_each_room_recognisable(area))
}

/// The census recognises a room of up to 500 tiles, and a big single hall is rare, so a large
/// building is cut into rooms of about 300 tiles at most.
const fn rooms_to_keep_each_room_recognisable(area: u32) -> usize {
    const MOST_TILES_PER_ROOM: u32 = 300;
    (area.div_ceil(MOST_TILES_PER_ROOM)) as usize
}

fn mix(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9e37_79b9_7f4a_7c15);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_same_seed_gives_the_same_count() {
        for seed in 0..50 {
            assert_eq!(
                sampled_room_count(60, BuildingArchetype::House, seed),
                sampled_room_count(60, BuildingArchetype::House, seed)
            );
        }
    }

    #[test]
    fn small_houses_are_nearly_always_one_room() {
        let ones = (0..400)
            .filter(|seed| sampled_room_count(30, BuildingArchetype::House, *seed) == 1)
            .count();
        // The real share for 20-40 tiles is 136 of 167.
        assert!((280..=360).contains(&ones), "{ones} of 400");
    }

    #[test]
    fn bigger_buildings_have_more_rooms_on_average() {
        let mean = |area| {
            (0..400)
                .map(|seed| sampled_room_count(area, BuildingArchetype::House, seed))
                .sum::<usize>()
        };
        assert!(mean(30) < mean(90));
        assert!(mean(90) < mean(300));
    }

    #[test]
    fn a_castle_has_at_least_three_rooms() {
        assert!((0..200).all(|seed| sampled_room_count(10, BuildingArchetype::Castle, seed) >= 3));
    }
}
