//! Authored NPC state. Pure patches validate completely before mutation.
//!
//! Simulated positions, queues and animation clocks are never stored here; they belong
//! to `osrs-playtest` and never enter authoring history.
use crate::{ChangeImpact, PatchApplyError};
use osrs_core::{
    ids::NpcId,
    scenario::{
        ActorBlocking, ActorLocation, MovementRestriction, NpcBehavior, NpcSpawn, NpcSpawnId,
        ScenarioDefinition, SpawnSource,
    },
};
use std::collections::BTreeSet;

/// Authored scenario state. `revision` counts effective applications (including undo and redo)
/// so projections can tell when to rebuild; it is deliberately not part of equality.
#[derive(Debug, Clone, Default)]
pub struct ScenarioDocument {
    definition: Option<ScenarioDefinition>,
    revision: u64,
}

impl PartialEq for ScenarioDocument {
    fn eq(&self, other: &Self) -> bool {
        self.definition == other.definition
    }
}
impl Eq for ScenarioDocument {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpawnChange {
    pub id: NpcSpawnId,
    pub before: Option<NpcSpawn>,
    pub after: Option<NpcSpawn>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScenarioPatch {
    Replace {
        before: Option<ScenarioDefinition>,
        after: Option<ScenarioDefinition>,
    },
    Spawns(Vec<SpawnChange>),
}

impl ScenarioPatch {
    pub fn inverse(&self) -> Self {
        match self {
            Self::Replace { before, after } => Self::Replace {
                before: after.clone(),
                after: before.clone(),
            },
            Self::Spawns(changes) => Self::Spawns(
                changes
                    .iter()
                    .rev()
                    .map(|c| SpawnChange {
                        id: c.id,
                        before: c.after.clone(),
                        after: c.before.clone(),
                    })
                    .collect(),
            ),
        }
    }

    pub fn is_empty(&self) -> bool {
        match self {
            Self::Replace { before, after } => before == after,
            Self::Spawns(changes) => changes.iter().all(|c| c.before == c.after),
        }
    }
}

impl ScenarioDocument {
    pub fn definition(&self) -> Option<&ScenarioDefinition> {
        self.definition.as_ref()
    }

    /// Advances on every effective application, including undo and redo.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn spawn(&self, id: NpcSpawnId) -> Option<&NpcSpawn> {
        self.definition.as_ref()?.spawns.iter().find(|s| s.id == id)
    }

    pub fn apply(&mut self, patch: &ScenarioPatch) -> Result<ChangeImpact, PatchApplyError> {
        if patch.is_empty() {
            return Ok(ChangeImpact::NONE);
        }
        match patch {
            ScenarioPatch::Replace { before, after } => {
                if &self.definition != before {
                    return Err(PatchApplyError::ScenarioMismatch { op_index: 0 });
                }
                if after.as_ref().is_some_and(|s| s.validate().is_err()) {
                    return Err(PatchApplyError::InvalidScenario { op_index: 0 });
                }
                self.definition = after.clone();
            }
            ScenarioPatch::Spawns(changes) => {
                let definition = self
                    .definition
                    .as_ref()
                    .ok_or(PatchApplyError::ScenarioMismatch { op_index: 0 })?;
                let mut seen = BTreeSet::new();
                for (index, c) in changes.iter().enumerate() {
                    if !seen.insert(c.id)
                        || definition.spawns.iter().find(|s| s.id == c.id) != c.before.as_ref()
                    {
                        return Err(PatchApplyError::ScenarioMismatch { op_index: index });
                    }
                    if c.after.as_ref().is_some_and(|s| s.id != c.id) {
                        return Err(PatchApplyError::InvalidScenario { op_index: index });
                    }
                }
                let mut candidate = definition.clone();
                for c in changes {
                    candidate.spawns.retain(|s| s.id != c.id);
                    if let Some(after) = &c.after {
                        candidate.spawns.push(after.clone());
                    }
                }
                candidate.spawns.sort_by_key(|s| s.id);
                candidate
                    .validate()
                    .map_err(|_| PatchApplyError::InvalidScenario { op_index: 0 })?;
                self.definition = Some(candidate);
            }
        }
        self.revision += 1;
        Ok(ChangeImpact::SCENARIO)
    }
}

/// Why an authoring gesture cannot become a patch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScenarioPlanError {
    NoScenario,
    UnknownSpawn(NpcSpawnId),
    Invalid(String),
}

impl std::fmt::Display for ScenarioPlanError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoScenario => f.write_str("there is no scenario; create or import one first"),
            Self::UnknownSpawn(id) => write!(f, "spawn {} does not exist", id.0),
            Self::Invalid(reason) => f.write_str(reason),
        }
    }
}

/// Everything about a new spawn except its identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpawnDraft {
    pub npc: NpcId,
    pub symbol: String,
    pub location: ActorLocation,
    pub facing: u16,
    pub behavior: NpcBehavior,
    pub restriction: MovementRestriction,
    pub blocking: ActorBlocking,
}

impl SpawnDraft {
    pub fn new(npc: NpcId, symbol: impl Into<String>, location: ActorLocation) -> Self {
        Self {
            npc,
            symbol: symbol.into(),
            location,
            facing: 0,
            behavior: NpcBehavior::Wander { radius: 5 },
            restriction: MovementRestriction::Normal,
            blocking: ActorBlocking::Npc,
        }
    }
}

fn definition_of(document: &ScenarioDocument) -> Result<&ScenarioDefinition, ScenarioPlanError> {
    document.definition().ok_or(ScenarioPlanError::NoScenario)
}

/// Install `after` (or clear the scenario) in one undoable step.
pub fn plan_replace_scenario(
    document: &ScenarioDocument,
    after: Option<ScenarioDefinition>,
) -> Result<ScenarioPatch, ScenarioPlanError> {
    if let Some(after) = &after {
        after.validate().map_err(ScenarioPlanError::Invalid)?;
    }
    Ok(ScenarioPatch::Replace {
        before: document.definition().cloned(),
        after,
    })
}

/// Add one spawn with the next free identity. Returns the patch and the new identity.
pub fn plan_add_spawn(
    document: &ScenarioDocument,
    draft: SpawnDraft,
) -> Result<(ScenarioPatch, NpcSpawnId), ScenarioPlanError> {
    let definition = definition_of(document)?;
    let id = NpcSpawnId(definition.spawns.iter().map(|s| s.id.0).max().unwrap_or(0) + 1);
    let spawn = NpcSpawn {
        id,
        npc: draft.npc,
        symbol: draft.symbol,
        location: draft.location,
        facing: draft.facing,
        behavior: draft.behavior,
        restriction: draft.restriction,
        blocking: draft.blocking,
        source: SpawnSource {
            file: "authored".into(),
            entry: u32::try_from(id.0).unwrap_or(u32::MAX),
        },
    };
    let patch = ScenarioPatch::Spawns(vec![SpawnChange {
        id,
        before: None,
        after: Some(spawn),
    }]);
    validate_plan(document, &patch)?;
    Ok((patch, id))
}

/// Remove spawns; duplicate and unknown identities are rejected rather than ignored.
pub fn plan_remove_spawns(
    document: &ScenarioDocument,
    ids: &[NpcSpawnId],
) -> Result<ScenarioPatch, ScenarioPlanError> {
    definition_of(document)?;
    let mut changes = Vec::with_capacity(ids.len());
    let mut seen = BTreeSet::new();
    for &id in ids {
        let before = document
            .spawn(id)
            .cloned()
            .ok_or(ScenarioPlanError::UnknownSpawn(id))?;
        if seen.insert(id) {
            changes.push(SpawnChange {
                id,
                before: Some(before),
                after: None,
            });
        }
    }
    Ok(ScenarioPatch::Spawns(changes))
}

/// Edit one spawn. An edit that changes nothing yields an empty patch (a no-op commit).
pub fn plan_update_spawn(
    document: &ScenarioDocument,
    id: NpcSpawnId,
    edit: impl FnOnce(&mut NpcSpawn),
) -> Result<ScenarioPatch, ScenarioPlanError> {
    definition_of(document)?;
    let before = document
        .spawn(id)
        .cloned()
        .ok_or(ScenarioPlanError::UnknownSpawn(id))?;
    let mut after = before.clone();
    edit(&mut after);
    after.id = id;
    let patch = ScenarioPatch::Spawns(vec![SpawnChange {
        id,
        before: Some(before),
        after: Some(after),
    }]);
    validate_plan(document, &patch)?;
    Ok(patch)
}

fn validate_plan(
    document: &ScenarioDocument,
    patch: &ScenarioPatch,
) -> Result<(), ScenarioPlanError> {
    document
        .clone()
        .apply(patch)
        .map(|_| ())
        .map_err(|error| ScenarioPlanError::Invalid(format!("{error:?}")))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::{HistoryDocument, WorkspaceDocument, WorkspaceHistory, WorkspacePatch};
    use osrs_core::coords::{MapTile, PlaneIndex};

    fn location(x: i32, y: i32) -> ActorLocation {
        ActorLocation {
            tile: MapTile::new(x, y),
            plane: PlaneIndex::new(0).unwrap(),
        }
    }

    fn empty() -> ScenarioDefinition {
        ScenarioDefinition {
            version: 1,
            build: 240,
            client_fingerprint: "a".repeat(64),
            seed: 7,
            spawns: Vec::new(),
        }
    }

    fn workspace_with_scenario() -> (WorkspaceDocument, WorkspaceHistory) {
        let mut document = WorkspaceDocument::default();
        let mut history = WorkspaceHistory::new();
        let patch = plan_replace_scenario(document.scenario(), Some(empty())).unwrap();
        history
            .commit(&mut document, WorkspacePatch::Scenario(patch))
            .unwrap();
        (document, history)
    }

    fn commit(
        document: &mut WorkspaceDocument,
        history: &mut WorkspaceHistory,
        patch: ScenarioPatch,
    ) {
        history
            .commit(document, WorkspacePatch::Scenario(patch))
            .unwrap();
    }

    #[test]
    fn add_update_remove_round_trip_exactly_through_history() {
        let (mut document, mut history) = workspace_with_scenario();
        let baseline = document.clone();
        let draft = SpawnDraft::new(NpcId::new(3), "npc.hans", location(3200, 3200));
        let (add, id) = plan_add_spawn(document.scenario(), draft).unwrap();
        commit(&mut document, &mut history, add);
        let update = plan_update_spawn(document.scenario(), id, |s| s.facing = 512).unwrap();
        commit(&mut document, &mut history, update);
        let remove = plan_remove_spawns(document.scenario(), &[id]).unwrap();
        commit(&mut document, &mut history, remove);
        assert!(document.scenario().spawn(id).is_none());

        history.undo(&mut document).unwrap();
        assert_eq!(document.scenario().spawn(id).unwrap().facing, 512);
        history.undo(&mut document).unwrap();
        assert_eq!(document.scenario().spawn(id).unwrap().facing, 0);
        history.undo(&mut document).unwrap();
        assert_eq!(document, {
            let mut expected = baseline.clone();
            expected.clone_from(&baseline);
            expected
        });
        history.redo(&mut document).unwrap();
        history.redo(&mut document).unwrap();
        history.redo(&mut document).unwrap();
        assert!(document.scenario().spawn(id).is_none());
    }

    #[test]
    fn empty_gesture_keeps_redo_and_does_not_advance_revision() {
        let (mut document, mut history) = workspace_with_scenario();
        let draft = SpawnDraft::new(NpcId::new(3), "npc.hans", location(10, 10));
        let (add, id) = plan_add_spawn(document.scenario(), draft).unwrap();
        commit(&mut document, &mut history, add);
        history.undo(&mut document).unwrap();
        assert!(history.can_redo());
        let revision = document.scenario().revision();
        let unchanged = plan_update_spawn(document.scenario(), id, |_| {});
        assert_eq!(unchanged, Err(ScenarioPlanError::UnknownSpawn(id)));
        let noop = ScenarioPatch::Replace {
            before: document.scenario().definition().cloned(),
            after: document.scenario().definition().cloned(),
        };
        assert!(noop.is_empty());
        history
            .commit(&mut document, WorkspacePatch::Scenario(noop))
            .unwrap();
        assert!(history.can_redo());
        assert_eq!(document.scenario().revision(), revision);
    }

    #[test]
    fn stale_or_invalid_patches_leave_the_document_untouched() {
        let (mut document, _) = workspace_with_scenario();
        let draft = SpawnDraft::new(NpcId::new(3), "npc.hans", location(10, 10));
        let (add, id) = plan_add_spawn(document.scenario(), draft).unwrap();
        let before = document.clone();
        // Applying the same addition twice is stale.
        document
            .apply_history_patch(&WorkspacePatch::Scenario(add.clone()))
            .unwrap();
        let after_first = document.clone();
        assert!(
            document
                .apply_history_patch(&WorkspacePatch::Scenario(add))
                .is_err()
        );
        assert_eq!(document, after_first);
        assert_ne!(document, before);
        // An out-of-range facing is rejected before any mutation.
        let bad = plan_update_spawn(document.scenario(), id, |s| s.facing = 4096);
        assert!(matches!(bad, Err(ScenarioPlanError::Invalid(_))));
        assert_eq!(document, after_first);
    }

    #[test]
    fn colocated_duplicates_keep_distinct_identities() {
        let (mut document, mut history) = workspace_with_scenario();
        for _ in 0..2 {
            let draft = SpawnDraft::new(NpcId::new(3), "npc.hans", location(10, 10));
            let (patch, _) = plan_add_spawn(document.scenario(), draft).unwrap();
            commit(&mut document, &mut history, patch);
        }
        let spawns = &document.scenario().definition().unwrap().spawns;
        assert_eq!(spawns.len(), 2);
        assert_ne!(spawns[0].id, spawns[1].id);
    }
}
