use osrs_core::coords::LocalPoint;
use std::{error::Error, fmt};

/// Renderer-owned origin expressed in exact semantic local units.
///
/// Rebasing happens in integer space before any future GPU/f32 packing so large
/// world coordinates cannot silently lose semantic precision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RenderOrigin {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl RenderOrigin {
    pub const fn new(x: i32, y: i32, z: i32) -> Self {
        Self { x, y, z }
    }

    pub const fn from_local_point(point: LocalPoint) -> Self {
        Self::new(point.x.units(), point.y.units(), point.z.units())
    }

    pub fn rebase(self, point: LocalPoint) -> Result<RenderPoint, RenderCoordinateError> {
        let x = point
            .x
            .units()
            .checked_sub(self.x)
            .ok_or(RenderCoordinateError::XOverflow)?;
        let y = point
            .y
            .units()
            .checked_sub(self.y)
            .ok_or(RenderCoordinateError::YOverflow)?;
        let z = point
            .z
            .units()
            .checked_sub(self.z)
            .ok_or(RenderCoordinateError::ZOverflow)?;
        Ok(RenderPoint { x, y, z })
    }
}

/// Exact renderer-relative coordinate before GPU packing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RenderPoint {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

/// Failure to express a semantic local point relative to the selected render origin.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderCoordinateError {
    XOverflow,
    YOverflow,
    ZOverflow,
}

impl fmt::Display for RenderCoordinateError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let axis = match self {
            Self::XOverflow => "x",
            Self::YOverflow => "y",
            Self::ZOverflow => "z",
        };
        write!(
            formatter,
            "renderer coordinate rebase overflow on {axis} axis"
        )
    }
}

impl Error for RenderCoordinateError {}

#[cfg(test)]
mod tests {
    use super::*;
    use osrs_core::coords::{LocalCoord, LocalPoint};

    #[test]
    fn rebase_preserves_exact_integer_offsets_at_large_world_coordinates() {
        let origin = RenderOrigin::new(1_500_000_000, -1_000_000_000, 1_250_000_000);
        let point = LocalPoint::new(
            LocalCoord::from_units(1_500_000_127),
            LocalCoord::from_units(-999_999_936),
            LocalCoord::from_units(1_249_999_872),
        );

        assert_eq!(
            origin.rebase(point),
            Ok(RenderPoint {
                x: 127,
                y: 64,
                z: -128,
            })
        );
    }

    #[test]
    fn rebase_reports_checked_integer_overflow() {
        let origin = RenderOrigin::new(i32::MIN, 0, 0);
        let point = LocalPoint::new(
            LocalCoord::from_units(i32::MAX),
            LocalCoord::from_units(0),
            LocalCoord::from_units(0),
        );

        assert_eq!(origin.rebase(point), Err(RenderCoordinateError::XOverflow));
    }
}
