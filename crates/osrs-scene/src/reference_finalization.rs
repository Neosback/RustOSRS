//! Initial non-flat scene finalization for M7.
//!
//! `ObjectComposition.getEntity(...)` may return scene-local ModelData for
//! `nonFlatShading` objects. `normal_finalization` owns the exact neighbor merge
//! traversal. This module joins those two already-audited stages and performs
//! final reference lighting only after the scene ModelData has been closed by
//! reconciliation.
//!
//! It deliberately does not own M4 model acquisition/combination, contouring,
//! pending/live replacement, or renderer extraction.

use crate::{
    normal_finalization::{SceneModelDataGrid, SceneModelDataId, SceneNormalError, SceneNormalMergeReport},
    placement::Footprint,
};
use osrs_core::{
    coords::{SceneTile, StoragePlane},
    lighting::{LightingError, LightingParameters, ReferenceLitModel, light_model_data},
    static_entity::SceneLocalModelDataEntity,
};
use std::{collections::BTreeMap, error::Error, fmt};

/// Failure while joining scene normal reconciliation to final reference lighting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SceneReferenceFinalizationError {
    Normal(SceneNormalError),
    UnknownModel(usize),
    ModelStillPending(usize),
    Lighting { model: usize, source: LightingError },
}

impl fmt::Display for SceneReferenceFinalizationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Normal(error) => error.fmt(formatter),
            Self::UnknownModel(model) => {
                write!(formatter, "unknown scene ModelData id {model} during final lighting")
            }
            Self::ModelStillPending(model) => write!(
                formatter,
                "scene ModelData id {model} is still pending normal reconciliation"
            ),
            Self::Lighting { model, source } => {
                write!(formatter, "final reference lighting failed for scene ModelData id {model}: {source}")
            }
        }
    }
}

impl Error for SceneReferenceFinalizationError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Normal(error) => Some(error),
            Self::Lighting { source, .. } => Some(source),
            Self::UnknownModel(_) | Self::ModelStillPending(_) => None,
        }
    }
}

impl From<SceneNormalError> for SceneReferenceFinalizationError {
    fn from(value: SceneNormalError) -> Self {
        Self::Normal(value)
    }
}

/// Scene-owned bridge from initial `nonFlatShading` ModelData to final lit models.
///
/// Only the non-flat `SceneLocalModelDataEntity` type can enter this bridge, so
/// already-lit initial entities cannot accidentally be routed through scene
/// normal reconciliation a second time.
#[derive(Debug)]
pub struct SceneReferenceFinalizer {
    model_data: SceneModelDataGrid,
    lighting: BTreeMap<SceneModelDataId, LightingParameters>,
    lit_models: BTreeMap<SceneModelDataId, ReferenceLitModel>,
}

impl SceneReferenceFinalizer {
    pub fn new(width: u32, height: u32, plane_count: u8) -> Result<Self, SceneNormalError> {
        Ok(Self {
            model_data: SceneModelDataGrid::new(width, height, plane_count)?,
            lighting: BTreeMap::new(),
            lit_models: BTreeMap::new(),
        })
    }

    pub const fn width(&self) -> u32 {
        self.model_data.width()
    }

    pub const fn height(&self) -> u32 {
        self.model_data.height()
    }

    pub const fn plane_count(&self) -> u8 {
        self.model_data.plane_count()
    }

    /// Admit one scene-local initial non-flat entity and retain its exact loc
    /// lighting parameters until normal reconciliation is complete.
    pub fn add_initial_model_data(&mut self, entity: SceneLocalModelDataEntity) -> SceneModelDataId {
        let (model, lighting) = entity.into_parts();
        let id = self.model_data.add_model(model);
        self.lighting.insert(id, lighting);
        self.lit_models.clear();
        id
    }

    pub fn model_data(&self) -> &SceneModelDataGrid {
        &self.model_data
    }

    pub fn lit_model(&self, id: SceneModelDataId) -> Option<&ReferenceLitModel> {
        self.lit_models.get(&id)
    }

    pub fn set_height_corner(
        &mut self,
        plane: StoragePlane,
        x: u32,
        y: u32,
        height: i32,
    ) -> Result<(), SceneNormalError> {
        self.lit_models.clear();
        self.model_data.set_height_corner(plane, x, y, height)
    }

    pub fn set_boundary(
        &mut self,
        plane: StoragePlane,
        tile: SceneTile,
        primary: SceneModelDataId,
        secondary: Option<SceneModelDataId>,
    ) -> Result<(), SceneNormalError> {
        self.lit_models.clear();
        self.model_data
            .set_boundary(plane, tile, primary, secondary)
    }

    pub fn set_floor_decoration(
        &mut self,
        plane: StoragePlane,
        tile: SceneTile,
        model: SceneModelDataId,
    ) -> Result<(), SceneNormalError> {
        self.lit_models.clear();
        self.model_data.set_floor_decoration(plane, tile, model)
    }

    pub fn insert_game_object(
        &mut self,
        plane: StoragePlane,
        start: SceneTile,
        footprint: Footprint,
        model: SceneModelDataId,
    ) -> Result<(), SceneNormalError> {
        self.lit_models.clear();
        self.model_data
            .insert_game_object(plane, start, footprint, model)
    }

    /// Execute exact scene normal reconciliation, then convert every registered
    /// initial non-flat ModelData to its final reference-lit semantic model.
    ///
    /// Lighting is atomic with respect to the exposed lit-model map: an error
    /// leaves no partially refreshed result set.
    pub fn reconcile_and_light(
        &mut self,
    ) -> Result<SceneNormalMergeReport, SceneReferenceFinalizationError> {
        self.lit_models.clear();
        let report = self.model_data.reconcile_normals()?;

        for id in self.lighting.keys().copied() {
            let pending = self
                .model_data
                .is_pending_model_data(id)
                .ok_or(SceneReferenceFinalizationError::UnknownModel(id.index()))?;
            if pending {
                return Err(SceneReferenceFinalizationError::ModelStillPending(id.index()));
            }
        }

        let mut finalized = BTreeMap::new();
        for (id, parameters) in &self.lighting {
            let model = self
                .model_data
                .model(*id)
                .ok_or(SceneReferenceFinalizationError::UnknownModel(id.index()))?;
            let lit = light_model_data(model, *parameters).map_err(|source| {
                SceneReferenceFinalizationError::Lighting {
                    model: id.index(),
                    source,
                }
            })?;
            finalized.insert(*id, lit);
        }

        self.lit_models = finalized;
        Ok(report)
    }
}
