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

/// Ensure the model retains its calculated base normals without recomputing
/// already-derived normal state.
///
/// This is the exact lifecycle operation used by the initial
/// `nonFlatShading` entity cache before a scene-local copy is returned.
pub fn ensure_base_normals(model: &mut WorkingModel) {
    ensure_base_normal_state(model);
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
        if translated_y > right_bounds.max_y {
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
                && translated_y == right_vertex.y
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
    let ab_x = b.x.wrapping_sub(a.x);
    let ab_y = b.y.wrapping_sub(a.y);
    let ab_z = b.z.wrapping_sub(a.z);
    let ac_x = c.x.wrapping_sub(a.x);
    let ac_y = c.y.wrapping_sub(a.y);
    let ac_z = c.z.wrapping_sub(a.z);

    let mut normal_x = ab_y
        .wrapping_mul(ac_z)
        .wrapping_sub(ab_z.wrapping_mul(ac_y));
    let mut normal_y = ab_z
        .wrapping_mul(ac_x)
        .wrapping_sub(ab_x.wrapping_mul(ac_z));
    let mut normal_z = ab_x
        .wrapping_mul(ac_y)
        .wrapping_sub(ab_y.wrapping_mul(ac_x));

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

    let magnitude_squared = normal_x
        .wrapping_mul(normal_x)
        .wrapping_add(normal_y.wrapping_mul(normal_y))
        .wrapping_add(normal_z.wrapping_mul(normal_z));
    let mut magnitude = f64::from(magnitude_squared).sqrt() as i32;
    if magnitude <= 0 {
        magnitude = 1;
    }

    FaceNormal {
        x: normal_x.wrapping_mul(256) / magnitude,
        y: normal_y.wrapping_mul(256) / magnitude,
        z: normal_z.wrapping_mul(256) / magnitude,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        definitions::DefinitionIdentity,
        ids::ModelId,
        model::{
            FacePriority, ModelEncoding, ModelFormatIdentity, SourceModel, SourceModelParts,
        },
        provenance::{CacheFingerprint, ProfileDigest, TargetProvenance},
    };

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

    fn working_model(
        vertices: Vec<ModelPoint>,
        faces: Vec<Triangle>,
        render_types: Option<Vec<i8>>,
    ) -> Result<WorkingModel, Box<dyn std::error::Error>> {
        let face_count = faces.len();
        Ok(SourceModel::from_parts(SourceModelParts {
            identity: DefinitionIdentity::new(ModelId::new(1), provenance()?),
            format: ModelFormatIdentity {
                encoding: ModelEncoding::TrailerFfFd,
                version: None,
            },
            vertices,
            faces,
            face_colors: vec![500; face_count],
            default_priority: FacePriority::ZERO,
            face_render_types: render_types,
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

    #[test]
    fn smooth_triangle_accumulates_exact_reference_normal()
    -> Result<(), Box<dyn std::error::Error>> {
        let model = working_model(
            vec![
                ModelPoint::new(0, 0, 0),
                ModelPoint::new(128, 0, 0),
                ModelPoint::new(0, 0, 128),
            ],
            vec![Triangle::new(0, 1, 2)],
            None,
        )?;
        let normals = calculate_base_normals(&model);

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
        Ok(())
    }

    #[test]
    fn flat_triangle_keeps_face_normal_and_zero_vertex_magnitudes()
    -> Result<(), Box<dyn std::error::Error>> {
        let model = working_model(
            vec![
                ModelPoint::new(0, 0, 0),
                ModelPoint::new(128, 0, 0),
                ModelPoint::new(0, 0, 128),
            ],
            vec![Triangle::new(0, 1, 2)],
            Some(vec![1]),
        )?;
        let normals = calculate_base_normals(&model);

        assert_eq!(normals.base_vertex_normals, vec![VertexNormal::default(); 3]);
        assert_eq!(
            normals.face_normals,
            Some(vec![Some(FaceNormal {
                x: 0,
                y: -256,
                z: 0,
            })])
        );
        Ok(())
    }

    #[test]
    fn hidden_render_type_does_not_contribute_normals()
    -> Result<(), Box<dyn std::error::Error>> {
        let model = working_model(
            vec![
                ModelPoint::new(0, 0, 0),
                ModelPoint::new(128, 0, 0),
                ModelPoint::new(0, 0, 128),
            ],
            vec![Triangle::new(0, 1, 2)],
            Some(vec![2]),
        )?;
        let normals = calculate_base_normals(&model);
        assert_eq!(normals.base_vertex_normals, vec![VertexNormal::default(); 3]);
        assert_eq!(normals.face_normals, None);
        Ok(())
    }

    #[test]
    fn merge_adds_cross_model_normals_without_welding_topology()
    -> Result<(), Box<dyn std::error::Error>> {
        let vertices = vec![
            ModelPoint::new(0, 0, 0),
            ModelPoint::new(128, 0, 0),
            ModelPoint::new(0, 0, 128),
        ];
        let mut left = working_model(vertices.clone(), vec![Triangle::new(0, 1, 2)], None)?;
        let mut right = working_model(vertices.clone(), vec![Triangle::new(0, 1, 2)], None)?;
        let left_faces = left.faces().to_vec();
        let right_faces = right.faces().to_vec();

        let outcome = merge_model_normals(&mut left, &mut right, ModelTranslation::ZERO, false);

        assert_eq!(outcome.matched_vertex_pairs(), 3);
        assert_eq!(left.vertices(), vertices);
        assert_eq!(right.vertices(), vertices);
        assert_eq!(left.faces(), left_faces);
        assert_eq!(right.faces(), right_faces);
        let ModelNormalState::Computed(left_normals) = left.normal_state() else {
            return Err("left normal state was not computed".into());
        };
        let ModelNormalState::Computed(right_normals) = right.normal_state() else {
            return Err("right normal state was not computed".into());
        };
        assert_eq!(
            left_normals.merged_vertex_normals,
            Some(vec![
                Some(VertexNormal {
                    x: 0,
                    y: -512,
                    z: 0,
                    magnitude: 2,
                });
                3
            ])
        );
        assert_eq!(
            right_normals.merged_vertex_normals,
            Some(vec![
                Some(VertexNormal {
                    x: 0,
                    y: -512,
                    z: 0,
                    magnitude: 2,
                });
                3
            ])
        );
        Ok(())
    }

    #[test]
    fn translated_negative_case_has_no_matches() -> Result<(), Box<dyn std::error::Error>> {
        let mut left = working_model(
            vec![
                ModelPoint::new(0, 0, 0),
                ModelPoint::new(128, 0, 0),
                ModelPoint::new(0, 0, 128),
            ],
            vec![Triangle::new(0, 1, 2)],
            None,
        )?;
        let mut right = working_model(
            vec![
                ModelPoint::new(256, 0, 0),
                ModelPoint::new(384, 0, 0),
                ModelPoint::new(256, 0, 128),
            ],
            vec![Triangle::new(0, 1, 2)],
            None,
        )?;

        let outcome = merge_model_normals(
            &mut left,
            &mut right,
            ModelTranslation {
                x: -128,
                y: 0,
                z: 0,
            },
            false,
        );

        assert_eq!(outcome.matched_vertex_pairs(), 0);
        let ModelNormalState::Computed(left_normals) = left.normal_state() else {
            return Err("left normal state was not computed".into());
        };
        let ModelNormalState::Computed(right_normals) = right.normal_state() else {
            return Err("right normal state was not computed".into());
        };
        assert_eq!(left_normals.merged_vertex_normals, None);
        assert_eq!(right_normals.merged_vertex_normals, None);
        Ok(())
    }

    #[test]
    fn hide_matched_faces_authors_render_type_two_on_both_models()
    -> Result<(), Box<dyn std::error::Error>> {
        let vertices = vec![
            ModelPoint::new(0, 0, 0),
            ModelPoint::new(128, 0, 0),
            ModelPoint::new(0, 0, 128),
        ];
        let mut left = working_model(vertices.clone(), vec![Triangle::new(0, 1, 2)], None)?;
        let mut right = working_model(vertices, vec![Triangle::new(0, 1, 2)], None)?;

        let outcome = merge_model_normals(&mut left, &mut right, ModelTranslation::ZERO, true);
        assert_eq!(outcome.matched_vertex_pairs(), 3);
        assert_eq!(outcome.hidden_left_faces(), 1);
        assert_eq!(outcome.hidden_right_faces(), 1);
        assert_eq!(left.face_render_types(), Some([2].as_slice()));
        assert_eq!(right.face_render_types(), Some([2].as_slice()));
        Ok(())
    }

    #[test]
    fn source_models_remain_immutable_through_scene_merge()
    -> Result<(), Box<dyn std::error::Error>> {
        let parts = SourceModelParts {
            identity: DefinitionIdentity::new(ModelId::new(42), provenance()?),
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
        };
        let source = SourceModel::from_parts(parts)?;
        let snapshot = source.clone();
        let mut left = source.to_working_copy();
        let mut right = source.to_working_copy();

        let outcome = merge_model_normals(&mut left, &mut right, ModelTranslation::ZERO, true);
        assert_eq!(outcome.matched_vertex_pairs(), 3);
        assert_eq!(source, snapshot);
        assert_eq!(source.face_render_types(), None);
        Ok(())
    }
}
