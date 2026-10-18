//! What real buildings are made of, measured on real maps.
//!
//! Given the ground and the placed objects of an area, this finds the enclosed rooms (tiles whose
//! every way out crosses a wall), groups rooms that share walls into buildings, and records for
//! each building: which walls face outside and which divide rooms, what kind of piece each is
//! (plain wall, door, window), the wall decorations on each side, the floor under each room, the
//! furniture in it and where it stands against the walls, the roof over it, and how the ground
//! under and around it is shaped and coloured.
//!
//! It is pure analysis over canonical values: it reads no cache and writes nothing, so the same
//! code runs over a decoded region, a loaded editor map, or a hand-made test grid.

pub mod description;
mod edges;
mod rooms;
mod tags;

pub use edges::{Dir, EdgeFeature, EdgePiece, Edges, OPENING_OBJECT};
pub use rooms::analyze;
pub use tags::{RoomTag, tag_room};

use osrs_core::ids::ObjectId;
use std::collections::BTreeMap;

/// What the census needs to know about an object definition.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ObjectInfo {
    /// Lowercase cache name (empty when none).
    pub name: String,
    /// Lowercase gameval symbol without the table prefix (empty when none).
    pub symbol: String,
    /// Lowercase menu verbs.
    pub actions: Vec<String>,
    pub size: (u8, u8),
}

impl ObjectInfo {
    /// Whether the name or symbol contains `word`.
    pub fn says(&self, word: &str) -> bool {
        self.name.contains(word) || self.symbol.contains(word)
    }
}

pub type ObjectTable = BTreeMap<ObjectId, ObjectInfo>;

/// One ground tile: floor ids, shape, and the height of its south-west corner.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Ground {
    pub underlay: u16,
    pub overlay: u16,
    pub shape: u8,
    pub settings: u8,
    pub height: i32,
    /// Whether the height was authored (the map file gives one) rather than computed noise.
    pub height_known: bool,
}

/// The ground of a rectangle of tiles on four planes.
#[derive(Debug, Clone)]
pub struct CensusArea {
    pub origin: (i32, i32),
    pub width: usize,
    pub height: usize,
    ground: Vec<Ground>,
}

impl CensusArea {
    pub fn new(origin: (i32, i32), width: usize, height: usize) -> Self {
        Self {
            origin,
            width,
            height,
            ground: vec![Ground::default(); 4 * width * height],
        }
    }

    fn index(&self, plane: u8, tile: (i32, i32)) -> Option<usize> {
        let x = usize::try_from(tile.0 - self.origin.0).ok()?;
        let y = usize::try_from(tile.1 - self.origin.1).ok()?;
        (x < self.width && y < self.height && plane < 4)
            .then(|| (usize::from(plane) * self.width + x) * self.height + y)
    }

    pub fn set(&mut self, plane: u8, tile: (i32, i32), ground: Ground) {
        if let Some(index) = self.index(plane, tile) {
            self.ground[index] = ground;
        }
    }

    pub fn ground(&self, plane: u8, tile: (i32, i32)) -> Option<Ground> {
        self.index(plane, tile).map(|index| self.ground[index])
    }

    pub fn contains(&self, tile: (i32, i32)) -> bool {
        self.index(0, tile).is_some()
    }
}

/// A floor material: overlay id and shape, or the underlay when there is no overlay.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Material {
    pub underlay: u16,
    pub overlay: u16,
    pub shape: u8,
}

impl From<Ground> for Material {
    fn from(ground: Ground) -> Self {
        Self {
            underlay: ground.underlay,
            overlay: ground.overlay,
            shape: if ground.overlay == 0 { 0 } else { ground.shape },
        }
    }
}

/// Where a piece of furniture stands in its room.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Furniture {
    pub object: ObjectId,
    pub kind: u8,
    pub tile: (i32, i32),
    pub orientation: u8,
    /// How many tiles from the nearest wall of the room (0 = against it).
    pub wall_distance: u8,
    /// The side of the nearest wall (west 0, north 1, east 2, south 3), when it is within 1 tile.
    pub against: Option<Dir>,
}

/// A decoration on a wall edge, and which side of the wall it is on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Decoration {
    pub object: ObjectId,
    pub kind: u8,
    pub tile: (i32, i32),
    pub orientation: u8,
    /// The edge piece it is attached to, when there is one on that tile edge.
    pub on: Option<ObjectId>,
    /// Which side of its own tile the wall edge it hangs on is (west 0 .. south 3).
    pub edge_side: Option<Dir>,
    pub inside: bool,
}

#[derive(Debug, Clone)]
pub struct Room {
    pub tiles: Vec<(i32, i32)>,
    pub plane: u8,
    pub tag: RoomTag,
    /// Floor materials and how many tiles use each.
    pub floor: BTreeMap<Material, u32>,
    pub furniture: Vec<Furniture>,
    pub decorations: Vec<Decoration>,
    /// Ground decoration (kind 22) objects inside.
    pub ground_decor: Vec<(ObjectId, (i32, i32))>,
    /// Fraction of tiles with a roof piece on the plane above.
    pub roofed: f32,
}

/// A wall edge of a building and what faces which way.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WallRecord {
    pub piece: EdgePiece,
    pub tile: (i32, i32),
    pub dir: Dir,
    /// True when one side is outside the building; false when it divides two rooms.
    pub exterior: bool,
}

#[derive(Debug, Clone)]
pub struct Building {
    pub plane: u8,
    pub rooms: Vec<Room>,
    pub walls: Vec<WallRecord>,
    /// Roof pieces over the building: object, kind.
    pub roof: BTreeMap<(ObjectId, u8), u32>,
    /// The ground just outside the walls (one tile ring) by material.
    pub ring: BTreeMap<Material, u32>,
    /// For each exterior door: the ground materials of the first and second tile outside it.
    pub door_aprons: Vec<(Material, Material)>,
    /// Corner-height spread (max - min, client units) under the building and in the outside ring.
    pub spread_inside: i32,
    pub spread_ring: i32,
}

impl Building {
    pub fn tiles(&self) -> impl Iterator<Item = (i32, i32)> + '_ {
        self.rooms
            .iter()
            .flat_map(|room| room.tiles.iter().copied())
    }

    pub fn bounds(&self) -> ((i32, i32), (i32, i32)) {
        let mut min = (i32::MAX, i32::MAX);
        let mut max = (i32::MIN, i32::MIN);
        for tile in self.tiles() {
            min = (min.0.min(tile.0), min.1.min(tile.1));
            max = (max.0.max(tile.0), max.1.max(tile.1));
        }
        (min, max)
    }

    pub fn size(&self) -> (i32, i32) {
        let (min, max) = self.bounds();
        (max.0 - min.0 + 1, max.1 - min.1 + 1)
    }
}

#[cfg(test)]
mod tests;
