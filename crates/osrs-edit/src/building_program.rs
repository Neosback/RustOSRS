//! Archetype-driven building programs and deterministic BSP macro-layout packing.
//!
//! A building program states what spaces an archetype needs and how those spaces should relate.
//! BSP packing assigns those spaces to non-overlapping rectangles while reserving connected
//! one-tile corridor bands between recursive partitions. This layer still does not choose OSRS
//! assets or run local WFC refinement.

use crate::{BuildingArchetype, RectSite, RoomId, RoomRect};
use osrs_core::coords::MapTile;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BuildingRoomRole {
    Living,
    Kitchen,
    Bedroom,
    Bathroom,
    Utility,
    SalesFloor,
    Storage,
    Office,
    Workshop,
    CommonRoom,
    Bar,
    GuestRoom,
    Lobby,
    Vault,
    Security,
    Nave,
    Altar,
    Vestry,
    Loading,
    GreatHall,
    Armory,
    GuardRoom,
    Cell,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ProgramRoom {
    pub id: RoomId,
    pub role: BuildingRoomRole,
    pub min_width: u16,
    pub min_height: u16,
    pub area_weight: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ProgramConnection {
    pub first: RoomId,
    pub second: RoomId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildingProgram {
    pub archetype: BuildingArchetype,
    pub entrance_room: RoomId,
    rooms: Vec<ProgramRoom>,
    connections: Vec<ProgramConnection>,
}

impl BuildingProgram {
    pub fn rooms(&self) -> &[ProgramRoom] {
        &self.rooms
    }

    pub fn connections(&self) -> &[ProgramConnection] {
        &self.connections
    }

    pub fn room(&self, id: RoomId) -> Option<&ProgramRoom> {
        self.rooms.iter().find(|room| room.id == id)
    }

    pub fn neighbors(&self, id: RoomId) -> impl Iterator<Item = RoomId> + '_ {
        self.connections.iter().filter_map(move |connection| {
            if connection.first == id {
                Some(connection.second)
            } else if connection.second == id {
                Some(connection.first)
            } else {
                None
            }
        })
    }
}

pub fn building_program(archetype: BuildingArchetype) -> BuildingProgram {
    match archetype {
        BuildingArchetype::House => program(
            archetype,
            [
                room(0, BuildingRoomRole::Living, 4),
                room(1, BuildingRoomRole::Kitchen, 3),
                room(2, BuildingRoomRole::Bedroom, 3),
                room(3, BuildingRoomRole::Bedroom, 2),
                room(4, BuildingRoomRole::Bathroom, 1),
            ],
            [(0, 1), (0, 2), (0, 3), (0, 4)],
        ),
        BuildingArchetype::Shop => program(
            archetype,
            [
                room(0, BuildingRoomRole::SalesFloor, 5),
                room(1, BuildingRoomRole::Storage, 3),
                room(2, BuildingRoomRole::Office, 2),
                room(3, BuildingRoomRole::Workshop, 2),
                room(4, BuildingRoomRole::Utility, 1),
            ],
            [(0, 1), (0, 2), (0, 3), (1, 4)],
        ),
        BuildingArchetype::Tavern => program(
            archetype,
            [
                room(0, BuildingRoomRole::CommonRoom, 6),
                room(1, BuildingRoomRole::Bar, 3),
                room(2, BuildingRoomRole::Kitchen, 3),
                room(3, BuildingRoomRole::Storage, 2),
                room(4, BuildingRoomRole::GuestRoom, 2),
            ],
            [(0, 1), (1, 2), (2, 3), (0, 4)],
        ),
        BuildingArchetype::Bank => program(
            archetype,
            [
                room(0, BuildingRoomRole::Lobby, 5),
                room(1, BuildingRoomRole::Vault, 4),
                room(2, BuildingRoomRole::Office, 2),
                room(3, BuildingRoomRole::Security, 2),
                room(4, BuildingRoomRole::Storage, 1),
            ],
            [(0, 2), (0, 3), (3, 1), (1, 4)],
        ),
        BuildingArchetype::Church => program(
            archetype,
            [
                room(0, BuildingRoomRole::Nave, 6),
                room(1, BuildingRoomRole::Altar, 3),
                room(2, BuildingRoomRole::Vestry, 2),
                room(3, BuildingRoomRole::Office, 2),
                room(4, BuildingRoomRole::Storage, 1),
            ],
            [(0, 1), (1, 2), (0, 3), (2, 4)],
        ),
        BuildingArchetype::Warehouse => program(
            archetype,
            [
                room(0, BuildingRoomRole::Loading, 4),
                room(1, BuildingRoomRole::Storage, 6),
                room(2, BuildingRoomRole::Storage, 4),
                room(3, BuildingRoomRole::Office, 2),
                room(4, BuildingRoomRole::Workshop, 2),
            ],
            [(0, 1), (1, 2), (0, 3), (1, 4)],
        ),
        BuildingArchetype::Castle => program(
            archetype,
            [
                room(0, BuildingRoomRole::GreatHall, 6),
                room(1, BuildingRoomRole::Kitchen, 3),
                room(2, BuildingRoomRole::Armory, 3),
                room(3, BuildingRoomRole::Bedroom, 3),
                room(4, BuildingRoomRole::Storage, 2),
            ],
            [(0, 1), (0, 2), (0, 3), (2, 4)],
        ),
        BuildingArchetype::Dungeon => program(
            archetype,
            [
                room(0, BuildingRoomRole::GuardRoom, 4),
                room(1, BuildingRoomRole::Cell, 2),
                room(2, BuildingRoomRole::Cell, 2),
                room(3, BuildingRoomRole::Storage, 2),
                room(4, BuildingRoomRole::Utility, 1),
            ],
            [(0, 1), (0, 2), (0, 3), (3, 4)],
        ),
    }
}

fn room(id: u16, role: BuildingRoomRole, area_weight: u16) -> ProgramRoom {
    ProgramRoom {
        id: RoomId::new(id),
        role,
        min_width: 2,
        min_height: 2,
        area_weight,
    }
}

fn program<const ROOM_COUNT: usize, const CONNECTION_COUNT: usize>(
    archetype: BuildingArchetype,
    rooms: [ProgramRoom; ROOM_COUNT],
    connections: [(u16, u16); CONNECTION_COUNT],
) -> BuildingProgram {
    BuildingProgram {
        archetype,
        entrance_room: RoomId::new(0),
        rooms: rooms.into_iter().collect(),
        connections: connections
            .into_iter()
            .map(|(first, second)| ProgramConnection {
                first: RoomId::new(first),
                second: RoomId::new(second),
            })
            .collect(),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BspRoomPlacement {
    pub room: RoomId,
    pub rect: RoomRect,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BspCorridor {
    pub rect: RoomRect,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BspLayout {
    pub site: RectSite,
    rooms: Vec<BspRoomPlacement>,
    corridors: Vec<BspCorridor>,
}

impl BspLayout {
    pub fn rooms(&self) -> &[BspRoomPlacement] {
        &self.rooms
    }

    pub fn corridors(&self) -> &[BspCorridor] {
        &self.corridors
    }

    pub fn room(&self, id: RoomId) -> Option<&BspRoomPlacement> {
        self.rooms.iter().find(|placement| placement.room == id)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BspLayoutError {
    SiteTooSmall {
        width: u16,
        height: u16,
    },
    NoProgramRooms,
    CannotSplitForRoomCount {
        room_count: usize,
    },
    RoomDoesNotFit {
        room: RoomId,
        width: u16,
        height: u16,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SplitAxis {
    Vertical,
    Horizontal,
}

impl SplitAxis {
    const fn opposite(self) -> Self {
        match self {
            Self::Vertical => Self::Horizontal,
            Self::Horizontal => Self::Vertical,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Leaf {
    rect: RoomRect,
    next_axis: SplitAxis,
}

pub fn pack_building_program(
    program: &BuildingProgram,
    site: RectSite,
    seed: u64,
) -> Result<BspLayout, BspLayoutError> {
    if program.rooms.is_empty() {
        return Err(BspLayoutError::NoProgramRooms);
    }
    if site.width() < 4 || site.height() < 4 {
        return Err(BspLayoutError::SiteTooSmall {
            width: site.width(),
            height: site.height(),
        });
    }

    let global_min_span = program
        .rooms
        .iter()
        .map(|room| room.min_width.max(room.min_height))
        .max()
        .unwrap_or(2)
        .max(2);
    let interior = RoomRect {
        origin: MapTile::new(site.origin().x + 1, site.origin().y + 1),
        width: site.width() - 2,
        height: site.height() - 2,
    };
    if interior.width < global_min_span || interior.height < global_min_span {
        return Err(BspLayoutError::SiteTooSmall {
            width: site.width(),
            height: site.height(),
        });
    }

    let first_axis = if interior.width >= interior.height {
        SplitAxis::Vertical
    } else {
        SplitAxis::Horizontal
    };
    let mut leaves = vec![Leaf {
        rect: interior,
        next_axis: first_axis,
    }];
    let mut corridors = Vec::new();
    let mut rng = SplitMix64::new(seed);

    while leaves.len() < program.rooms.len() {
        let Some(index) = best_split_candidate(&leaves, global_min_span) else {
            return Err(BspLayoutError::CannotSplitForRoomCount {
                room_count: program.rooms.len(),
            });
        };
        let leaf = leaves.remove(index);
        let Some((first, corridor, second)) = split_leaf(leaf, global_min_span, &mut rng) else {
            return Err(BspLayoutError::CannotSplitForRoomCount {
                room_count: program.rooms.len(),
            });
        };
        leaves.push(first);
        leaves.push(second);
        corridors.push(BspCorridor { rect: corridor });
    }

    leaves.sort_by(|first, second| {
        rect_area(second.rect)
            .cmp(&rect_area(first.rect))
            .then_with(|| first.rect.origin.cmp(&second.rect.origin))
    });
    let mut rooms = program.rooms.clone();
    rooms.sort_by(|first, second| {
        second
            .area_weight
            .cmp(&first.area_weight)
            .then_with(|| first.id.cmp(&second.id))
    });

    let mut placements = Vec::with_capacity(rooms.len());
    for (room, leaf) in rooms.into_iter().zip(leaves) {
        if leaf.rect.width < room.min_width || leaf.rect.height < room.min_height {
            return Err(BspLayoutError::RoomDoesNotFit {
                room: room.id,
                width: leaf.rect.width,
                height: leaf.rect.height,
            });
        }
        placements.push(BspRoomPlacement {
            room: room.id,
            rect: leaf.rect,
        });
    }
    placements.sort_by_key(|placement| placement.room);
    corridors.sort_by_key(|corridor| corridor.rect.origin);

    Ok(BspLayout {
        site,
        rooms: placements,
        corridors,
    })
}

fn best_split_candidate(leaves: &[Leaf], min_span: u16) -> Option<usize> {
    leaves
        .iter()
        .enumerate()
        .filter(|(_, leaf)| can_split(*leaf, min_span))
        .max_by(|(_, first), (_, second)| {
            rect_area(first.rect)
                .cmp(&rect_area(second.rect))
                .then_with(|| second.rect.origin.cmp(&first.rect.origin))
        })
        .map(|(index, _)| index)
}

fn can_split(leaf: &Leaf, min_span: u16) -> bool {
    match leaf.next_axis {
        SplitAxis::Vertical => leaf.rect.width >= min_span.saturating_mul(2).saturating_add(1),
        SplitAxis::Horizontal => leaf.rect.height >= min_span.saturating_mul(2).saturating_add(1),
    }
}

fn split_leaf(leaf: Leaf, min_span: u16, rng: &mut SplitMix64) -> Option<(Leaf, RoomRect, Leaf)> {
    let next_axis = leaf.next_axis.opposite();
    match leaf.next_axis {
        SplitAxis::Vertical => {
            let available = leaf.rect.width.checked_sub(1)?;
            let max_first = available.checked_sub(min_span)?;
            if max_first < min_span {
                return None;
            }
            let first_width = rng.range_inclusive(min_span, max_first);
            let second_width = available.checked_sub(first_width)?;
            let corridor_x = leaf.rect.origin.x.checked_add(i32::from(first_width))?;
            let second_x = corridor_x.checked_add(1)?;
            Some((
                Leaf {
                    rect: RoomRect {
                        origin: leaf.rect.origin,
                        width: first_width,
                        height: leaf.rect.height,
                    },
                    next_axis,
                },
                RoomRect {
                    origin: MapTile::new(corridor_x, leaf.rect.origin.y),
                    width: 1,
                    height: leaf.rect.height,
                },
                Leaf {
                    rect: RoomRect {
                        origin: MapTile::new(second_x, leaf.rect.origin.y),
                        width: second_width,
                        height: leaf.rect.height,
                    },
                    next_axis,
                },
            ))
        }
        SplitAxis::Horizontal => {
            let available = leaf.rect.height.checked_sub(1)?;
            let max_first = available.checked_sub(min_span)?;
            if max_first < min_span {
                return None;
            }
            let first_height = rng.range_inclusive(min_span, max_first);
            let second_height = available.checked_sub(first_height)?;
            let corridor_y = leaf.rect.origin.y.checked_add(i32::from(first_height))?;
            let second_y = corridor_y.checked_add(1)?;
            Some((
                Leaf {
                    rect: RoomRect {
                        origin: leaf.rect.origin,
                        width: leaf.rect.width,
                        height: first_height,
                    },
                    next_axis,
                },
                RoomRect {
                    origin: MapTile::new(leaf.rect.origin.x, corridor_y),
                    width: leaf.rect.width,
                    height: 1,
                },
                Leaf {
                    rect: RoomRect {
                        origin: MapTile::new(leaf.rect.origin.x, second_y),
                        width: leaf.rect.width,
                        height: second_height,
                    },
                    next_axis,
                },
            ))
        }
    }
}

fn rect_area(rect: RoomRect) -> u32 {
    u32::from(rect.width) * u32::from(rect.height)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    const fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut value = self.state;
        value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        value ^ (value >> 31)
    }

    fn range_inclusive(&mut self, min: u16, max: u16) -> u16 {
        if min >= max {
            return min;
        }
        let span = u64::from(max - min) + 1;
        min + u16::try_from(self.next_u64() % span).unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{BTreeMap, BTreeSet, VecDeque};

    fn site(width: u16, height: u16) -> RectSite {
        RectSite::new(MapTile::new(3200, 3200), width, height)
            .unwrap_or_else(|| unreachable!("test site is valid"))
    }

    #[test]
    fn every_archetype_program_is_connected_and_references_known_rooms() {
        for archetype in [
            BuildingArchetype::House,
            BuildingArchetype::Shop,
            BuildingArchetype::Tavern,
            BuildingArchetype::Bank,
            BuildingArchetype::Church,
            BuildingArchetype::Warehouse,
            BuildingArchetype::Castle,
            BuildingArchetype::Dungeon,
        ] {
            let program = building_program(archetype);
            let known = program
                .rooms()
                .iter()
                .map(|room| room.id)
                .collect::<BTreeSet<_>>();
            assert!(known.contains(&program.entrance_room));
            assert_eq!(known.len(), program.rooms().len());
            for connection in program.connections() {
                assert!(known.contains(&connection.first));
                assert!(known.contains(&connection.second));
            }

            let mut visited = BTreeSet::new();
            let mut queue = VecDeque::from([program.entrance_room]);
            while let Some(room) = queue.pop_front() {
                if !visited.insert(room) {
                    continue;
                }
                for neighbor in program.neighbors(room) {
                    queue.push_back(neighbor);
                }
            }
            assert_eq!(visited, known);
        }
    }

    #[test]
    fn same_seed_produces_identical_bsp_layout() {
        let program = building_program(BuildingArchetype::Tavern);
        let first = pack_building_program(&program, site(24, 18), 991)
            .unwrap_or_else(|error| unreachable!("valid BSP layout: {error:?}"));
        let second = pack_building_program(&program, site(24, 18), 991)
            .unwrap_or_else(|error| unreachable!("valid BSP layout: {error:?}"));
        assert_eq!(first, second);
    }

    #[test]
    fn bsp_rooms_and_corridors_do_not_overlap() {
        let program = building_program(BuildingArchetype::House);
        let layout = pack_building_program(&program, site(24, 18), 42)
            .unwrap_or_else(|error| unreachable!("valid BSP layout: {error:?}"));
        let mut occupied = BTreeMap::<MapTile, &'static str>::new();

        for placement in layout.rooms() {
            for tile in rect_tiles(placement.rect) {
                assert_eq!(occupied.insert(tile, "room"), None);
            }
        }
        for corridor in layout.corridors() {
            for tile in rect_tiles(corridor.rect) {
                assert_eq!(occupied.insert(tile, "corridor"), None);
            }
        }
    }

    #[test]
    fn reserved_corridor_bands_form_one_connected_network() {
        let program = building_program(BuildingArchetype::Castle);
        let layout = pack_building_program(&program, site(28, 22), 812)
            .unwrap_or_else(|error| unreachable!("valid BSP layout: {error:?}"));
        let corridor_tiles = layout
            .corridors()
            .iter()
            .flat_map(|corridor| rect_tiles(corridor.rect))
            .collect::<BTreeSet<_>>();
        let start = *corridor_tiles
            .iter()
            .next()
            .unwrap_or_else(|| unreachable!("multi-room program has corridors"));
        let mut visited = BTreeSet::new();
        let mut queue = VecDeque::from([start]);
        while let Some(tile) = queue.pop_front() {
            if !visited.insert(tile) {
                continue;
            }
            for neighbor in cardinal_neighbors(tile) {
                if corridor_tiles.contains(&neighbor) && !visited.contains(&neighbor) {
                    queue.push_back(neighbor);
                }
            }
        }

        assert_eq!(visited, corridor_tiles);
    }

    #[test]
    fn largest_program_room_receives_a_largest_bsp_leaf() {
        let program = building_program(BuildingArchetype::Warehouse);
        let layout = pack_building_program(&program, site(26, 20), 1234)
            .unwrap_or_else(|error| unreachable!("valid BSP layout: {error:?}"));
        let largest_weight_room = program
            .rooms()
            .iter()
            .max_by_key(|room| room.area_weight)
            .unwrap_or_else(|| unreachable!("program has rooms"));
        let assigned = layout
            .room(largest_weight_room.id)
            .unwrap_or_else(|| unreachable!("program room is placed"));
        let largest_area = layout
            .rooms()
            .iter()
            .map(|placement| rect_area(placement.rect))
            .max()
            .unwrap_or(0);

        assert_eq!(rect_area(assigned.rect), largest_area);
    }

    fn rect_tiles(rect: RoomRect) -> Vec<MapTile> {
        let mut tiles = Vec::new();
        for local_y in 0..rect.height {
            for local_x in 0..rect.width {
                tiles.push(MapTile::new(
                    rect.origin.x + i32::from(local_x),
                    rect.origin.y + i32::from(local_y),
                ));
            }
        }
        tiles
    }

    fn cardinal_neighbors(tile: MapTile) -> [MapTile; 4] {
        [
            MapTile::new(tile.x, tile.y + 1),
            MapTile::new(tile.x + 1, tile.y),
            MapTile::new(tile.x, tile.y - 1),
            MapTile::new(tile.x - 1, tile.y),
        ]
    }
}
