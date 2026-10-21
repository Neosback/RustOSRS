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

/// Which loc kind a roof uses for its three kinds of corner. Real roofs come in two styles that
/// differ only in the kind number: the outer corner of the edge trim is 19 or 21, the outer slope
/// corner 16 or 13, and the inner slope corner 15 or 14. The styles cluster by map area (one
/// author's roofs are all one or the other), the pieces are placed identically, and a kit's
/// objects model one of them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RoofKinds {
    pub outer_edge: u8,
    pub outer_slope: u8,
    pub inner_slope: u8,
}

impl RoofKinds {
    /// The kinds the Lumbridge roof uses.
    pub const STANDARD: Self = Self {
        outer_edge: 19,
        outer_slope: 16,
        inner_slope: 15,
    };

    /// Whether each kind is one of the two a roof piece can have.
    pub const fn is_valid(self) -> bool {
        matches!(self.outer_edge, 19 | 21)
            && matches!(self.outer_slope, 16 | 13)
            && matches!(self.inner_slope, 15 | 14)
    }
}

/// The two objects a roof is made of, how its corners are numbered, and how it is chosen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HipRoofAssets {
    /// Slope and flat top.
    pub slope: ObjectId,
    /// Edge trim.
    pub edge: ObjectId,
    pub kinds: RoofKinds,
    /// Whether the roof has the edge ring. Some real roofs are all slope to the eaves.
    pub trim: bool,
    /// Out of 100: how often a plain rectangle gets a gable instead of a hip (see
    /// `building_roof_gable`). Real rectangular roofs with trim are about one in five gables.
    pub gable_percent: u8,
}

impl HipRoofAssets {
    /// A hip roof of the standard kinds with trim, chosen as the measured mix of hips and gables.
    pub const fn new(slope: ObjectId, edge: ObjectId) -> Self {
        Self {
            slope,
            edge,
            kinds: RoofKinds::STANDARD,
            trim: true,
            gable_percent: 20,
        }
    }
}

/// Loc kinds by layer: index 0 is the edge ring, index 1 the slope ring.
const STRAIGHT: [u8; 2] = [18, 12];
const INNER_EDGE: u8 = 20;
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
fn piece(
    inside: &BTreeSet<(i32, i32)>,
    tile: (i32, i32),
    layer: usize,
    kinds: RoofKinds,
) -> Option<(u8, u8)> {
    let outer = [kinds.outer_edge, kinds.outer_slope];
    let inner = [INNER_EDGE, kinds.inner_slope];
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
            Some((inner[layer], orientation))
        }
        1 => Some((STRAIGHT[layer], side_orientation(no_s, no_n, no_w))),
        2 if (no_n && no_s) || (no_e && no_w) => {
            // Opposite sides outside: a ridge one tile wide. Lie flat.
            Some((if layer == 1 { FLAT } else { STRAIGHT[0] }, 1))
        }
        2 => Some((outer[layer], diagonal_orientation(no_s, no_w, no_n))),
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
    // Without trim the outermost ring is the slope ring: layer 0 is the slope layer.
    let edge = if assets.trim {
        ring(&layer0)
    } else {
        BTreeSet::new()
    };
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
            if let Some((kind, orientation)) = piece(&layer0, tile, 0, assets.kinds) {
                put(tile, assets.edge, kind, orientation);
            }
        } else if slope.contains(&tile) {
            if let Some((kind, orientation)) = piece(&layer1, tile, 1, assets.kinds) {
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
