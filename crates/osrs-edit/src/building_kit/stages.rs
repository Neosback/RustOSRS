//! Stages are views of the canonical generated patch, never a second edit history.
use super::*;
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BuildingStage {
    Foundation,
    Floors,
    Circulation,
    Shell,
    Roof,
    Furnishings,
    Decoration,
}
impl BuildingKit {
    pub fn stage_of(&self, op: &MapOp) -> BuildingStage {
        match op {
            MapOp::SetHeight { .. } => BuildingStage::Foundation,
            MapOp::SetTerrain { .. } => BuildingStage::Floors,
            MapOp::AddObject { placement } | MapOp::RemoveObject { placement } => {
                if self.hip.is_some_and(|h| [h.slope,h.edge].contains(&placement.object_id))
                    || self.patterns.as_ref().is_some_and(|r| r.catalog.all_patterns().iter().any(|id| r.palette.asset(id).is_some_and(|a| a.object_id==placement.object_id)))
                { return BuildingStage::Roof; }
                if placement.object_id == self.detail.base.door.object_id
                    || self
                        .detail
                        .door_variants
                        .iter()
                        .any(|d| d.asset.object_id == placement.object_id)
                {
                    return BuildingStage::Circulation;
                }
                match placement.kind.get() {
                    0..=3 => BuildingStage::Shell,
                    4..=8 | 22 => BuildingStage::Decoration,
                    12..=21 => BuildingStage::Roof,
                    _ => BuildingStage::Furnishings,
                }
            }
        }
    }
}
