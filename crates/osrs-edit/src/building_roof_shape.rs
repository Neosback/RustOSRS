//! Which roof a footprint gets, and the gable roofs real buildings have besides the hip.
//!
//! Measured on the 470 roofed buildings of the real map (see `building_roof_shape/real_roofs.json`):
//! of the plain rectangles, about four in five are hips (`building_roof_hip`), one in five gables.
//! A gable has the edge trim all round and slopes on two opposite sides only:
//!
//! - **Pointed**, when the span between the eaves (inside the trim) is even: each half is slope,
//!   facing its eave, and the two halves meet at the ridge with no flat piece.
//! - **Flat ridge**, when that span is odd: one row of slope on each side and flat top between.
//!
//! The ridge runs along the longer side (21 of 23 real gables). The four corner tiles of the trim
//! are filled one of three ways, all seen in the real map: an outer corner piece, or the trim of
//! the north and south sides running through, or the trim of the west and east sides.
//!
//! Anything that is not a filled rectangle is roofed as a hip over its whole outline, which
//! reproduces real L and T roofs piece for piece (69 of the real ones). A gabled wing meeting a
//! hip, which real roofs do with valley pieces, is not generated.

use crate::{
    HipRoofAssets, LocKind, MapPosition, ObjectOrientation, ObjectPlacement, hip_roof_placements,
};
use osrs_core::{
    coords::{MapTile, SourcePlane},
    ids::ObjectId,
};
use std::collections::BTreeSet;

/// The direction a ridge runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoofAxis {
    EastWest,
    NorthSouth,
}

/// What stands on the four corner tiles of a gable's edge trim.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GableCorner {
    /// The trim's outer corner piece (kind 19 or 21).
    Outer,
    /// The north and south trim runs through the corner.
    Rows,
    /// The west and east trim runs through the corner.
    Columns,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoofChoice {
    Hip,
    Gable {
        ridge: RoofAxis,
        corner: GableCorner,
    },
}

fn mix(seed: u64, salt: u64) -> u64 {
    let mut value = (seed ^ salt.rotate_left(29)).wrapping_add(0x9e37_79b9_7f4a_7c15);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

/// `(min x, min y, width, height)` when the footprint is a filled rectangle.
fn rectangle(footprint: &[MapTile]) -> Option<(i32, i32, i32, i32)> {
    let set: BTreeSet<(i32, i32)> = footprint.iter().map(|t| (t.x, t.y)).collect();
    let (x0, x1) = (
        set.iter().map(|t| t.0).min()?,
        set.iter().map(|t| t.0).max()?,
    );
    let (y0, y1) = (
        set.iter().map(|t| t.1).min()?,
        set.iter().map(|t| t.1).max()?,
    );
    let (w, h) = (x1 - x0 + 1, y1 - y0 + 1);
    (usize::try_from(w * h).ok()? == set.len()).then_some((x0, y0, w, h))
}

/// The roof for `footprint`: a gable on `assets.gable_percent` of the plain rectangles that can
/// take one (trimmed, three tiles by four at least), else a hip. The same `seed` and `salt` give
/// the same choice, so a storey's roof does not change when another is regenerated.
pub fn plan_roof_choice(
    footprint: &[MapTile],
    assets: HipRoofAssets,
    seed: u64,
    salt: u64,
) -> RoofChoice {
    let Some((_, _, w, h)) = rectangle(footprint) else {
        return RoofChoice::Hip;
    };
    let ridge = match w.cmp(&h) {
        std::cmp::Ordering::Greater => RoofAxis::EastWest,
        std::cmp::Ordering::Less => RoofAxis::NorthSouth,
        std::cmp::Ordering::Equal => {
            if mix(seed, salt ^ 0x51d6).is_multiple_of(2) {
                RoofAxis::EastWest
            } else {
                RoofAxis::NorthSouth
            }
        }
    };
    let (along, across) = match ridge {
        RoofAxis::EastWest => (w, h),
        RoofAxis::NorthSouth => (h, w),
    };
    // Inside the trim: at least one tile along the ridge, and two across it.
    let fits = assets.trim && along >= 3 && across >= 4;
    if !fits || mix(seed, salt) % 100 >= u64::from(assets.gable_percent) {
        return RoofChoice::Hip;
    }
    // Measured among real gables: outer corners 6, north-south trim 9, west-east trim 11.
    let corner = match mix(seed, salt ^ 0xc0a1) % 26 {
        0..=5 => GableCorner::Outer,
        6..=14 => GableCorner::Rows,
        _ => GableCorner::Columns,
    };
    RoofChoice::Gable { ridge, corner }
}

/// The pieces of the roof `choice` for `footprint` on `plane`. A gable the footprint cannot take
/// (not a filled rectangle, too small, no trim) is roofed as a hip instead.
pub fn roof_placements(
    footprint: &[MapTile],
    choice: RoofChoice,
    assets: HipRoofAssets,
    plane: SourcePlane,
) -> Vec<ObjectPlacement> {
    if let RoofChoice::Gable { ridge, corner } = choice
        && let Some(pieces) = gable_roof_placements(footprint, ridge, corner, assets, plane)
    {
        return pieces;
    }
    hip_roof_placements(footprint, assets, plane)
}

/// A gable over a filled rectangle; `None` when `footprint` is not one, has no room for a ridge
/// inside its trim, or `assets` has no trim.
pub fn gable_roof_placements(
    footprint: &[MapTile],
    ridge: RoofAxis,
    corner: GableCorner,
    assets: HipRoofAssets,
    plane: SourcePlane,
) -> Option<Vec<ObjectPlacement>> {
    let (x0, y0, w, h) = rectangle(footprint)?;
    let (along, across) = match ridge {
        RoofAxis::EastWest => (w - 2, h - 2),
        RoofAxis::NorthSouth => (h - 2, w - 2),
    };
    if !assets.trim || along < 1 || across < 2 {
        return None;
    }
    let mut pieces = Vec::with_capacity(footprint.len());
    for y in y0..y0 + h {
        for x in x0..x0 + w {
            let (south, north, west, east) = (y == y0, y == y0 + h - 1, x == x0, x == x0 + w - 1);
            let (object, kind, orientation) = if south || north || west || east {
                let corner_tile = (south || north) && (west || east);
                let (kind, orientation) = if !corner_tile {
                    (18, side(south, north, west))
                } else {
                    match corner {
                        GableCorner::Outer => {
                            (assets.kinds.outer_edge, diagonal(south, west, north))
                        }
                        GableCorner::Rows => (18, if south { 1 } else { 3 }),
                        GableCorner::Columns => (18, if west { 2 } else { 0 }),
                    }
                };
                (assets.edge, kind, orientation)
            } else {
                let (kind, orientation) = interior(ridge, x - x0 - 1, y - y0 - 1, w - 2, h - 2);
                (assets.slope, kind, orientation)
            };
            pieces.push(ObjectPlacement {
                object_id: object,
                position: MapPosition::new(MapTile::new(x, y), plane),
                kind: LocKind::new(kind)?,
                orientation: ObjectOrientation::new(orientation)?,
            });
        }
    }
    Some(pieces)
}

/// A straight trim piece's orientation names the side that is outside (south 1, north 3, west 2,
/// east 0).
const fn side(south: bool, north: bool, west: bool) -> u8 {
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

/// A corner piece's orientation names the outside diagonal (south-east 0, south-west 1,
/// north-west 2, north-east 3).
const fn diagonal(south: bool, west: bool, north: bool) -> u8 {
    match (south, west, north) {
        (true, false, _) => 0,
        (true, true, _) => 1,
        (false, true, true) => 2,
        _ => 3,
    }
}

/// The slope or flat piece at inside position `(ix, iy)` of an `iw` by `ih` interior. Slopes are
/// oriented toward the eave they drain to.
fn interior(ridge: RoofAxis, ix: i32, iy: i32, iw: i32, ih: i32) -> (u8, u8) {
    // Position across the ridge and the span across it.
    let (at, span, low, high) = match ridge {
        RoofAxis::EastWest => (iy, ih, 1, 3),
        RoofAxis::NorthSouth => (ix, iw, 2, 0),
    };
    if span % 2 == 0 {
        (12, if at < span / 2 { low } else { high })
    } else if at == 0 {
        (12, low)
    } else if at == span - 1 {
        (12, high)
    } else {
        (17, 1)
    }
}

#[cfg(test)]
mod tests;
