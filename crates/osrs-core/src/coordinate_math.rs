//! Exact pure coordinate helpers shared by later semantic milestones.
//!
//! This module does not perform loc dispatch, orientation-based footprint swaps,
//! height sampling, or scene insertion. Those remain M6 responsibilities.

use crate::coords::{HALF_TILE, LocalCoord, LocalXZ, MapTile};

/// Exact semantic center of one map tile.
///
/// One tile is 128 local units, so its center is exactly `+64` on each
/// horizontal axis from the tile origin.
pub fn tile_center(tile: MapTile) -> Option<LocalXZ> {
    footprint_center(tile, 1, 1)
}

/// Exact semantic center of a nonzero tile footprint whose south-west/source
/// origin is `origin`.
///
/// This is the pure integer center term used by later placement code:
///
/// `origin_local + footprint_tiles * 64`
///
/// Orientation-driven width/depth transposition and height sampling are not
/// performed here.
pub fn footprint_center(origin: MapTile, width_tiles: u16, depth_tiles: u16) -> Option<LocalXZ> {
    if width_tiles == 0 || depth_tiles == 0 {
        return None;
    }

    let origin = origin.local_origin()?;
    let x_offset = i32::from(width_tiles).checked_mul(HALF_TILE)?;
    let z_offset = i32::from(depth_tiles).checked_mul(HALF_TILE)?;

    Some(LocalXZ::new(
        origin.x.checked_add(LocalCoord::from_units(x_offset))?,
        origin.z.checked_add(LocalCoord::from_units(z_offset))?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_by_one_tile_center_is_exact_half_tile() {
        assert_eq!(
            tile_center(MapTile::new(10, 20)),
            Some(LocalXZ::new(
                LocalCoord::from_units(10 * 128 + 64),
                LocalCoord::from_units(20 * 128 + 64),
            ))
        );
    }

    #[test]
    fn non_square_footprint_centers_are_exact() {
        let origin = MapTile::new(50, 60);

        assert_eq!(
            footprint_center(origin, 2, 3),
            Some(LocalXZ::new(
                LocalCoord::from_units(50 * 128 + 2 * 64),
                LocalCoord::from_units(60 * 128 + 3 * 64),
            ))
        );
        assert_eq!(
            footprint_center(origin, 3, 2),
            Some(LocalXZ::new(
                LocalCoord::from_units(50 * 128 + 3 * 64),
                LocalCoord::from_units(60 * 128 + 2 * 64),
            ))
        );
    }

    #[test]
    fn negative_world_tiles_keep_exact_euclidean_world_identity() {
        assert_eq!(
            tile_center(MapTile::new(-1, -1)),
            Some(LocalXZ::new(
                LocalCoord::from_units(-64),
                LocalCoord::from_units(-64),
            ))
        );
    }

    #[test]
    fn zero_sized_footprints_are_rejected() {
        let origin = MapTile::new(0, 0);
        assert_eq!(footprint_center(origin, 0, 1), None);
        assert_eq!(footprint_center(origin, 1, 0), None);
    }

    #[test]
    fn overflow_fails_closed() {
        assert_eq!(tile_center(MapTile::new(i32::MAX, 0)), None);
    }
}
