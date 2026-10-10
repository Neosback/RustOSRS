//! Exact semantic identity for constructed ModelData across M4 -> M7.
//!
//! Raw `SourceModel` values have one real cache `ModelId`, but M4 may combine
//! multiple source models for an untyped type-10 object. A combined model has
//! no truthful singular `ModelId`. This module therefore carries the ordered,
//! non-empty source set as semantic identity and exposes a singular model id
//! only when exactly one real source contributed.

use crate::{
    definitions::DefinitionIdentity,
    ids::ModelId,
    model::{ModelFormatIdentity, SourceModel},
    model_construction::{AssembledModel, ModelSourceDescriptor},
    provenance::TargetProvenance,
};
use std::{error::Error, fmt};

/// Exact provenance for one raw source contributing to a constructed model.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ModelSemanticSource {
    identity: DefinitionIdentity<ModelId>,
    format: ModelFormatIdentity,
}

impl ModelSemanticSource {
    fn from_source(source: &SourceModel) -> Self {
        Self {
            identity: source.identity().clone(),
            format: source.format(),
        }
    }

    fn from_descriptor(source: &ModelSourceDescriptor) -> Self {
        Self {
            identity: source.identity().clone(),
            format: source.format(),
        }
    }

    pub fn identity(&self) -> &DefinitionIdentity<ModelId> {
        &self.identity
    }

    pub const fn format(&self) -> ModelFormatIdentity {
        self.format
    }
}

/// Exact source provenance for one constructed semantic model.
///
/// Source order is retained because M4 combination order is semantic. The set
/// is always non-empty and all sources must belong to the same target
/// provenance. Composite identities intentionally have no singular `ModelId`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ModelSemanticIdentity {
    sources: Box<[ModelSemanticSource]>,
}

impl ModelSemanticIdentity {
    /// Capture one validated raw source without a fallible reconstruction step.
    pub fn from_source(source: &SourceModel) -> Self {
        Self {
            sources: vec![ModelSemanticSource::from_source(source)].into_boxed_slice(),
        }
    }

    /// Admit an explicit ordered source set after enforcing identity invariants.
    pub fn try_from_sources(
        sources: Vec<ModelSemanticSource>,
    ) -> Result<Self, ModelSemanticIdentityError> {
        let Some(first) = sources.first() else {
            return Err(ModelSemanticIdentityError::EmptySources);
        };

        for other in &sources[1..] {
            if other.identity().provenance != first.identity().provenance {
                return Err(ModelSemanticIdentityError::TargetProvenanceMismatch {
                    first_model: first.identity().id,
                    other_model: other.identity().id,
                });
            }
        }

        Ok(Self {
            sources: sources.into_boxed_slice(),
        })
    }

    /// Capture the exact M4 provenance of an assembled model.
    pub fn from_assembled(model: &AssembledModel) -> Result<Self, ModelSemanticIdentityError> {
        Self::try_from_sources(
            model
                .sources()
                .iter()
                .map(ModelSemanticSource::from_descriptor)
                .collect(),
        )
    }

    pub fn sources(&self) -> &[ModelSemanticSource] {
        &self.sources
    }

    pub fn source_count(&self) -> usize {
        self.sources.len()
    }

    pub fn is_composite(&self) -> bool {
        self.sources.len() > 1
    }

    /// Return the one real source only when the identity is singular.
    pub fn singular_source(&self) -> Option<&ModelSemanticSource> {
        if self.sources.len() == 1 {
            self.sources.first()
        } else {
            None
        }
    }

    /// Return a real cache model id only for a one-source constructed model.
    ///
    /// Composite models deliberately return `None`; callers must not invent an
    /// object id, first-source id, or synthetic id to stand in for the assembly.
    pub fn singular_model_id(&self) -> Option<ModelId> {
        self.singular_source().map(|source| source.identity().id)
    }

    pub fn singular_definition_identity(&self) -> Option<&DefinitionIdentity<ModelId>> {
        self.singular_source().map(ModelSemanticSource::identity)
    }

    pub fn singular_format(&self) -> Option<ModelFormatIdentity> {
        self.singular_source().map(ModelSemanticSource::format)
    }

    pub fn target_provenance(&self) -> &TargetProvenance {
        &self.sources[0].identity().provenance
    }
}

/// Invalid source set supplied as constructed-model semantic identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelSemanticIdentityError {
    EmptySources,
    TargetProvenanceMismatch {
        first_model: ModelId,
        other_model: ModelId,
    },
}

impl fmt::Display for ModelSemanticIdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptySources => formatter.write_str("constructed model identity cannot be empty"),
            Self::TargetProvenanceMismatch {
                first_model,
                other_model,
            } => write!(
                formatter,
                "constructed model identity cannot combine model {} with model {} from a different target provenance",
                first_model.get(),
                other_model.get()
            ),
        }
    }
}

impl Error for ModelSemanticIdentityError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        coords::ModelPoint,
        definitions::{
            LocType, ModelScale, ModelTranslation, ObjectDefinition, ObjectPlacementFlags,
        },
        ids::ObjectId,
        model::{FacePriority, ModelEncoding, SourceModelParts, Triangle},
        model_construction::{apply_object_model_instance_transforms, combine_source_models},
        provenance::{CacheFingerprint, ProfileDigest},
    };

    const PROFILE_DIGEST: &str = "cfdefa9ef99eff799fcef4fdf0ec78d9fdcd72d8e5be78e1c154d018ab4575b7";
    const CACHE_FINGERPRINT: &str =
        "ae76dad78b4990d1b404e68e77a85ed2c96cf4a56c16f7b017cb97d1e92fdb38";

    #[test]
    fn direct_single_source_identity_exposes_only_its_real_model_id()
    -> Result<(), Box<dyn Error>> {
        let source = source_model(100, "target-a")?;
        let identity = ModelSemanticIdentity::from_source(&source);

        assert_eq!(identity.source_count(), 1);
        assert!(!identity.is_composite());
        assert_eq!(identity.singular_model_id(), Some(ModelId::new(100)));
        assert_eq!(
            identity.singular_definition_identity(),
            Some(source.identity())
        );
        assert_eq!(identity.singular_format(), Some(source.format()));
        Ok(())
    }

    #[test]
    fn combined_identity_retains_order_and_refuses_fake_singular_id() -> Result<(), Box<dyn Error>>
    {
        let first = source_model(100, "target-a")?;
        let second = source_model(200, "target-a")?;
        let assembled = combine_source_models(&[&first, &second])?;
        let identity = ModelSemanticIdentity::from_assembled(&assembled)?;

        assert_eq!(identity.source_count(), 2);
        assert!(identity.is_composite());
        assert_eq!(identity.singular_model_id(), None);
        assert_eq!(identity.singular_definition_identity(), None);
        assert_eq!(identity.singular_format(), None);
        assert_eq!(identity.sources()[0].identity(), first.identity());
        assert_eq!(identity.sources()[1].identity(), second.identity());
        Ok(())
    }

    #[test]
    fn instance_transforms_do_not_change_constructed_identity() -> Result<(), Box<dyn Error>> {
        let first = source_model(300, "target-a")?;
        let second = source_model(400, "target-a")?;
        let mut assembled = combine_source_models(&[&first, &second])?;
        let before = ModelSemanticIdentity::from_assembled(&assembled)?;
        let definition = object_definition()?;

        apply_object_model_instance_transforms(&mut assembled, &definition, LocType::new(4), 5);

        let after = ModelSemanticIdentity::from_assembled(&assembled)?;
        assert_eq!(after, before);
        Ok(())
    }

    #[test]
    fn explicit_identity_rejects_empty_and_cross_target_sources() -> Result<(), Box<dyn Error>> {
        assert_eq!(
            ModelSemanticIdentity::try_from_sources(Vec::new()),
            Err(ModelSemanticIdentityError::EmptySources)
        );

        let first = source_model(500, "target-a")?;
        let other = source_model(600, "target-b")?;
        let error = ModelSemanticIdentity::try_from_sources(vec![
            ModelSemanticSource::from_source(&first),
            ModelSemanticSource::from_source(&other),
        ]);
        assert_eq!(
            error,
            Err(ModelSemanticIdentityError::TargetProvenanceMismatch {
                first_model: ModelId::new(500),
                other_model: ModelId::new(600),
            })
        );
        Ok(())
    }

    fn source_model(id: u32, target: &str) -> Result<SourceModel, Box<dyn Error>> {
        Ok(SourceModel::from_parts(SourceModelParts {
            identity: DefinitionIdentity::new(ModelId::new(id), provenance(target)?),
            format: ModelFormatIdentity {
                encoding: ModelEncoding::TrailerFfFd,
                version: None,
            },
            vertices: vec![
                ModelPoint::new(0, 0, 0),
                ModelPoint::new(128, 0, 0),
                ModelPoint::new(0, 0, 128),
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

    fn object_definition() -> Result<ObjectDefinition, Box<dyn Error>> {
        Ok(ObjectDefinition {
            identity: DefinitionIdentity::new(ObjectId::new(1), provenance("target-a")?),
            name: Some("identity transform fixture".to_owned()),
            models: None,
            size_x: 1,
            size_y: 1,
            placement: ObjectPlacementFlags {
                interact_type: 2,
                blocks_projectiles: true,
                clipped: true,
                model_clipped: false,
                obstructs_ground: false,
                solid: false,
            },
            decoration_displacement: 16,
            support_items: None,
            is_rotated: false,
            non_flat_shading: false,
            contour_clip: None,
            animation: None,
            ambient: 0,
            contrast: 0,
            scale: ModelScale {
                x: 130,
                y: 126,
                z: 129,
            },
            translation: ModelTranslation { x: 3, y: -2, z: 5 },
            recolors: Vec::new(),
            retextures: Vec::new(),
            morphs: None,
            map_scene: None,
            map_icon: None,
            category: None,
            actions: [None, None, None, None, None],
        })
    }

    fn provenance(target: &str) -> Result<TargetProvenance, Box<dyn Error>> {
        Ok(TargetProvenance::new(
            target,
            ProfileDigest::from_lower_hex(PROFILE_DIGEST)?,
            CacheFingerprint::from_lower_hex(CACHE_FINGERPRINT)?,
            1,
        )?)
    }
}