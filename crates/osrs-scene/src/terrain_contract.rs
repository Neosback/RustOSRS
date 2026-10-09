//! Small exact terrain-surface contracts shared by M6 semantic scene tests.
//!
//! These helpers preserve the reference sentinel meaning without introducing
//! renderer packing, UV, batching, or raster policy into `osrs-scene`.

use crate::terrain::{FlatTerrainSurface, TerrainFace};

/// Reference terrain color sentinel used to skip a paint or shaped face.
pub const REFERENCE_TERRAIN_SKIP_COLOR: i32 = 12_345_678;

/// The pinned flat-paint upload path skips the complete paint when the NE color
/// carries the reference sentinel.
pub const fn flat_paint_is_reference_skipped(surface: &FlatTerrainSurface) -> bool {
    surface.colors.northeast == REFERENCE_TERRAIN_SKIP_COLOR
}

/// The pinned shaped-terrain upload path skips an individual face when its first
/// authored face color carries the reference sentinel.
pub fn shaped_face_is_reference_skipped(face: &TerrainFace) -> bool {
    face.colors[0] == REFERENCE_TERRAIN_SKIP_COLOR
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terrain::{TerrainColorSource, TerrainCorner, TerrainCorners, TerrainFace};

    #[test]
    fn flat_paint_skip_contract_uses_northeast_color_only() {
        let skipped = FlatTerrainSurface::new(
            TerrainCorners::new(10, 20, 30, 40),
            TerrainCorners::new(1, 2, REFERENCE_TERRAIN_SKIP_COLOR, 4),
            None,
        );
        assert!(flat_paint_is_reference_skipped(&skipped));

        let visible = FlatTerrainSurface::new(
            TerrainCorners::new(10, 20, 30, 40),
            TerrainCorners::new(REFERENCE_TERRAIN_SKIP_COLOR, 2, 3, 4),
            None,
        );
        assert!(!flat_paint_is_reference_skipped(&visible));
        assert_eq!(
            FlatTerrainSurface::REFERENCE_TRIANGLES,
            [
                [
                    TerrainCorner::Northeast,
                    TerrainCorner::Northwest,
                    TerrainCorner::Southeast,
                ],
                [
                    TerrainCorner::Southwest,
                    TerrainCorner::Southeast,
                    TerrainCorner::Northwest,
                ],
            ]
        );
    }

    #[test]
    fn shaped_face_skip_contract_uses_first_face_color_only() {
        let skipped = TerrainFace {
            indices: [0, 1, 2],
            colors: [REFERENCE_TERRAIN_SKIP_COLOR, 2, 3],
            color_source: TerrainColorSource::Underlay,
            texture_id: None,
        };
        assert!(shaped_face_is_reference_skipped(&skipped));

        let visible = TerrainFace {
            colors: [1, REFERENCE_TERRAIN_SKIP_COLOR, 3],
            ..skipped
        };
        assert!(!shaped_face_is_reference_skipped(&visible));
    }
}
