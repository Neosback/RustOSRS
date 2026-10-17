use osrs_core::{
    contour::{ContourGroundInput, contour_ground_copy, contour_ground_in_place},
    coords::ModelPoint,
    definitions::DefinitionIdentity,
    ids::ModelId,
    lighting::ReferenceLitModel,
    model::{FacePriority, ModelEncoding, ModelFormatIdentity, SourceModel, SourceModelParts},
    model_identity::ModelSemanticIdentity,
    provenance::{CacheFingerprint, ProfileDigest, TargetProvenance},
};
use std::{borrow::Cow, error::Error};

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

fn lit_model(vertices: Vec<ModelPoint>) -> Result<ReferenceLitModel, Box<dyn Error>> {
    let format = ModelFormatIdentity {
        encoding: ModelEncoding::Legacy,
        version: None,
    };
    let source = SourceModel::from_parts(SourceModelParts {
        identity: DefinitionIdentity::new(ModelId::new(1), provenance()?),
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
        vertex_skins: None,
        face_skins: None,
        skeletal_vertices: None,
    })?;

    Ok(ReferenceLitModel {
        identity: ModelSemanticIdentity::from_source(&source),
        format: Some(format),
        vertices,
        faces: Vec::new(),
        face_colors: Vec::new(),
        default_priority: FacePriority::ZERO,
        face_render_types: None,
        face_priorities: None,
        face_alphas: None,
        face_textures: None,
        texture_triangles: Vec::new(),
        texture_faces: None,
        face_biases: None,
        vertex_skins: None,
        face_skins: None,
        skeletal_vertices: None,
    })
}

fn flat_grid(value: i32) -> Vec<Vec<i32>> {
    vec![vec![value; 8]; 8]
}

fn historical_slope_grid() -> Vec<Vec<i32>> {
    (0_i32..8)
        .map(|x| (0_i32..8).map(|z| x * 10 + z).collect())
        .collect()
}

#[test]
fn historical_deob_contour_flat_and_slope_outputs_match() -> Result<(), Box<dyn Error>> {
    let model = lit_model(vec![ModelPoint::new(0, 0, 0)])?;

    let flat = flat_grid(100);
    let flat_result = contour_ground_copy(
        &model,
        ContourGroundInput {
            heights: &flat,
            origin_x: 256,
            base_height: 100,
            origin_z: 256,
            clip: 0,
        },
    )?;
    assert!(matches!(flat_result, Cow::Borrowed(_)));
    assert_eq!(flat_result.vertices[0].y, 0);

    let slope = historical_slope_grid();
    let slope_result = contour_ground_copy(
        &model,
        ContourGroundInput {
            heights: &slope,
            origin_x: 256,
            base_height: 50,
            origin_z: 256,
            clip: 0,
        },
    )?;
    let Cow::Owned(contoured) = slope_result else {
        panic!("non-flat historical contour case must create a safe working copy");
    };
    assert_eq!(contoured.vertices[0].y, -28);
    assert_eq!(model.vertices[0].y, 0);
    Ok(())
}

#[test]
fn nonzero_clip_preserves_threshold_and_exact_tile_boundary_math() -> Result<(), Box<dyn Error>> {
    let terrain: Vec<Vec<i32>> = (0_i32..8)
        .map(|x| (0_i32..8).map(|z| x * 128 + z * 16).collect())
        .collect();
    let model = lit_model(vec![
        ModelPoint::new(0, -25, 0),
        ModelPoint::new(0, -100, 0),
        ModelPoint::new(128, -25, 0),
        ModelPoint::new(128, 25, 0),
    ])?;
    let original = model.clone();

    let result = contour_ground_copy(
        &model,
        ContourGroundInput {
            heights: &terrain,
            origin_x: 256,
            base_height: 288,
            origin_z: 256,
            clip: 32_768,
        },
    )?;
    let Cow::Owned(contoured) = result else {
        panic!("non-equal contour corners must execute the copying contour path");
    };

    let ys: Vec<i32> = contoured.vertices.iter().map(|vertex| vertex.y).collect();
    assert_eq!(ys, [-25, -100, 39, 217]);
    assert_eq!(
        model, original,
        "copy contour must not mutate the cached base model"
    );
    Ok(())
}

#[test]
fn dynamic_private_instance_can_contour_in_place() -> Result<(), Box<dyn Error>> {
    let slope = historical_slope_grid();
    let mut model = lit_model(vec![ModelPoint::new(0, 0, 0)])?;

    let applied = contour_ground_in_place(
        &mut model,
        ContourGroundInput {
            heights: &slope,
            origin_x: 256,
            base_height: 50,
            origin_z: 256,
            clip: 0,
        },
    )?;

    assert!(applied);
    assert_eq!(model.vertices[0].y, -28);
    Ok(())
}
