//! Runtime pending-spawn replacement contract for M8.
//!
//! The pinned client treats a pending spawn as a live scene mutation, not as a
//! second initial-region-build path. The existing scene category is selected
//! independently from the replacement loc type, removal happens first, and a
//! failed/absent replacement does not roll the removal back.

use crate::placement::{PlacementPlan, SceneLayer};
use osrs_core::{
    coords::{SceneTile, StoragePlane},
    ids::ObjectId,
};
use std::{error::Error, fmt};

/// Reference pending-spawn scene category (`type` in the pinned client call).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PendingSceneCategory {
    Boundary,
    WallDecoration,
    GameObject,
    FloorDecoration,
}

impl PendingSceneCategory {
    /// Decode the exact reference category numbering used by live replacement.
    pub const fn from_reference_type(value: u8) -> Result<Self, PendingReplacementPlanError> {
        match value {
            0 => Ok(Self::Boundary),
            1 => Ok(Self::WallDecoration),
            2 => Ok(Self::GameObject),
            3 => Ok(Self::FloorDecoration),
            other => Err(PendingReplacementPlanError::InvalidSceneCategory(other)),
        }
    }

    pub const fn layer(self) -> SceneLayer {
        match self {
            Self::Boundary => SceneLayer::Boundary,
            Self::WallDecoration => SceneLayer::WallDecoration,
            Self::GameObject => SceneLayer::GameObject,
            Self::FloorDecoration => SceneLayer::FloorDecoration,
        }
    }
}

/// Existing scene occupancy selected for the removal half of a pending update.
///
/// For [`PendingSceneCategory::GameObject`], `tile` is specifically the object
/// anchor (`startX/startY`) selector used by the reference `Scene.removeGameObject`.
/// An object that merely overlaps this tile must not match.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PendingRemoval {
    pub plane: StoragePlane,
    pub tile: SceneTile,
    pub category: PendingSceneCategory,
}

/// Replacement placement attempted after the selected old occupancy is removed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PendingInsertion {
    pub plane: StoragePlane,
    pub tile: SceneTile,
    pub object_id: ObjectId,
    pub placement: PlacementPlan,
}

/// Source-ordered live replacement request.
///
/// `removal` always exists because the client probes/removes the selected scene
/// category even when the update is a deletion (`replacement == None`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PendingReplacementPlan {
    pub removal: PendingRemoval,
    pub replacement: Option<PendingInsertion>,
}

/// Observable mutation result when both source-ordered operations completed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PendingReplacementReport {
    pub removed: bool,
    pub inserted: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PendingReplacementPlanError {
    InvalidSceneCategory(u8),
}

impl fmt::Display for PendingReplacementPlanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSceneCategory(value) => {
                write!(
                    formatter,
                    "pending-spawn scene category {value} is outside 0..=3"
                )
            }
        }
    }
}

impl Error for PendingReplacementPlanError {}

/// Storage adapter used by the M8 runtime path.
///
/// Concrete scene mutation is deliberately separated from planning so initial
/// scene construction cannot accidentally inherit live replacement ordering.
pub trait PendingSceneMutation {
    type Error;

    /// Remove the selected old category. Game-object implementations must match
    /// the anchor tile, not any object merely covering the tile.
    fn remove_pending(&mut self, removal: PendingRemoval) -> Result<bool, Self::Error>;

    /// Attempt the replacement after removal. `false` is a source-compatible
    /// insertion rejection and does not request rollback of the removal.
    fn insert_pending(&mut self, insertion: PendingInsertion) -> Result<bool, Self::Error>;
}

/// Build one pending/live replacement plan from the exact reference category.
pub const fn plan_pending_replacement(
    plane: StoragePlane,
    tile: SceneTile,
    reference_category: u8,
    replacement: Option<(ObjectId, PlacementPlan)>,
) -> Result<PendingReplacementPlan, PendingReplacementPlanError> {
    let category = match PendingSceneCategory::from_reference_type(reference_category) {
        Ok(category) => category,
        Err(error) => return Err(error),
    };
    let replacement = match replacement {
        Some((object_id, placement)) => Some(PendingInsertion {
            plane,
            tile,
            object_id,
            placement,
        }),
        None => None,
    };

    Ok(PendingReplacementPlan {
        removal: PendingRemoval {
            plane,
            tile,
            category,
        },
        replacement,
    })
}

/// Execute the pinned pending-spawn ordering: remove first, then insert.
///
/// There is intentionally no rollback. If insertion returns `false`, the old
/// occupancy stays removed. If insertion returns an error, that error is
/// propagated after the successful removal has already mutated the adapter.
pub fn apply_pending_replacement<M: PendingSceneMutation>(
    mutation: &mut M,
    plan: PendingReplacementPlan,
) -> Result<PendingReplacementReport, M::Error> {
    let removed = mutation.remove_pending(plan.removal)?;
    let inserted = match plan.replacement {
        Some(insertion) => mutation.insert_pending(insertion)?,
        None => false,
    };

    Ok(PendingReplacementReport { removed, inserted })
}
