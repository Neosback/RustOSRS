//! Floor application of a solved symbolic catalog. Object generation remains a separate planner.
use super::{LearnedCatalog, LearningError};
use crate::{
    MapOp, MapPatch, MapPosition, MapQuery, OverlayRotation, OverlayShape, TerrainOverlay,
    TerrainTile, WfcCoord, WfcSolution,
};
use osrs_core::{
    coords::{MapTile, SourcePlane},
    ids::{FloorOverlayId, FloorUnderlayId},
};
/// Plan against the authored document; a canceled preview never commits this patch.
pub fn plan_wfc_floors(
    document: &impl MapQuery,
    data: &LearnedCatalog,
    solution: &WfcSolution,
    origin: MapTile,
    plane: SourcePlane,
) -> Result<MapPatch, LearningError> {
    let shape = OverlayShape::new(0).ok_or(LearningError::InvalidCatalog)?;
    let rotation = OverlayRotation::new(0).ok_or(LearningError::InvalidCatalog)?;
    let mut ops = Vec::new();
    for y in 0..solution.height() {
        for x in 0..solution.width() {
            let Some(pat_id) = solution.at(WfcCoord::new(x, y)) else {
                continue;
            };
            let pat = data
                .patterns
                .get(usize::from(pat_id.get()))
                .ok_or(LearningError::InvalidCatalog)?;
            if pat.floor_class == 0 {
                continue;
            }
            let tile = MapTile::new(
                origin
                    .x
                    .checked_add(i32::from(x))
                    .ok_or(LearningError::InvalidGrid)?,
                origin
                    .y
                    .checked_add(i32::from(y))
                    .ok_or(LearningError::InvalidGrid)?,
            );
            let coord = MapPosition::new(tile, plane);
            let before = document.terrain_at(coord);
            let raw_underlay = (pat.floor_class >> 16) & 0xFFFF;
            let raw_overlay = pat.floor_class & 0xFFFF;
            let underlay = if raw_underlay != 0 {
                Some(FloorUnderlayId::new(raw_underlay))
            } else {
                before.underlay
            };
            let overlay = (raw_overlay != 0).then(|| TerrainOverlay {
                id: FloorOverlayId::new(raw_overlay),
                shape,
                rotation,
            });
            let after = TerrainTile {
                underlay,
                overlay,
                flags: before.flags,
            };
            if before != after {
                ops.push(MapOp::SetTerrain {
                    coord,
                    before,
                    after,
                });
            }
        }
    }
    Ok(MapPatch::new(ops))
}
