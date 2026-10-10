//! Runtime dynamic-loc model resolution for M8.
//!
//! This module composes the already-verified morph/placement, legacy pose, and
//! contour kernels in the pinned `DynamicObject.getModel` / object dynamic-model
//! order. Cache transport and raw-model construction stay outside `osrs-scene`:
//! callers expose the cached reference-lit base selected for the active object
//! definition and exact model request.

use crate::{
    dynamic_placement::{
        DynamicPlacementError, DynamicPlacementInput, ObjectDefinitionLookup,
        ResolvedDynamicPlacement, resolve_dynamic_placement,
    },
    placement::{ModelRequest, PlacementPlan},
};
use osrs_core::{
    animation_pose::{LegacyAnimationFrame, LegacyPoseError, pose_legacy_object_model},
    contour::{ContourGroundError, ContourGroundInput, contour_ground_in_place},
    definitions::ObjectDefinition,
    lighting::ReferenceLitModel,
    morph::MorphVariableState,
};
use std::{borrow::Cow, error::Error, fmt};

/// Cache-independent access to the cached lit base used by the dynamic object
/// model path.
///
/// The lookup key is the active (post-morph) object definition plus the exact
/// placement model request. `None` preserves the reference null-model result.
pub trait DynamicLitModelLookup {
    fn dynamic_lit_model(
        &self,
        definition: &ObjectDefinition,
        request: ModelRequest,
    ) -> Option<&ReferenceLitModel>;
}

/// Inputs for one runtime dynamic-model resolution.
#[derive(Debug, Clone, Copy)]
pub struct DynamicModelInput<'a> {
    /// Resolves the current active definition and recomputes footprint-dependent
    /// placement/height state before model lookup.
    pub placement: DynamicPlacementInput<'a>,
    /// Exact model type/orientation for this renderable arm.
    pub model_request: ModelRequest,
    /// Current canonical legacy frame, when the caller's deterministic sequence
    /// state selects one. Cached-skeletal posing remains outside this M8 scope.
    pub legacy_frame: Option<&'a LegacyAnimationFrame>,
}

/// Active definition, recomputed placement, and resulting runtime model.
///
/// `Borrowed` is possible only when neither pose nor contour requires a private
/// instance. Any mutating dynamic path returns an owned working model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedDynamicModel<'a> {
    pub active_definition: &'a ObjectDefinition,
    pub placement: PlacementPlan,
    pub model: Cow<'a, ReferenceLitModel>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DynamicModelError {
    Placement(DynamicPlacementError),
    LegacyPose(LegacyPoseError),
    ContourClipOutOfRange(u32),
    Contour(ContourGroundError),
}

impl fmt::Display for DynamicModelError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Placement(error) => error.fmt(formatter),
            Self::LegacyPose(error) => write!(formatter, "dynamic legacy pose failed: {error}"),
            Self::ContourClipOutOfRange(clip) => write!(
                formatter,
                "object contour clip {clip} exceeds reference signed-int range"
            ),
            Self::Contour(error) => write!(formatter, "dynamic contour failed: {error}"),
        }
    }
}

impl Error for DynamicModelError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Placement(error) => Some(error),
            Self::LegacyPose(error) => Some(error),
            Self::Contour(error) => Some(error),
            Self::ContourClipOutOfRange(_) => None,
        }
    }
}

impl From<DynamicPlacementError> for DynamicModelError {
    fn from(value: DynamicPlacementError) -> Self {
        Self::Placement(value)
    }
}

impl From<LegacyPoseError> for DynamicModelError {
    fn from(value: LegacyPoseError) -> Self {
        Self::LegacyPose(value)
    }
}

impl From<ContourGroundError> for DynamicModelError {
    fn from(value: ContourGroundError) -> Self {
        Self::Contour(value)
    }
}

/// Resolve one dynamic loc from current morph state through its final semantic
/// runtime model.
///
/// Exact ordering:
///
/// 1. resolve the active morph and footprint-dependent placement;
/// 2. null morph returns `None` before any model lookup;
/// 3. look up the cached lit base using the active definition;
/// 4. a current legacy frame creates a private posed model;
/// 5. otherwise contouring still creates a private working copy;
/// 6. contour that private model in place, after pose;
/// 7. when neither pose nor contour applies, borrow the immutable cached base.
///
/// The shared cached base is never mutated.
pub fn resolve_dynamic_model<'a, D, S, M>(
    definitions: &'a D,
    state: &S,
    models: &'a M,
    input: DynamicModelInput<'a>,
) -> Result<Option<ResolvedDynamicModel<'a>>, DynamicModelError>
where
    D: ObjectDefinitionLookup + ?Sized,
    S: MorphVariableState + ?Sized,
    M: DynamicLitModelLookup + ?Sized,
{
    let Some(ResolvedDynamicPlacement {
        active_definition,
        placement,
    }) = resolve_dynamic_placement(definitions, state, input.placement)?
    else {
        return Ok(None);
    };

    let Some(base) = models.dynamic_lit_model(active_definition, input.model_request) else {
        return Ok(None);
    };

    let contour_clip = active_definition
        .contour_clip
        .map(|clip| i32::try_from(clip).map_err(|_| DynamicModelError::ContourClipOutOfRange(clip)))
        .transpose()?;

    if input.legacy_frame.is_none() && contour_clip.is_none() {
        return Ok(Some(ResolvedDynamicModel {
            active_definition,
            placement,
            model: Cow::Borrowed(base),
        }));
    }

    let mut working = match input.legacy_frame {
        Some(frame) => pose_legacy_object_model(base, frame, input.model_request.orientation)?,
        None => base.clone(),
    };

    if let Some(clip) = contour_clip {
        contour_ground_in_place(
            &mut working,
            ContourGroundInput {
                heights: input.placement.heights,
                origin_x: placement.model_center.x.units(),
                base_height: placement.model_center.y.units(),
                origin_z: placement.model_center.z.units(),
                clip,
            },
        )?;
    }

    Ok(Some(ResolvedDynamicModel {
        active_definition,
        placement,
        model: Cow::Owned(working),
    }))
}
