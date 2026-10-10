//! Initial static object entity lifecycle for M7.
//!
//! This module mirrors the pinned `ObjectComposition.getEntity(...)` branch
//! after exact object ModelData construction has produced one mutable semantic
//! working model. It deliberately does not own cache transport, contour-ground
//! execution, pending/live replacement, or the M4 multi-source acquisition
//! bridge.
//!
//! The critical ownership distinction is preserved:
//! - flat-shaded initial entities are lit once and cached as final reference
//!   models;
//! - `nonFlatShading` entities retain pre-lighting ModelData state, exact loc
//!   lighting parameters, and calculated base normals in the reusable entity
//!   cache, while each scene retrieval receives a fresh mutable copy.

use crate::{
    definitions::{LocType, ObjectDefinition, ObjectModels},
    lighting::{LightingError, LightingParameters, ReferenceLitModel, light_model_data},
    model::{ModelNormalState, WorkingModel},
    normals::calculate_base_normals,
};
use std::collections::HashMap;

/// Exact Java `ObjectDefinition_cachedEntities` key used by initial placement.
///
/// The reference computes the expression in signed 32-bit integer space before
/// widening it to `long`; `raw` therefore retains that signed result exactly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct InitialStaticEntityKey {
    raw: i64,
}

impl InitialStaticEntityKey {
    pub const fn raw(self) -> i64 {
        self.raw
    }

    /// Build the audited key for `ObjectComposition.getEntity(type, orientation, ...)`.
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

/// Scene-local pre-lighting ModelData result for a `nonFlatShading` entity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SceneLocalModelDataEntity {
    model: Box<WorkingModel>,
    lighting: LightingParameters,
}

impl SceneLocalModelDataEntity {
    pub fn model(&self) -> &WorkingModel {
        &self.model
    }

    pub fn model_mut(&mut self) -> &mut WorkingModel {
        &mut self.model
    }

    pub const fn lighting(&self) -> LightingParameters {
        self.lighting
    }

    pub fn into_parts(self) -> (WorkingModel, LightingParameters) {
        (*self.model, self.lighting)
    }
}

/// Semantic result returned to initial static scene placement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InitialStaticEntity {
    /// `nonFlatShading == false`: already converted with exact loc lighting.
    Lit(ReferenceLitModel),
    /// `nonFlatShading == true`: mutable scene-local ModelData copy awaiting
    /// scene normal reconciliation and final lighting.
    ModelData(SceneLocalModelDataEntity),
}

impl InitialStaticEntity {
    pub const fn is_lit(&self) -> bool {
        matches!(self, Self::Lit(_))
    }

    pub const fn is_model_data(&self) -> bool {
        matches!(self, Self::ModelData(_))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum CachedInitialStaticEntity {
    Lit(ReferenceLitModel),
    ModelData {
        model: Box<WorkingModel>,
        lighting: LightingParameters,
    },
}

impl CachedInitialStaticEntity {
    fn instantiate(&self) -> InitialStaticEntity {
        match self {
            Self::Lit(model) => InitialStaticEntity::Lit(model.clone()),
            Self::ModelData { model, lighting } => {
                InitialStaticEntity::ModelData(SceneLocalModelDataEntity {
                    model: Box::new((**model).clone()),
                    lighting: *lighting,
                })
            }
        }
    }
}

/// Reusable semantic cache for the initial static `getEntity` path.
///
/// The supplied builder is invoked only on a cache miss. It must return the
/// already-selected/mirrored/combined/transformed ModelData working copy for the
/// requested object/type/orientation. Returning `None` preserves semantic model
/// absence and does not create a cache entry.
#[derive(Debug, Default)]
pub struct InitialStaticEntityCache {
    entries: HashMap<InitialStaticEntityKey, CachedInitialStaticEntity>,
}

impl InitialStaticEntityCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn contains_key(&self, key: InitialStaticEntityKey) -> bool {
        self.entries.contains_key(&key)
    }

    /// Resolve one initial static entity with exact flat/non-flat cache ownership.
    pub fn get_or_build<F>(
        &mut self,
        definition: &ObjectDefinition,
        requested_type: LocType,
        orientation: u8,
        build_model_data: F,
    ) -> Result<Option<InitialStaticEntity>, LightingError>
    where
        F: FnOnce() -> Option<WorkingModel>,
    {
        let key = InitialStaticEntityKey::for_object(definition, requested_type, orientation);
        if let Some(cached) = self.entries.get(&key) {
            return Ok(Some(cached.instantiate()));
        }

        let Some(mut model) = build_model_data() else {
            return Ok(None);
        };
        let lighting = LightingParameters::for_loc(definition.ambient, definition.contrast);

        let cached = if definition.non_flat_shading {
            // The pinned getEntity path calculates base normals before caching
            // ModelData, then copies that cached semantic state per scene use.
            if matches!(model.normal_state(), ModelNormalState::Uncomputed) {
                let normals = calculate_base_normals(&model);
                model.set_computed_normals(normals);
            }
            CachedInitialStaticEntity::ModelData {
                model: Box::new(model),
                lighting,
            }
        } else {
            CachedInitialStaticEntity::Lit(light_model_data(&model, lighting)?)
        };

        let entity = cached.instantiate();
        self.entries.insert(key, cached);
        Ok(Some(entity))
    }
}
