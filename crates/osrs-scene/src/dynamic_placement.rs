//! Runtime active-definition placement resolution for M8.
//!
//! Morph selection itself is owned by `osrs-core`. This module composes that
//! selection with canonical object-definition lookup and the existing exact M6
//! height/placement planners so footprint-dependent inputs are recomputed from
//! the active transformed definition before later animation or contouring work.

use crate::placement::{PlacementError, PlacementInput, PlacementPlan, plan_placement};
use crate::placement_height::{
    PlacementHeightError, PlacementHeightInput, sample_placement_height,
};
use osrs_core::{
    coords::SceneTile,
    definitions::{LocType, ObjectDefinition},
    ids::ObjectId,
    morph::{MorphResolveError, MorphVariableState, resolve_object_morph},
};
use std::{error::Error, fmt};

/// Cache/editor-independent access to canonical object definitions.
pub trait ObjectDefinitionLookup {
    fn object_definition(&self, id: ObjectId) -> Option<&ObjectDefinition>;
}

/// Inputs whose footprint-dependent values must be rebuilt from the active
/// definition at runtime.
#[derive(Debug, Clone, Copy)]
pub struct DynamicPlacementInput<'a> {
    pub source_definition: &'a ObjectDefinition,
    pub loc_type: LocType,
    pub orientation: u8,
    pub tile: SceneTile,
    pub heights: &'a [Vec<i32>],
    pub scene_width: u32,
    pub scene_height: u32,
    pub existing_wall_displacement: Option<u16>,
}

/// Active transformed definition plus exact placement inputs derived from it.
#[derive(Debug, Clone, Copy)]
pub struct ResolvedDynamicPlacement<'a> {
    pub active_definition: &'a ObjectDefinition,
    pub placement: PlacementPlan,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DynamicPlacementError {
    Morph(MorphResolveError),
    MissingObjectDefinition(ObjectId),
    ObjectDefinitionMismatch {
        requested: ObjectId,
        actual: ObjectId,
    },
    Height(PlacementHeightError),
    Placement(PlacementError),
}

impl fmt::Display for DynamicPlacementError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Morph(error) => write!(formatter, "morph resolution failed: {error}"),
            Self::MissingObjectDefinition(id) => {
                write!(formatter, "missing active object definition {id}")
            }
            Self::ObjectDefinitionMismatch { requested, actual } => write!(
                formatter,
                "object lookup returned definition {actual} for requested {requested}"
            ),
            Self::Height(error) => write!(formatter, "dynamic placement height failed: {error}"),
            Self::Placement(error) => {
                write!(formatter, "dynamic placement planning failed: {error}")
            }
        }
    }
}

impl Error for DynamicPlacementError {}

impl From<MorphResolveError> for DynamicPlacementError {
    fn from(value: MorphResolveError) -> Self {
        Self::Morph(value)
    }
}

impl From<PlacementHeightError> for DynamicPlacementError {
    fn from(value: PlacementHeightError) -> Self {
        Self::Height(value)
    }
}

impl From<PlacementError> for DynamicPlacementError {
    fn from(value: PlacementError) -> Self {
        Self::Placement(value)
    }
}

/// Resolve the current object definition and rebuild every footprint-dependent
/// placement input from that active definition.
///
/// `Ok(None)` is the exact semantic null-morph result. No placement/model must
/// be invented in that state.
pub fn resolve_dynamic_placement<'a, D, S>(
    definitions: &'a D,
    state: &S,
    input: DynamicPlacementInput<'a>,
) -> Result<Option<ResolvedDynamicPlacement<'a>>, DynamicPlacementError>
where
    D: ObjectDefinitionLookup + ?Sized,
    S: MorphVariableState + ?Sized,
{
    let active_definition = if let Some(morphs) = &input.source_definition.morphs {
        let Some(active_id) = resolve_object_morph(morphs, state)? else {
            return Ok(None);
        };
        let definition = definitions
            .object_definition(active_id)
            .ok_or(DynamicPlacementError::MissingObjectDefinition(active_id))?;
        if definition.identity.id != active_id {
            return Err(DynamicPlacementError::ObjectDefinitionMismatch {
                requested: active_id,
                actual: definition.identity.id,
            });
        }
        definition
    } else {
        input.source_definition
    };

    let sampled_height = sample_placement_height(
        input.heights,
        input.scene_width,
        input.scene_height,
        PlacementHeightInput {
            tile: input.tile,
            size_x: active_definition.size_x,
            size_y: active_definition.size_y,
            orientation: input.orientation,
        },
    )?;

    let placement = plan_placement(PlacementInput {
        loc_type: input.loc_type,
        orientation: input.orientation,
        tile: input.tile,
        size_x: active_definition.size_x,
        size_y: active_definition.size_y,
        sampled_height,
        existing_wall_displacement: input.existing_wall_displacement,
    })?;

    Ok(Some(ResolvedDynamicPlacement {
        active_definition,
        placement,
    }))
}
