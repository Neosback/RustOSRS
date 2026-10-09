//! Verified model-index repository and reusable immutable raw-model cache.
//!
//! This layer binds model index `7` transport to the target-aware ModelData
//! decoder. Cached entries are shared immutable `SourceModel` values; instance
//! mutation starts only after callers request an owned `WorkingModel` copy.

use crate::decode::{DecodeError, decode_model_data};
use crate::transport::{CacheError, CacheRepository};
use osrs_core::ids::ModelId;
use osrs_core::model::SourceModel;
use osrs_core::provenance::TargetProvenance;
use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

pub const MODEL_INDEX_ID: u8 = 7;

/// Reusable raw-model representation before instance transforms are applied.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RawModelVariant {
    Unmirrored,
    Mirrored,
}

/// Exact reusable raw-model cache identity.
///
/// The target provenance includes profile digest, cache fingerprint, and decoder
/// schema version. `build` is also explicit so revision identity cannot be lost
/// even if a future profile naming convention changes.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RawModelCacheKey {
    build: u32,
    target: TargetProvenance,
    model_id: ModelId,
    variant: RawModelVariant,
}

impl RawModelCacheKey {
    pub fn new(
        build: u32,
        target: TargetProvenance,
        model_id: ModelId,
        variant: RawModelVariant,
    ) -> Self {
        Self {
            build,
            target,
            model_id,
            variant,
        }
    }

    pub const fn build(&self) -> u32 {
        self.build
    }

    pub fn target(&self) -> &TargetProvenance {
        &self.target
    }

    pub const fn model_id(&self) -> ModelId {
        self.model_id
    }

    pub const fn variant(&self) -> RawModelVariant {
        self.variant
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelRepositoryError {
    Cache(CacheError),
    Decode(DecodeError),
    UnexpectedModelFileCount {
        model_id: ModelId,
        count: usize,
    },
    DerivedVariantMustBeMirrored {
        model_id: ModelId,
    },
    ModelIdentityMismatch {
        expected: ModelId,
        actual: ModelId,
    },
    TargetIdentityMismatch {
        model_id: ModelId,
    },
    VariantConflict {
        model_id: ModelId,
        variant: RawModelVariant,
    },
}

impl fmt::Display for ModelRepositoryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cache(error) => error.fmt(formatter),
            Self::Decode(error) => error.fmt(formatter),
            Self::UnexpectedModelFileCount { model_id, count } => write!(
                formatter,
                "model {} in cache index {MODEL_INDEX_ID} contains {count} logical files; expected exactly one",
                model_id.get()
            ),
            Self::DerivedVariantMustBeMirrored { model_id } => write!(
                formatter,
                "model {} attempted external admission of the authoritative unmirrored variant",
                model_id.get()
            ),
            Self::ModelIdentityMismatch { expected, actual } => write!(
                formatter,
                "raw model cache identity mismatch: key model {}, source model {}",
                expected.get(),
                actual.get()
            ),
            Self::TargetIdentityMismatch { model_id } => write!(
                formatter,
                "raw model {} target provenance does not match its cache key",
                model_id.get()
            ),
            Self::VariantConflict { model_id, variant } => write!(
                formatter,
                "raw model {} variant {variant:?} was admitted twice with different semantic data",
                model_id.get()
            ),
        }
    }
}

impl std::error::Error for ModelRepositoryError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Cache(error) => Some(error),
            Self::Decode(error) => Some(error),
            _ => None,
        }
    }
}

impl From<CacheError> for ModelRepositoryError {
    fn from(value: CacheError) -> Self {
        Self::Cache(value)
    }
}

impl From<DecodeError> for ModelRepositoryError {
    fn from(value: DecodeError) -> Self {
        Self::Decode(value)
    }
}

#[derive(Default)]
struct RawModelCache {
    entries: HashMap<RawModelCacheKey, Arc<SourceModel>>,
}

impl RawModelCache {
    fn get(&self, key: &RawModelCacheKey) -> Option<Arc<SourceModel>> {
        self.entries.get(key).cloned()
    }

    fn admit(
        &mut self,
        key: RawModelCacheKey,
        model: SourceModel,
    ) -> Result<Arc<SourceModel>, ModelRepositoryError> {
        validate_model_identity(&key, &model)?;
        if let Some(existing) = self.entries.get(&key) {
            if existing.as_ref() == &model {
                return Ok(Arc::clone(existing));
            }
            return Err(ModelRepositoryError::VariantConflict {
                model_id: key.model_id,
                variant: key.variant,
            });
        }

        let shared = Arc::new(model);
        self.entries.insert(key, Arc::clone(&shared));
        Ok(shared)
    }

    fn len(&self) -> usize {
        self.entries.len()
    }
}

fn validate_model_identity(
    key: &RawModelCacheKey,
    model: &SourceModel,
) -> Result<(), ModelRepositoryError> {
    if model.identity().id != key.model_id {
        return Err(ModelRepositoryError::ModelIdentityMismatch {
            expected: key.model_id,
            actual: model.identity().id,
        });
    }
    if model.identity().provenance != key.target {
        return Err(ModelRepositoryError::TargetIdentityMismatch {
            model_id: key.model_id,
        });
    }
    Ok(())
}

/// Target-verified source model repository.
///
/// Construction consumes an already-open `CacheRepository`, so cache/profile
/// fingerprint validation occurs exactly once at the transport boundary.
pub struct ModelSourceRepository {
    repository: CacheRepository,
    raw_models: RawModelCache,
}

impl ModelSourceRepository {
    pub fn new(repository: CacheRepository) -> Self {
        Self {
            repository,
            raw_models: RawModelCache::default(),
        }
    }

    pub fn cache_repository(&self) -> &CacheRepository {
        &self.repository
    }

    pub fn into_cache_repository(self) -> CacheRepository {
        self.repository
    }

    pub fn key(&self, model_id: ModelId, variant: RawModelVariant) -> RawModelCacheKey {
        RawModelCacheKey::new(
            self.repository.context().build(),
            self.repository.context().target_provenance().clone(),
            model_id,
            variant,
        )
    }

    pub fn cached(&self, model_id: ModelId, variant: RawModelVariant) -> Option<Arc<SourceModel>> {
        self.raw_models.get(&self.key(model_id, variant))
    }

    /// Load and decode the authoritative unmirrored raw model from cache index 7.
    /// Repeated requests return the same shared immutable cache entry.
    pub fn load_unmirrored(
        &mut self,
        model_id: ModelId,
    ) -> Result<Arc<SourceModel>, ModelRepositoryError> {
        let key = self.key(model_id, RawModelVariant::Unmirrored);
        if let Some(model) = self.raw_models.get(&key) {
            return Ok(model);
        }

        let metadata = self
            .repository
            .group_metadata(MODEL_INDEX_ID, model_id.get())?;
        if metadata.file_ids.len() != 1 {
            return Err(ModelRepositoryError::UnexpectedModelFileCount {
                model_id,
                count: metadata.file_ids.len(),
            });
        }
        let file_id = metadata.file_ids[0];
        let file = self
            .repository
            .read_file(MODEL_INDEX_ID, model_id.get(), file_id, None)?;
        let model = decode_model_data(
            &file.bytes,
            self.repository.context(),
            &file.provenance,
            model_id,
        )?;
        self.raw_models.admit(key, model)
    }

    /// Admit a separately constructed mirrored raw variant.
    ///
    /// M4 Checkpoint 3 establishes variant ownership and key separation only;
    /// Checkpoint 4 owns the actual audited mirror geometry/winding operation.
    pub fn cache_derived_variant(
        &mut self,
        model_id: ModelId,
        variant: RawModelVariant,
        model: SourceModel,
    ) -> Result<Arc<SourceModel>, ModelRepositoryError> {
        if variant == RawModelVariant::Unmirrored {
            return Err(ModelRepositoryError::DerivedVariantMustBeMirrored { model_id });
        }
        let key = self.key(model_id, variant);
        self.raw_models.admit(key, model)
    }

    pub fn cached_variant_count(&self) -> usize {
        self.raw_models.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decode::test_support;
    use osrs_core::coords::ModelPoint;
    use osrs_core::definitions::DefinitionIdentity;
    use osrs_core::model::{
        FacePriority, ModelEncoding, ModelFormatIdentity, SourceModelParts, Triangle,
    };

    fn source_model(
        model_id: ModelId,
        target: TargetProvenance,
    ) -> Result<SourceModel, Box<dyn std::error::Error>> {
        Ok(SourceModel::from_parts(SourceModelParts {
            identity: DefinitionIdentity::new(model_id, target),
            format: ModelFormatIdentity {
                encoding: ModelEncoding::TrailerFfFd,
                version: None,
            },
            vertices: vec![
                ModelPoint::new(0, 0, 0),
                ModelPoint::new(128, 0, 0),
                ModelPoint::new(0, 128, 0),
            ],
            faces: vec![Triangle::new(0, 1, 2)],
            face_colors: vec![500],
            default_priority: FacePriority::ZERO,
            face_render_types: None,
            face_priorities: None,
            face_alphas: None,
            face_textures: None,
            texture_face_selectors: None,
            face_biases: None,
            texture_triangles: Vec::new(),
            vertex_skins: None,
            face_skins: None,
            skeletal_vertices: None,
        })?)
    }

    #[test]
    fn raw_cache_key_separates_revision_and_mirror_variant()
    -> Result<(), Box<dyn std::error::Error>> {
        let context = test_support::target_context()?;
        let target = context.target_provenance().clone();
        let model_id = ModelId::new(42);
        let base = RawModelCacheKey::new(
            context.build(),
            target.clone(),
            model_id,
            RawModelVariant::Unmirrored,
        );
        let mirrored = RawModelCacheKey::new(
            context.build(),
            target.clone(),
            model_id,
            RawModelVariant::Mirrored,
        );
        let different_build = RawModelCacheKey::new(
            context.build() + 1,
            target,
            model_id,
            RawModelVariant::Unmirrored,
        );

        assert_ne!(base, mirrored);
        assert_ne!(base, different_build);
        assert_eq!(base.model_id(), model_id);
        assert_eq!(base.variant(), RawModelVariant::Unmirrored);
        Ok(())
    }

    #[test]
    fn raw_cache_reuses_shared_source_and_keeps_variants_distinct()
    -> Result<(), Box<dyn std::error::Error>> {
        let context = test_support::target_context()?;
        let target = context.target_provenance().clone();
        let model_id = ModelId::new(77);
        let base_key = RawModelCacheKey::new(
            context.build(),
            target.clone(),
            model_id,
            RawModelVariant::Unmirrored,
        );
        let mirrored_key = RawModelCacheKey::new(
            context.build(),
            target.clone(),
            model_id,
            RawModelVariant::Mirrored,
        );
        let mut cache = RawModelCache::default();

        let base = cache.admit(base_key.clone(), source_model(model_id, target.clone())?)?;
        let repeat = cache.get(&base_key).ok_or("missing base cache entry")?;
        assert!(Arc::ptr_eq(&base, &repeat));
        assert!(cache.get(&mirrored_key).is_none());

        let mirrored = cache.admit(mirrored_key, source_model(model_id, target)?)?;
        assert!(!Arc::ptr_eq(&base, &mirrored));
        assert_eq!(cache.len(), 2);
        Ok(())
    }

    #[test]
    fn cache_rejects_model_and_target_identity_mismatch() -> Result<(), Box<dyn std::error::Error>>
    {
        let context = test_support::target_context()?;
        let target = context.target_provenance().clone();
        let key = RawModelCacheKey::new(
            context.build(),
            target.clone(),
            ModelId::new(5),
            RawModelVariant::Mirrored,
        );
        let mut cache = RawModelCache::default();

        let wrong_model = source_model(ModelId::new(6), target.clone())?;
        assert!(matches!(
            cache.admit(key.clone(), wrong_model),
            Err(ModelRepositoryError::ModelIdentityMismatch { .. })
        ));

        let mut other_target = target;
        other_target = TargetProvenance::new(
            "different-profile",
            other_target.profile_digest(),
            other_target.cache_fingerprint(),
            other_target.decoder_schema_version(),
        )?;
        let wrong_target = source_model(ModelId::new(5), other_target)?;
        assert!(matches!(
            cache.admit(key, wrong_target),
            Err(ModelRepositoryError::TargetIdentityMismatch { .. })
        ));
        Ok(())
    }

    #[test]
    fn working_copy_mutation_cannot_change_cached_source() -> Result<(), Box<dyn std::error::Error>>
    {
        let context = test_support::target_context()?;
        let target = context.target_provenance().clone();
        let model_id = ModelId::new(99);
        let key = RawModelCacheKey::new(
            context.build(),
            target.clone(),
            model_id,
            RawModelVariant::Unmirrored,
        );
        let mut cache = RawModelCache::default();
        let source = cache.admit(key, source_model(model_id, target)?)?;
        let before = source.vertices().to_vec();

        let mut working = source.to_working_copy();
        working.vertices_mut()[0] = ModelPoint::new(999, 888, 777);

        assert_eq!(source.vertices(), before.as_slice());
        assert_ne!(working.vertices(), source.vertices());
        Ok(())
    }
}
