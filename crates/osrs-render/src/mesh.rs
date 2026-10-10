use crate::{RenderCoordinateError, RenderOrigin, RenderPoint};
use osrs_core::{
    coords::{LocalCoord, LocalXZ},
    ids::TextureId,
    lighting::{LitFaceColors, ReferenceLitModel},
    model::{FacePriority, Triangle},
    model_identity::ModelSemanticIdentity,
};
use std::{error::Error, fmt};

/// Final semantic placement applied before renderer-relative rebasing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RenderPlacement {
    pub origin: LocalXZ,
    pub base_y: LocalCoord,
}

impl RenderPlacement {
    pub const fn new(origin: LocalXZ, base_y: LocalCoord) -> Self {
        Self { origin, base_y }
    }
}

/// Renderer-owned CPU mesh extracted from one finalized reference-lit model.
///
/// Optional parallel arrays intentionally remain optional. This preserves the
/// distinction between absent semantic metadata and an authored zero/default
/// value until a later renderer policy explicitly interprets it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderMesh {
    identity: ModelSemanticIdentity,
    vertices: Vec<RenderPoint>,
    faces: Vec<Triangle>,
    face_colors: Vec<LitFaceColors>,
    default_priority: FacePriority,
    face_render_types: Option<Vec<i8>>,
    face_priorities: Option<Vec<FacePriority>>,
    face_alphas: Option<Vec<i8>>,
    face_textures: Option<Vec<Option<TextureId>>>,
    texture_triangles: Vec<Triangle>,
    texture_faces: Option<Vec<Option<u32>>>,
    face_biases: Option<Vec<i8>>,
}

impl RenderMesh {
    pub fn extract(
        model: &ReferenceLitModel,
        placement: RenderPlacement,
        render_origin: RenderOrigin,
    ) -> Result<Self, RenderExtractionError> {
        let mut vertices = Vec::with_capacity(model.vertices.len());
        for (vertex_index, vertex) in model.vertices.iter().copied().enumerate() {
            let semantic = vertex
                .place(placement.origin, placement.base_y)
                .ok_or(RenderExtractionError::PlacementOverflow { vertex_index })?;
            let render = render_origin
                .rebase(semantic)
                .map_err(|source| RenderExtractionError::Coordinate {
                    vertex_index,
                    source,
                })?;
            vertices.push(render);
        }

        Ok(Self {
            identity: model.identity.clone(),
            vertices,
            faces: model.faces.clone(),
            face_colors: model.face_colors.clone(),
            default_priority: model.default_priority,
            face_render_types: model.face_render_types.clone(),
            face_priorities: model.face_priorities.clone(),
            face_alphas: model.face_alphas.clone(),
            face_textures: model.face_textures.clone(),
            texture_triangles: model.texture_triangles.clone(),
            texture_faces: model.texture_faces.clone(),
            face_biases: model.face_biases.clone(),
        })
    }

    pub fn identity(&self) -> &ModelSemanticIdentity {
        &self.identity
    }

    pub fn vertices(&self) -> &[RenderPoint] {
        &self.vertices
    }

    pub fn faces(&self) -> &[Triangle] {
        &self.faces
    }

    pub fn face_colors(&self) -> &[LitFaceColors] {
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

    pub fn texture_triangles(&self) -> &[Triangle] {
        &self.texture_triangles
    }

    pub fn texture_faces(&self) -> Option<&[Option<u32>]> {
        self.texture_faces.as_deref()
    }

    pub fn face_biases(&self) -> Option<&[i8]> {
        self.face_biases.as_deref()
    }
}

/// Exact CPU extraction failure. No renderer fallback is applied.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderExtractionError {
    PlacementOverflow {
        vertex_index: usize,
    },
    Coordinate {
        vertex_index: usize,
        source: RenderCoordinateError,
    },
}

impl fmt::Display for RenderExtractionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PlacementOverflow { vertex_index } => write!(
                formatter,
                "semantic placement overflow while extracting vertex {vertex_index}"
            ),
            Self::Coordinate {
                vertex_index,
                source,
            } => write!(
                formatter,
                "renderer coordinate conversion failed for vertex {vertex_index}: {source}"
            ),
        }
    }
}

impl Error for RenderExtractionError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::PlacementOverflow { .. } => None,
            Self::Coordinate { source, .. } => Some(source),
        }
    }
}
