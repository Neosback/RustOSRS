//! Cache-backed object model resolution for exact M4 construction semantics.
//!
//! Pure selection/mirror/combine rules live in `osrs-core`. This adapter only
//! acquires the selected raw variants from the verified model repository and
//! returns an owned assembled model for the later instance transform pipeline.

use crate::model_repository::{ModelRepositoryError, ModelSourceRepository, RawModelVariant};
use osrs_core::definitions::{LocType, ObjectDefinition};
use osrs_core::ids::ModelId;
use osrs_core::model::SourceModel;
use osrs_core::model_construction::{
    AssembledModel, ModelConstructionError, combine_source_models, mirror_source_model,
    select_object_model,
};
use std::fmt;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObjectModelResolveError {
    Repository(ModelRepositoryError),
    Construction(ModelConstructionError),
}

impl fmt::Display for ObjectModelResolveError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Repository(error) => error.fmt(formatter),
            Self::Construction(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for ObjectModelResolveError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Repository(error) => Some(error),
            Self::Construction(error) => Some(error),
        }
    }
}

impl From<ModelRepositoryError> for ObjectModelResolveError {
    fn from(value: ModelRepositoryError) -> Self {
        Self::Repository(value)
    }
}

impl From<ModelConstructionError> for ObjectModelResolveError {
    fn from(value: ModelConstructionError) -> Self {
        Self::Construction(value)
    }
}

/// Resolve one object request into an owned pre-transform semantic model.
///
/// `Ok(None)` is an exact semantic outcome for a missing typed match, an
/// untyped non-type-10 request, or a definition with no model IDs. No fallback
/// geometry is substituted.
pub fn resolve_object_model(
    repository: &mut ModelSourceRepository,
    definition: &ObjectDefinition,
    requested_type: LocType,
    orientation: u8,
) -> Result<Option<AssembledModel>, ObjectModelResolveError> {
    let Some(selection) = select_object_model(definition, requested_type, orientation) else {
        return Ok(None);
    };

    let mut sources = Vec::<Arc<SourceModel>>::with_capacity(selection.model_ids().len());
    for model_id in selection.model_ids().iter().copied() {
        sources.push(load_raw_variant(repository, model_id, selection.mirror())?);
    }

    let refs: Vec<&SourceModel> = sources.iter().map(Arc::as_ref).collect();
    Ok(Some(combine_source_models(&refs)?))
}

fn load_raw_variant(
    repository: &mut ModelSourceRepository,
    model_id: ModelId,
    mirrored: bool,
) -> Result<Arc<SourceModel>, ObjectModelResolveError> {
    if !mirrored {
        return Ok(repository.load_unmirrored(model_id)?);
    }

    if let Some(model) = repository.cached(model_id, RawModelVariant::Mirrored) {
        return Ok(model);
    }

    let source = repository.load_unmirrored(model_id)?;
    let mirrored = mirror_source_model(source.as_ref())?;
    Ok(repository.cache_derived_variant(model_id, RawModelVariant::Mirrored, mirrored)?)
}
