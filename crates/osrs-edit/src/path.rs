//! Semantic commit adapter for the existing terrain-path tool.
//!
//! Planning and preview remain in `terrain_path` / `spline_tool`. This module only turns the
//! already-planned tile intent into one reversible `MapPatch` against read-only semantic state.

use crate::{
    HeightVertexCoord, MapOp, MapPatch, MapPosition, MapQuery, OverlayRotation, OverlayShape,
    TerrainOverlay, TerrainTile,
};
use osrs_core::coords::{MapTile, SourcePlane};
use osrs_core::ids::FloorOverlayId;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerrainPathIntent {
    pub tile: MapTile,
    pub overlay: Option<u16>,
    pub shape: u8,
    pub rotation: u8,
    pub height: Option<i32>,
    pub flags: Option<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathCommitError {
    InvalidOverlayShape { tile: MapTile, shape: u8 },
    InvalidOverlayRotation { tile: MapTile, rotation: u8 },
    MissingHeight { tile: MapTile },
}

/// Build one compact semantic patch for a completed terrain-path gesture.
///
/// Duplicate tile intents are folded into one final terrain operation and one final height
/// operation per coordinate. This keeps one laid path deterministic and prevents transient
/// duplicate planner output from becoming redundant history operations.
pub fn build_terrain_path_patch(
    query: &impl MapQuery,
    plane: SourcePlane,
    intents: &[TerrainPathIntent],
) -> Result<MapPatch, PathCommitError> {
    let mut terrain_order = Vec::new();
    let mut terrain = HashMap::<MapPosition, (TerrainTile, TerrainTile)>::new();
    let mut height_order = Vec::new();
    let mut heights = HashMap::<HeightVertexCoord, (i32, i32)>::new();

    for intent in intents {
        let map_tile = intent.tile;
        let position = MapPosition::new(map_tile, plane);
        let current = terrain
            .get(&position)
            .map_or_else(|| query.terrain_at(position), |(_, after)| *after);
        let entry = terrain.entry(position).or_insert_with(|| {
            terrain_order.push(position);
            (current, current)
        });
        let mut after = entry.1;

        if let Some(id) = intent.overlay {
            if id == 0 {
                after.overlay = None;
            } else {
                let Some(shape) = OverlayShape::new(intent.shape) else {
                    return Err(PathCommitError::InvalidOverlayShape {
                        tile: intent.tile,
                        shape: intent.shape,
                    });
                };
                let Some(rotation) = OverlayRotation::new(intent.rotation) else {
                    return Err(PathCommitError::InvalidOverlayRotation {
                        tile: intent.tile,
                        rotation: intent.rotation,
                    });
                };
                after.overlay = Some(TerrainOverlay {
                    id: FloorOverlayId::new(u32::from(id)),
                    shape,
                    rotation,
                });
            }
        }
        if let Some(flags) = intent.flags {
            after.flags = flags;
        }
        entry.1 = after;

        if let Some(target_height) = intent.height {
            let vertex = HeightVertexCoord::new(map_tile, plane);
            let current_height = heights
                .get(&vertex)
                .map(|(_, after)| *after)
                .or_else(|| query.height_at(vertex));
            let Some(current_height) = current_height else {
                return Err(PathCommitError::MissingHeight { tile: intent.tile });
            };
            let entry = heights.entry(vertex).or_insert_with(|| {
                height_order.push(vertex);
                (current_height, current_height)
            });
            entry.1 = target_height;
        }
    }

    let mut ops = Vec::with_capacity(terrain_order.len() + height_order.len());
    for coord in terrain_order {
        if let Some((before, after)) = terrain.get(&coord).copied()
            && before != after
        {
            ops.push(MapOp::SetTerrain {
                coord,
                before,
                after,
            });
        }
    }
    for coord in height_order {
        if let Some((before, after)) = heights.get(&coord).copied()
            && before != after
        {
            ops.push(MapOp::SetHeight {
                coord,
                before,
                after,
            });
        }
    }

    Ok(MapPatch::new(ops))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MapDocument;
    use osrs_core::ids::FloorUnderlayId;

    fn plane() -> SourcePlane {
        SourcePlane::new(0).unwrap_or_else(|| unreachable!("plane zero is valid"))
    }

    fn position(tile: (i32, i32)) -> MapPosition {
        MapPosition::new(MapTile::new(tile.0, tile.1), plane())
    }

    fn vertex(tile: (i32, i32)) -> HeightVertexCoord {
        HeightVertexCoord::new(MapTile::new(tile.0, tile.1), plane())
    }

    fn intent(tile: (i32, i32), overlay: u16, shape: u8, rotation: u8) -> TerrainPathIntent {
        TerrainPathIntent {
            tile: MapTile::new(tile.0, tile.1),
            overlay: Some(overlay),
            shape,
            rotation,
            height: None,
            flags: None,
        }
    }

    #[test]
    fn path_patch_preserves_unpainted_channels_and_updates_armed_channels() {
        let tile = (3200, 3200);
        let before = TerrainTile {
            underlay: Some(FloorUnderlayId::new(3)),
            overlay: None,
            flags: 1,
        };
        let mut document =
            MapDocument::from_parts([(position(tile), before)], [(vertex(tile), 32)], []);
        let mut path = intent(tile, 7, 4, 2);
        path.height = Some(64);
        path.flags = Some(5);

        let patch = build_terrain_path_patch(&document, plane(), &[path])
            .unwrap_or_else(|error| unreachable!("valid path patch: {error:?}"));
        assert_eq!(patch.len(), 2);
        assert!(document.apply_patch(&patch).is_ok());

        let after = document.terrain_at(position(tile));
        assert_eq!(after.underlay, before.underlay);
        assert_eq!(after.flags, 5);
        let overlay = after
            .overlay
            .unwrap_or_else(|| unreachable!("path should set an overlay"));
        assert_eq!(overlay.id.get(), 7);
        assert_eq!(overlay.shape.get(), 4);
        assert_eq!(overlay.rotation.get(), 2);
        assert_eq!(document.height_at(vertex(tile)), Some(64));
    }

    #[test]
    fn duplicate_tile_intents_collapse_to_one_final_terrain_op() {
        let tile = (3200, 3200);
        let mut document = MapDocument::from_parts([], [(vertex(tile), 0)], []);
        let path = [intent(tile, 7, 1, 0), intent(tile, 8, 2, 3)];

        let patch = build_terrain_path_patch(&document, plane(), &path)
            .unwrap_or_else(|error| unreachable!("valid path patch: {error:?}"));
        assert_eq!(patch.len(), 1);
        assert!(document.apply_patch(&patch).is_ok());
        let overlay = document
            .terrain_at(position(tile))
            .overlay
            .unwrap_or_else(|| unreachable!("path should set an overlay"));
        assert_eq!(overlay.id.get(), 8);
        assert_eq!(overlay.shape.get(), 2);
        assert_eq!(overlay.rotation.get(), 3);
    }

    #[test]
    fn semantic_no_op_produces_an_empty_patch() {
        let tile = (3200, 3200);
        let overlay = TerrainOverlay {
            id: FloorOverlayId::new(7),
            shape: OverlayShape::new(4)
                .unwrap_or_else(|| unreachable!("overlay shape four is valid")),
            rotation: OverlayRotation::new(2)
                .unwrap_or_else(|| unreachable!("overlay rotation two is valid")),
        };
        let document = MapDocument::from_parts(
            [(
                position(tile),
                TerrainTile {
                    underlay: None,
                    overlay: Some(overlay),
                    flags: 0,
                },
            )],
            [(vertex(tile), 32)],
            [],
        );

        let patch = build_terrain_path_patch(&document, plane(), &[intent(tile, 7, 4, 2)])
            .unwrap_or_else(|error| unreachable!("valid path patch: {error:?}"));
        assert!(patch.is_empty());
    }

    #[test]
    fn armed_height_requires_an_explicit_height_vertex() {
        let tile = (3200, 3200);
        let document = MapDocument::new();
        let mut path = intent(tile, 7, 0, 0);
        path.height = Some(64);

        assert_eq!(
            build_terrain_path_patch(&document, plane(), &[path]),
            Err(PathCommitError::MissingHeight {
                tile: MapTile::new(tile.0, tile.1)
            })
        );
    }
}
