//! Exact pre-instance object model selection, mirroring, and combination.
//!
//! This module owns the cache-independent `MODEL-BUILD-001/002` rules. It does
//! not load cache bytes and it does not apply the later instance transform
//! pipeline owned by M4 Checkpoint 5.

use crate::coords::ModelPoint;
use crate::definitions::{DefinitionIdentity, LocType, ObjectDefinition, ObjectModels};
use crate::ids::{ModelId, TextureId};
use crate::model::{
    FacePriority, ModelFormatIdentity, ModelValidationError, SkeletalVertexData, SourceModel,
    SourceModelParts, TextureTriangle, TextureTriangleIndex, Triangle,
};
use std::collections::HashMap;
use std::fmt;

/// Audited raw-model selection before cache loading or instance transforms.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectModelSelection {
    model_ids: Vec<ModelId>,
    mirror: bool,
}

impl ObjectModelSelection {
    pub fn model_ids(&self) -> &[ModelId] {
        &self.model_ids
    }

    pub const fn mirror(&self) -> bool {
        self.mirror
    }
}

/// Select the exact model IDs and raw mirror variant for one object request.
///
/// Untyped opcode-5-style definitions are valid only for requested loc type 10.
/// Their audited branch uses `isRotated` directly: the source's extra
/// `type == 2 && orientation > 3` toggle is unreachable after the type-10 gate.
/// Typed opcode-1-style definitions use `isRotated XOR (orientation > 3)`.
pub fn select_object_model(
    definition: &ObjectDefinition,
    requested_type: LocType,
    orientation: u8,
) -> Option<ObjectModelSelection> {
    let models = definition.models.as_ref()?;
    match models {
        ObjectModels::Untyped(model_ids) => {
            if requested_type.get() != 10 || model_ids.is_empty() {
                return None;
            }
            Some(ObjectModelSelection {
                model_ids: model_ids.clone(),
                mirror: definition.is_rotated,
            })
        }
        ObjectModels::Typed(entries) => {
            let entry = entries
                .iter()
                .find(|entry| entry.loc_type == requested_type)?;
            Some(ObjectModelSelection {
                model_ids: vec![entry.model_id],
                mirror: definition.is_rotated ^ (orientation > 3),
            })
        }
    }
}

/// Exact source descriptor retained by a constructed/combined model.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ModelSourceDescriptor {
    identity: DefinitionIdentity<ModelId>,
    format: ModelFormatIdentity,
}

impl ModelSourceDescriptor {
    pub fn identity(&self) -> &DefinitionIdentity<ModelId> {
        &self.identity
    }

    pub const fn format(&self) -> ModelFormatIdentity {
        self.format
    }
}

/// Owned model assembled from one or more immutable raw source variants.
///
/// This is the object-construction result before Checkpoint 5's orientation,
/// recolor/retexture, resize, and translation operations. A combined model has
/// no fake singular `ModelId`; every contributing source identity is retained.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssembledModel {
    sources: Vec<ModelSourceDescriptor>,
    vertices: Vec<ModelPoint>,
    faces: Vec<Triangle>,
    face_colors: Vec<u16>,
    default_priority: FacePriority,
    face_render_types: Option<Vec<i8>>,
    face_priorities: Option<Vec<FacePriority>>,
    face_alphas: Option<Vec<i8>>,
    face_textures: Option<Vec<Option<TextureId>>>,
    texture_face_selectors: Option<Vec<Option<TextureTriangleIndex>>>,
    face_biases: Option<Vec<i8>>,
    texture_triangles: Vec<TextureTriangle>,
    vertex_skins: Option<Vec<i32>>,
    face_skins: Option<Vec<i32>>,
    skeletal_vertices: Option<Vec<Option<SkeletalVertexData>>>,
}

impl AssembledModel {
    /// Produce an owned construction model from one immutable raw source.
    pub fn from_source(source: &SourceModel) -> Self {
        Self {
            sources: vec![source_descriptor(source)],
            vertices: source.vertices().to_vec(),
            faces: source.faces().to_vec(),
            face_colors: source.face_colors().to_vec(),
            default_priority: source.default_priority(),
            face_render_types: source.face_render_types().map(ToOwned::to_owned),
            face_priorities: source.face_priorities().map(ToOwned::to_owned),
            face_alphas: source.face_alphas().map(ToOwned::to_owned),
            face_textures: source.face_textures().map(ToOwned::to_owned),
            texture_face_selectors: source.texture_face_selectors().map(ToOwned::to_owned),
            face_biases: source.face_biases().map(ToOwned::to_owned),
            texture_triangles: source.texture_triangles().to_vec(),
            vertex_skins: source.vertex_skins().map(ToOwned::to_owned),
            face_skins: source.face_skins().map(ToOwned::to_owned),
            skeletal_vertices: source.skeletal_vertices().map(ToOwned::to_owned),
        }
    }

    pub fn sources(&self) -> &[ModelSourceDescriptor] {
        &self.sources
    }

    pub fn vertices(&self) -> &[ModelPoint] {
        &self.vertices
    }

    pub fn faces(&self) -> &[Triangle] {
        &self.faces
    }

    pub fn face_colors(&self) -> &[u16] {
        &self.face_colors
    }

    pub const fn default_priority(&self) -> FacePriority {
        self.default_priority
    }

    pub fn face_render_types(&self) -> Option<&[i8]> {
        self.face_render_types.as_deref()
    }

    pub fn face_priorities(&self) -> Option<&[FacePriority]> {
        self.face_priorities.as_deref()
    }

    pub fn face_alphas(&self) -> Option<&[i8]> {
        self.face_alphas.as_deref()
    }

    pub fn face_textures(&self) -> Option<&[Option<TextureId>]> {
        self.face_textures.as_deref()
    }

    pub fn texture_face_selectors(&self) -> Option<&[Option<TextureTriangleIndex>]> {
        self.texture_face_selectors.as_deref()
    }

    pub fn face_biases(&self) -> Option<&[i8]> {
        self.face_biases.as_deref()
    }

    pub fn texture_triangles(&self) -> &[TextureTriangle] {
        &self.texture_triangles
    }

    pub fn vertex_skins(&self) -> Option<&[i32]> {
        self.vertex_skins.as_deref()
    }

    pub fn face_skins(&self) -> Option<&[i32]> {
        self.face_skins.as_deref()
    }

    pub fn skeletal_vertices(&self) -> Option<&[Option<SkeletalVertexData>]> {
        self.skeletal_vertices.as_deref()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelConstructionError {
    EmptyCombination,
    TargetProvenanceMismatch {
        first_model: ModelId,
        other_model: ModelId,
    },
    IndexOverflow {
        field: &'static str,
    },
    InvalidMirroredModel(ModelValidationError),
}

impl fmt::Display for ModelConstructionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyCombination => formatter.write_str("cannot combine zero source models"),
            Self::TargetProvenanceMismatch {
                first_model,
                other_model,
            } => write!(
                formatter,
                "cannot combine model {} with model {} from a different target provenance",
                first_model.get(),
                other_model.get()
            ),
            Self::IndexOverflow { field } => {
                write!(formatter, "combined model {field} exceeded u32 index space")
            }
            Self::InvalidMirroredModel(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for ModelConstructionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidMirroredModel(error) => Some(error),
            _ => None,
        }
    }
}

impl From<ModelValidationError> for ModelConstructionError {
    fn from(value: ModelValidationError) -> Self {
        Self::InvalidMirroredModel(value)
    }
}

/// Construct the audited mirrored raw variant of one immutable source model.
///
/// Reference `ModelData.method5280()` negates model Z and swaps triangle index
/// 1/3 (canonical A/C). Texture-triangle metadata is not rewound by that method.
pub fn mirror_source_model(source: &SourceModel) -> Result<SourceModel, ModelConstructionError> {
    let vertices = source
        .vertices()
        .iter()
        .map(|vertex| ModelPoint::new(vertex.x, vertex.y, vertex.z.wrapping_neg()))
        .collect();
    let faces = source
        .faces()
        .iter()
        .map(|face| Triangle {
            a: face.c,
            b: face.b,
            c: face.a,
        })
        .collect();

    Ok(SourceModel::from_parts(SourceModelParts {
        identity: source.identity().clone(),
        format: source.format(),
        vertices,
        faces,
        face_colors: source.face_colors().to_vec(),
        default_priority: source.default_priority(),
        face_render_types: source.face_render_types().map(ToOwned::to_owned),
        face_priorities: source.face_priorities().map(ToOwned::to_owned),
        face_alphas: source.face_alphas().map(ToOwned::to_owned),
        face_textures: source.face_textures().map(ToOwned::to_owned),
        texture_face_selectors: source.texture_face_selectors().map(ToOwned::to_owned),
        face_biases: source.face_biases().map(ToOwned::to_owned),
        texture_triangles: source.texture_triangles().to_vec(),
        vertex_skins: source.vertex_skins().map(ToOwned::to_owned),
        face_skins: source.face_skins().map(ToOwned::to_owned),
        skeletal_vertices: source.skeletal_vertices().map(ToOwned::to_owned),
    })?)
}

/// Combine multiple raw models with the audited ModelData constructor semantics.
///
/// Vertices are admitted lazily from faces and render-type-0 texture triangles,
/// deduplicated by exact XYZ equality in first-seen order. Optional face arrays
/// are materialized only when the reference constructor would materialize them,
/// with reference zero/default sentinels for source models that omit an array.
pub fn combine_source_models(models: &[&SourceModel]) -> Result<AssembledModel, ModelConstructionError> {
    let first = *models.first().ok_or(ModelConstructionError::EmptyCombination)?;
    for model in &models[1..] {
        if model.identity().provenance != first.identity().provenance {
            return Err(ModelConstructionError::TargetProvenanceMismatch {
                first_model: first.identity().id,
                other_model: model.identity().id,
            });
        }
    }

    if models.len() == 1 {
        return Ok(AssembledModel::from_source(first));
    }

    let has_render_types = models.iter().any(|model| model.face_render_types().is_some());
    let has_alphas = models.iter().any(|model| model.face_alphas().is_some());
    let has_face_skins = models.iter().any(|model| model.face_skins().is_some());
    let has_textures = models.iter().any(|model| model.face_textures().is_some());
    let has_selectors = models
        .iter()
        .any(|model| model.texture_face_selectors().is_some());
    let has_skeletal = models.iter().any(|model| model.skeletal_vertices().is_some());
    let has_biases = models.iter().any(|model| model.face_biases().is_some());

    let mut uniform_priority = None;
    let mut has_per_face_priorities = false;
    for model in models {
        if model.face_priorities().is_some() {
            has_per_face_priorities = true;
        } else if let Some(priority) = uniform_priority {
            if priority != model.default_priority() {
                has_per_face_priorities = true;
            }
        } else {
            uniform_priority = Some(model.default_priority());
        }
    }
    let default_priority = uniform_priority.unwrap_or_else(|| first.default_priority());

    let total_faces = models.iter().map(|model| model.faces().len()).sum();
    let total_textures = models
        .iter()
        .map(|model| model.texture_triangles().len())
        .sum();

    let mut assembled = AssembledModel {
        sources: models.iter().map(|model| source_descriptor(model)).collect(),
        vertices: Vec::new(),
        faces: Vec::with_capacity(total_faces),
        face_colors: Vec::with_capacity(total_faces),
        default_priority,
        face_render_types: has_render_types.then(|| Vec::with_capacity(total_faces)),
        face_priorities: has_per_face_priorities.then(|| Vec::with_capacity(total_faces)),
        face_alphas: has_alphas.then(|| Vec::with_capacity(total_faces)),
        face_textures: has_textures.then(|| Vec::with_capacity(total_faces)),
        texture_face_selectors: has_selectors.then(|| Vec::with_capacity(total_faces)),
        face_biases: has_biases.then(|| Vec::with_capacity(total_faces)),
        texture_triangles: Vec::with_capacity(total_textures),
        // The reference combination constructor always allocates vertexSkins;
        // missing source entries therefore remain Java's integer default zero.
        vertex_skins: Some(Vec::new()),
        face_skins: has_face_skins.then(|| Vec::with_capacity(total_faces)),
        skeletal_vertices: has_skeletal.then(Vec::new),
    };
    let mut vertex_lookup = HashMap::<ModelPoint, u32>::new();

    for model in models {
        let texture_base = u32::try_from(assembled.texture_triangles.len())
            .map_err(|_| ModelConstructionError::IndexOverflow {
                field: "texture triangle count",
            })?;

        for (face_index, face) in model.faces().iter().copied().enumerate() {
            let a = map_vertex(&mut assembled, &mut vertex_lookup, model, face.a.get())?;
            let b = map_vertex(&mut assembled, &mut vertex_lookup, model, face.b.get())?;
            let c = map_vertex(&mut assembled, &mut vertex_lookup, model, face.c.get())?;
            assembled.faces.push(Triangle::new(a, b, c));
            assembled.face_colors.push(model.face_colors()[face_index]);

            if let Some(output) = assembled.face_render_types.as_mut() {
                output.push(model.face_render_types().map_or(0, |values| values[face_index]));
            }
            if let Some(output) = assembled.face_priorities.as_mut() {
                output.push(
                    model
                        .face_priorities()
                        .map_or(model.default_priority(), |values| values[face_index]),
                );
            }
            if let Some(output) = assembled.face_alphas.as_mut() {
                output.push(model.face_alphas().map_or(0, |values| values[face_index]));
            }
            if let Some(output) = assembled.face_textures.as_mut() {
                output.push(model.face_textures().map_or(None, |values| values[face_index]));
            }
            if let Some(output) = assembled.texture_face_selectors.as_mut() {
                let selector = model
                    .texture_face_selectors()
                    .and_then(|values| values[face_index])
                    .map(|selector| {
                        selector
                            .get()
                            .checked_add(texture_base)
                            .map(TextureTriangleIndex::new)
                            .ok_or(ModelConstructionError::IndexOverflow {
                                field: "texture selector",
                            })
                    })
                    .transpose()?;
                output.push(selector);
            }
            if let Some(output) = assembled.face_biases.as_mut() {
                output.push(model.face_biases().map_or(0, |values| values[face_index]));
            }
            if let Some(output) = assembled.face_skins.as_mut() {
                output.push(model.face_skins().map_or(0, |values| values[face_index]));
            }
        }

        for triangle in model.texture_triangles() {
            let mut combined = *triangle;
            if triangle.render_type == 0 {
                combined.vertices = Triangle::new(
                    map_vertex(
                        &mut assembled,
                        &mut vertex_lookup,
                        model,
                        triangle.vertices.a.get(),
                    )?,
                    map_vertex(
                        &mut assembled,
                        &mut vertex_lookup,
                        model,
                        triangle.vertices.b.get(),
                    )?,
                    map_vertex(
                        &mut assembled,
                        &mut vertex_lookup,
                        model,
                        triangle.vertices.c.get(),
                    )?,
                );
            }
            assembled.texture_triangles.push(combined);
        }
    }

    Ok(assembled)
}

fn source_descriptor(source: &SourceModel) -> ModelSourceDescriptor {
    ModelSourceDescriptor {
        identity: source.identity().clone(),
        format: source.format(),
    }
}

fn map_vertex(
    assembled: &mut AssembledModel,
    lookup: &mut HashMap<ModelPoint, u32>,
    source: &SourceModel,
    source_index: u32,
) -> Result<u32, ModelConstructionError> {
    let point = source.vertices()[source_index as usize];
    if let Some(index) = lookup.get(&point) {
        return Ok(*index);
    }

    let index = u32::try_from(assembled.vertices.len()).map_err(|_| {
        ModelConstructionError::IndexOverflow {
            field: "vertex count",
        }
    })?;
    assembled.vertices.push(point);
    lookup.insert(point, index);

    assembled
        .vertex_skins
        .as_mut()
        .expect("combined model always owns vertex skins")
        .push(source.vertex_skins().map_or(0, |values| values[source_index as usize]));
    if let Some(output) = assembled.skeletal_vertices.as_mut() {
        output.push(
            source
                .skeletal_vertices()
                .map_or(None, |values| values[source_index as usize].clone()),
        );
    }

    Ok(index)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::definitions::{
        ModelScale, ModelTranslation, ObjectModels, ObjectPlacementFlags, TypedObjectModel,
    };
    use crate::ids::{ObjectId, TextureId};
    use crate::model::{
        FacePriority, ModelEncoding, ModelFormatIdentity, SkeletalVertexData, SourceModelParts,
        TextureMappingParameters, TextureTriangle,
    };
    use crate::provenance::{CacheFingerprint, ProfileDigest, TargetProvenance};

    const PROFILE_DIGEST: &str =
        "cfdefa9ef99eff799fcef4fdf0ec78d9fdcd72d8e5be78e1c154d018ab4575b7";
    const CACHE_FINGERPRINT: &str =
        "ae76dad78b4990d1b404e68e77a85ed2c96cf4a56c16f7b017cb97d1e92fdb38";

    fn provenance() -> Result<TargetProvenance, Box<dyn std::error::Error>> {
        Ok(TargetProvenance::new(
            "osrs-live-241-2026-09-30-openrs2-2727",
            ProfileDigest::from_lower_hex(PROFILE_DIGEST)?,
            CacheFingerprint::from_lower_hex(CACHE_FINGERPRINT)?,
            1,
        )?)
    }

    fn object(models: Option<ObjectModels>, is_rotated: bool) -> Result<ObjectDefinition, Box<dyn std::error::Error>> {
        Ok(ObjectDefinition {
            identity: DefinitionIdentity::new(ObjectId::new(1), provenance()?),
            name: Some("fixture".to_owned()),
            models,
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
            is_rotated,
            non_flat_shading: false,
            contour_clip: None,
            animation: None,
            ambient: 0,
            contrast: 0,
            scale: ModelScale::IDENTITY,
            translation: ModelTranslation::ZERO,
            recolors: Vec::new(),
            retextures: Vec::new(),
            morphs: None,
            map_scene: None,
            map_icon: None,
            category: None,
            actions: [None, None, None, None, None],
        })
    }

    fn source_model(
        id: u32,
        vertices: Vec<ModelPoint>,
        faces: Vec<Triangle>,
    ) -> Result<SourceModel, Box<dyn std::error::Error>> {
        let face_count = faces.len();
        Ok(SourceModel::from_parts(SourceModelParts {
            identity: DefinitionIdentity::new(ModelId::new(id), provenance()?),
            format: ModelFormatIdentity {
                encoding: ModelEncoding::TrailerFfFd,
                version: None,
            },
            vertices,
            faces,
            face_colors: vec![500; face_count],
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
    fn typed_selection_requires_exact_type_and_uses_xor_mirror_rule()
    -> Result<(), Box<dyn std::error::Error>> {
        let models = ObjectModels::Typed(vec![
            TypedObjectModel {
                loc_type: LocType::new(2),
                model_id: ModelId::new(100),
            },
            TypedObjectModel {
                loc_type: LocType::new(4),
                model_id: ModelId::new(200),
            },
        ]);

        for (is_rotated, orientation, expected_mirror) in [
            (false, 3, false),
            (false, 4, true),
            (true, 3, true),
            (true, 4, false),
        ] {
            let definition = object(Some(models.clone()), is_rotated)?;
            let selected = select_object_model(&definition, LocType::new(4), orientation)
                .ok_or("typed hit unexpectedly absent")?;
            assert_eq!(selected.model_ids(), &[ModelId::new(200)]);
            assert_eq!(selected.mirror(), expected_mirror);
        }

        let definition = object(Some(models), false)?;
        assert_eq!(select_object_model(&definition, LocType::new(10), 0), None);
        Ok(())
    }

    #[test]
    fn untyped_selection_is_type_10_only_and_keeps_special_mirror_behavior()
    -> Result<(), Box<dyn std::error::Error>> {
        let ids = vec![ModelId::new(10), ModelId::new(11), ModelId::new(12)];
        let normal = object(Some(ObjectModels::Untyped(ids.clone())), false)?;
        let rotated = object(Some(ObjectModels::Untyped(ids.clone())), true)?;

        assert_eq!(select_object_model(&normal, LocType::new(4), 7), None);
        assert_eq!(
            select_object_model(&normal, LocType::new(10), 7)
                .ok_or("untyped type-10 selection missing")?
                .mirror(),
            false
        );
        let selected = select_object_model(&rotated, LocType::new(10), 7)
            .ok_or("rotated untyped type-10 selection missing")?;
        assert_eq!(selected.model_ids(), ids.as_slice());
        assert!(selected.mirror());

        let empty = object(Some(ObjectModels::Untyped(Vec::new())), false)?;
        assert_eq!(select_object_model(&empty, LocType::new(10), 0), None);
        assert_eq!(select_object_model(&object(None, false)?, LocType::new(10), 0), None);
        Ok(())
    }

    #[test]
    fn mirror_negates_z_and_reverses_face_winding_without_mutating_source()
    -> Result<(), Box<dyn std::error::Error>> {
        let source = source_model(
            77,
            vec![
                ModelPoint::new(1, 2, 3),
                ModelPoint::new(4, 5, -6),
                ModelPoint::new(7, 8, i32::MIN),
            ],
            vec![Triangle::new(0, 1, 2)],
        )?;
        let source_vertices = source.vertices().to_vec();
        let mirrored = mirror_source_model(&source)?;

        assert_eq!(
            mirrored.vertices(),
            &[
                ModelPoint::new(1, 2, -3),
                ModelPoint::new(4, 5, 6),
                ModelPoint::new(7, 8, i32::MIN),
            ]
        );
        assert_eq!(mirrored.faces(), &[Triangle::new(2, 1, 0)]);
        assert_eq!(source.vertices(), source_vertices.as_slice());
        assert_eq!(source.faces(), &[Triangle::new(0, 1, 2)]);
        Ok(())
    }

    #[test]
    fn combination_deduplicates_vertices_and_offsets_texture_selectors()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut first_parts = SourceModelParts {
            identity: DefinitionIdentity::new(ModelId::new(1), provenance()?),
            format: ModelFormatIdentity {
                encoding: ModelEncoding::TrailerFfFd,
                version: None,
            },
            vertices: vec![
                ModelPoint::new(0, 0, 0),
                ModelPoint::new(10, 0, 0),
                ModelPoint::new(0, 10, 0),
            ],
            faces: vec![Triangle::new(0, 1, 2)],
            face_colors: vec![100],
            default_priority: FacePriority::new(2).ok_or("priority")?,
            face_render_types: None,
            face_priorities: None,
            face_alphas: None,
            face_textures: Some(vec![Some(TextureId::new(7))]),
            texture_face_selectors: Some(vec![Some(TextureTriangleIndex::new(0))]),
            face_biases: None,
            texture_triangles: vec![TextureTriangle {
                render_type: 0,
                vertices: Triangle::new(0, 1, 2),
                mapping: TextureMappingParameters::default(),
            }],
            vertex_skins: Some(vec![5, 6, 7]),
            face_skins: None,
            skeletal_vertices: Some(vec![
                Some(SkeletalVertexData {
                    bone_ids: vec![1],
                    weights: vec![100],
                }),
                None,
                None,
            ]),
        };
        let first = SourceModel::from_parts(first_parts.clone())?;

        first_parts.identity = DefinitionIdentity::new(ModelId::new(2), provenance()?);
        first_parts.vertices = vec![
            ModelPoint::new(10, 0, 0),
            ModelPoint::new(20, 0, 0),
            ModelPoint::new(10, 10, 0),
        ];
        first_parts.faces = vec![Triangle::new(0, 1, 2)];
        first_parts.face_colors = vec![200];
        first_parts.default_priority = FacePriority::new(3).ok_or("priority")?;
        first_parts.face_textures = Some(vec![Some(TextureId::new(8))]);
        first_parts.texture_face_selectors = Some(vec![Some(TextureTriangleIndex::new(0))]);
        first_parts.texture_triangles = vec![TextureTriangle {
            render_type: 0,
            vertices: Triangle::new(0, 1, 2),
            mapping: TextureMappingParameters::default(),
        }];
        first_parts.vertex_skins = Some(vec![99, 8, 9]);
        first_parts.skeletal_vertices = Some(vec![
            Some(SkeletalVertexData {
                bone_ids: vec![9],
                weights: vec![9],
            }),
            None,
            None,
        ]);
        let second = SourceModel::from_parts(first_parts)?;

        let combined = combine_source_models(&[&first, &second])?;
        assert_eq!(combined.vertices().len(), 5);
        assert_eq!(
            combined.vertices(),
            &[
                ModelPoint::new(0, 0, 0),
                ModelPoint::new(10, 0, 0),
                ModelPoint::new(0, 10, 0),
                ModelPoint::new(20, 0, 0),
                ModelPoint::new(10, 10, 0),
            ]
        );
        assert_eq!(
            combined.faces(),
            &[Triangle::new(0, 1, 2), Triangle::new(1, 3, 4)]
        );
        assert_eq!(
            combined.texture_face_selectors(),
            Some(&[
                Some(TextureTriangleIndex::new(0)),
                Some(TextureTriangleIndex::new(1)),
            ][..])
        );
        assert_eq!(
            combined.face_priorities(),
            Some(&[
                FacePriority::new(2).ok_or("priority")?,
                FacePriority::new(3).ok_or("priority")?,
            ][..])
        );
        // Duplicate coordinate (10,0,0) keeps the first source's vertex skin.
        assert_eq!(combined.vertex_skins(), Some(&[5, 6, 7, 8, 9][..]));
        assert_eq!(
            combined.skeletal_vertices().and_then(|values| values[1].as_ref()),
            None
        );
        assert_eq!(combined.sources().len(), 2);
        Ok(())
    }

    #[test]
    fn complex_texture_mapping_is_preserved_without_vertex_remap()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut parts = SourceModelParts {
            identity: DefinitionIdentity::new(ModelId::new(3), provenance()?),
            format: ModelFormatIdentity {
                encoding: ModelEncoding::TrailerFfFd,
                version: None,
            },
            vertices: vec![
                ModelPoint::new(0, 0, 0),
                ModelPoint::new(1, 0, 0),
                ModelPoint::new(0, 1, 0),
            ],
            faces: vec![Triangle::new(0, 1, 2)],
            face_colors: vec![1],
            default_priority: FacePriority::ZERO,
            face_render_types: None,
            face_priorities: None,
            face_alphas: None,
            face_textures: None,
            texture_face_selectors: None,
            face_biases: None,
            texture_triangles: vec![TextureTriangle {
                render_type: 2,
                vertices: Triangle::new(32_769, 65_535, 4_096),
                mapping: TextureMappingParameters {
                    scale: Some([2, 3, 4]),
                    rotation: Some(5),
                    direction: Some(6),
                    speed: Some(7),
                    translation: Some([8, 9]),
                },
            }],
            vertex_skins: None,
            face_skins: None,
            skeletal_vertices: None,
        };
        let first = SourceModel::from_parts(parts.clone())?;
        parts.identity = DefinitionIdentity::new(ModelId::new(4), provenance()?);
        let second = SourceModel::from_parts(parts)?;

        let combined = combine_source_models(&[&first, &second])?;
        assert_eq!(combined.texture_triangles().len(), 2);
        assert_eq!(combined.texture_triangles()[0], first.texture_triangles()[0]);
        assert_eq!(combined.texture_triangles()[1], second.texture_triangles()[0]);
        Ok(())
    }
}
