//! Hip roofs from the shapes real buildings use.
//!
//! A real OSRS hip roof is three layers over the footprint, measured on Lumbridge: an edge ring
//! (loc kinds 18 straight, 19 outer corner, 20 inner corner), a slope ring just inside it (kinds
//! 12, 16, 15) and a flat top (kind 17). The orientation of a straight piece names the side that
//! is outside (south 1, north 3, west 2, east 0); a corner piece names the diagonal that is
//! outside (south-east 0, south-west 1, north-west 2, north-east 3). Which tiles are ring, slope
//! or top follows from the footprint alone, so the same function roofs a rectangle, an L or a T.

use crate::{LocKind, MapPosition, ObjectOrientation, ObjectPlacement};
use osrs_core::{
    coords::{MapTile, SourcePlane},
    ids::ObjectId,
};
use std::collections::BTreeSet;

/// The two objects a hip roof is made of: one for the slope and flat top, one for the edge trim.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HipRoofAssets {
    pub slope: ObjectId,
    pub edge: ObjectId,
}

/// Loc kinds by layer: index 0 is the edge ring, index 1 the slope ring.
const STRAIGHT: [u8; 2] = [18, 12];
const OUTER: [u8; 2] = [19, 16];
const INNER: [u8; 2] = [20, 15];
const FLAT: u8 = 17;

/// Orientation of a piece with its outside to the `south`, `north`, `west` or `east`.
const fn side_orientation(south: bool, north: bool, west: bool) -> u8 {
    if south {
        1
    } else if north {
        3
    } else if west {
        2
    } else {
        0
    }
}

/// Orientation of a corner whose outside diagonal is toward the given sides.
const fn diagonal_orientation(south: bool, west: bool, north: bool) -> u8 {
    match (south, west, north) {
        (true, false, _) => 0,
        (true, true, _) => 1,
        (false, true, true) => 2,
        (false, false, true) => 3,
        // A diagonal is always one of the four; this arm cannot be reached for real input.
        (false, true, false) => 2,
        (false, false, false) => 0,
    }
}

/// One tile's piece: `layer` 0 is the edge ring (second object), 1 the slope ring.
fn piece(inside: &BTreeSet<(i32, i32)>, tile: (i32, i32), layer: usize) -> Option<(u8, u8)> {
    let has = |dx: i32, dy: i32| inside.contains(&(tile.0 + dx, tile.1 + dy));
    let (n, s, e, w) = (has(0, 1), has(0, -1), has(1, 0), has(-1, 0));
    let (ne, nw, se, sw) = (has(1, 1), has(-1, 1), has(1, -1), has(-1, -1));
    let (no_n, no_s, no_e, no_w) = (!n, !s, !e, !w);
    let missing_sides = [no_n, no_s, no_e, no_w].iter().filter(|m| **m).count();
    match missing_sides {
        0 => {
            // All four sides are inside: only a missing diagonal makes this a ring tile.
            let (m_ne, m_nw, m_se, m_sw) = (!ne, !nw, !se, !sw);
            if !(m_ne || m_nw || m_se || m_sw) {
                return None;
            }
            // The outside diagonal: south-east 0, south-west 1, north-west 2, north-east 3.
            let orientation = if m_se {
                0
            } else if m_sw {
                1
            } else if m_nw {
                2
            } else {
                3
            };
            Some((INNER[layer], orientation))
        }
        1 => Some((STRAIGHT[layer], side_orientation(no_s, no_n, no_w))),
        2 if (no_n && no_s) || (no_e && no_w) => {
            // Opposite sides outside: a ridge one tile wide. Lie flat.
            Some((if layer == 1 { FLAT } else { STRAIGHT[0] }, 1))
        }
        2 => Some((OUTER[layer], diagonal_orientation(no_s, no_w, no_n))),
        _ => Some((if layer == 1 { FLAT } else { STRAIGHT[0] }, 1)),
    }
}

/// The roof pieces for `footprint` on `plane`, in tile order. Empty when the footprint is.
pub fn hip_roof_placements(
    footprint: &[MapTile],
    assets: HipRoofAssets,
    plane: SourcePlane,
) -> Vec<ObjectPlacement> {
    let layer0: BTreeSet<(i32, i32)> = footprint.iter().map(|t| (t.x, t.y)).collect();
    // Edge ring: any of the eight neighbours outside. Slope ring and top are measured the same
    // way on what is left.
    let ring = |set: &BTreeSet<(i32, i32)>| -> BTreeSet<(i32, i32)> {
        set.iter()
            .copied()
            .filter(|&(x, y)| {
                (-1..=1).any(|dx| (-1..=1).any(|dy| !set.contains(&(x + dx, y + dy))))
            })
            .collect()
    };
    let edge = ring(&layer0);
    let layer1: BTreeSet<(i32, i32)> = layer0.difference(&edge).copied().collect();
    let slope = ring(&layer1);
    let mut placements = Vec::with_capacity(layer0.len());
    let mut put = |tile: (i32, i32), object: ObjectId, kind: u8, orientation: u8| {
        if let (Some(kind), Some(orientation)) =
            (LocKind::new(kind), ObjectOrientation::new(orientation))
        {
            placements.push(ObjectPlacement {
                object_id: object,
                position: MapPosition::new(MapTile::new(tile.0, tile.1), plane),
                kind,
                orientation,
            });
        }
    };
    for &tile in &layer0 {
        if edge.contains(&tile) {
            if let Some((kind, orientation)) = piece(&layer0, tile, 0) {
                put(tile, assets.edge, kind, orientation);
            }
        } else if slope.contains(&tile) {
            if let Some((kind, orientation)) = piece(&layer1, tile, 1) {
                put(tile, assets.slope, kind, orientation);
            }
        } else {
            put(tile, assets.slope, FLAT, 1);
        }
    }
    placements
}

#[cfg(test)]
mod tests;
