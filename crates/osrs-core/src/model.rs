//! Canonical source-model metadata and mutable working-model state.
//!
//! `SourceModel` is immutable-by-API after validation. `WorkingModel` is an
//! owned semantic copy intended for later M4/M7 transformations. Cache codecs,
//! renderer packing, GPU types, and scene ownership do not cross this boundary.

use crate::coords::ModelPoint;
use crate::definitions::DefinitionIdentity;
use crate::ids::{ModelId, TextureId};
use core::fmt;

/// Stable vertex index in canonical model topology.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct VertexIndex(u32);

impl VertexIndex {
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u32 {
        self.0
    }
}

/// Stable face index used by derived animation groups.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FaceIndex(u32);

impl FaceIndex {
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u32 {
        self.0
    }
}

/// Stable texture-triangle index used by an optional per-face mapping selector.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TextureTriangleIndex(u32);

impl TextureTriangleIndex {
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u32 {
        self.0
    }
}

/// Triangle topology. Winding is semantic and must survive renderer handoff.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Triangle {
    pub a: VertexIndex,
    pub b: VertexIndex,
    pub c: VertexIndex,
}

impl Triangle {
    pub const fn new(a: u32, b: u32, c: u32) -> Self {
        Self {
            a: VertexIndex::new(a),
            b: VertexIndex::new(b),
            c: VertexIndex::new(c),
        }
    }

    const fn indices(self) -> [VertexIndex; 3] {
        [self.a, self.b, self.c]
    }
}

/// Reference face priority. `FACE-002` defines the semantic domain as `0..=11`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FacePriority(u8);

impl FacePriority {
    pub const ZERO: Self = Self(0);

    pub const fn new(value: u8) -> Option<Self> {
        if value <= 11 { Some(Self(value)) } else { None }
    }

    pub const fn get(self) -> u8 {
        self.0
    }
}

/// ModelData encoding family identified by the audited final two source bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ModelEncoding {
    /// Encoding without one of the audited two-byte sentinel trailers.
    Legacy,
    /// Trailer bytes `FF FF`.
    TrailerFfFf,
    /// Trailer bytes `FF FE`.
    TrailerFfFe,
    /// Trailer bytes `FF FD`.
    TrailerFfFd,
}

/// Decode-format identity retained for provenance and differential fixtures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ModelFormatIdentity {
    pub encoding: ModelEncoding,
    /// Version byte/value when the selected encoding exposes one.
    pub version: Option<u8>,
}

/// Optional texture-triangle mapping parameters retained from target model data.
///
/// These are source inputs only. UV generation belongs to later model/render
/// algorithms and must not destructively filter these values during decode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct TextureMappingParameters {
    pub scale: Option<[i32; 3]>,
    pub rotation: Option<i32>,
    pub direction: Option<i32>,
    pub speed: Option<i32>,
    pub translation: Option<[i32; 2]>,
}

/// Texture triangle and its source mapping metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TextureTriangle {
    pub render_type: i8,
    pub vertices: Triangle,
    pub mapping: TextureMappingParameters,
}

/// Per-vertex skeletal source information.
///
/// Raw integer values are retained here. M8 owns the interpretation and pose
/// algorithms, so M2 does not normalize them into renderer-specific weights.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SkeletalVertexData {
    pub bone_ids: Vec<i32>,
    pub weights: Vec<i32>,
}

/// Derived animation grouping state built from source vertex/face skin metadata.
///
/// Group construction and animation execution are later responsibilities. M2
/// only provides a representation that does not overwrite source skin arrays.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AnimationGroups {
    pub vertex_groups: Vec<Vec<VertexIndex>>,
    pub face_alpha_groups: Vec<Vec<FaceIndex>>,
}

/// Construction payload accepted at the cache-decoder -> semantic boundary.
///
/// Optional outer arrays deliberately remain optional. `None` means the source
/// array was absent; `Some` with entry-level `None` preserves an encoded
/// sentinel such as an untextured face or missing texture-face selector.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SourceModelParts {
    pub identity: DefinitionIdentity<ModelId>,
    pub format: ModelFormatIdentity,
    pub vertices: Vec<ModelPoint>,
    pub faces: Vec<Triangle>,
    pub face_colors: Vec<u16>,
    pub default_priority: FacePriority,
    pub face_render_types: Option<Vec<i8>>,
    pub face_priorities: Option<Vec<FacePriority>>,
    /// Signed source bytes are retained exactly, including `-1`.
    pub face_alphas: Option<Vec<i8>>,
    pub face_textures: Option<Vec<Option<TextureId>>>,
    pub texture_face_selectors: Option<Vec<Option<TextureTriangleIndex>>>,
    /// Authored face-bias bytes, when supplied by the selected model format.
    pub face_biases: Option<Vec<i8>>,
    pub texture_triangles: Vec<TextureTriangle>,
    pub vertex_skins: Option<Vec<i32>>,
    pub face_skins: Option<Vec<i32>>,
    pub skeletal_vertices: Option<Vec<Option<SkeletalVertexData>>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct ModelSemanticData {
    identity: DefinitionIdentity<ModelId>,
    format: ModelFormatIdentity,
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

impl From<SourceModelParts> for ModelSemanticData {
    fn from(parts: SourceModelParts) -> Self {
        Self {
            identity: parts.identity,
            format: parts.format,
            vertices: parts.vertices,
            faces: parts.faces,
            face_colors: parts.face_colors,
            default_priority: parts.default_priority,
            face_render_types: parts.face_render_types,
            face_priorities: parts.face_priorities,
            face_alphas: parts.face_alphas,
            face_textures: parts.face_textures,
            texture_face_selectors: parts.texture_face_selectors,
            face_biases: parts.face_biases,
            texture_triangles: parts.texture_triangles,
            vertex_skins: parts.vertex_skins,
            face_skins: parts.face_skins,
            skeletal_vertices: parts.skeletal_vertices,
        }
    }
}

/// Immutable validated decoded/raw model asset.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SourceModel {
    data: ModelSemanticData,
}

impl SourceModel {
    /// Validate all topology/parallel-array invariants and admit a source model.
    pub fn from_parts(parts: SourceModelParts) -> Result<Self, ModelValidationError> {
        validate_parts(&parts)?;
        Ok(Self { data: parts.into() })
    }

    pub fn identity(&self) -> &DefinitionIdentity<ModelId> {
        &self.data.identity
    }

    pub const fn format(&self) -> ModelFormatIdentity {
        self.data.format
    }

    pub fn vertices(&self) -> &[ModelPoint] {
        &self.data.vertices
    }

    pub fn faces(&self) -> &[Triangle] {
        &self.data.faces
    }

    pub fn face_colors(&self) -> &[u16] {
        &self.data.face_colors
    }

    pub const fn default_priority(&self) -> FacePriority {
        self.data.default_priority
    }

    pub fn face_render_types(&self) -> Option<&[i8]> {
        self.data.face_render_types.as_deref()
    }

    pub fn face_priorities(&self) -> Option<&[FacePriority]> {
        self.data.face_priorities.as_deref()
    }

    pub fn face_alphas(&self) -> Option<&[i8]> {
        self.data.face_alphas.as_deref()
    }

    pub fn face_textures(&self) -> Option<&[Option<TextureId>]> {
        self.data.face_textures.as_deref()
    }

    pub fn texture_face_selectors(&self) -> Option<&[Option<TextureTriangleIndex>]> {
        self.data.texture_face_selectors.as_deref()
    }

    pub fn face_biases(&self) -> Option<&[i8]> {
        self.data.face_biases.as_deref()
    }

    pub fn texture_triangles(&self) -> &[TextureTriangle] {
        &self.data.texture_triangles
    }

    pub fn vertex_skins(&self) -> Option<&[i32]> {
        self.data.vertex_skins.as_deref()
    }

    pub fn face_skins(&self) -> Option<&[i32]> {
        self.data.face_skins.as_deref()
    }

    pub fn skeletal_vertices(&self) -> Option<&[Option<SkeletalVertexData>]> {
        self.data.skeletal_vertices.as_deref()
    }

    /// Create an owned mutable semantic copy. Mutating the returned value cannot
    /// mutate this cached source model.
    pub fn to_working_copy(&self) -> WorkingModel {
        WorkingModel {
            data: self.data.clone(),
            normals: ModelNormalState::Uncomputed,
            animation_groups: None,
        }
    }
}

/// Accumulated integer vertex normal used by reference normal/lighting stages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct VertexNormal {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub magnitude: i32,
}

/// Integer flat-face normal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct FaceNormal {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

/// Derived normal state after base-normal computation and optional scene merge.
///
/// Base and merged vertex normals are separate. A populated merged slot means
/// cross-model accumulation exists for that vertex; it does not weld topology.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ModelNormals {
    pub base_vertex_normals: Vec<VertexNormal>,
    pub face_normals: Option<Vec<Option<FaceNormal>>>,
    pub merged_vertex_normals: Option<Vec<Option<VertexNormal>>>,
}

/// Derived normal state is explicit so all-zero arrays are never confused with
/// "normals have not been calculated yet".
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub enum ModelNormalState {
    #[default]
    Uncomputed,
    Computed(ModelNormals),
}

/// Owned mutable model state used by exact semantic transformations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkingModel {
    data: ModelSemanticData,
    normals: ModelNormalState,
    animation_groups: Option<AnimationGroups>,
}

impl WorkingModel {
    pub fn identity(&self) -> &DefinitionIdentity<ModelId> {
        &self.data.identity
    }

    pub const fn format(&self) -> ModelFormatIdentity {
        self.data.format
    }

    pub fn vertices(&self) -> &[ModelPoint] {
        &self.data.vertices
    }

    /// Mutable geometry access invalidates all derived normal state.
    pub fn vertices_mut(&mut self) -> &mut [ModelPoint] {
        self.normals = ModelNormalState::Uncomputed;
        &mut self.data.vertices
    }

    pub fn faces(&self) -> &[Triangle] {
        &self.data.faces
    }

    /// Mutable topology access invalidates all derived normal state.
    pub fn faces_mut(&mut self) -> &mut [Triangle] {
        self.normals = ModelNormalState::Uncomputed;
        &mut self.data.faces
    }

    pub fn face_colors(&self) -> &[u16] {
        &self.data.face_colors
    }

    pub fn face_colors_mut(&mut self) -> &mut [u16] {
        &mut self.data.face_colors
    }

    pub const fn default_priority(&self) -> FacePriority {
        self.data.default_priority
    }

    pub fn face_render_types(&self) -> Option<&[i8]> {
        self.data.face_render_types.as_deref()
    }

    /// Render type participates in normal ownership, so mutation invalidates
    /// existing normal state.
    pub fn face_render_types_mut(&mut self) -> Option<&mut [i8]> {
        self.normals = ModelNormalState::Uncomputed;
        self.data.face_render_types.as_deref_mut()
    }

    /// Materialize the reference default render-type array when a later semantic
    /// stage needs to author a per-face type such as matched-face suppression.
    pub fn ensure_face_render_types(&mut self, default: i8) -> &mut [i8] {
        self.normals = ModelNormalState::Uncomputed;
        self.data
            .face_render_types
            .get_or_insert_with(|| vec![default; self.data.faces.len()])
    }

    pub fn face_priorities(&self) -> Option<&[FacePriority]> {
        self.data.face_priorities.as_deref()
    }

    pub fn face_alphas(&self) -> Option<&[i8]> {
        self.data.face_alphas.as_deref()
    }

    pub fn face_alphas_mut(&mut self) -> Option<&mut [i8]> {
        self.data.face_alphas.as_deref_mut()
    }

    pub fn face_textures(&self) -> Option<&[Option<TextureId>]> {
        self.data.face_textures.as_deref()
    }

    pub fn face_textures_mut(&mut self) -> Option<&mut [Option<TextureId>]> {
        self.data.face_textures.as_deref_mut()
    }

    pub fn texture_face_selectors(&self) -> Option<&[Option<TextureTriangleIndex>]> {
        self.data.texture_face_selectors.as_deref()
    }

    pub fn face_biases(&self) -> Option<&[i8]> {
        self.data.face_biases.as_deref()
    }

    pub fn texture_triangles(&self) -> &[TextureTriangle] {
        &self.data.texture_triangles
    }

    pub fn vertex_skins(&self) -> Option<&[i32]> {
        self.data.vertex_skins.as_deref()
    }

    pub fn face_skins(&self) -> Option<&[i32]> {
        self.data.face_skins.as_deref()
    }

    pub fn skeletal_vertices(&self) -> Option<&[Option<SkeletalVertexData>]> {
        self.data.skeletal_vertices.as_deref()
    }

    pub fn normal_state(&self) -> &ModelNormalState {
        &self.normals
    }

    /// Install freshly calculated base normals on this scene-local working copy.
    pub(crate) fn set_computed_normals(&mut self, normals: ModelNormals) {
        self.normals = ModelNormalState::Computed(normals);
    }

    /// Mutate already-computed normal state without exposing it outside the core crate.
    pub(crate) fn computed_normals_mut(&mut self) -> Option<&mut ModelNormals> {
        match &mut self.normals {
            ModelNormalState::Uncomputed => None,
            ModelNormalState::Computed(normals) => Some(normals),
        }
    }

    /// Reference normal reconciliation authors render type `2` after base normals
    /// are calculated. That operation must not invalidate the merged normal state.
    pub(crate) fn mark_normal_merge_hidden_face(&mut self, face_index: usize) {
        let render_types = self
            .data
            .face_render_types
            .get_or_insert_with(|| vec![0; self.data.faces.len()]);
        render_types[face_index] = 2;
    }

    pub fn animation_groups(&self) -> Option<&AnimationGroups> {
        self.animation_groups.as_ref()
    }
}

/// Invalid canonical model shape/topology.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelValidationError {
    ParallelArrayLength {
        field: &'static str,
        expected: usize,
        actual: usize,
    },
    FaceVertexOutOfRange {
        face: usize,
        vertex: u32,
        vertex_count: usize,
    },
    TextureTriangleVertexOutOfRange {
        triangle: usize,
        vertex: u32,
        vertex_count: usize,
    },
    TextureSelectorOutOfRange {
        face: usize,
        selector: u32,
        texture_triangle_count: usize,
    },
    SkeletalInfluenceLength {
        vertex: usize,
        bone_count: usize,
        weight_count: usize,
    },
}

impl fmt::Display for ModelValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ParallelArrayLength {
                field,
                expected,
                actual,
            } => write!(
                formatter,
                "model field {field} has length {actual}, expected {expected}"
            ),
            Self::FaceVertexOutOfRange {
                face,
                vertex,
                vertex_count,
            } => write!(
                formatter,
                "face {face} references vertex {vertex}, but vertex count is {vertex_count}"
            ),
            Self::TextureTriangleVertexOutOfRange {
                triangle,
                vertex,
                vertex_count,
            } => write!(
                formatter,
                "texture triangle {triangle} references vertex {vertex}, but vertex count is {vertex_count}"
            ),
            Self::TextureSelectorOutOfRange {
                face,
                selector,
                texture_triangle_count,
            } => write!(
                formatter,
                "face {face} selects texture triangle {selector}, but texture triangle count is {texture_triangle_count}"
            ),
            Self::SkeletalInfluenceLength {
                vertex,
                bone_count,
                weight_count,
            } => write!(
                formatter,
                "skeletal vertex {vertex} has {bone_count} bone ids but {weight_count} weights"
            ),
        }
    }
}

impl std::error::Error for ModelValidationError {}

fn validate_parts(parts: &SourceModelParts) -> Result<(), ModelValidationError> {
    let face_count = parts.faces.len();
    let vertex_count = parts.vertices.len();

    require_len("face_colors", face_count, parts.face_colors.len())?;
    require_optional_len("face_render_types", face_count, &parts.face_render_types)?;
    require_optional_len("face_priorities", face_count, &parts.face_priorities)?;
    require_optional_len("face_alphas", face_count, &parts.face_alphas)?;
    require_optional_len("face_textures", face_count, &parts.face_textures)?;
    require_optional_len(
        "texture_face_selectors",
        face_count,
        &parts.texture_face_selectors,
    )?;
    require_optional_len("face_biases", face_count, &parts.face_biases)?;
    require_optional_len("vertex_skins", vertex_count, &parts.vertex_skins)?;
    require_optional_len("face_skins", face_count, &parts.face_skins)?;
    require_optional_len("skeletal_vertices", vertex_count, &parts.skeletal_vertices)?;

    for (face_index, face) in parts.faces.iter().copied().enumerate() {
        for vertex in face.indices() {
            if vertex.get() as usize >= vertex_count {
                return Err(ModelValidationError::FaceVertexOutOfRange {
                    face: face_index,
                    vertex: vertex.get(),
                    vertex_count,
                });
            }
        }
    }

    for (triangle_index, triangle) in parts.texture_triangles.iter().enumerate() {
        // Only render type 0 encodes an explicit triangle in model-vertex index space.
        // Target FF FD complex mappings (types 1..=3) reuse the three raw source
        // words for mapping parameters and may legitimately exceed vertex_count.
        if triangle.render_type == 0 {
            for vertex in triangle.vertices.indices() {
                if vertex.get() as usize >= vertex_count {
                    return Err(ModelValidationError::TextureTriangleVertexOutOfRange {
                        triangle: triangle_index,
                        vertex: vertex.get(),
                        vertex_count,
                    });
                }
            }
        }
    }

    if let Some(selectors) = &parts.texture_face_selectors {
        for (face, selector) in selectors.iter().enumerate() {
            if let Some(selector) = selector
                && selector.get() as usize >= parts.texture_triangles.len()
            {
                return Err(ModelValidationError::TextureSelectorOutOfRange {
                    face,
                    selector: selector.get(),
                    texture_triangle_count: parts.texture_triangles.len(),
                });
            }
        }
    }

    if let Some(vertices) = &parts.skeletal_vertices {
        for (vertex, data) in vertices.iter().enumerate() {
            if let Some(data) = data
                && data.bone_ids.len() != data.weights.len()
            {
                return Err(ModelValidationError::SkeletalInfluenceLength {
                    vertex,
                    bone_count: data.bone_ids.len(),
                    weight_count: data.weights.len(),
                });
            }
        }
    }

    Ok(())
}

fn require_len(
    field: &'static str,
    expected: usize,
    actual: usize,
) -> Result<(), ModelValidationError> {
    if expected == actual {
        Ok(())
    } else {
        Err(ModelValidationError::ParallelArrayLength {
            field,
            expected,
            actual,
        })
    }
}

fn require_optional_len<T>(
    field: &'static str,
    expected: usize,
    values: &Option<Vec<T>>,
) -> Result<(), ModelValidationError> {
    match values {
        Some(values) => require_len(field, expected, values.len()),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provenance::{CacheFingerprint, ProfileDigest, TargetProvenance};

    const PROFILE_DIGEST: &str = "cfdefa9ef99eff799fcef4fdf0ec78d9fdcd72d8e5be78e1c154d018ab4575b7";
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

    fn minimal_parts() -> Result<SourceModelParts, Box<dyn std::error::Error>> {
        Ok(SourceModelParts {
            identity: DefinitionIdentity::new(ModelId::new(77), provenance()?),
            format: ModelFormatIdentity {
                encoding: ModelEncoding::TrailerFfFd,
                version: Some(15),
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
        })
    }

    #[test]
    fn face_priority_accepts_only_reference_domain() {
        assert_eq!(FacePriority::new(0).map(FacePriority::get), Some(0));
        assert_eq!(FacePriority::new(11).map(FacePriority::get), Some(11));
        assert_eq!(FacePriority::new(12), None);
    }

    #[test]
    fn model_encoding_names_preserve_audited_trailer_byte_order() {
        let format = ModelFormatIdentity {
            encoding: ModelEncoding::TrailerFfFd,
            version: Some(15),
        };
        assert_eq!(format.encoding, ModelEncoding::TrailerFfFd);
    }

    #[test]
    fn optional_array_absence_is_preserved() -> Result<(), Box<dyn std::error::Error>> {
        let source = SourceModel::from_parts(minimal_parts()?)?;
        assert_eq!(source.face_render_types(), None);
        assert_eq!(source.face_priorities(), None);
        assert_eq!(source.face_alphas(), None);
        assert_eq!(source.face_textures(), None);
        assert_eq!(source.texture_face_selectors(), None);
        assert_eq!(source.face_biases(), None);
        Ok(())
    }

    #[test]
    fn optional_entries_preserve_sentinels() -> Result<(), Box<dyn std::error::Error>> {
        let mut parts = minimal_parts()?;
        parts.face_alphas = Some(vec![-1]);
        parts.face_textures = Some(vec![None]);
        parts.texture_face_selectors = Some(vec![None]);
        parts.face_biases = Some(vec![-7]);
        let source = SourceModel::from_parts(parts)?;

        assert_eq!(source.face_alphas(), Some(&[-1][..]));
        assert_eq!(source.face_textures(), Some(&[None][..]));
        assert_eq!(source.texture_face_selectors(), Some(&[None][..]));
        assert_eq!(source.face_biases(), Some(&[-7][..]));
        Ok(())
    }

    #[test]
    fn validation_rejects_parallel_array_mismatch() -> Result<(), Box<dyn std::error::Error>> {
        let mut parts = minimal_parts()?;
        parts.face_alphas = Some(vec![0, 1]);
        let result = SourceModel::from_parts(parts);
        let Err(error) = result else {
            return Err("mismatched alpha array unexpectedly succeeded".into());
        };
        assert_eq!(
            error,
            ModelValidationError::ParallelArrayLength {
                field: "face_alphas",
                expected: 1,
                actual: 2,
            }
        );
        Ok(())
    }

    #[test]
    fn validation_rejects_bad_topology() -> Result<(), Box<dyn std::error::Error>> {
        let mut parts = minimal_parts()?;
        parts.faces[0] = Triangle::new(0, 1, 99);
        let result = SourceModel::from_parts(parts);
        let Err(error) = result else {
            return Err("bad vertex index unexpectedly succeeded".into());
        };
        assert_eq!(
            error,
            ModelValidationError::FaceVertexOutOfRange {
                face: 0,
                vertex: 99,
                vertex_count: 3,
            }
        );
        Ok(())
    }

    #[test]
    fn texture_mapping_and_selector_are_preserved() -> Result<(), Box<dyn std::error::Error>> {
        let mut parts = minimal_parts()?;
        parts.texture_triangles = vec![TextureTriangle {
            render_type: 2,
            vertices: Triangle::new(0, 1, 2),
            mapping: TextureMappingParameters {
                scale: Some([1024, 2048, 4096]),
                rotation: Some(64),
                direction: Some(3),
                speed: Some(7),
                translation: Some([-2, 9]),
            },
        }];
        parts.face_textures = Some(vec![Some(TextureId::new(999))]);
        parts.texture_face_selectors = Some(vec![Some(TextureTriangleIndex::new(0))]);
        let source = SourceModel::from_parts(parts)?;

        assert_eq!(source.texture_triangles()[0].mapping.speed, Some(7));
        assert_eq!(
            source.face_textures(),
            Some(&[Some(TextureId::new(999))][..])
        );
        assert_eq!(
            source.texture_face_selectors(),
            Some(&[Some(TextureTriangleIndex::new(0))][..])
        );
        Ok(())
    }

    #[test]
    fn complex_texture_raw_triplet_is_not_geometry_topology()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut parts = minimal_parts()?;
        parts.texture_triangles = vec![TextureTriangle {
            render_type: 2,
            vertices: Triangle::new(32_769, 65_535, 4_096),
            mapping: TextureMappingParameters::default(),
        }];
        let source = SourceModel::from_parts(parts)?;
        assert_eq!(source.texture_triangles()[0].vertices.a.get(), 32_769);

        let mut type_zero = minimal_parts()?;
        type_zero.texture_triangles = vec![TextureTriangle {
            render_type: 0,
            vertices: Triangle::new(0, 1, 32_769),
            mapping: TextureMappingParameters::default(),
        }];
        let result = SourceModel::from_parts(type_zero);
        let Err(ModelValidationError::TextureTriangleVertexOutOfRange { vertex, .. }) = result
        else {
            return Err("type-0 out-of-range texture triangle unexpectedly succeeded".into());
        };
        assert_eq!(vertex, 32_769);
        Ok(())
    }

    #[test]
    fn validation_rejects_bad_texture_selector() -> Result<(), Box<dyn std::error::Error>> {
        let mut parts = minimal_parts()?;
        parts.texture_face_selectors = Some(vec![Some(TextureTriangleIndex::new(0))]);
        let result = SourceModel::from_parts(parts);
        let Err(error) = result else {
            return Err("missing texture triangle unexpectedly succeeded".into());
        };
        assert_eq!(
            error,
            ModelValidationError::TextureSelectorOutOfRange {
                face: 0,
                selector: 0,
                texture_triangle_count: 0,
            }
        );
        Ok(())
    }

    #[test]
    fn working_copy_is_independent() -> Result<(), Box<dyn std::error::Error>> {
        let source = SourceModel::from_parts(minimal_parts()?)?;
        let original = source.vertices()[0];
        let mut working = source.to_working_copy();
        working.vertices_mut()[0] = ModelPoint::new(900, 800, 700);

        assert_eq!(source.vertices()[0], original);
        assert_ne!(working.vertices()[0], original);
        Ok(())
    }

    #[test]
    fn mutation_invalidates_normals() -> Result<(), Box<dyn std::error::Error>> {
        let source = SourceModel::from_parts(minimal_parts()?)?;
        let mut working = source.to_working_copy();
        working.normals = ModelNormalState::Computed(ModelNormals {
            base_vertex_normals: vec![VertexNormal::default(); 3],
            face_normals: None,
            merged_vertex_normals: Some(vec![
                Some(VertexNormal {
                    x: 1,
                    y: 2,
                    z: 3,
                    magnitude: 4,
                });
                3
            ]),
        });

        let _ = working.vertices_mut();
        assert_eq!(working.normal_state(), &ModelNormalState::Uncomputed);

        working.normals = ModelNormalState::Computed(ModelNormals {
            base_vertex_normals: vec![VertexNormal::default(); 3],
            face_normals: None,
            merged_vertex_normals: None,
        });
        working.ensure_face_render_types(0)[0] = 2;
        assert_eq!(working.normal_state(), &ModelNormalState::Uncomputed);
        assert_eq!(working.face_render_types(), Some(&[2][..]));
        Ok(())
    }

    #[test]
    fn base_and_merged_normals_are_distinct_state() {
        let normals = ModelNormals {
            base_vertex_normals: vec![VertexNormal {
                x: 10,
                y: 20,
                z: 30,
                magnitude: 1,
            }],
            face_normals: Some(vec![None]),
            merged_vertex_normals: Some(vec![Some(VertexNormal {
                x: 40,
                y: 50,
                z: 60,
                magnitude: 2,
            })]),
        };

        let merged = normals
            .merged_vertex_normals
            .as_ref()
            .and_then(|values| values[0])
            .unwrap_or_default();
        assert_ne!(normals.base_vertex_normals[0], merged);
    }

    #[test]
    fn groups_preserve_source_skins() -> Result<(), Box<dyn std::error::Error>> {
        let mut parts = minimal_parts()?;
        parts.vertex_skins = Some(vec![3, 3, 8]);
        parts.face_skins = Some(vec![4]);
        let source = SourceModel::from_parts(parts)?;
        let mut working = source.to_working_copy();
        working.animation_groups = Some(AnimationGroups {
            vertex_groups: vec![vec![VertexIndex::new(0), VertexIndex::new(1)]],
            face_alpha_groups: vec![vec![FaceIndex::new(0)]],
        });

        assert_eq!(source.vertex_skins(), Some(&[3, 3, 8][..]));
        assert_eq!(source.face_skins(), Some(&[4][..]));
        assert_eq!(
            working
                .animation_groups()
                .map(|groups| groups.vertex_groups[0].as_slice()),
            Some(&[VertexIndex::new(0), VertexIndex::new(1)][..])
        );
        Ok(())
    }

    #[test]
    fn skeletal_bone_and_weight_lengths_must_match() -> Result<(), Box<dyn std::error::Error>> {
        let mut parts = minimal_parts()?;
        parts.skeletal_vertices = Some(vec![
            Some(SkeletalVertexData {
                bone_ids: vec![1, 2],
                weights: vec![100],
            }),
            None,
            None,
        ]);
        let result = SourceModel::from_parts(parts);
        let Err(error) = result else {
            return Err("mismatched skeletal influences unexpectedly succeeded".into());
        };
        assert_eq!(
            error,
            ModelValidationError::SkeletalInfluenceLength {
                vertex: 0,
                bone_count: 2,
                weight_count: 1,
            }
        );
        Ok(())
    }
}
