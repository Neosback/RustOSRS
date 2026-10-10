use crate::{RenderMesh, RenderPoint};
use osrs_core::model::{Triangle, VertexIndex};
use std::{error::Error, fmt};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReferenceUv {
    pub u: f32,
    pub v: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReferenceFaceUvs {
    pub a: ReferenceUv,
    pub b: ReferenceUv,
    pub c: ReferenceUv,
}

impl ReferenceFaceUvs {
    pub const CANONICAL: Self = Self {
        a: ReferenceUv { u: 0.0, v: 0.0 },
        b: ReferenceUv { u: 1.0, v: 0.0 },
        c: ReferenceUv { u: 0.0, v: 1.0 },
    };
}

/// Reference UV preparation mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReferenceUvMode {
    /// Evaluate the face vertices directly against the authored texture triangle.
    Direct,
    /// Project each face vertex from the camera ray onto the texture plane first.
    ProjectedFromCamera { camera: RenderPoint },
}

/// Prepare RuneLite-reference model UVs for one textured face.
///
/// Untextured faces return `Ok(None)`. A textured face without an explicit
/// texture-face selector uses the canonical `(0,0)`, `(1,0)`, `(0,1)` mapping.
pub fn prepare_reference_face_uvs(
    mesh: &RenderMesh,
    face_index: usize,
    mode: ReferenceUvMode,
) -> Result<Option<ReferenceFaceUvs>, ReferenceUvError> {
    let face = *mesh
        .faces()
        .get(face_index)
        .ok_or(ReferenceUvError::FaceOutOfRange { face_index })?;

    let Some(face_textures) = mesh.face_textures() else {
        return Ok(None);
    };
    let texture = face_textures
        .get(face_index)
        .ok_or(ReferenceUvError::FaceTextureMetadataOutOfRange { face_index })?;
    if texture.is_none() {
        return Ok(None);
    }

    let selector = match mesh.texture_faces() {
        None => None,
        Some(selectors) => *selectors
            .get(face_index)
            .ok_or(ReferenceUvError::TextureSelectorMetadataOutOfRange { face_index })?,
    };

    let Some(selector) = selector else {
        return Ok(Some(ReferenceFaceUvs::CANONICAL));
    };
    let selector_index = usize::try_from(selector)
        .map_err(|_| ReferenceUvError::TextureTriangleOutOfRange { selector })?;
    let texture_triangle = *mesh
        .texture_triangles()
        .get(selector_index)
        .ok_or(ReferenceUvError::TextureTriangleOutOfRange { selector })?;

    let face_points = triangle_points(mesh, face)?;
    let texture_points = triangle_points(mesh, texture_triangle)?;
    let camera = match mode {
        ReferenceUvMode::Direct => None,
        ReferenceUvMode::ProjectedFromCamera { camera } => Some(point3(camera)),
    };

    Ok(Some(compute_explicit_uvs(
        face_points,
        texture_points,
        camera,
    )))
}

#[derive(Debug, Clone, Copy)]
struct Point3 {
    x: f32,
    y: f32,
    z: f32,
}

impl Point3 {
    fn sub(self, other: Self) -> Self {
        Self {
            x: self.x - other.x,
            y: self.y - other.y,
            z: self.z - other.z,
        }
    }

    fn add_scaled(self, vector: Self, scale: f32) -> Self {
        Self {
            x: self.x + vector.x * scale,
            y: self.y + vector.y * scale,
            z: self.z + vector.z * scale,
        }
    }
}

fn point3(point: RenderPoint) -> Point3 {
    Point3 {
        x: point.x as f32,
        y: point.y as f32,
        z: point.z as f32,
    }
}

fn triangle_points(mesh: &RenderMesh, triangle: Triangle) -> Result<[Point3; 3], ReferenceUvError> {
    Ok([
        vertex_point(mesh, triangle.a)?,
        vertex_point(mesh, triangle.b)?,
        vertex_point(mesh, triangle.c)?,
    ])
}

fn vertex_point(mesh: &RenderMesh, index: VertexIndex) -> Result<Point3, ReferenceUvError> {
    let raw = index.get();
    let index = usize::try_from(raw)
        .map_err(|_| ReferenceUvError::VertexOutOfRange { vertex_index: raw })?;
    mesh.vertices()
        .get(index)
        .copied()
        .map(point3)
        .ok_or(ReferenceUvError::VertexOutOfRange { vertex_index: raw })
}

fn compute_explicit_uvs(
    mut face: [Point3; 3],
    texture: [Point3; 3],
    camera: Option<Point3>,
) -> ReferenceFaceUvs {
    let t1 = texture[0];
    let tangent = texture[1].sub(t1);
    let bitangent = texture[2].sub(t1);
    let normal = cross(tangent, bitangent);

    if let Some(camera) = camera {
        face[0] = project_to_plane(face[0], t1, camera, normal);
        face[1] = project_to_plane(face[1], texture[1], camera, normal);
        face[2] = project_to_plane(face[2], texture[2], camera, normal);
    }

    let relative = [face[0].sub(t1), face[1].sub(t1), face[2].sub(t1)];

    let u_axis = cross(bitangent, normal);
    let inverse_u_denominator = 1.0 / dot(u_axis, tangent);
    let v_axis = cross(tangent, normal);
    let inverse_v_denominator = 1.0 / dot(v_axis, bitangent);

    let uv = relative.map(|point| ReferenceUv {
        u: dot(u_axis, point) * inverse_u_denominator,
        v: dot(v_axis, point) * inverse_v_denominator,
    });

    ReferenceFaceUvs {
        a: uv[0],
        b: uv[1],
        c: uv[2],
    }
}

fn project_to_plane(face: Point3, target: Point3, camera: Point3, normal: Point3) -> Point3 {
    let ray = camera.sub(face);
    let scale = dot(target.sub(face), normal) / dot(ray, normal);
    face.add_scaled(ray, scale)
}

fn cross(a: Point3, b: Point3) -> Point3 {
    Point3 {
        x: a.y * b.z - a.z * b.y,
        y: a.z * b.x - a.x * b.z,
        z: a.x * b.y - a.y * b.x,
    }
}

fn dot(a: Point3, b: Point3) -> f32 {
    a.x * b.x + a.y * b.y + a.z * b.z
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReferenceUvError {
    FaceOutOfRange { face_index: usize },
    FaceTextureMetadataOutOfRange { face_index: usize },
    TextureSelectorMetadataOutOfRange { face_index: usize },
    TextureTriangleOutOfRange { selector: u32 },
    VertexOutOfRange { vertex_index: u32 },
}

impl fmt::Display for ReferenceUvError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FaceOutOfRange { face_index } => {
                write!(formatter, "render face index {face_index} is out of range")
            }
            Self::FaceTextureMetadataOutOfRange { face_index } => write!(
                formatter,
                "face texture metadata is missing render face index {face_index}"
            ),
            Self::TextureSelectorMetadataOutOfRange { face_index } => write!(
                formatter,
                "texture selector metadata is missing render face index {face_index}"
            ),
            Self::TextureTriangleOutOfRange { selector } => {
                write!(
                    formatter,
                    "texture triangle selector {selector} is out of range"
                )
            }
            Self::VertexOutOfRange { vertex_index } => {
                write!(
                    formatter,
                    "render vertex index {vertex_index} is out of range"
                )
            }
        }
    }
}

impl Error for ReferenceUvError {}
