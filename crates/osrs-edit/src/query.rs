use crate::{HeightVertexCoord, MapDocument, MapPosition, ObjectPlacement, TerrainTile};

/// Read-only semantic access to authoritative editable-map state.
///
/// Tools and planners should depend on this contract when they only need to inspect
/// map state. The contract deliberately exposes semantic values rather than
/// renderer/runtime/cache representations and provides no mutation capability.
pub trait MapQuery {
    /// Query one terrain tile. Missing sparse terrain is the semantic default tile.
    fn terrain_at(&self, coord: MapPosition) -> TerrainTile;

    /// Query one explicit terrain-height vertex.
    ///
    /// `None` means the authoritative document has no value for that vertex; it is
    /// not equivalent to a numeric height of zero.
    fn height_at(&self, coord: HeightVertexCoord) -> Option<i32>;

    /// Test whether an exact semantic object placement exists.
    fn contains_object(&self, placement: ObjectPlacement) -> bool;
}

impl MapQuery for MapDocument {
    fn terrain_at(&self, coord: MapPosition) -> TerrainTile {
        MapDocument::terrain_at(self, coord)
    }

    fn height_at(&self, coord: HeightVertexCoord) -> Option<i32> {
        MapDocument::height_at(self, coord)
    }

    fn contains_object(&self, placement: ObjectPlacement) -> bool {
        MapDocument::contains_object(self, placement)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{LocKind, ObjectOrientation};
    use osrs_core::coords::{MapTile, SourcePlane};
    use osrs_core::ids::{FloorUnderlayId, ObjectId};

    fn plane() -> SourcePlane {
        SourcePlane::new(0).unwrap_or_else(|| unreachable!("plane zero is valid"))
    }

    fn position(x: i32, y: i32) -> MapPosition {
        MapPosition::new(MapTile::new(x, y), plane())
    }

    fn height(x: i32, y: i32) -> HeightVertexCoord {
        HeightVertexCoord::new(MapTile::new(x, y), plane())
    }

    fn object(orientation: u8) -> ObjectPlacement {
        ObjectPlacement {
            object_id: ObjectId::new(100),
            position: position(3200, 3200),
            kind: LocKind::new(10).unwrap_or_else(|| unreachable!("loc kind is valid")),
            orientation: ObjectOrientation::new(orientation)
                .unwrap_or_else(|| unreachable!("orientation is valid")),
        }
    }

    fn snapshot(
        query: &impl MapQuery,
        tile: MapPosition,
        vertex: HeightVertexCoord,
        placement: ObjectPlacement,
    ) -> (TerrainTile, Option<i32>, bool) {
        (
            query.terrain_at(tile),
            query.height_at(vertex),
            query.contains_object(placement),
        )
    }

    #[test]
    fn map_document_satisfies_read_only_query_contract() {
        let tile = position(3200, 3200);
        let vertex = height(3200, 3200);
        let placement = object(2);
        let terrain = TerrainTile {
            underlay: Some(FloorUnderlayId::new(7)),
            ..TerrainTile::default()
        };
        let document = MapDocument::from_parts([(tile, terrain)], [(vertex, 64)], [placement]);

        assert_eq!(
            snapshot(&document, tile, vertex, placement),
            (terrain, Some(64), true)
        );
    }

    #[test]
    fn query_preserves_sparse_terrain_and_explicit_height_semantics() {
        let document = MapDocument::new();

        assert_eq!(
            MapQuery::terrain_at(&document, position(3200, 3200)),
            TerrainTile::default()
        );
        assert_eq!(MapQuery::height_at(&document, height(3200, 3200)), None);
    }

    #[test]
    fn object_query_uses_exact_semantic_placement_identity() {
        let present = object(2);
        let different_orientation = object(3);
        let document = MapDocument::from_parts([], [], [present]);

        assert!(MapQuery::contains_object(&document, present));
        assert!(!MapQuery::contains_object(&document, different_orientation));
    }
}
