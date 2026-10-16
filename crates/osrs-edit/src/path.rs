//! Semantic commit adapter for the existing terrain-path tool.
//!
//! Planning and preview remain in `terrain_path` / `spline_tool`. This module only turns the
//! already-planned tile intent into one reversible `MapPatch` against read-only semantic state.
//! Smoothing the ground under a path is `path_height`.

use crate::{
    MapOp, MapPatch, MapPosition, MapQuery, OverlayRotation, OverlayShape, TerrainOverlay,
    TerrainTile,
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathCommitError {
    InvalidOverlayShape { tile: MapTile, shape: u8 },
    InvalidOverlayRotation { tile: MapTile, rotation: u8 },
    MissingHeight { tile: MapTile },
}

/// Build one compact semantic patch for a completed terrain-path gesture: the fitted overlay,
/// shape and rotation of every tile. The tile's underlay, height and flags are left as they are.
///
/// Duplicate tile intents are folded into one final terrain operation per coordinate. This keeps
/// one laid path deterministic and prevents transient duplicate planner output from becoming
/// redundant history operations.
pub fn build_terrain_path_patch(
    query: &impl MapQuery,
    plane: SourcePlane,
    intents: &[TerrainPathIntent],
) -> Result<MapPatch, PathCommitError> {
    let mut terrain_order = Vec::new();
    let mut terrain = HashMap::<MapPosition, (TerrainTile, TerrainTile)>::new();

    for intent in intents {
        let position = MapPosition::new(intent.tile, plane);
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
        entry.1 = after;
    }

    let ops = terrain_order.into_iter().filter_map(|coord| {
        let (before, after) = terrain.get(&coord).copied()?;
        (before != after).then_some(MapOp::SetTerrain {
            coord,
            before,
            after,
        })
    });
    Ok(MapPatch::new(ops))
}

#[cfg(test)]
mod tests;
