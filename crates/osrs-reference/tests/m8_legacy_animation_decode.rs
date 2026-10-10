use osrs_cache::{
    decode::{
        ArchiveFileProvenance, DecoderContext, decode_legacy_animation_frame,
        decode_legacy_skeleton, legacy_frame_skeleton_id,
    },
    profile::TargetProfile,
};
use osrs_core::{
    animation_pose::pose_legacy_object_model,
    coords::ModelPoint,
    definitions::DefinitionIdentity,
    ids::{FrameId, ModelId},
    lighting::{LitFaceColors, ReferenceLitModel},
    model::{FacePriority, ModelEncoding, ModelFormatIdentity, SourceModel, SourceModelParts},
    model_identity::ModelSemanticIdentity,
    provenance::{CacheFingerprint, ProfileDigest, TargetProvenance},
};
use std::error::Error;

const PROFILE_YAML: &str =
    include_str!("../../../profiles/osrs-live-241-2026-09-30-openrs2-2727.yaml");
const PROFILE_DIGEST: &str = "cfdefa9ef99eff799fcef4fdf0ec78d9fdcd72d8e5be78e1c154d018ab4575b7";
const CACHE_FINGERPRINT: &str =
    "ae76dad78b4990d1b404e68e77a85ed2c96cf4a56c16f7b017cb97d1e92fdb38";

fn decoder_context() -> Result<DecoderContext, Box<dyn Error>> {
    let profile = TargetProfile::from_yaml_str(PROFILE_YAML)?;
    Ok(DecoderContext::from_profile(&profile)?)
}

fn provenance() -> Result<TargetProvenance, Box<dyn Error>> {
    Ok(TargetProvenance::new(
        "osrs-live-241-2026-09-30-openrs2-2727",
        ProfileDigest::from_lower_hex(PROFILE_DIGEST)?,
        CacheFingerprint::from_lower_hex(CACHE_FINGERPRINT)?,
        1,
    )?)
}

fn one_vertex_lit_model() -> Result<ReferenceLitModel, Box<dyn Error>> {
    let format = ModelFormatIdentity {
        encoding: ModelEncoding::Legacy,
        version: None,
    };
    let vertices = vec![ModelPoint::new(10, 0, 0)];
    let source = SourceModel::from_parts(SourceModelParts {
        identity: DefinitionIdentity::new(ModelId::new(91), provenance()?),
        format,
        vertices: vertices.clone(),
        faces: Vec::new(),
        face_colors: Vec::new(),
        default_priority: FacePriority::ZERO,
        face_render_types: None,
        face_priorities: None,
        face_alphas: None,
        face_textures: None,
        texture_face_selectors: None,
        face_biases: None,
        texture_triangles: Vec::new(),
        vertex_skins: Some(vec![0]),
        face_skins: None,
        skeletal_vertices: None,
    })?;

    Ok(ReferenceLitModel {
        identity: ModelSemanticIdentity::from_source(&source),
        format: Some(format),
        vertices,
        faces: Vec::new(),
        face_colors: Vec::<LitFaceColors>::new(),
        default_priority: FacePriority::ZERO,
        face_priorities: None,
        face_alphas: None,
        face_textures: None,
        texture_triangles: Vec::new(),
        texture_faces: None,
        face_biases: None,
        vertex_skins: Some(vec![0]),
        face_skins: None,
        skeletal_vertices: None,
    })
}

#[test]
fn decoded_cache_frame_flows_directly_into_private_object_pose() -> Result<(), Box<dyn Error>> {
    let context = decoder_context()?;
    let skeleton_id = 9_u16;
    let skeleton_source = ArchiveFileProvenance::new(1, u32::from(skeleton_id), Some(0));
    let frame_id = FrameId::new(0x0001_0000);
    let frame_source = ArchiveFileProvenance::new(0, 1, Some(0));

    // count=2, transform types=[0,1], label lengths=[1,1], labels=[0,0].
    let skeleton_bytes = [2, 0, 1, 1, 1, 0, 0];
    // skeleton=9, slot count=2, masks=[0,1], x short-smart=5.
    let frame_bytes = [0, 9, 2, 0, 1, 69];

    let skeleton = decode_legacy_skeleton(
        skeleton_id,
        &skeleton_bytes,
        &context,
        &skeleton_source,
    )?;
    assert_eq!(
        legacy_frame_skeleton_id(frame_id, &frame_bytes, &context, &frame_source)?,
        skeleton_id
    );
    let frame = decode_legacy_animation_frame(
        frame_id,
        &frame_bytes,
        skeleton_id,
        &skeleton,
        &context,
        &frame_source,
    )?;

    let base = one_vertex_lit_model()?;
    let posed = pose_legacy_object_model(&base, &frame, 1)?;

    assert_eq!(base.vertices, vec![ModelPoint::new(10, 0, 0)]);
    assert_eq!(posed.vertices, vec![ModelPoint::new(10, 0, -5)]);
    assert_eq!(posed.vertex_skins, base.vertex_skins);
    Ok(())
}
