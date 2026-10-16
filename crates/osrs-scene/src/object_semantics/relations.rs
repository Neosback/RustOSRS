//! Relations measured on real placements: what continues what, what caps a run, which piece is
//! a corner between two walls, and what sits inside a wall.
//!
//! Boundary pieces live on tile edges numbered by the client's orientation: 0 west, 1 north,
//! 2 east, 3 south. A straight wall (type 0, orientation `o`) owns edge `o`; a corner (type 2)
//! owns edges `o` and `o + 1`; a pillar (types 1 and 3) owns the vertex between those same two
//! edges. Only the planes and tiles given are looked at, so relations that cross the border of
//! the supplied placements are not seen (a run that leaves the data looks open).
use osrs_core::ids::ObjectId;
use std::collections::{BTreeMap, HashMap};

/// One placed object, in absolute tile coordinates on its source plane.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Placed {
    pub id: ObjectId,
    pub kind: u8,
    pub orientation: u8,
    pub tile: (i32, i32),
    pub plane: u8,
}

/// What stands at the end of a straight run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum End {
    /// Nothing at the end tile's edge or vertex.
    Open,
    /// A wall meeting it at a right angle on the same tile.
    Perpendicular(ObjectId),
    /// A corner post on the shared vertex of the same tile.
    Pillar(ObjectId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Relation {
    /// Two straight walls on consecutive tiles of one edge line (ordered by ID).
    Continues(ObjectId, ObjectId),
    /// A corner piece at the end of a run of `arm`.
    CornerArm { corner: ObjectId, arm: ObjectId },
    /// A straight wall whose run stops here.
    RunEnd { wall: ObjectId, end: End },
    /// `inset` stands between two walls of the same edge line and is not one of them.
    Inset { host: ObjectId, inset: ObjectId },
    /// A wall decoration on the same tile edge as `host`.
    Decorates {
        decoration: ObjectId,
        host: ObjectId,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Piece {
    Straight,
    Corner,
}

type EdgeKey = (u8, (i32, i32), u8);

/// Counted relations and per-object occurrences. Regions can be added one at a time; analysis
/// is deterministic for the same placements in any order.
#[derive(Debug, Default, Clone)]
pub struct Census {
    relations: BTreeMap<Relation, u32>,
    occurrences: BTreeMap<ObjectId, u32>,
}

/// Step along the edge line: west and east edges run north-south, north and south run east-west.
fn axis(edge: u8) -> (i32, i32) {
    if edge.is_multiple_of(2) {
        (0, 1)
    } else {
        (1, 0)
    }
}

/// The edge crossing the `+axis` end of an edge, and the one crossing the `-axis` end.
const fn end_edges(edge: u8) -> (u8, u8) {
    match edge {
        0 | 2 => (1, 3),
        _ => (2, 0),
    }
}

/// Vertex index (the pillar orientation `o` owns the vertex between edges `o` and `o + 1`).
const fn vertex_of(a: u8, b: u8) -> u8 {
    // Adjacent edges differ by one modulo four; the vertex is named by the lower in that cycle.
    if (a + 1) % 4 == b { a } else { b }
}

impl Census {
    pub fn relations(&self) -> &BTreeMap<Relation, u32> {
        &self.relations
    }

    pub fn occurrences(&self, id: ObjectId) -> u32 {
        self.occurrences.get(&id).copied().unwrap_or(0)
    }

    pub fn objects(&self) -> impl Iterator<Item = ObjectId> + '_ {
        self.occurrences.keys().copied()
    }

    fn count(&mut self, relation: Relation) {
        *self.relations.entry(relation).or_default() += 1;
    }

    /// Add every placement of a region (or any set that is closed under adjacency).
    pub fn add(&mut self, placements: &[Placed]) {
        let mut grid = Grid::default();
        for p in placements {
            *self.occurrences.entry(p.id).or_default() += 1;
            let o = p.orientation % 4;
            match p.kind {
                0 => grid.put_edge(p, o, Piece::Straight),
                2 => {
                    grid.put_edge(p, o, Piece::Corner);
                    grid.put_edge(p, (o + 1) % 4, Piece::Corner);
                }
                1 | 3 => {
                    grid.vertices.insert((p.plane, p.tile, o), p.id);
                }
                _ => {}
            }
        }
        for p in placements {
            let o = p.orientation % 4;
            match p.kind {
                0 => self.straight(&grid, p, o),
                4 | 5 => {
                    for (host, _) in grid.at(p.plane, p.tile, o) {
                        self.count(Relation::Decorates {
                            decoration: p.id,
                            host: *host,
                        });
                    }
                }
                _ => {}
            }
        }
    }

    fn straight(&mut self, grid: &Grid, p: &Placed, edge: u8) {
        let (dx, dy) = axis(edge);
        let ahead = grid.at(p.plane, (p.tile.0 + dx, p.tile.1 + dy), edge);
        let behind = grid.at(p.plane, (p.tile.0 - dx, p.tile.1 - dy), edge);
        let (end_ahead, end_behind) = end_edges(edge);
        // Only the forward neighbour counts a straight continuation, so each pair is seen once;
        // a corner is an arm's neighbour on either side.
        for (other, piece) in ahead {
            match piece {
                Piece::Straight => {
                    self.count(Relation::Continues(p.id.min(*other), p.id.max(*other)));
                }
                Piece::Corner => self.count(Relation::CornerArm {
                    corner: *other,
                    arm: p.id,
                }),
            }
        }
        for (other, _) in behind.iter().filter(|(_, piece)| *piece == Piece::Corner) {
            self.count(Relation::CornerArm {
                corner: *other,
                arm: p.id,
            });
        }
        for (neighbours, end_edge) in [(ahead, end_ahead), (behind, end_behind)] {
            if !neighbours.is_empty() {
                continue;
            }
            let end = if let Some((wall, _)) = grid.at(p.plane, p.tile, end_edge).first() {
                End::Perpendicular(*wall)
            } else if let Some(pillar) =
                grid.vertices
                    .get(&(p.plane, p.tile, vertex_of(edge, end_edge)))
            {
                End::Pillar(*pillar)
            } else {
                End::Open
            };
            self.count(Relation::RunEnd { wall: p.id, end });
        }
        let straight = |list: &[(ObjectId, Piece)]| -> Vec<ObjectId> {
            let mut ids: Vec<_> = list
                .iter()
                .filter(|(_, piece)| *piece == Piece::Straight)
                .map(|(id, _)| *id)
                .collect();
            ids.dedup();
            ids
        };
        let (before, after) = (straight(behind), straight(ahead));
        // A piece is an inset only when it is a different object from both of its neighbours;
        // the end of a run of two styles is a join, not an inset.
        if !before.is_empty() && !after.is_empty() {
            let mut hosts: Vec<ObjectId> = before.into_iter().chain(after).collect();
            hosts.sort_unstable();
            hosts.dedup();
            if !hosts.contains(&p.id) {
                for host in hosts {
                    self.count(Relation::Inset { host, inset: p.id });
                }
            }
        }
    }

    /// Fold another census in (regions analysed separately or in parallel).
    pub fn merge(&mut self, other: &Census) {
        for (relation, n) in &other.relations {
            *self.relations.entry(*relation).or_default() += n;
        }
        for (id, n) in &other.occurrences {
            *self.occurrences.entry(*id).or_default() += n;
        }
    }
}

/// Edge and vertex occupancy of the placements being analysed.
#[derive(Default)]
struct Grid {
    edges: HashMap<EdgeKey, Vec<(ObjectId, Piece)>>,
    vertices: HashMap<EdgeKey, ObjectId>,
}

impl Grid {
    fn put_edge(&mut self, p: &Placed, edge: u8, piece: Piece) {
        self.edges
            .entry((p.plane, p.tile, edge))
            .or_default()
            .push((p.id, piece));
    }

    fn at(&self, plane: u8, tile: (i32, i32), edge: u8) -> &[(ObjectId, Piece)] {
        self.edges
            .get(&(plane, tile, edge))
            .map_or(&[], Vec::as_slice)
    }
}

#[cfg(test)]
mod tests;
