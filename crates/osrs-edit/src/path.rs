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
    build_terrain_path_patch_with_grade(query, plane, intents, &[])
}

/// [`build_terrain_path_patch`] plus a terrain grade: `grade` lists vertices and their desired
/// heights (see [`crate::plan_path_grade`]). They are quantised to what the map file stores and
/// joined into the same patch, so the path and its grading are one reversible step. A graded
/// vertex replaces any height an intent also set for it.
pub fn build_terrain_path_patch_with_grade(
    query: &impl MapQuery,
    plane: SourcePlane,
    intents: &[TerrainPathIntent],
    grade: &[((i32, i32), i32)],
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

    for &((x, y), desired) in grade {
        let map_tile = MapTile::new(x, y);
        let vertex = HeightVertexCoord::new(map_tile, plane);
        let current_height = heights
            .get(&vertex)
            .map(|(_, after)| *after)
            .or_else(|| query.height_at(vertex))
            .ok_or(PathCommitError::MissingHeight { tile: map_tile })?;
        let after = crate::height::commit::quantize_height(query, plane, map_tile, desired)
            .map_err(|_| PathCommitError::MissingHeight { tile: map_tile })?;
        let entry = heights.entry(vertex).or_insert_with(|| {
            height_order.push(vertex);
            (current_height, current_height)
        });
        entry.1 = after;
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

    #[test]
    fn graded_path_is_one_patch_that_undoes_exactly() {
        let tile = (3200, 3200);
        let heights: Vec<_> = (3198..=3203)
            .flat_map(|x| (3198..=3203).map(move |y| (vertex((x, y)), -16 * (y - 3198))))
            .collect();
        let mut document = MapDocument::from_parts([], heights.clone(), []);
        let original = document.clone();
        let grade = [
            ((3200, 3200), -32),
            ((3201, 3200), -32),
            ((3200, 3201), -40),
        ];

        let patch = build_terrain_path_patch_with_grade(
            &document,
            plane(),
            &[intent(tile, 7, 1, 0)],
            &grade,
        )
        .unwrap_or_else(|error| unreachable!("valid graded patch: {error:?}"));
        // One overlay op plus the heights that actually change.
        assert!(document.apply_patch(&patch).is_ok());
        assert!(document.terrain_at(position(tile)).overlay.is_some());
        assert_eq!(document.height_at(vertex((3200, 3200))), Some(-32));
        assert_eq!(document.height_at(vertex((3200, 3201))), Some(-40));
        // Vertices outside the grade are untouched.
        assert_eq!(
            document.height_at(vertex((3203, 3203))),
            original.height_at(vertex((3203, 3203)))
        );

        assert!(document.apply_patch(&patch.inverse()).is_ok());
        assert_eq!(document, original);
    }

    #[test]
    fn grading_an_unknown_vertex_fails_instead_of_inventing_a_height() {
        let document = MapDocument::new();
        assert_eq!(
            build_terrain_path_patch_with_grade(&document, plane(), &[], &[((1, 1), -8)]),
            Err(PathCommitError::MissingHeight {
                tile: MapTile::new(1, 1)
            })
        );
    }

    #[test]
    fn a_graded_vertex_replaces_an_armed_flat_height() {
        let tile = (3200, 3200);
        let mut document = MapDocument::from_parts([], [(vertex(tile), 0)], []);
        let mut path = intent(tile, 7, 1, 0);
        path.height = Some(64);
        let patch =
            build_terrain_path_patch_with_grade(&document, plane(), &[path], &[(tile, -24)])
                .unwrap_or_else(|error| unreachable!("valid patch: {error:?}"));
        assert!(document.apply_patch(&patch).is_ok());
        assert_eq!(document.height_at(vertex(tile)), Some(-24));
    }
}
