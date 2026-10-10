//! Dynamic object lit-model caching and private instance assembly for M8.
//!
//! The pinned `ObjectComposition.getModelDynamic(...)` path caches a lit base
//! model independently from the initial `getEntity(...)` cache. Animation and
//! contouring must never mutate that shared base. This module owns that exact
//! cache/working-copy boundary for the currently verified legacy-frame path.

use crate::{
    animation_pose::{LegacyAnimationFrame, LegacyPoseError, pose_legacy_object_model},
    contour::{ContourGroundError, ContourGroundInput, contour_ground_in_place},
    definitions::{LocType, ObjectDefinition, ObjectModels},
    lighting::{LightingError, LightingParameters, ReferenceLitModel, light_model_data},
    model::WorkingModel,
};
use std::{borrow::Cow, collections::HashMap, error::Error, fmt};

/// Exact Java `ObjectDefinition_cachedModels` key used by `getModelDynamic`.
///
/// This key intentionally mirrors the numeric expression used by the reference
/// while remaining a distinct type from the initial-entity cache: the two
/// caches have different lifecycle/ownership semantics even though their key
/// arithmetic is the same.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DynamicModelKey {
    raw: i64,
}

impl DynamicModelKey {
    pub const fn raw(self) -> i64 {
        self.raw
    }

    pub fn for_object(
        definition: &ObjectDefinition,
        requested_type: LocType,
        orientation: u8,
    ) -> Self {
        let mut raw = (definition.identity.id.get() as i32).wrapping_shl(10);
        if matches!(definition.models.as_ref(), Some(ObjectModels::Typed(_))) {
            raw = raw.wrapping_add(i32::from(requested_type.get()).wrapping_shl(3));
        }
        raw = raw.wrapping_add(i32::from(orientation));
        Self {
            raw: i64::from(raw),
        }
    }
}

/// Terrain inputs supplied by the already-resolved dynamic placement path.
///
/// `origin_x`, `base_height`, and `origin_z` are the active definition's exact
/// model center inputs. The contour clip itself remains definition-owned and is
/// injected by [`DynamicModelCache::get_or_build_legacy`].
#[derive(Debug, Clone, Copy)]
pub struct DynamicModelTerrain<'a> {
    pub heights: &'a [Vec<i32>],
    pub origin_x: i32,
    pub base_height: i32,
    pub origin_z: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DynamicModelError {
    Lighting(LightingError),
    LegacyPose(LegacyPoseError),
    Contour(ContourGroundError),
    ContourClipOutOfRange(u32),
}

impl fmt::Display for DynamicModelError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Lighting(error) => write!(formatter, "dynamic base-model lighting failed: {error}"),
            Self::LegacyPose(error) => write!(formatter, "dynamic legacy pose failed: {error}"),
            Self::Contour(error) => write!(formatter, "dynamic contouring failed: {error}"),
            Self::ContourClipOutOfRange(value) => write!(
                formatter,
                "object contour clip {value} exceeds the reference signed-int domain"
            ),
        }
    }
}

impl Error for DynamicModelError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Lighting(error) => Some(error),
            Self::LegacyPose(error) => Some(error),
            Self::Contour(error) => Some(error),
            Self::ContourClipOutOfRange(_) => None,
        }
    }
}

impl From<LightingError> for DynamicModelError {
    fn from(value: LightingError) -> Self {
        Self::Lighting(value)
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

/// Reusable lit-model cache for the dynamic `getModelDynamic` path.
///
/// The supplied builder produces the already-selected/mirrored/combined and
/// instance-transformed semantic ModelData (`WorkingModel`) on a cache miss.
/// Lighting happens once before insertion. Per-instance animation and contouring
/// happen only after the cached base has been borrowed or cloned.
#[derive(Debug, Default)]
pub struct DynamicModelCache {
    entries: HashMap<DynamicModelKey, ReferenceLitModel>,
}

impl DynamicModelCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn cached_base(&self, key: DynamicModelKey) -> Option<&ReferenceLitModel> {
        self.entries.get(&key)
    }

    /// Resolve one currently verified legacy-frame dynamic model.
    ///
    /// Exact ownership/order contract:
    ///
    /// 1. obtain/cache the lit base model;
    /// 2. if neither animation nor contouring applies, return that base borrowed;
    /// 3. otherwise create a private working model (the pose helper clones when
    ///    a frame exists; the no-frame contour branch clones explicitly);
    /// 4. apply contouring in place only to that private instance.
    ///
    /// A definition contour clip with no terrain still forces the private-copy
    /// branch, matching `getModelDynamic`'s ownership decision before its null
    /// height-map guard. `Ok(None)` preserves semantic model absence.
    pub fn get_or_build_legacy<'a, F>(
        &'a mut self,
        definition: &ObjectDefinition,
        requested_type: LocType,
        orientation: u8,
        frame: Option<&LegacyAnimationFrame>,
        terrain: Option<DynamicModelTerrain<'_>>,
        build_model_data: F,
    ) -> Result<Option<Cow<'a, ReferenceLitModel>>, DynamicModelError>
    where
        F: FnOnce() -> Option<WorkingModel>,
    {
        let key = DynamicModelKey::for_object(definition, requested_type, orientation);
        if !self.entries.contains_key(&key) {
            let Some(model_data) = build_model_data() else {
                return Ok(None);
            };
            let lighting = LightingParameters::for_loc(definition.ambient, definition.contrast);
            let lit = light_model_data(&model_data, lighting)?;
            self.entries.insert(key, lit);
        }

        let Some(base) = self.entries.get(&key) else {
            unreachable!("dynamic model cache entry was inserted or already present");
        };
        let contour_clip = definition
            .contour_clip
            .map(|value| {
                i32::try_from(value).map_err(|_| DynamicModelError::ContourClipOutOfRange(value))
            })
            .transpose()?;

        if frame.is_none() && contour_clip.is_none() {
            return Ok(Some(Cow::Borrowed(base)));
        }

        let mut working = match frame {
            Some(frame) => pose_legacy_object_model(base, frame, orientation)?,
            None => base.clone(),
        };

        if let (Some(clip), Some(terrain)) = (contour_clip, terrain) {
            contour_ground_in_place(
                &mut working,
                ContourGroundInput {
                    heights: terrain.heights,
                    origin_x: terrain.origin_x,
                    base_height: terrain.base_height,
                    origin_z: terrain.origin_z,
                    clip,
                },
            )?;
        }

        Ok(Some(Cow::Owned(working)))
    }
}
