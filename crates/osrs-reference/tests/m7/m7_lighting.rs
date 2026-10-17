use osrs_core::{
    coords::ModelPoint,
    definitions::{DefinitionIdentity, ModelTranslation},
    ids::{ModelId, TextureId},
    lighting::{
        LightingParameters, LitFaceColors, adjust_hsl_lightness, clamp_texture_lightness,
        light_model_data,
    },
    model::{
        FacePriority, ModelEncoding, ModelFormatIdentity, SourceModel, SourceModelParts,
        TextureMappingParameters, TextureTriangle, TextureTriangleIndex, Triangle, WorkingModel,
    },
    normals::merge_model_normals,
    provenance::{CacheFingerprint, ProfileDigest, TargetProvenance},
};
use std::error::Error;

const PROFILE_DIGEST: &str = "cfdefa9ef99eff799fcef4fdf0ec78d9fdcd72d8e5be78e1c154d018ab4575b7";
const CACHE_FINGERPRINT: &str = "ae76dad78b4990d1b404e68e77a85ed2c96cf4a56c16f7b017cb97d1e92fdb38";
const BASE_TRIANGLE: [ModelPoint; 3] = [
    ModelPoint::new(0, 0, 0),
    ModelPoint::new(128, 0, 0),
    ModelPoint::new(0, 0, 128),
];

#[test]
fn historical_loc_rig_triangle_matches_checked_in_golden() -> Result<(), Box<dyn Error>> {
    let model = working_triangle(
        9_001,
        BASE_TRIANGLE,
        Triangle::new(0, 1, 2),
        None,
        false,
        None,
        false,
    )?;
    let lit = light_model_data(&model, LightingParameters::for_loc(0, 0))?;

    assert_eq!(
        lit.face_colors,
        vec![LitFaceColors {
            a: 4_638,
            b: 4_638,
            c: 4_638,
        }]
    );
    assert_eq!(lit.vertices, BASE_TRIANGLE);
    assert_eq!(lit.faces, vec![Triangle::new(0, 1, 2)]);
    Ok(())
}

#[test]
fn lighting_helpers_match_historical_harness_clamps() {
    assert_eq!(adjust_hsl_lightness(9_029, 100), 9_013);
    assert_eq!(adjust_hsl_lightness(65_535, 200), 65_534);
    assert_eq!(adjust_hsl_lightness(4_096, 0), 4_098);
    assert_eq!(adjust_hsl_lightness(4_096, 300), 4_098);

    assert_eq!(clamp_texture_lightness(0), 2);
    assert_eq!(clamp_texture_lightness(1), 2);
    assert_eq!(clamp_texture_lightness(2), 2);
    assert_eq!(clamp_texture_lightness(100), 100);
    assert_eq!(clamp_texture_lightness(126), 126);
    assert_eq!(clamp_texture_lightness(127), 126);
    assert_eq!(clamp_texture_lightness(200), 126);
}

#[test]
fn flat_and_textured_faces_follow_distinct_reference_paths() -> Result<(), Box<dyn Error>> {
    let flat = working_triangle(
        9_101,
        BASE_TRIANGLE,
        Triangle::new(0, 1, 2),
        Some(1),
        false,
        None,
        false,
    )?;
    let flat_lit = light_model_data(&flat, LightingParameters::for_loc(0, 0))?;
    assert_eq!(
        flat_lit.face_colors,
        vec![LitFaceColors {
            a: 4_637,
            b: 0,
            c: -1,
        }]
    );

    let textured = working_triangle(
        9_102,
        BASE_TRIANGLE,
        Triangle::new(0, 1, 2),
        None,
        true,
        None,
        true,
    )?;
    let textured_lit = light_model_data(&textured, LightingParameters::for_loc(0, 0))?;
    assert_eq!(
        textured_lit.face_colors,
        vec![LitFaceColors {
            a: 76,
            b: 76,
            c: 76,
        }]
    );
    assert_eq!(textured_lit.texture_triangles, vec![Triangle::new(0, 1, 2)]);
    assert_eq!(textured_lit.texture_faces, Some(vec![Some(0)]));

    let textured_flat = working_triangle(
        9_103,
        BASE_TRIANGLE,
        Triangle::new(0, 1, 2),
        Some(1),
        true,
        None,
        false,
    )?;
    let textured_flat_lit = light_model_data(&textured_flat, LightingParameters::for_loc(0, 0))?;
    assert_eq!(
        textured_flat_lit.face_colors,
        vec![LitFaceColors { a: 72, b: 0, c: -1 }]
    );
    Ok(())
}

#[test]
fn merged_normal_takes_precedence_and_changes_final_lighting() -> Result<(), Box<dyn Error>> {
    let mut left = working_triangle(
        9_201,
        BASE_TRIANGLE,
        Triangle::new(0, 1, 2),
        None,
        false,
        None,
        false,
    )?;
    let control = left.clone();
    let mut opposite = working_triangle(
        9_202,
        BASE_TRIANGLE,
        Triangle::new(0, 2, 1),
        None,
        false,
        None,
        false,
    )?;

    let outcome = merge_model_normals(&mut left, &mut opposite, ModelTranslation::ZERO, false);
    assert_eq!(outcome.matched_vertex_pairs(), 3);

    let control_lit = light_model_data(&control, LightingParameters::for_loc(0, 0))?;
    let merged_lit = light_model_data(&left, LightingParameters::for_loc(0, 0))?;
    assert_eq!(control_lit.face_colors[0].a, 4_638);
    assert_eq!(merged_lit.face_colors[0].a, 4_634);
    assert_ne!(control_lit.face_colors, merged_lit.face_colors);
    Ok(())
}

#[test]
fn loc_ambient_and_contrast_extremes_keep_signed_integer_semantics() -> Result<(), Box<dyn Error>> {
    let model = working_triangle(
        9_301,
        BASE_TRIANGLE,
        Triangle::new(0, 1, 2),
        None,
        false,
        None,
        false,
    )?;

    let minimum = light_model_data(&model, LightingParameters::for_loc(-128, -3_200))?;
    let maximum = light_model_data(&model, LightingParameters::for_loc(127, 3_175))?;

    assert_eq!(minimum.face_colors[0].a, 4_610);
    assert_eq!(maximum.face_colors[0].a, 4_686);
    Ok(())
}

#[test]
fn alpha_sentinels_override_authored_render_type_during_lighting() -> Result<(), Box<dyn Error>> {
    let hidden = working_triangle(
        9_401,
        BASE_TRIANGLE,
        Triangle::new(0, 1, 2),
        None,
        false,
        Some(-1),
        false,
    )?;
    let hidden_lit = light_model_data(&hidden, LightingParameters::for_loc(0, 0))?;
    assert_eq!(
        hidden_lit.face_colors[0],
        LitFaceColors { a: 0, b: 0, c: -2 }
    );

    let constant = working_triangle(
        9_402,
        BASE_TRIANGLE,
        Triangle::new(0, 1, 2),
        None,
        false,
        Some(-2),
        false,
    )?;
    let constant_lit = light_model_data(&constant, LightingParameters::for_loc(0, 0))?;
    assert_eq!(
        constant_lit.face_colors[0],
        LitFaceColors {
            a: 128,
            b: 0,
            c: -1
        }
    );
    Ok(())
}

fn working_triangle(
    model_id: u32,
    vertices: [ModelPoint; 3],
    face: Triangle,
    render_type: Option<i8>,
    textured: bool,
    alpha: Option<i8>,
    mapped_texture: bool,
) -> Result<WorkingModel, Box<dyn Error>> {
    let texture_triangles = if mapped_texture {
        vec![TextureTriangle {
            render_type: 0,
            vertices: Triangle::new(0, 1, 2),
            mapping: TextureMappingParameters::default(),
        }]
    } else {
        Vec::new()
    };
    let texture_face_selectors = if mapped_texture {
        Some(vec![Some(TextureTriangleIndex::new(0))])
    } else {
        None
    };

    Ok(SourceModel::from_parts(SourceModelParts {
        identity: DefinitionIdentity::new(ModelId::new(model_id), provenance()?),
        format: ModelFormatIdentity {
            encoding: ModelEncoding::TrailerFfFd,
            version: None,
        },
        vertices: vertices.to_vec(),
        faces: vec![face],
        face_colors: vec![0x1234],
        default_priority: FacePriority::ZERO,
        face_render_types: render_type.map(|value| vec![value]),
        face_priorities: None,
        face_alphas: alpha.map(|value| vec![value]),
        face_textures: textured.then(|| vec![Some(TextureId::new(7))]),
        texture_face_selectors,
        face_biases: None,
        texture_triangles,
        vertex_skins: None,
        face_skins: None,
        skeletal_vertices: None,
    })?
    .to_working_copy())
}

fn provenance() -> Result<TargetProvenance, Box<dyn Error>> {
    Ok(TargetProvenance::new(
        "osrs-live-build-241",
        ProfileDigest::from_lower_hex(PROFILE_DIGEST)?,
        CacheFingerprint::from_lower_hex(CACHE_FINGERPRINT)?,
        1,
    )?)
}
