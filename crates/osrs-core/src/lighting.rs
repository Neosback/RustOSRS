//! Exact reference ModelData -> lit-model conversion for M7.
//!
//! This module mirrors the pinned `ModelData.toModel(...)`, `method5263`, and
//! `method5264` integer lighting behavior. It consumes semantic `WorkingModel`
//! state after any scene normal reconciliation and produces renderer-neutral
//! baked reference colors. Object-definition cache/lifecycle policy remains in
//! the scene/model-construction layer rather than this pure conversion.

use crate::{
    coords::ModelPoint,
    definitions::DefinitionIdentity,
    ids::{ModelId, TextureId},
    model::{
        FaceNormal, FacePriority, ModelFormatIdentity, ModelNormalState, SkeletalVertexData,
        Triangle, VertexNormal, WorkingModel,
    },
    normals::calculate_base_normals,
};
use std::{error::Error, fmt};

/// Pinned initial/static object light vector from `ObjectComposition`.
pub const LOC_LIGHT_X: i32 = -50;
pub const LOC_LIGHT_Y: i32 = -10;
pub const LOC_LIGHT_Z: i32 = -50;

/// Exact integer parameters accepted by the reference `ModelData.toModel` path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LightingParameters {
    pub ambient: i32,
    pub contrast: i32,
    pub light_x: i32,
    pub light_y: i32,
    pub light_z: i32,
}

impl LightingParameters {
    /// Build the static-object rig from decoded object-definition adjustments.
    pub const fn for_loc(definition_ambient: i16, definition_contrast: i16) -> Self {
        Self {
            ambient: definition_ambient as i32 + 64,
            contrast: definition_contrast as i32 + 768,
            light_x: LOC_LIGHT_X,
            light_y: LOC_LIGHT_Y,
            light_z: LOC_LIGHT_Z,
        }
    }
}

/// Three exact reference baked color slots for one face.
///
/// `c == -1` denotes a flat-lit face and `c == -2` denotes a suppressed face,
/// matching the reference Model representation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LitFaceColors {
    pub a: i32,
    pub b: i32,
    pub c: i32,
}

/// Renderer-neutral semantic result of the reference ModelData lighting pass.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReferenceLitModel {
    pub identity: DefinitionIdentity<ModelId>,
    pub format: ModelFormatIdentity,
    pub vertices: Vec<ModelPoint>,
    pub faces: Vec<Triangle>,
    pub face_colors: Vec<LitFaceColors>,
    pub default_priority: FacePriority,
    pub face_priorities: Option<Vec<FacePriority>>,
    pub face_alphas: Option<Vec<i8>>,
    pub face_textures: Option<Vec<Option<TextureId>>>,
    /// Compacted used type-0 texture triangles, matching `ModelData.toModel`.
    pub texture_triangles: Vec<Triangle>,
    /// Per-face compact texture-triangle selector; `None` entry is reference `-1`.
    pub texture_faces: Option<Vec<Option<u32>>>,
    pub face_biases: Option<Vec<i8>>,
    pub vertex_skins: Option<Vec<i32>>,
    pub face_skins: Option<Vec<i32>>,
    pub skeletal_vertices: Option<Vec<Option<SkeletalVertexData>>>,
}

/// Invalid/internally inconsistent input to exact reference lighting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LightingError {
    ZeroLightDivisor { face: usize, vertex: Option<u32> },
    MissingFaceNormal(usize),
    TextureSelectorOutOfBounds { face: usize, selector: u32 },
    TextureIndexOverflow,
}

impl fmt::Display for LightingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroLightDivisor { face, vertex } => match vertex {
                Some(vertex) => write!(
                    formatter,
                    "reference lighting divisor is zero for face {face}, vertex {vertex}"
                ),
                None => write!(
                    formatter,
                    "reference flat-lighting divisor is zero for face {face}"
                ),
            },
            Self::MissingFaceNormal(face) => {
                write!(
                    formatter,
                    "reference flat face {face} has no computed face normal"
                )
            }
            Self::TextureSelectorOutOfBounds { face, selector } => write!(
                formatter,
                "face {face} references texture triangle {selector} outside the model"
            ),
            Self::TextureIndexOverflow => {
                formatter.write_str("compacted texture-triangle index exceeds u32")
            }
        }
    }
}

impl Error for LightingError {}

type CompactedTextureData = (Vec<Triangle>, Option<Vec<Option<u32>>>);

/// Convert one semantic ModelData/WorkingModel into exact reference baked colors.
///
/// If scene reconciliation authored a merged normal for a vertex, that merged
/// normal is selected before its base normal (`NORMALS-004`). The input model is
/// not mutated by lighting.
pub fn light_model_data(
    model: &WorkingModel,
    parameters: LightingParameters,
) -> Result<ReferenceLitModel, LightingError> {
    let calculated_normals = if matches!(model.normal_state(), ModelNormalState::Uncomputed) {
        Some(calculate_base_normals(model))
    } else {
        None
    };
    let normals = match model.normal_state() {
        ModelNormalState::Computed(normals) => normals,
        ModelNormalState::Uncomputed => {
            let Some(normals) = calculated_normals.as_ref() else {
                unreachable!("uncomputed normal state always creates base normals");
            };
            normals
        }
    };

    let light_vector_squared = parameters
        .light_z
        .wrapping_mul(parameters.light_z)
        .wrapping_add(parameters.light_x.wrapping_mul(parameters.light_x))
        .wrapping_add(parameters.light_y.wrapping_mul(parameters.light_y));
    let light_vector_magnitude = f64::from(light_vector_squared).sqrt() as i32;
    let scaled_contrast = light_vector_magnitude.wrapping_mul(parameters.contrast) >> 8;

    let mut face_colors = vec![LitFaceColors::default(); model.faces().len()];

    for (face_index, face) in model.faces().iter().copied().enumerate() {
        let mut render_type = model
            .face_render_types()
            .map_or(0, |values| values[face_index]);
        let alpha = model.face_alphas().map_or(0, |values| values[face_index]);
        let textured = model
            .face_textures()
            .and_then(|values| values[face_index])
            .is_some();

        if alpha == -2 {
            render_type = 3;
        }
        if alpha == -1 {
            render_type = 2;
        }

        let colors = &mut face_colors[face_index];
        if !textured {
            match render_type {
                0 => {
                    let source_color = i32::from(model.face_colors()[face_index]);
                    colors.a = adjust_hsl_lightness(
                        source_color,
                        vertex_light(
                            selected_vertex_normal(normals, face.a.get()),
                            parameters,
                            scaled_contrast,
                            face_index,
                            face.a.get(),
                        )?,
                    );
                    colors.b = adjust_hsl_lightness(
                        source_color,
                        vertex_light(
                            selected_vertex_normal(normals, face.b.get()),
                            parameters,
                            scaled_contrast,
                            face_index,
                            face.b.get(),
                        )?,
                    );
                    colors.c = adjust_hsl_lightness(
                        source_color,
                        vertex_light(
                            selected_vertex_normal(normals, face.c.get()),
                            parameters,
                            scaled_contrast,
                            face_index,
                            face.c.get(),
                        )?,
                    );
                }
                1 => {
                    let normal = face_normal(normals, face_index)?;
                    let light = flat_light(normal, parameters, scaled_contrast, face_index)?;
                    colors.a =
                        adjust_hsl_lightness(i32::from(model.face_colors()[face_index]), light);
                    colors.c = -1;
                }
                3 => {
                    colors.a = 128;
                    colors.c = -1;
                }
                _ => colors.c = -2,
            }
        } else {
            match render_type {
                0 => {
                    colors.a = clamp_texture_lightness(vertex_light(
                        selected_vertex_normal(normals, face.a.get()),
                        parameters,
                        scaled_contrast,
                        face_index,
                        face.a.get(),
                    )?);
                    colors.b = clamp_texture_lightness(vertex_light(
                        selected_vertex_normal(normals, face.b.get()),
                        parameters,
                        scaled_contrast,
                        face_index,
                        face.b.get(),
                    )?);
                    colors.c = clamp_texture_lightness(vertex_light(
                        selected_vertex_normal(normals, face.c.get()),
                        parameters,
                        scaled_contrast,
                        face_index,
                        face.c.get(),
                    )?);
                }
                1 => {
                    let normal = face_normal(normals, face_index)?;
                    colors.a = clamp_texture_lightness(flat_light(
                        normal,
                        parameters,
                        scaled_contrast,
                        face_index,
                    )?);
                    colors.c = -1;
                }
                _ => colors.c = -2,
            }
        }
    }

    let (texture_triangles, texture_faces) = compact_texture_triangles(model)?;

    Ok(ReferenceLitModel {
        identity: model.identity().clone(),
        format: model.format(),
        vertices: model.vertices().to_vec(),
        faces: model.faces().to_vec(),
        face_colors,
        default_priority: model.default_priority(),
        face_priorities: model.face_priorities().map(<[_]>::to_vec),
        face_alphas: model.face_alphas().map(<[_]>::to_vec),
        face_textures: model.face_textures().map(<[_]>::to_vec),
        texture_triangles,
        texture_faces,
        face_biases: model.face_biases().map(<[_]>::to_vec),
        vertex_skins: model.vertex_skins().map(<[_]>::to_vec),
        face_skins: model.face_skins().map(<[_]>::to_vec),
        skeletal_vertices: model.skeletal_vertices().map(<[_]>::to_vec),
    })
}

/// Reference `ModelData.method5263`: apply HSL lightness with exact clamps.
pub fn adjust_hsl_lightness(color: i32, lightness: i32) -> i32 {
    let mut adjusted = (color & 127).wrapping_mul(lightness) >> 7;
    adjusted = adjusted.clamp(2, 126);
    (color & 65_408).wrapping_add(adjusted)
}

/// Reference `ModelData.method5264`: clamp textured-face lightness.
pub const fn clamp_texture_lightness(lightness: i32) -> i32 {
    if lightness < 2 {
        2
    } else if lightness > 126 {
        126
    } else {
        lightness
    }
}

fn selected_vertex_normal(normals: &crate::model::ModelNormals, vertex: u32) -> VertexNormal {
    let index = vertex as usize;
    normals
        .merged_vertex_normals
        .as_ref()
        .and_then(|values| values[index])
        .unwrap_or(normals.base_vertex_normals[index])
}

fn face_normal(
    normals: &crate::model::ModelNormals,
    face_index: usize,
) -> Result<FaceNormal, LightingError> {
    normals
        .face_normals
        .as_ref()
        .and_then(|values| values.get(face_index))
        .copied()
        .flatten()
        .ok_or(LightingError::MissingFaceNormal(face_index))
}

fn vertex_light(
    normal: VertexNormal,
    parameters: LightingParameters,
    scaled_contrast: i32,
    face_index: usize,
    vertex: u32,
) -> Result<i32, LightingError> {
    let divisor = scaled_contrast.wrapping_mul(normal.magnitude);
    if divisor == 0 {
        return Err(LightingError::ZeroLightDivisor {
            face: face_index,
            vertex: Some(vertex),
        });
    }
    let quotient = java_div(
        reference_dot(parameters, normal.x, normal.y, normal.z),
        divisor,
    );
    Ok(quotient.wrapping_add(parameters.ambient))
}

fn flat_light(
    normal: FaceNormal,
    parameters: LightingParameters,
    scaled_contrast: i32,
    face_index: usize,
) -> Result<i32, LightingError> {
    let divisor = (scaled_contrast / 2).wrapping_add(scaled_contrast);
    if divisor == 0 {
        return Err(LightingError::ZeroLightDivisor {
            face: face_index,
            vertex: None,
        });
    }
    let quotient = java_div(
        reference_dot(parameters, normal.x, normal.y, normal.z),
        divisor,
    );
    Ok(quotient.wrapping_add(parameters.ambient))
}

fn reference_dot(parameters: LightingParameters, x: i32, y: i32, z: i32) -> i32 {
    parameters
        .light_y
        .wrapping_mul(y)
        .wrapping_add(parameters.light_z.wrapping_mul(z))
        .wrapping_add(parameters.light_x.wrapping_mul(x))
}

fn java_div(numerator: i32, denominator: i32) -> i32 {
    if numerator == i32::MIN && denominator == -1 {
        i32::MIN
    } else {
        numerator / denominator
    }
}

fn compact_texture_triangles(model: &WorkingModel) -> Result<CompactedTextureData, LightingError> {
    let Some(selectors) = model.texture_face_selectors() else {
        return Ok((Vec::new(), None));
    };
    if model.texture_triangles().is_empty() {
        return Ok((Vec::new(), None));
    }

    let mut usage = vec![0usize; model.texture_triangles().len()];
    for (face, selector) in selectors.iter().copied().enumerate() {
        let Some(selector) = selector else {
            continue;
        };
        let index = selector.get() as usize;
        let Some(value) = usage.get_mut(index) else {
            return Err(LightingError::TextureSelectorOutOfBounds {
                face,
                selector: selector.get(),
            });
        };
        *value += 1;
    }

    let mut compact = Vec::new();
    let mut mapping = vec![None; model.texture_triangles().len()];
    for (index, triangle) in model.texture_triangles().iter().enumerate() {
        if usage[index] > 0 && triangle.render_type == 0 {
            let compact_index =
                u32::try_from(compact.len()).map_err(|_| LightingError::TextureIndexOverflow)?;
            mapping[index] = Some(compact_index);
            compact.push(triangle.vertices);
        }
    }

    let mut texture_faces = Vec::with_capacity(selectors.len());
    for (face, selector) in selectors.iter().copied().enumerate() {
        let mapped = match selector {
            None => None,
            Some(selector) => {
                let index = selector.get() as usize;
                let Some(mapped) = mapping.get(index) else {
                    return Err(LightingError::TextureSelectorOutOfBounds {
                        face,
                        selector: selector.get(),
                    });
                };
                *mapped
            }
        };
        texture_faces.push(mapped);
    }

    Ok((compact, Some(texture_faces)))
}
