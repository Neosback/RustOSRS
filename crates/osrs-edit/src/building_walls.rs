//! One wall piece per tile.
//!
//! The game keeps a single boundary-object slot on each tile: a second straight wall placed on the
//! same tile (a corner, a T-junction, a one-wide corridor between two rooms) is lost when the map
//! loads, which shows as walls missing from a building. A wall is an edge between two tiles, and
//! either tile may own it, so a piece that shares its tile is moved to the neighbouring tile on the
//! same edge (turned to face back the other way). This assigns every edge to a tile with at most
//! one edge per tile by bipartite matching, keeping each piece on its original tile when it can.
//! A piece that cannot be placed (more edges than free tiles around it) is reported, not dropped.

use crate::{MapOp, MapPatch, MapPosition, ObjectOrientation, ObjectPlacement};
use osrs_core::coords::MapTile;
use std::collections::HashMap;

const fn step(orientation: u8) -> (i32, i32) {
    match orientation & 3 {
        0 => (-1, 0),
        1 => (0, 1),
        2 => (1, 0),
        _ => (0, -1),
    }
}

/// The tile on the other side of the wall and the orientation that faces back.
fn across(tile: MapTile, orientation: u8) -> (MapTile, u8) {
    let (dx, dy) = step(orientation);
    (
        MapTile::new(tile.x + dx, tile.y + dy),
        (orientation + 2) & 3,
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Piece {
    placement: ObjectPlacement,
    /// Where it may stand: its own tile first, then the tile across the edge.
    homes: [(MapTile, u8); 2],
}

/// Try to place `piece` on a free tile, moving pieces already placed along their own edges when
/// that frees a tile (an augmenting path). `owner` maps tile to the piece index standing there.
fn place(
    pieces: &[Piece],
    index: usize,
    owner: &mut HashMap<MapTile, usize>,
    seen: &mut Vec<MapTile>,
    chosen: &mut [usize],
) -> bool {
    for (slot, (tile, _)) in pieces[index].homes.iter().enumerate() {
        if seen.contains(tile) {
            continue;
        }
        seen.push(*tile);
        let free = match owner.get(tile) {
            None => true,
            Some(&other) => place(pieces, other, owner, seen, chosen),
        };
        if free {
            owner.insert(*tile, index);
            chosen[index] = slot;
            return true;
        }
    }
    false
}

/// Rewrite the straight wall pieces (kind 0) of `patch` so that no tile holds more than one.
/// Returns the patch and how many pieces could not be given a tile of their own.
pub fn resolve_wall_slots(patch: &MapPatch) -> (MapPatch, usize) {
    let mut pieces: Vec<Piece> = Vec::new();
    let mut other: Vec<MapOp> = Vec::with_capacity(patch.ops().len());
    for op in patch.ops() {
        match *op {
            MapOp::AddObject { placement } if placement.kind.get() == 0 => {
                let o = placement.orientation.get();
                pieces.push(Piece {
                    placement,
                    homes: [
                        (placement.position.tile, o),
                        across(placement.position.tile, o),
                    ],
                });
            }
            keep => other.push(keep),
        }
    }
    let mut owner: HashMap<MapTile, usize> = HashMap::new();
    let mut chosen = vec![0usize; pieces.len()];
    let mut unplaced = 0;
    let mut stuck: Vec<usize> = Vec::new();
    for index in 0..pieces.len() {
        let mut seen = Vec::new();
        if !place(&pieces, index, &mut owner, &mut seen, &mut chosen) {
            unplaced += 1;
            stuck.push(index);
        }
    }
    // Rebuild in the original order so the patch stays deterministic.
    let mut ops = other;
    for (index, piece) in pieces.iter().enumerate() {
        let (tile, orientation) = if stuck.contains(&index) {
            piece.homes[0]
        } else {
            piece.homes[chosen[index]]
        };
        let mut placement = piece.placement;
        placement.position = MapPosition::new(tile, placement.position.plane);
        if let Some(o) = ObjectOrientation::new(orientation) {
            placement.orientation = o;
        }
        ops.push(MapOp::AddObject { placement });
    }
    (MapPatch::new(ops), unplaced)
}

#[cfg(test)]
mod tests;
