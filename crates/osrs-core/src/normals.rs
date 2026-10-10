//! Exact semantic normal generation and cross-model reconciliation.
//!
//! M7 owns normal and lighting semantics. This module mirrors the pinned
//! `ModelData.calculateVertexNormals()` and `ModelData.method5262(...)` integer
//! behavior without welding topology, mutating shared source assets, or leaking
//! renderer state into semantic ownership.

use crate::{
    coords::ModelPoint,
    definitions::ModelTranslation,
    model::{FaceNormal, ModelNormalState, ModelNormals, Triangle, VertexNormal, WorkingModel},
};

/// Result of one exact cross-model normal reconciliation pass.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct NormalMergeOutcome {
    matched_vertex_pairs: usize,
    hidden_left_faces: usize,
    hidden_right_faces: usize,
}

impl NormalMergeOutcome {
    pub const fn matched_vertex_pairs(self) -> usize {
        self.matched_vertex_pairs
    }

    pub const fn hidden_left_faces(self) -> usize {
        self.hidden_left_faces
    }

    pub const fn hidden_right_faces(self) -> usize {
        self.hidden_right_faces
    }
}

/// Calculate the pinned reference base normals for one semantic working model.
///
/// This operation is pure with respect to the model. Scene-local lifecycle code
/// stores the result only when reconciliation or final lighting requires it.
pub fn calculate_base_normals(model: &WorkingModel) -> ModelNormals {
    calculate_base_normals_from_parts(model.vertices(), model.faces(), model.face_render_types())
}

/// Reconcile two separate scene-local working models using the pinned
/// `ModelData.method5262(...)` contract.
///
/// `translation` is subtracted from the left model's vertex coordinates before
/// comparing them against the right model, exactly matching the reference call.
/// Topology and model identity remain separate. Existing merged-normal slots are
/// preserved and receive another contribution on repeated reconciliation calls.
pub fn merge_model_normals(
    left: &mut WorkingModel,
    right: &mut WorkingModel,
    translation: ModelTranslation,
    hide_matched_faces: bool,
) -> NormalMergeOutcome {
    merge_model_normals_with_tolerance(left, right, translation, hide_matched_faces, 0)
}

/// [`merge_model_normals`] with a vertical match tolerance.
///
/// The reference compares `y` strictly (`y_tolerance == 0`). A positive tolerance is a
/// deliberate, opt-in deviation: pieces authored with a small vertical offset (for example
/// object 1904's `translation.y = 1` next to 1907) then still merge normals and hide their
/// coincident end caps.
pub fn merge_model_normals_with_tolerance(
    left: &mut WorkingModel,
    right: &mut WorkingModel,
    translation: ModelTranslation,
    hide_matched_faces: bool,
    y_tolerance: i32,
) -> NormalMergeOutcome {
    ensure_base_normal_state(left);
    ensure_base_normal_state(right);

    let left_base = base_vertex_normals(left).to_vec();
    let right_base = base_vertex_normals(right).to_vec();
    let right_bounds = ReferenceMergeBounds::calculate(right.vertices());
    let mut matched_left = vec![false; left.vertices().len()];
    let mut matched_right = vec![false; right.vertices().len()];
    let mut matched_pairs = Vec::<(usize, usize)>::new();

    for (left_index, left_vertex) in left.vertices().iter().copied().enumerate() {
        let left_normal = left_base[left_index];
        if left_normal.magnitude == 0 {
            continue;
        }

        let translated_y = left_vertex.y.wrapping_sub(translation.y);
        if translated_y > right_bounds.max_y.saturating_add(y_tolerance) {
            continue;
        }
        let translated_x = left_vertex.x.wrapping_sub(translation.x);
        if translated_x < right_bounds.min_x || translated_x > right_bounds.max_x {
            continue;
        }
        let translated_z = left_vertex.z.wrapping_sub(translation.z);
        if translated_z < right_bounds.min_z || translated_z > right_bounds.max_z {
            continue;
        }

        for (right_index, right_vertex) in right.vertices().iter().copied().enumerate() {
            let right_normal = right_base[right_index];
            if right_normal.magnitude == 0 {
                continue;
            }
            if translated_x == right_vertex.x
                && translated_y.abs_diff(right_vertex.y) <= y_tolerance.unsigned_abs()
                && translated_z == right_vertex.z
            {
                matched_pairs.push((left_index, right_index));
                matched_left[left_index] = true;
                matched_right[right_index] = true;
            }
        }
    }

    if !matched_pairs.is_empty() {
        apply_merge_contributions(left, &left_base, &right_base, &matched_pairs, true);
        apply_merge_contributions(right, &right_base, &left_base, &matched_pairs, false);
    }

    let (hidden_left_faces, hidden_right_faces) = if matched_pairs.len() >= 3 && hide_matched_faces
    {
        (
            hide_fully_matched_faces(left, &matched_left),
            hide_fully_matched_faces(right, &matched_right),
        )
    } else {
        (0, 0)
    };

    NormalMergeOutcome {
        matched_vertex_pairs: matched_pairs.len(),
        hidden_left_faces,
        hidden_right_faces,
    }
}

fn ensure_base_normal_state(model: &mut WorkingModel) {
    if matches!(model.normal_state(), ModelNormalState::Uncomputed) {
        let normals = calculate_base_normals(model);
        model.set_computed_normals(normals);
    }
}

fn base_vertex_normals(model: &WorkingModel) -> &[VertexNormal] {
    let ModelNormalState::Computed(normals) = model.normal_state() else {
        unreachable!("base normal state is ensured before reconciliation");
    };
    &normals.base_vertex_normals
}

fn apply_merge_contributions(
    model: &mut WorkingModel,
    own_base: &[VertexNormal],
    other_base: &[VertexNormal],
    matched_pairs: &[(usize, usize)],
    model_is_left: bool,
) {
    let vertex_count = own_base.len();
    let Some(normals) = model.computed_normals_mut() else {
        unreachable!("base normal state is ensured before reconciliation");
    };
    let merged = normals
        .merged_vertex_normals
        .get_or_insert_with(|| vec![None; vertex_count]);

    for &(left_index, right_index) in matched_pairs {
        let (own_index, other_index) = if model_is_left {
            (left_index, right_index)
        } else {
            (right_index, left_index)
        };
        let target = merged[own_index].get_or_insert(own_base[own_index]);
        let contribution = other_base[other_index];
        target.x = target.x.wrapping_add(contribution.x);
        target.y = target.y.wrapping_add(contribution.y);
        target.z = target.z.wrapping_add(contribution.z);
        target.magnitude = target.magnitude.wrapping_add(contribution.magnitude);
    }
}

fn hide_fully_matched_faces(model: &mut WorkingModel, matched_vertices: &[bool]) -> usize {
    let matched_faces: Vec<usize> = model
        .faces()
        .iter()
        .copied()
        .enumerate()
        .filter_map(|(face_index, face)| {
            let all_matched = matched_vertices[face.a.get() as usize]
                && matched_vertices[face.b.get() as usize]
                && matched_vertices[face.c.get() as usize];
            all_matched.then_some(face_index)
        })
        .collect();

    for face_index in matched_faces.iter().copied() {
        model.mark_normal_merge_hidden_face(face_index);
    }
    matched_faces.len()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ReferenceMergeBounds {
    max_y: i32,
    min_x: i32,
    max_x: i32,
    min_z: i32,
    max_z: i32,
}

impl ReferenceMergeBounds {
    fn calculate(vertices: &[ModelPoint]) -> Self {
        let mut bounds = Self {
            max_y: 0,
            min_x: 999_999,
            max_x: -999_999,
            min_z: 99_999,
            max_z: -99_999,
        };

        for vertex in vertices {
            if vertex.x < bounds.min_x {
                bounds.min_x = vertex.x;
            }
            if vertex.x > bounds.max_x {
                bounds.max_x = vertex.x;
            }
            if vertex.z < bounds.min_z {
                bounds.min_z = vertex.z;
            }
            if vertex.z > bounds.max_z {
                bounds.max_z = vertex.z;
            }
            if vertex.y > bounds.max_y {
                bounds.max_y = vertex.y;
            }
        }
        bounds
    }
}

fn calculate_base_normals_from_parts(
    vertices: &[ModelPoint],
    faces: &[Triangle],
    face_render_types: Option<&[i8]>,
) -> ModelNormals {
    let mut vertex_normals = vec![VertexNormal::default(); vertices.len()];
    let mut face_normals: Option<Vec<Option<FaceNormal>>> = None;

    for (face_index, face) in faces.iter().copied().enumerate() {
        let a = vertices[face.a.get() as usize];
        let b = vertices[face.b.get() as usize];
        let c = vertices[face.c.get() as usize];
        let normal = reference_face_normal(a, b, c);
        let render_type = face_render_types.map_or(0, |types| types[face_index]);

        if render_type == 0 {
            for vertex_index in [face.a, face.b, face.c] {
                let target = &mut vertex_normals[vertex_index.get() as usize];
                target.x = target.x.wrapping_add(normal.x);
                target.y = target.y.wrapping_add(normal.y);
                target.z = target.z.wrapping_add(normal.z);
                target.magnitude = target.magnitude.wrapping_add(1);
            }
        } else if render_type == 1 {
            let normals = face_normals.get_or_insert_with(|| vec![None; faces.len()]);
            normals[face_index] = Some(normal);
        }
    }

    ModelNormals {
        base_vertex_normals: vertex_normals,
        face_normals,
        merged_vertex_normals: None,
    }
}

fn reference_face_normal(a: ModelPoint, b: ModelPoint, c: ModelPoint) -> FaceNormal {
    let edge1_x = b.x.wrapping_sub(a.x);
    let edge1_y = b.y.wrapping_sub(a.y);
    let edge1_z = b.z.wrapping_sub(a.z);
    let edge2_x = c.x.wrapping_sub(a.x);
    let edge2_y = c.y.wrapping_sub(a.y);
    let edge2_z = c.z.wrapping_sub(a.z);

    let mut normal_x = edge1_y
        .wrapping_mul(edge2_z)
        .wrapping_sub(edge2_y.wrapping_mul(edge1_z));
    let mut normal_y = edge1_z
        .wrapping_mul(edge2_x)
        .wrapping_sub(edge2_z.wrapping_mul(edge1_x));
    let mut normal_z = edge1_x
        .wrapping_mul(edge2_y)
        .wrapping_sub(edge2_x.wrapping_mul(edge1_y));

    while normal_x > 8192
        || normal_y > 8192
        || normal_z > 8192
        || normal_x < -8192
        || normal_y < -8192
        || normal_z < -8192
    {
        normal_x >>= 1;
        normal_y >>= 1;
        normal_z >>= 1;
    }

    let squared_length = normal_x
        .wrapping_mul(normal_x)
        .wrapping_add(normal_y.wrapping_mul(normal_y))
        .wrapping_add(normal_z.wrapping_mul(normal_z));
    let mut length = f64::from(squared_length).sqrt() as i32;
    if length <= 0 {
        length = 1;
    }

    FaceNormal {
        x: normal_x.wrapping_mul(256) / length,
        y: normal_y.wrapping_mul(256) / length,
        z: normal_z.wrapping_mul(256) / length,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        definitions::DefinitionIdentity,
        ids::ModelId,
        model::{FacePriority, ModelEncoding, ModelFormatIdentity, SourceModel, SourceModelParts},
        provenance::{CacheFingerprint, ProfileDigest, TargetProvenance},
    };
    use std::error::Error;

    const TRIANGLE_VERTICES: [ModelPoint; 3] = [
        ModelPoint::new(0, 0, 0),
        ModelPoint::new(128, 0, 0),
        ModelPoint::new(0, 0, 128),
    ];
    const PROFILE_DIGEST: &str = "cfdefa9ef99eff799fcef4fdf0ec78d9fdcd72d8e5be78e1c154d018ab4575b7";
    const CACHE_FINGERPRINT: &str =
        "ae76dad78b4990d1b404e68e77a85ed2c96cf4a56c16f7b017cb97d1e92fdb38";

    #[test]
    fn smooth_triangle_accumulates_exact_reference_vertex_normals() {
        let normals =
            calculate_base_normals_from_parts(&TRIANGLE_VERTICES, &[Triangle::new(0, 1, 2)], None);
        assert_eq!(
            normals.base_vertex_normals,
            vec![
                VertexNormal {
                    x: 0,
                    y: -256,
                    z: 0,
                    magnitude: 1,
                };
                3
            ]
        );
        assert_eq!(normals.face_normals, None);
        assert_eq!(normals.merged_vertex_normals, None);
    }

    #[test]
    fn flat_triangle_preserves_vertex_zeroes_and_authors_face_normal() {
        let normals = calculate_base_normals_from_parts(
            &TRIANGLE_VERTICES,
            &[Triangle::new(0, 1, 2)],
            Some(&[1]),
        );
        assert_eq!(
            normals.base_vertex_normals,
            vec![VertexNormal::default(); 3]
        );
        assert_eq!(
            normals.face_normals,
            Some(vec![Some(FaceNormal {
                x: 0,
                y: -256,
                z: 0,
            })])
        );
    }

    #[test]
    fn two_triangle_quad_accumulates_shared_smooth_vertices() {
        let vertices = [
            ModelPoint::new(0, 0, 0),
            ModelPoint::new(128, 0, 0),
            ModelPoint::new(128, 0, 128),
            ModelPoint::new(0, 0, 128),
        ];
        let normals = calculate_base_normals_from_parts(
            &vertices,
            &[Triangle::new(0, 1, 2), Triangle::new(0, 2, 3)],
            None,
        );
        assert_eq!(normals.base_vertex_normals[0].magnitude, 2);
        assert_eq!(normals.base_vertex_normals[1].magnitude, 1);
        assert_eq!(normals.base_vertex_normals[2].magnitude, 2);
        assert_eq!(normals.base_vertex_normals[3].magnitude, 1);
        assert!(
            normals
                .base_vertex_normals
                .iter()
                .all(|normal| normal.x == 0 && normal.y < 0 && normal.z == 0)
        );
    }

    #[test]
    fn winding_reversal_reverses_the_exact_normal_direction() {
        let forward =
            calculate_base_normals_from_parts(&TRIANGLE_VERTICES, &[Triangle::new(0, 1, 2)], None);
        let reversed =
            calculate_base_normals_from_parts(&TRIANGLE_VERTICES, &[Triangle::new(0, 2, 1)], None);
        assert_eq!(forward.base_vertex_normals[0].y, -256);
        assert_eq!(reversed.base_vertex_normals[0].y, 256);
    }

    #[test]
    fn non_smooth_non_flat_render_types_do_not_contribute_base_normals() {
        let normals = calculate_base_normals_from_parts(
            &TRIANGLE_VERTICES,
            &[Triangle::new(0, 1, 2)],
            Some(&[2]),
        );
        assert_eq!(
            normals.base_vertex_normals,
            vec![VertexNormal::default(); 3]
        );
        assert_eq!(normals.face_normals, None);
    }

    #[test]
    fn positive_translation_matches_after_left_coordinate_subtraction() -> Result<(), Box<dyn Error>>
    {
        let mut left = working_triangle(
            7_101,
            [
                ModelPoint::new(128, 0, 0),
                ModelPoint::new(256, 0, 0),
                ModelPoint::new(128, 0, 128),
            ],
        )?;
        let mut right = working_triangle(7_102, TRIANGLE_VERTICES)?;
        let outcome = merge_model_normals(
            &mut left,
            &mut right,
            ModelTranslation { x: 128, y: 0, z: 0 },
            false,
        );
        assert_eq!(outcome.matched_vertex_pairs(), 3);
        assert_eq!(merged_at(&left, 0).magnitude, 2);
        assert_eq!(merged_at(&right, 0).magnitude, 2);
        Ok(())
    }

    #[test]
    fn repeated_reconciliation_accumulates_from_existing_merged_slots() -> Result<(), Box<dyn Error>>
    {
        let mut left = working_triangle(7_201, TRIANGLE_VERTICES)?;
        let mut right = working_triangle(7_202, TRIANGLE_VERTICES)?;
        let translation = ModelTranslation::ZERO;

        let first = merge_model_normals(&mut left, &mut right, translation, false);
        let second = merge_model_normals(&mut left, &mut right, translation, false);

        assert_eq!(first.matched_vertex_pairs(), 3);
        assert_eq!(second.matched_vertex_pairs(), 3);
        assert_eq!(merged_at(&left, 0).y, -768);
        assert_eq!(merged_at(&left, 0).magnitude, 3);
        assert_eq!(merged_at(&right, 0).y, -768);
        assert_eq!(merged_at(&right, 0).magnitude, 3);
        Ok(())
    }

    fn working_triangle(
        model_id: u32,
        vertices: [ModelPoint; 3],
    ) -> Result<WorkingModel, Box<dyn Error>> {
        Ok(SourceModel::from_parts(SourceModelParts {
            identity: DefinitionIdentity::new(ModelId::new(model_id), provenance()?),
            format: ModelFormatIdentity {
                encoding: ModelEncoding::TrailerFfFd,
                version: None,
            },
            vertices: vertices.to_vec(),
            faces: vec![Triangle::new(0, 1, 2)],
            face_colors: vec![0],
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
        })?
        .to_working_copy())
    }

    fn merged_at(model: &WorkingModel, vertex: usize) -> VertexNormal {
        let ModelNormalState::Computed(normals) = model.normal_state() else {
            unreachable!("test merge must calculate normal state");
        };
        normals
            .merged_vertex_normals
            .as_ref()
            .and_then(|values| values[vertex])
            .unwrap_or_default()
    }

    fn provenance() -> Result<TargetProvenance, Box<dyn Error>> {
        Ok(TargetProvenance::new(
            "osrs-live-build-241",
            ProfileDigest::from_lower_hex(PROFILE_DIGEST)?,
            CacheFingerprint::from_lower_hex(CACHE_FINGERPRINT)?,
            1,
        )?)
    }
}
