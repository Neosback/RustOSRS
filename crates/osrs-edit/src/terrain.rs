use osrs_core::ids::{FloorOverlayId, FloorUnderlayId};

/// Valid OSRS terrain overlay shape (`0..=11`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct OverlayShape(u8);

impl OverlayShape {
    pub const MAX: u8 = 11;

    pub const fn new(value: u8) -> Option<Self> {
        if value <= Self::MAX {
            Some(Self(value))
        } else {
            None
        }
    }

    pub const fn get(self) -> u8 {
        self.0
    }
}

/// Valid OSRS terrain overlay rotation (`0..=3`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct OverlayRotation(u8);

impl OverlayRotation {
    pub const MAX: u8 = 3;

    pub const fn new(value: u8) -> Option<Self> {
        if value <= Self::MAX {
            Some(Self(value))
        } else {
            None
        }
    }

    pub const fn get(self) -> u8 {
        self.0
    }
}

/// Semantic overlay value owned by one terrain tile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TerrainOverlay {
    pub id: FloorOverlayId,
    pub shape: OverlayShape,
    pub rotation: OverlayRotation,
}

/// Editor-facing semantic channels of one terrain tile.
///
/// Height is intentionally absent: terrain heights belong to corner vertices and
/// are addressed with [`crate::HeightVertexCoord`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct TerrainTile {
    pub underlay: Option<FloorUnderlayId>,
    pub overlay: Option<TerrainOverlay>,
    pub flags: u8,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overlay_shape_rejects_values_outside_cache_semantics() {
        assert_eq!(OverlayShape::new(0).map(OverlayShape::get), Some(0));
        assert_eq!(OverlayShape::new(11).map(OverlayShape::get), Some(11));
        assert_eq!(OverlayShape::new(12), None);
    }

    #[test]
    fn overlay_rotation_rejects_values_outside_cache_semantics() {
        assert_eq!(OverlayRotation::new(0).map(OverlayRotation::get), Some(0));
        assert_eq!(OverlayRotation::new(3).map(OverlayRotation::get), Some(3));
        assert_eq!(OverlayRotation::new(4), None);
    }
}
