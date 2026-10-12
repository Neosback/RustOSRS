use osrs_core::coords::{MapTile, SourcePlane};

/// A source-map tile together with its source plane.
///
/// This is intentionally distinct from scene-storage coordinates, render levels,
/// collision planes, and renderer-local positions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MapPosition {
    pub tile: MapTile,
    pub plane: SourcePlane,
}

impl MapPosition {
    pub const fn new(tile: MapTile, plane: SourcePlane) -> Self {
        Self { tile, plane }
    }
}

/// One terrain-height corner in world tile-grid coordinates.
///
/// An 8x8 tile area has a 9x9 corner lattice. Keeping height vertices distinct
/// from tile positions prevents tools from silently treating tile attributes and
/// terrain corners as if they shared the same topology.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct HeightVertexCoord {
    pub corner: MapTile,
    pub plane: SourcePlane,
}

impl HeightVertexCoord {
    pub const fn new(corner: MapTile, plane: SourcePlane) -> Self {
        Self { corner, plane }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tile_and_height_vertex_coordinates_remain_distinct_types() {
        let plane = SourcePlane::new(2).unwrap_or_else(|| unreachable!("test plane is valid"));
        let tile = MapTile::new(3200, 3200);

        assert_eq!(MapPosition::new(tile, plane).tile, tile);
        assert_eq!(HeightVertexCoord::new(tile, plane).corner, tile);
    }
}
