//! Wall edges: which sides of which tiles have a wall, a door or a window on them.
//!
//! Boundary pieces sit on tile edges numbered by the client's orientation: 0 west, 1 north,
//! 2 east, 3 south. A straight wall (type 0, orientation `o`) owns edge `o`; an L corner (type 2)
//! owns edges `o` and `o + 1`. Pillars (types 1 and 3) own no edge.

use super::{ObjectInfo, ObjectTable};
use crate::object_semantics::Placed;
use osrs_core::ids::ObjectId;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Dir {
    West,
    North,
    East,
    South,
}

impl Dir {
    pub const ALL: [Self; 4] = [Self::West, Self::North, Self::East, Self::South];

    pub const fn from_orientation(orientation: u8) -> Self {
        match orientation & 3 {
            0 => Self::West,
            1 => Self::North,
            2 => Self::East,
            _ => Self::South,
        }
    }

    pub const fn step(self) -> (i32, i32) {
        match self {
            Self::West => (-1, 0),
            Self::North => (0, 1),
            Self::East => (1, 0),
            Self::South => (0, -1),
        }
    }

    pub const fn opposite(self) -> Self {
        match self {
            Self::West => Self::East,
            Self::North => Self::South,
            Self::East => Self::West,
            Self::South => Self::North,
        }
    }

    pub const fn next(self) -> Self {
        match self {
            Self::West => Self::North,
            Self::North => Self::East,
            Self::East => Self::South,
            Self::South => Self::West,
        }
    }

    pub fn from(self, tile: (i32, i32)) -> (i32, i32) {
        let (dx, dy) = self.step();
        (tile.0 + dx, tile.1 + dy)
    }
}

/// What a boundary piece is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EdgeFeature {
    Wall,
    Door,
    Window,
    /// A gap in a straight wall run that is open (no door object): a doorway or a wide entrance.
    /// There is no object behind it.
    Opening,
}

/// The object id standing in for "no object" on an opening.
pub const OPENING_OBJECT: ObjectId = ObjectId::new(u32::MAX);

/// Longest gap in a wall run still taken for an open doorway.
const MAX_OPENING: i32 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EdgePiece {
    pub object: ObjectId,
    pub kind: u8,
    pub feature: EdgeFeature,
}

/// Decide whether a boundary object is a door or a window (by name, symbol and verbs), else a wall.
pub fn feature_of(info: Option<&ObjectInfo>) -> EdgeFeature {
    let Some(info) = info else {
        return EdgeFeature::Wall;
    };
    let door_word = ["door", "gate"].iter().any(|w| info.says(w));
    let verb = info.actions.iter().any(|a| a == "open" || a == "close");
    if door_word || verb {
        EdgeFeature::Door
    } else if info.says("window") {
        EdgeFeature::Window
    } else {
        EdgeFeature::Wall
    }
}

/// Every edge piece on one plane, found from either side.
#[derive(Debug, Default, Clone)]
pub struct Edges {
    owned: HashMap<((i32, i32), Dir), EdgePiece>,
}

impl Edges {
    pub fn of_plane(placements: &[Placed], plane: u8, objects: &ObjectTable) -> Self {
        let mut edges = Self::default();
        for placed in placements.iter().filter(|p| p.plane == plane) {
            let feature = feature_of(objects.get(&placed.id));
            let piece = EdgePiece {
                object: placed.id,
                kind: placed.kind,
                feature,
            };
            let dir = Dir::from_orientation(placed.orientation);
            match placed.kind {
                0 => edges.insert(placed.tile, dir, piece),
                2 => {
                    edges.insert(placed.tile, dir, piece);
                    edges.insert(placed.tile, dir.next(), piece);
                }
                _ => {}
            }
        }
        edges.close_openings();
        edges
    }

    /// Doorways without a door: a gap of up to two edges in a straight wall run whose far end is a
    /// wall again. They keep the room closed and are recorded as `Opening` pieces.
    fn close_openings(&mut self) {
        let mut found = Vec::new();
        for &(tile, dir) in self.owned.keys() {
            // The run goes along the edge: north-south edges run east-west and the reverse.
            let along = match dir {
                Dir::West | Dir::East => (0, 1),
                Dir::North | Dir::South => (1, 0),
            };
            for gap in 1..=MAX_OPENING {
                let far = (tile.0 + along.0 * (gap + 1), tile.1 + along.1 * (gap + 1));
                if self.between(far, dir).is_none() {
                    continue;
                }
                let tiles: Vec<_> = (1..=gap)
                    .map(|k| (tile.0 + along.0 * k, tile.1 + along.1 * k))
                    .collect();
                if tiles.iter().all(|t| self.between(*t, dir).is_none()) {
                    found.extend(tiles.into_iter().map(|t| (t, dir)));
                }
                break;
            }
        }
        for (tile, dir) in found {
            self.owned.entry((tile, dir)).or_insert(EdgePiece {
                object: OPENING_OBJECT,
                kind: 255,
                feature: EdgeFeature::Opening,
            });
        }
    }

    fn insert(&mut self, tile: (i32, i32), dir: Dir, piece: EdgePiece) {
        // A door or window wins over a plain wall at the same place (the wall is its frame).
        let keep = self.owned.get(&(tile, dir)).is_some_and(|old| {
            old.feature != EdgeFeature::Wall && piece.feature == EdgeFeature::Wall
        });
        if !keep {
            self.owned.insert((tile, dir), piece);
        }
    }

    /// The piece on the edge of `tile` toward `dir`, whichever tile owns it.
    pub fn between(&self, tile: (i32, i32), dir: Dir) -> Option<EdgePiece> {
        self.owned
            .get(&(tile, dir))
            .or_else(|| self.owned.get(&(dir.from(tile), dir.opposite())))
            .copied()
    }

    pub fn pieces(&self) -> impl Iterator<Item = ((i32, i32), Dir, EdgePiece)> + '_ {
        self.owned
            .iter()
            .map(|(&(tile, dir), &piece)| (tile, dir, piece))
    }
}
