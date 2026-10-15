//! Pure semantic planning for Height-tool strokes.
//!
//! Completed Height strokes use this headless transaction boundary: ordered brush samples are evaluated against authoritative semantic heights, storage
//! quantization is applied exactly, and the final result is emitted as one reversible `MapPatch`.

use super::{HeightTool, apply as apply_height, decode_height, encode_height};
use crate::brush::BrushShape;
use crate::{HeightVertexCoord, MapOp, MapPatch, MapQuery};
use osrs_core::coords::{MapTile, SourcePlane};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeightCommitError {
    /// A non-zero source plane cannot be encoded without the explicit height directly below it.
    MissingBaseHeight { coord: HeightVertexCoord },
}

/// Immutable meaning of one logical Height stroke.
///
/// Capturing this once prevents UI changes during an open stroke from silently changing the
/// operation that will eventually be committed to the semantic session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeightStrokeSpec {
    plane: SourcePlane,
    tool: HeightTool,
    radius: u8,
    shape: BrushShape,
    anchor: i32,
    seed: u64,
}

impl HeightStrokeSpec {
    pub const fn new(
        plane: SourcePlane,
        tool: HeightTool,
        radius: u8,
        shape: BrushShape,
        anchor: i32,
        seed: u64,
    ) -> Self {
        Self {
            plane,
            tool,
            radius,
            shape,
            anchor,
            seed,
        }
    }

    /// Evaluate one preview/planner sample using the exact settings captured for this stroke.
    pub fn apply_sample(
        &self,
        center: MapTile,
        height: &dyn Fn((i32, i32)) -> Option<i32>,
    ) -> Vec<((i32, i32), i32)> {
        apply_height(
            &self.tool,
            (center.x, center.y),
            self.radius,
            self.shape,
            self.anchor,
            self.seed,
            height,
        )
    }
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

    Ok(decode_height(base, encode_height(base, desired)))
}

/// Plan one logical Height stroke as one semantic patch.
///
/// `centers` is ordered input: repeated samples are intentional and later samples see the
/// storage-quantized result of earlier samples, matching the current held/dragged sculpt behavior.
/// Missing target heights are skipped rather than treated as numeric zero. On planes above zero,
/// an edited vertex requires an explicit height on the plane below so cache encoding never invents
/// a base height.
pub fn build_terrain_height_patch(
    query: &impl MapQuery,
    spec: &HeightStrokeSpec,
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
                    .or_else(|| query_height(query, spec.plane, tile))
            };
            spec.apply_sample(*center, &height)
        };

        for (tile, desired) in proposed {
            let tile = MapTile::new(tile.0, tile.1);
            let current = staged
                .get(&tile)
                .copied()
                .or_else(|| query_height(query, spec.plane, tile))
                .unwrap_or_else(|| {
                    unreachable!("Height arithmetic only returns vertices with explicit heights")
                });
            let after = quantize_height(query, spec.plane, tile, desired)?;
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
            coord: HeightVertexCoord::new(tile, spec.plane),
            before,
            after,
        })
    });

    Ok(MapPatch::new(ops))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MapDocument;
    use crate::height::{Falloff, HeightOp};

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

    fn spec(
        source_plane: SourcePlane,
        op: HeightOp,
        strength: i32,
        anchor: i32,
        seed: u64,
    ) -> HeightStrokeSpec {
        HeightStrokeSpec::new(
            source_plane,
            tool(op, strength),
            0,
            BrushShape::Square,
            anchor,
            seed,
        )
    }

    #[test]
    fn repeated_samples_fold_into_one_final_height_operation() {
        let tile = MapTile::new(3200, 3200);
        let source_plane = plane(0);
        let document = MapDocument::from_parts([], [(vertex(tile, source_plane), 0)], []);
        let stroke = spec(source_plane, HeightOp::Raise, 16, 0, 7);
        let patch = build_terrain_height_patch(&document, &stroke, &[tile, tile])
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
        let stroke = spec(source_plane, HeightOp::Raise, 16, 0, 1);
        let patch = build_terrain_height_patch(&document, &stroke, &[tile])
            .unwrap_or_else(|error| unreachable!("missing target vertices are skipped: {error:?}"));

        assert!(patch.is_empty());
    }

    #[test]
    fn non_zero_plane_requires_explicit_height_below() {
        let tile = MapTile::new(3200, 3200);
        let upper = plane(1);
        let lower = plane(0);
        let document = MapDocument::from_parts([], [(vertex(tile, upper), -96)], []);
        let stroke = spec(upper, HeightOp::Lower, 8, -96, 1);
        let error = match build_terrain_height_patch(&document, &stroke, &[tile]) {
            Err(error) => error,
            Ok(_) => unreachable!("upper-plane cache encoding must require a lower-plane base"),
        };

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
        let stroke = spec(upper, HeightOp::Lower, 8, -96, 1);
        let patch = build_terrain_height_patch(&document, &stroke, &[tile])
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
        let stroke = spec(source_plane, HeightOp::Lower, 8, 0, 1);
        let patch = build_terrain_height_patch(&document, &stroke, &[tile])
            .unwrap_or_else(|error| unreachable!("plane-zero Height stroke is valid: {error:?}"));

        assert!(patch.is_empty());
    }

    #[test]
    fn negative_world_coordinates_remain_typed_and_exact() {
        let tile = MapTile::new(-65, -1);
        let source_plane = plane(0);
        let document = MapDocument::from_parts([], [(vertex(tile, source_plane), -40)], []);
        let stroke = spec(source_plane, HeightOp::Raise, 16, -40, 99);
        let patch = build_terrain_height_patch(&document, &stroke, &[tile])
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
        let stroke = spec(source_plane, HeightOp::Raise, 16, 0, 1);
        let patch = build_terrain_height_patch(&document, &stroke, &[a, b])
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
