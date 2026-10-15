//! Pure semantic commit planning for the terrain Paint tool.
//!
//! Runtime input and projection stay in `editing` for now. This module only folds one logical
//! Paint stroke into one reversible `MapPatch` against read-only authoritative semantic state.

use crate::{
    HeightVertexCoord, MapOp, MapPatch, MapPosition, MapQuery, OverlayRotation, OverlayShape,
    TerrainOverlay,
};
use osrs_core::{
    coords::{MapTile, SourcePlane},
    ids::{FloorOverlayId, FloorUnderlayId},
};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerrainPaintIntent {
    pub tile: MapTile,
    /// `None` preserves the channel, `Some(None)` clears it, `Some(Some(id))` sets it.
    pub underlay: Option<Option<FloorUnderlayId>>,
    /// `None` preserves the channel, `Some(None)` clears it, `Some(Some(id))` sets it.
    pub overlay: Option<Option<FloorOverlayId>>,
    /// Shape/rotation are applied only when the resulting tile has an overlay.
    pub overlay_shape: Option<(OverlayShape, OverlayRotation)>,
    pub height: Option<i32>,
    pub flags: Option<u8>,
}

impl TerrainPaintIntent {
    pub const fn new(tile: MapTile) -> Self {
        Self {
            tile,
            underlay: None,
            overlay: None,
            overlay_shape: None,
            height: None,
            flags: None,
        }
    }

    const fn touches_terrain(self) -> bool {
        self.underlay.is_some()
            || self.overlay.is_some()
            || self.overlay_shape.is_some()
            || self.flags.is_some()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaintCommitError {
    MissingHeight { tile: MapTile },
}

/// Build one compact semantic patch for one completed Paint stroke.
///
/// Repeated samples of the same tile are folded into one final terrain operation and one final
/// height operation. `before` values always come from `MapQuery`, while later duplicate intents
/// evolve the same in-memory working value. Operation order is coordinate-sorted for deterministic
/// patches independent of hash iteration order.
pub fn build_terrain_paint_patch(
    query: &impl MapQuery,
    plane: SourcePlane,
    intents: &[TerrainPaintIntent],
) -> Result<MapPatch, PaintCommitError> {
    let mut terrain = BTreeMap::<MapPosition, (crate::TerrainTile, crate::TerrainTile)>::new();
    let mut heights = BTreeMap::<HeightVertexCoord, (i32, i32)>::new();

    for intent in intents.iter().copied() {
        let map_tile = intent.tile;

        if intent.touches_terrain() {
            let position = MapPosition::new(map_tile, plane);
            let current = terrain
                .get(&position)
                .map_or_else(|| query.terrain_at(position), |(_, after)| *after);
            let entry = terrain.entry(position).or_insert((current, current));
            let mut after = entry.1;

            if let Some(underlay) = intent.underlay {
                after.underlay = underlay;
            }

            if let Some(overlay) = intent.overlay {
                after.overlay = match overlay {
                    None => None,
                    Some(id) => {
                        let (shape, rotation) = intent
                            .overlay_shape
                            .or_else(|| {
                                after
                                    .overlay
                                    .map(|existing| (existing.shape, existing.rotation))
                            })
                            .unwrap_or_else(|| {
                                (
                                    OverlayShape::new(0)
                                        .unwrap_or_else(|| unreachable!("shape zero is valid")),
                                    OverlayRotation::new(0)
                                        .unwrap_or_else(|| unreachable!("rotation zero is valid")),
                                )
                            });
                        Some(TerrainOverlay {
                            id,
                            shape,
                            rotation,
                        })
                    }
                };
            }

            if let Some((shape, rotation)) = intent.overlay_shape
                && let Some(overlay) = &mut after.overlay
            {
                overlay.shape = shape;
                overlay.rotation = rotation;
            }

            if let Some(flags) = intent.flags {
                after.flags = flags;
            }
            entry.1 = after;
        }

        if let Some(target_height) = intent.height {
            let vertex = HeightVertexCoord::new(map_tile, plane);
            let current_height = heights
                .get(&vertex)
                .map(|(_, after)| *after)
                .or_else(|| query.height_at(vertex));
            let Some(current_height) = current_height else {
                return Err(PaintCommitError::MissingHeight { tile: intent.tile });
            };
            heights
                .entry(vertex)
                .and_modify(|(_, after)| *after = target_height)
                .or_insert((current_height, target_height));
        }
    }

    let mut ops = Vec::with_capacity(terrain.len() + heights.len());
    for (coord, (before, after)) in terrain {
        if before != after {
            ops.push(MapOp::SetTerrain {
                coord,
                before,
                after,
            });
        }
    }
    for (coord, (before, after)) in heights {
        if before != after {
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
    use crate::{MapDocument, TerrainTile};

    fn plane() -> SourcePlane {
        SourcePlane::new(0).unwrap_or_else(|| unreachable!("plane zero is valid"))
    }

    fn position(tile: (i32, i32)) -> MapPosition {
        MapPosition::new(MapTile::new(tile.0, tile.1), plane())
    }

    fn vertex(tile: (i32, i32)) -> HeightVertexCoord {
        HeightVertexCoord::new(MapTile::new(tile.0, tile.1), plane())
    }

    fn shape(value: u8) -> OverlayShape {
        OverlayShape::new(value).unwrap_or_else(|| unreachable!("test shape is valid"))
    }

    fn rotation(value: u8) -> OverlayRotation {
        OverlayRotation::new(value).unwrap_or_else(|| unreachable!("test rotation is valid"))
    }

    #[test]
    fn paint_patch_updates_only_armed_channels() {
        let tile = (3200, 3200);
        let before = TerrainTile {
            underlay: Some(FloorUnderlayId::new(3)),
            overlay: Some(TerrainOverlay {
                id: FloorOverlayId::new(5),
                shape: shape(2),
                rotation: rotation(1),
            }),
            flags: 1,
        };
        let mut document =
            MapDocument::from_parts([(position(tile), before)], [(vertex(tile), 32)], []);
        let mut intent = TerrainPaintIntent::new(osrs_core::coords::MapTile::new(tile.0, tile.1));
        intent.overlay = Some(Some(FloorOverlayId::new(7)));
        intent.flags = Some(5);

        let patch = build_terrain_paint_patch(&document, plane(), &[intent])
            .unwrap_or_else(|error| unreachable!("valid paint patch: {error:?}"));
        assert_eq!(patch.len(), 1);
        assert!(document.apply_patch(&patch).is_ok());

        let after = document.terrain_at(position(tile));
        assert_eq!(after.underlay, before.underlay);
        assert_eq!(after.flags, 5);
        let overlay = after
            .overlay
            .unwrap_or_else(|| unreachable!("overlay should remain present"));
        assert_eq!(overlay.id, FloorOverlayId::new(7));
        assert_eq!(overlay.shape, shape(2));
        assert_eq!(overlay.rotation, rotation(1));
        assert_eq!(document.height_at(vertex(tile)), Some(32));
    }

    #[test]
    fn overlay_clear_removes_shape_and_rotation_semantically() {
        let tile = (3200, 3200);
        let before = TerrainTile {
            overlay: Some(TerrainOverlay {
                id: FloorOverlayId::new(7),
                shape: shape(4),
                rotation: rotation(3),
            }),
            ..TerrainTile::default()
        };
        let mut document =
            MapDocument::from_parts([(position(tile), before)], [(vertex(tile), 0)], []);
        let mut intent = TerrainPaintIntent::new(osrs_core::coords::MapTile::new(tile.0, tile.1));
        intent.overlay = Some(None);
        intent.overlay_shape = Some((shape(8), rotation(2)));

        let patch = build_terrain_paint_patch(&document, plane(), &[intent])
            .unwrap_or_else(|error| unreachable!("valid clear patch: {error:?}"));
        assert!(document.apply_patch(&patch).is_ok());
        assert_eq!(document.terrain_at(position(tile)).overlay, None);
    }

    #[test]
    fn setting_overlay_on_empty_tile_uses_canonical_default_shape_when_shape_is_unarmed() {
        let tile = (3200, 3200);
        let mut document = MapDocument::from_parts([], [(vertex(tile), 0)], []);
        let mut intent = TerrainPaintIntent::new(osrs_core::coords::MapTile::new(tile.0, tile.1));
        intent.overlay = Some(Some(FloorOverlayId::new(7)));

        let patch = build_terrain_paint_patch(&document, plane(), &[intent])
            .unwrap_or_else(|error| unreachable!("valid overlay patch: {error:?}"));
        assert!(document.apply_patch(&patch).is_ok());
        let overlay = document
            .terrain_at(position(tile))
            .overlay
            .unwrap_or_else(|| unreachable!("overlay should be present"));
        assert_eq!(overlay.shape, shape(0));
        assert_eq!(overlay.rotation, rotation(0));
    }

    #[test]
    fn semantic_no_op_produces_empty_patch() {
        let tile = (3200, 3200);
        let before = TerrainTile {
            underlay: Some(FloorUnderlayId::new(3)),
            ..TerrainTile::default()
        };
        let document =
            MapDocument::from_parts([(position(tile), before)], [(vertex(tile), 32)], []);
        let mut intent = TerrainPaintIntent::new(osrs_core::coords::MapTile::new(tile.0, tile.1));
        intent.underlay = Some(before.underlay);
        intent.height = Some(32);

        let patch = build_terrain_paint_patch(&document, plane(), &[intent])
            .unwrap_or_else(|error| unreachable!("valid no-op patch: {error:?}"));
        assert!(patch.is_empty());
    }

    #[test]
    fn repeated_samples_fold_to_one_final_operation_per_channel() {
        let tile = (3200, 3200);
        let mut document = MapDocument::from_parts([], [(vertex(tile), 0)], []);
        let mut first = TerrainPaintIntent::new(osrs_core::coords::MapTile::new(tile.0, tile.1));
        first.underlay = Some(Some(FloorUnderlayId::new(3)));
        first.height = Some(64);
        let mut second = TerrainPaintIntent::new(osrs_core::coords::MapTile::new(tile.0, tile.1));
        second.underlay = Some(Some(FloorUnderlayId::new(9)));
        second.height = Some(128);

        let patch = build_terrain_paint_patch(&document, plane(), &[first, second])
            .unwrap_or_else(|error| unreachable!("valid repeated paint: {error:?}"));
        assert_eq!(patch.len(), 2);
        assert!(document.apply_patch(&patch).is_ok());
        assert_eq!(
            document.terrain_at(position(tile)).underlay,
            Some(FloorUnderlayId::new(9))
        );
        assert_eq!(document.height_at(vertex(tile)), Some(128));
    }

    #[test]
    fn paint_patch_keeps_region_boundary_tiles_distinct() {
        let left = (3263, 3200);
        let right = (3264, 3200);
        let document = MapDocument::from_parts([], [(vertex(left), 0), (vertex(right), 0)], []);
        let mut left_intent =
            TerrainPaintIntent::new(osrs_core::coords::MapTile::new(left.0, left.1));
        left_intent.flags = Some(1);
        let mut right_intent =
            TerrainPaintIntent::new(osrs_core::coords::MapTile::new(right.0, right.1));
        right_intent.flags = Some(2);

        let patch = build_terrain_paint_patch(&document, plane(), &[right_intent, left_intent])
            .unwrap_or_else(|error| unreachable!("valid boundary paint: {error:?}"));
        assert_eq!(patch.len(), 2);
        let coords: Vec<_> = patch
            .ops()
            .iter()
            .filter_map(|op| match op {
                MapOp::SetTerrain { coord, .. } => Some(coord.tile),
                _ => None,
            })
            .collect();
        assert_eq!(
            coords,
            vec![MapTile::new(3263, 3200), MapTile::new(3264, 3200)]
        );
    }

    #[test]
    fn negative_world_coordinates_are_planned_without_truncation() {
        let tiles = [(-65, -1), (-64, -1)];
        let document = MapDocument::from_parts([], tiles.map(|tile| (vertex(tile), 0)), []);
        let intents = tiles.map(|tile| {
            let mut intent =
                TerrainPaintIntent::new(osrs_core::coords::MapTile::new(tile.0, tile.1));
            intent.underlay = Some(Some(FloorUnderlayId::new(4)));
            intent
        });

        let patch = build_terrain_paint_patch(&document, plane(), &intents)
            .unwrap_or_else(|error| unreachable!("valid negative paint: {error:?}"));
        assert_eq!(patch.len(), 2);
        let coords: Vec<_> = patch
            .ops()
            .iter()
            .filter_map(|op| match op {
                MapOp::SetTerrain { coord, .. } => Some(coord.tile),
                _ => None,
            })
            .collect();
        assert_eq!(coords, vec![MapTile::new(-65, -1), MapTile::new(-64, -1)]);
    }

    #[test]
    fn armed_height_requires_explicit_semantic_height() {
        let tile = (3200, 3200);
        let document = MapDocument::new();
        let mut intent = TerrainPaintIntent::new(osrs_core::coords::MapTile::new(tile.0, tile.1));
        intent.height = Some(64);

        assert_eq!(
            build_terrain_paint_patch(&document, plane(), &[intent]),
            Err(PaintCommitError::MissingHeight {
                tile: MapTile::new(tile.0, tile.1)
            })
        );
    }
}
