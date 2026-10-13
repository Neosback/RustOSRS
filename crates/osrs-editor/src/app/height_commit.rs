//! Pure semantic planning for Height-tool strokes.
//!
//! The live editor still previews Height changes through the legacy worker path. This module defines
//! the headless transaction boundary that a completed Height stroke will use in the next runtime
//! checkpoint: ordered brush samples are evaluated against authoritative semantic heights, storage
//! quantization is applied exactly, and the final result is emitted as one reversible `MapPatch`.

use crate::heights::{self, HeightTool};
use crate::paint::BrushShape;
use osrs_core::coords::{MapTile, SourcePlane};
use osrs_edit::{HeightVertexCoord, MapOp, MapPatch, MapQuery};
use std::collections::BTreeMap;

#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum HeightCommitError {
    /// A non-zero source plane cannot be encoded without the explicit height directly below it.
    MissingBaseHeight { coord: HeightVertexCoord },
}

fn query_height(query: &impl MapQuery, plane: SourcePlane, tile: MapTile) -> Option<i32> {
    query.height_at(HeightVertexCoord::new(tile, plane))
}

fn quantize_height(
    query: &impl MapQuery,
    plane: SourcePlane,
    tile: MapTile,
    desired: i32,
) -> Result<i32, HeightCommitError> {
    let plane_index = plane.index().get();
    let base = if plane_index == 0 {
        0
    } else {
        let below = SourcePlane::new(plane_index - 1)
            .unwrap_or_else(|| unreachable!("a validated non-zero source plane has a lower plane"));
        let coord = HeightVertexCoord::new(tile, below);
        query
            .height_at(coord)
            .ok_or(HeightCommitError::MissingBaseHeight { coord })?
    };

    Ok(heights::decode_height(
        base,
        heights::encode_height(base, desired),
    ))
}

/// Plan one logical Height stroke as one semantic patch.
///
/// `centers` is ordered input: repeated samples are intentional and later samples see the
/// storage-quantized result of earlier samples, matching the current held/dragged sculpt behavior.
/// Missing target heights are skipped rather than treated as numeric zero. On planes above zero,
/// an edited vertex requires an explicit height on the plane below so cache encoding never invents
/// a base height.
#[cfg_attr(not(test), allow(dead_code))]
pub(super) fn build_terrain_height_patch(
    query: &impl MapQuery,
    plane: SourcePlane,
    tool: &HeightTool,
    radius: u8,
    shape: BrushShape,
    anchor: i32,
    seed: u64,
    centers: &[MapTile],
) -> Result<MapPatch, HeightCommitError> {
    let mut staged = BTreeMap::<MapTile, i32>::new();
    let mut original = BTreeMap::<MapTile, i32>::new();

    for center in centers {
        // One application is evaluated against a stable pre-application snapshot. Changes from
        // earlier applications in the same stroke are visible, while sibling vertices produced by
        // this application do not influence each other until the next sample.
        let proposed = {
            let height = |tile: (i32, i32)| {
                let tile = MapTile::new(tile.0, tile.1);
                staged
                    .get(&tile)
                    .copied()
                    .or_else(|| query_height(query, plane, tile))
            };
            heights::apply(
                tool,
                (center.x, center.y),
                radius,
                shape,
                anchor,
                seed,
                &height,
            )
        };

        for (tile, desired) in proposed {
            let tile = MapTile::new(tile.0, tile.1);
            let current = staged
                .get(&tile)
                .copied()
                .or_else(|| query_height(query, plane, tile))
                .unwrap_or_else(|| {
                    unreachable!("Height arithmetic only returns vertices with explicit heights")
                });
            let after = quantize_height(query, plane, tile, desired)?;
            if after == current {
                continue;
            }
            original.entry(tile).or_insert(current);
            staged.insert(tile, after);
        }
    }

    let ops = staged.into_iter().filter_map(|(tile, after)| {
        let before = original
            .get(&tile)
            .copied()
            .unwrap_or_else(|| unreachable!("every staged Height vertex records its original"));
        (before != after).then_some(MapOp::SetHeight {
            coord: HeightVertexCoord::new(tile, plane),
            before,
            after,
        })
    });

    Ok(MapPatch::new(ops))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::heights::{Falloff, HeightOp};
    use osrs_edit::MapDocument;

    fn plane(value: u8) -> SourcePlane {
        SourcePlane::new(value).unwrap_or_else(|| unreachable!("test plane is valid"))
    }

    fn vertex(tile: MapTile, plane: SourcePlane) -> HeightVertexCoord {
        HeightVertexCoord::new(tile, plane)
    }

    fn tool(op: HeightOp, strength: i32) -> HeightTool {
        HeightTool {
            op,
            strength,
            falloff: Falloff::Flat,
            flatten_to: None,
        }
    }

    #[test]
    fn repeated_samples_fold_into_one_final_height_operation() {
        let tile = MapTile::new(3200, 3200);
        let source_plane = plane(0);
        let document = MapDocument::from_parts([], [(vertex(tile, source_plane), 0)], []);
        let patch = build_terrain_height_patch(
            &document,
            source_plane,
            &tool(HeightOp::Raise, 16),
            0,
            BrushShape::Square,
            0,
            7,
            &[tile, tile],
        )
        .unwrap_or_else(|error| unreachable!("plane-zero Height stroke is valid: {error:?}"));

        assert_eq!(
            patch.ops(),
            &[MapOp::SetHeight {
                coord: vertex(tile, source_plane),
                before: 0,
                after: -32,
            }]
        );
    }

    #[test]
    fn missing_target_height_is_not_treated_as_zero() {
        let tile = MapTile::new(3200, 3200);
        let source_plane = plane(0);
        let document = MapDocument::new();
        let patch = build_terrain_height_patch(
            &document,
            source_plane,
            &tool(HeightOp::Raise, 16),
            0,
            BrushShape::Square,
            0,
            1,
            &[tile],
        )
        .unwrap_or_else(|error| unreachable!("missing target vertices are skipped: {error:?}"));

        assert!(patch.is_empty());
    }

    #[test]
    fn non_zero_plane_requires_explicit_height_below() {
        let tile = MapTile::new(3200, 3200);
        let upper = plane(1);
        let lower = plane(0);
        let document = MapDocument::from_parts([], [(vertex(tile, upper), -96)], []);
        let error = build_terrain_height_patch(
            &document,
            upper,
            &tool(HeightOp::Lower, 8),
            0,
            BrushShape::Square,
            -96,
            1,
            &[tile],
        )
        .expect_err("upper-plane cache encoding needs the lower-plane base height");

        assert_eq!(
            error,
            HeightCommitError::MissingBaseHeight {
                coord: vertex(tile, lower)
            }
        );
    }

    #[test]
    fn upper_plane_quantization_uses_the_actual_lower_plane_base() {
        let tile = MapTile::new(3200, 3200);
        let lower = plane(0);
        let upper = plane(1);
        let document = MapDocument::from_parts(
            [],
            [(vertex(tile, lower), -80), (vertex(tile, upper), -96)],
            [],
        );
        let patch = build_terrain_height_patch(
            &document,
            upper,
            &tool(HeightOp::Lower, 8),
            0,
            BrushShape::Square,
            -96,
            1,
            &[tile],
        )
        .unwrap_or_else(|error| unreachable!("explicit lower-plane base is valid: {error:?}"));

        // Desired -88 is one cache step below the -80 base. Stored byte 1 decodes as zero steps,
        // so the nearest representable semantic height is -80 rather than an invented -88.
        assert_eq!(
            patch.ops(),
            &[MapOp::SetHeight {
                coord: vertex(tile, upper),
                before: -96,
                after: -80,
            }]
        );
    }

    #[test]
    fn storage_quantization_can_turn_an_application_into_a_semantic_no_op() {
        let tile = MapTile::new(3200, 3200);
        let source_plane = plane(0);
        let document = MapDocument::from_parts([], [(vertex(tile, source_plane), 0)], []);
        let patch = build_terrain_height_patch(
            &document,
            source_plane,
            &tool(HeightOp::Lower, 8),
            0,
            BrushShape::Square,
            0,
            1,
            &[tile],
        )
        .unwrap_or_else(|error| unreachable!("plane-zero Height stroke is valid: {error:?}"));

        assert!(patch.is_empty());
    }

    #[test]
    fn negative_world_coordinates_remain_typed_and_exact() {
        let tile = MapTile::new(-65, -1);
        let source_plane = plane(0);
        let document = MapDocument::from_parts([], [(vertex(tile, source_plane), -40)], []);
        let patch = build_terrain_height_patch(
            &document,
            source_plane,
            &tool(HeightOp::Raise, 16),
            0,
            BrushShape::Square,
            -40,
            99,
            &[tile],
        )
        .unwrap_or_else(|error| unreachable!("negative coordinates are valid: {error:?}"));

        assert_eq!(
            patch.ops(),
            &[MapOp::SetHeight {
                coord: vertex(tile, source_plane),
                before: -40,
                after: -56,
            }]
        );
    }

    #[test]
    fn final_patch_order_is_coordinate_stable() {
        let a = MapTile::new(3201, 3200);
        let b = MapTile::new(3200, 3200);
        let source_plane = plane(0);
        let document = MapDocument::from_parts(
            [],
            [(vertex(a, source_plane), 0), (vertex(b, source_plane), 0)],
            [],
        );
        let patch = build_terrain_height_patch(
            &document,
            source_plane,
            &tool(HeightOp::Raise, 16),
            0,
            BrushShape::Square,
            0,
            1,
            &[a, b],
        )
        .unwrap_or_else(|error| unreachable!("plane-zero Height stroke is valid: {error:?}"));

        assert_eq!(
            patch.ops(),
            &[
                MapOp::SetHeight {
                    coord: vertex(b, source_plane),
                    before: 0,
                    after: -16,
                },
                MapOp::SetHeight {
                    coord: vertex(a, source_plane),
                    before: 0,
                    after: -16,
                },
            ]
        );
    }
}
