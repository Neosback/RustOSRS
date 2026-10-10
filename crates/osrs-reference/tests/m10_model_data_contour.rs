//! `ModelData.method5320` contouring (the pre-lighting counterpart of `Model.contourGround`).

use osrs_core::{
    contour::{ContourGroundInput, contour_model_data_copy},
    coords::ModelPoint,
    definitions::DefinitionIdentity,
    ids::ModelId,
    model::{
        FacePriority, ModelEncoding, ModelFormatIdentity, ModelNormalState, SourceModel,
        SourceModelParts, Triangle,
    },
    provenance::{CacheFingerprint, ProfileDigest, TargetProvenance},
};
use std::{borrow::Cow, error::Error};

const PROFILE_DIGEST: &str = "cfdefa9ef99eff799fcef4fdf0ec78d9fdcd72d8e5be78e1c154d018ab4575b7";
const CACHE_FINGERPRINT: &str = "ae76dad78b4990d1b404e68e77a85ed2c96cf4a56c16f7b017cb97d1e92fdb38";

fn model(vertices: Vec<ModelPoint>) -> Result<SourceModel, Box<dyn Error>> {
    let provenance = TargetProvenance::new(
        "osrs-live-build-241",
        ProfileDigest::from_lower_hex(PROFILE_DIGEST)?,
        CacheFingerprint::from_lower_hex(CACHE_FINGERPRINT)?,
        1,
    )?;
    Ok(SourceModel::from_parts(SourceModelParts {
        identity: DefinitionIdentity::new(ModelId::new(1), provenance),
        format: ModelFormatIdentity {
            encoding: ModelEncoding::TrailerFfFd,
            version: None,
        },
        vertices,
        faces: vec![Triangle::new(0, 1, 2)],
        face_colors: vec![0x1234],
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

fn grid(height: impl Fn(usize, usize) -> i32) -> Vec<Vec<i32>> {
    (0..8)
        .map(|x| (0..8).map(|z| height(x, z)).collect())
        .collect()
}

#[test]
fn flat_ground_returns_the_original_model() -> Result<(), Box<dyn Error>> {
    let source = model(vec![
        ModelPoint::new(-64, 0, -64),
        ModelPoint::new(64, 0, -64),
        ModelPoint::new(0, -100, 64),
    ])?;
    let working = source.to_working_copy();
    let heights = grid(|_, _| 40);
    let result = contour_model_data_copy(
        &working,
        ContourGroundInput {
            heights: &heights,
            origin_x: 3 * 128 + 64,
            base_height: 40,
            origin_z: 3 * 128 + 64,
            clip: 0,
        },
    )?;
    assert!(matches!(result, Cow::Borrowed(_)));
    Ok(())
}

#[test]
fn sloped_ground_contours_a_copy_and_discards_normals() -> Result<(), Box<dyn Error>> {
    let source = model(vec![
        ModelPoint::new(-64, 0, -64),
        ModelPoint::new(64, 0, -64),
        ModelPoint::new(0, -100, 64),
    ])?;
    let working = source.to_working_copy();
    let heights = grid(|x, _| 8 * i32::try_from(x).unwrap_or(0));
    let result = contour_model_data_copy(
        &working,
        ContourGroundInput {
            heights: &heights,
            origin_x: 3 * 128 + 64,
            base_height: 24,
            origin_z: 3 * 128 + 64,
            clip: 0,
        },
    )?;
    let Cow::Owned(contoured) = result else {
        return Err("sloped ground must produce a contoured copy".into());
    };
    // x = 3*128+64-64 = 384 -> tile 3, offset 0 -> ground 24; vertex y: 24 + 0 - 24 = 0.
    assert_eq!(contoured.vertices()[0].y, 0);
    // x = 3*128+64+64 = 512 -> tile 4, offset 0 -> ground 32; y: 32 + 0 - 24 = 8.
    assert_eq!(contoured.vertices()[1].y, 8);
    // x = 448 -> tile 3 offset 64 -> ground 28; y: 28 - 100 - 24 = -96.
    assert_eq!(contoured.vertices()[2].y, -96);
    assert!(matches!(
        contoured.normal_state(),
        ModelNormalState::Uncomputed
    ));
    // The shared source model is untouched.
    assert_eq!(working.vertices()[1].y, 0);
    Ok(())
}
