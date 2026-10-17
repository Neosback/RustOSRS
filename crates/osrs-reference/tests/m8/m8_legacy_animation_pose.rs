use osrs_core::{
    animation_pose::{
        LegacyAnimationFrame, LegacyFrameTransform, LegacyPoseError, LegacySkeletonTransform,
        pose_legacy_object_model,
    },
    coords::ModelPoint,
    definitions::DefinitionIdentity,
    ids::ModelId,
    lighting::{LitFaceColors, ReferenceLitModel},
    model::{
        FacePriority, ModelEncoding, ModelFormatIdentity, SourceModel, SourceModelParts, Triangle,
    },
    model_identity::ModelSemanticIdentity,
    provenance::{CacheFingerprint, ProfileDigest, TargetProvenance},
};
use std::error::Error;

const PROFILE_DIGEST: &str = "cfdefa9ef99eff799fcef4fdf0ec78d9fdcd72d8e5be78e1c154d018ab4575b7";
const CACHE_FINGERPRINT: &str = "ae76dad78b4990d1b404e68e77a85ed2c96cf4a56c16f7b017cb97d1e92fdb38";

fn provenance() -> Result<TargetProvenance, Box<dyn Error>> {
    Ok(TargetProvenance::new(
        "osrs-live-241-2026-09-30-openrs2-2727",
        ProfileDigest::from_lower_hex(PROFILE_DIGEST)?,
        CacheFingerprint::from_lower_hex(CACHE_FINGERPRINT)?,
        1,
    )?)
}

fn lit_model(
    vertices: Vec<ModelPoint>,
    faces: Vec<Triangle>,
    vertex_skins: Option<Vec<i32>>,
    face_skins: Option<Vec<i32>>,
    face_alphas: Option<Vec<i8>>,
) -> Result<ReferenceLitModel, Box<dyn Error>> {
    let format = ModelFormatIdentity {
        encoding: ModelEncoding::Legacy,
        version: None,
    };
    let source = SourceModel::from_parts(SourceModelParts {
        identity: DefinitionIdentity::new(ModelId::new(77), provenance()?),
        format,
        vertices: vertices.clone(),
        faces: faces.clone(),
        face_colors: vec![500; faces.len()],
        default_priority: FacePriority::ZERO,
        face_render_types: None,
        face_priorities: None,
        face_alphas: face_alphas.clone(),
        face_textures: None,
        texture_face_selectors: None,
        face_biases: None,
        texture_triangles: Vec::new(),
        vertex_skins: vertex_skins.clone(),
        face_skins: face_skins.clone(),
        skeletal_vertices: None,
    })?;

    Ok(ReferenceLitModel {
        identity: ModelSemanticIdentity::from_source(&source),
        format: Some(format),
        vertices,
        faces,
        face_colors: vec![LitFaceColors::default(); source.faces().len()],
        default_priority: FacePriority::ZERO,
        face_render_types: None,
        face_priorities: None,
        face_alphas,
        face_textures: None,
        texture_triangles: Vec::new(),
        texture_faces: None,
        face_biases: None,
        vertex_skins,
        face_skins,
        skeletal_vertices: None,
    })
}

fn frame(
    skeleton: Vec<LegacySkeletonTransform>,
    transforms: Vec<LegacyFrameTransform>,
) -> LegacyAnimationFrame {
    LegacyAnimationFrame {
        skeleton,
        transforms,
    }
}

#[test]
fn object_orientation_wraps_private_frame_translation_without_mutating_base()
-> Result<(), Box<dyn Error>> {
    let base = lit_model(
        vec![ModelPoint::new(10, 0, 0)],
        Vec::new(),
        Some(vec![0]),
        None,
        None,
    )?;
    let animation = frame(
        vec![LegacySkeletonTransform {
            transform_type: 1,
            labels: vec![0],
        }],
        vec![LegacyFrameTransform {
            skeleton_transform: 0,
            x: 5,
            y: 0,
            z: 0,
        }],
    );

    let posed = pose_legacy_object_model(&base, &animation, 1)?;

    assert_eq!(base.vertices, vec![ModelPoint::new(10, 0, 0)]);
    assert_eq!(posed.vertices, vec![ModelPoint::new(10, 0, -5)]);
    assert_eq!(posed.vertex_skins, base.vertex_skins);
    Ok(())
}

#[test]
fn pivot_then_reference_y_rotation_uses_exact_integer_trig_order() -> Result<(), Box<dyn Error>> {
    let base = lit_model(
        vec![ModelPoint::new(10, 0, 0), ModelPoint::new(0, 0, 0)],
        Vec::new(),
        Some(vec![0, 1]),
        None,
        None,
    )?;
    let animation = frame(
        vec![
            LegacySkeletonTransform {
                transform_type: 0,
                labels: vec![1],
            },
            LegacySkeletonTransform {
                transform_type: 2,
                labels: vec![0],
            },
        ],
        vec![
            LegacyFrameTransform {
                skeleton_transform: 0,
                x: 0,
                y: 0,
                z: 0,
            },
            LegacyFrameTransform {
                skeleton_transform: 1,
                x: 0,
                y: 64,
                z: 0,
            },
        ],
    );

    let posed = pose_legacy_object_model(&base, &animation, 0)?;

    assert_eq!(posed.vertices[0], ModelPoint::new(0, 0, -10));
    assert_eq!(posed.vertices[1], ModelPoint::new(0, 0, 0));
    assert_eq!(base.vertices[0], ModelPoint::new(10, 0, 0));
    Ok(())
}

#[test]
fn scale_transform_uses_reference_pivot_and_divide_by_128() -> Result<(), Box<dyn Error>> {
    let base = lit_model(
        vec![ModelPoint::new(10, 4, -6), ModelPoint::new(0, 0, 0)],
        Vec::new(),
        Some(vec![0, 1]),
        None,
        None,
    )?;
    let animation = frame(
        vec![
            LegacySkeletonTransform {
                transform_type: 0,
                labels: vec![1],
            },
            LegacySkeletonTransform {
                transform_type: 3,
                labels: vec![0],
            },
        ],
        vec![
            LegacyFrameTransform {
                skeleton_transform: 0,
                x: 0,
                y: 0,
                z: 0,
            },
            LegacyFrameTransform {
                skeleton_transform: 1,
                x: 256,
                y: 64,
                z: 128,
            },
        ],
    );

    let posed = pose_legacy_object_model(&base, &animation, 0)?;

    assert_eq!(posed.vertices[0], ModelPoint::new(20, 2, -6));
    assert_eq!(posed.vertices[1], ModelPoint::new(0, 0, 0));
    Ok(())
}

#[test]
fn alpha_transform_uses_unsigned_byte_math_and_reference_clamp() -> Result<(), Box<dyn Error>> {
    let base = lit_model(
        vec![
            ModelPoint::new(0, 0, 0),
            ModelPoint::new(1, 0, 0),
            ModelPoint::new(0, 0, 1),
        ],
        vec![Triangle::new(0, 1, 2)],
        Some(vec![0, 0, 0]),
        Some(vec![0]),
        Some(vec![0]),
    )?;
    let animation = frame(
        vec![LegacySkeletonTransform {
            transform_type: 5,
            labels: vec![0],
        }],
        vec![LegacyFrameTransform {
            skeleton_transform: 0,
            x: 20,
            y: 0,
            z: 0,
        }],
    );

    let posed = pose_legacy_object_model(&base, &animation, 0)?;

    assert_eq!(base.face_alphas, Some(vec![0]));
    assert_eq!(posed.face_alphas, Some(vec![-96]));
    Ok(())
}

#[test]
fn missing_vertex_labels_skips_even_alpha_only_frames_like_model_animate()
-> Result<(), Box<dyn Error>> {
    let base = lit_model(
        vec![
            ModelPoint::new(0, 0, 0),
            ModelPoint::new(1, 0, 0),
            ModelPoint::new(0, 0, 1),
        ],
        vec![Triangle::new(0, 1, 2)],
        None,
        Some(vec![0]),
        Some(vec![0]),
    )?;
    let animation = frame(
        vec![LegacySkeletonTransform {
            transform_type: 5,
            labels: vec![0],
        }],
        vec![LegacyFrameTransform {
            skeleton_transform: 0,
            x: 20,
            y: 0,
            z: 0,
        }],
    );

    let posed = pose_legacy_object_model(&base, &animation, 3)?;

    assert_eq!(posed.vertices, base.vertices);
    assert_eq!(posed.face_alphas, Some(vec![0]));
    Ok(())
}

#[test]
fn invalid_frame_and_skin_state_fail_explicitly() -> Result<(), Box<dyn Error>> {
    let invalid_skin = lit_model(
        vec![ModelPoint::new(0, 0, 0)],
        Vec::new(),
        Some(vec![256]),
        None,
        None,
    )?;
    let valid_frame = frame(Vec::new(), Vec::new());
    assert_eq!(
        pose_legacy_object_model(&invalid_skin, &valid_frame, 0),
        Err(LegacyPoseError::InvalidVertexSkin {
            vertex: 0,
            skin: 256,
        })
    );

    let base = lit_model(
        vec![ModelPoint::new(0, 0, 0)],
        Vec::new(),
        Some(vec![0]),
        None,
        None,
    )?;
    let invalid_frame = frame(
        Vec::new(),
        vec![LegacyFrameTransform {
            skeleton_transform: 4,
            x: 0,
            y: 0,
            z: 0,
        }],
    );
    assert_eq!(
        pose_legacy_object_model(&base, &invalid_frame, 0),
        Err(LegacyPoseError::SkeletonTransformOutOfRange {
            frame_transform: 0,
            skeleton_transform: 4,
            skeleton_len: 0,
        })
    );
    Ok(())
}
