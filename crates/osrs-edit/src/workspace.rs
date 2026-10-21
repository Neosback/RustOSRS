//! One chronological typed history for map and authored scenario changes.
//! This does not change MapPatch or serialize live actors into MapDocument.
use crate::{
    ChangeImpact, HistoryDocument, MapDocument, MapPatch, MapRegionSnapshot, PatchApplyError,
    PatchHistory, ReversiblePatch, ScenarioDocument, ScenarioPatch,
};
use osrs_core::coords::RegionCoord;

/// Map plus authored scenario. `map_revision` counts effective map applications (edits, undo,
/// redo, project application) so simulations know when collision inputs
/// changed; it is deliberately not part of equality.
#[derive(Debug, Clone, Default)]
pub struct WorkspaceDocument {
    map: MapDocument,
    scenario: ScenarioDocument,
    map_revision: u64,
}

impl PartialEq for WorkspaceDocument {
    fn eq(&self, other: &Self) -> bool {
        self.map == other.map && self.scenario == other.scenario
    }
}
impl Eq for WorkspaceDocument {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkspacePatch {
    Map(MapPatch),
    Scenario(ScenarioPatch),
}
pub type WorkspaceHistory = PatchHistory<WorkspaceDocument>;

impl WorkspaceDocument {
    pub fn new(map: MapDocument) -> Self {
        Self::from_parts(map, ScenarioDocument::default())
    }
    /// Compose already validated authored domains as a fresh baseline, without a history edit.
    pub fn from_parts(map: MapDocument, scenario: ScenarioDocument) -> Self {
        Self {
            map,
            scenario,
            map_revision: 0,
        }
    }
    pub fn map(&self) -> &MapDocument {
        &self.map
    }
    pub fn scenario(&self) -> &ScenarioDocument {
        &self.scenario
    }
    pub fn map_revision(&self) -> u64 {
        self.map_revision
    }
    pub fn apply_map_untracked(
        &mut self,
        patch: &MapPatch,
    ) -> Result<ChangeImpact, PatchApplyError> {
        let impact = self.map.apply_patch(patch)?;
        if !patch.is_empty() {
            self.map_revision += 1;
        }
        Ok(impact)
    }
    /// Streaming hydration is not an authored edit and does not advance `map_revision`.
    pub fn replace_region(&mut self, snapshot: &MapRegionSnapshot) -> bool {
        self.map.replace_region(snapshot)
    }
    pub fn remove_region(&mut self, region: RegionCoord) -> bool {
        self.map.remove_region(region)
    }
}

impl std::ops::Deref for WorkspaceDocument {
    type Target = MapDocument;
    fn deref(&self) -> &MapDocument {
        &self.map
    }
}

impl ReversiblePatch for WorkspacePatch {
    fn inverse(&self) -> Self {
        match self {
            Self::Map(p) => Self::Map(p.inverse()),
            Self::Scenario(p) => Self::Scenario(p.inverse()),
        }
    }
    fn is_empty(&self) -> bool {
        match self {
            Self::Map(p) => p.is_empty(),
            Self::Scenario(p) => p.is_empty(),
        }
    }
}

impl HistoryDocument for WorkspaceDocument {
    type Patch = WorkspacePatch;
    fn apply_history_patch(
        &mut self,
        patch: &WorkspacePatch,
    ) -> Result<ChangeImpact, PatchApplyError> {
        match patch {
            WorkspacePatch::Map(p) => self.apply_map_untracked(p),
            WorkspacePatch::Scenario(p) => self.scenario.apply(p),
        }
    }
}
