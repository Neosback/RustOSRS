use osrs_core::{
    coords::ModelPoint,
    definitions::DefinitionIdentity,
    ids::ModelId,
    lighting::{LightingParameters, light_model_data},
    model::{
        FacePriority, ModelEncoding, ModelFormatIdentity, SourceModel, SourceModelParts, Triangle,
        WorkingModel,
    },
    model_construction::combine_source_models,
    model_identity::ModelSemanticIdentity,
    provenance::{CacheFingerprint, ProfileDigest, TargetProvenance},
};
use std::error::Error;

const PROFILE_DIGEST: &str = "cfdefa9ef99eff799fcef4fdf0ec78d9fdcd72d8e5be78e1c154d018ab4575b7";
const CACHE_FINGERPRINT: &str = "ae76dad78b4990d1b404e68e77a85ed2c96cf4a56c16f7b017cb97d1e92fdb38";

#[test]
fn real_two_source_assembly_survives_working_and_final_lighting_identity()
-> Result<(), Box<dyn Error>> {
    let first = source_model(
        9_001,
        0,
        ModelFormatIdentity {
            encoding: ModelEncoding::TrailerFfFd,
            version: Some(15),
        },
    )?;
    let second = source_model(
        9_002,
        256,
        ModelFormatIdentity {
            encoding: ModelEncoding::Legacy,
            version: None,
        },
    )?;
    let assembled = combine_source_models(&[&first, &second])?;
    let expected_identity = ModelSemanticIdentity::from_assembled(&assembled)?;

    assert_eq!(expected_identity.source_count(), 2);
    assert!(expected_identity.is_composite());
    assert_eq!(expected_identity.singular_model_id(), None);
    assert_eq!(expected_identity.singular_format(), None);
    assert_eq!(expected_identity.sources()[0].identity(), first.identity());
    assert_eq!(expected_identity.sources()[1].identity(), second.identity());
    assert_eq!(expected_identity.sources()[0].format(), first.format());
    assert_eq!(expected_identity.sources()[1].format(), second.format());

    let working = WorkingModel::from_assembled(&assembled)?;

    assert_eq!(working.identity(), &expected_identity);
    assert_eq!(working.format(), None);
    assert_eq!(working.vertices(), assembled.vertices());
    assert_eq!(working.faces(), assembled.faces());
    assert_eq!(working.face_colors(), assembled.face_colors());
    assert_eq!(working.face_render_types(), assembled.face_render_types());
    assert_eq!(working.face_priorities(), assembled.face_priorities());
    assert_eq!(working.face_alphas(), assembled.face_alphas());
    assert_eq!(working.face_textures(), assembled.face_textures());
    assert_eq!(
        working.texture_face_selectors(),
        assembled.texture_face_selectors()
    );
    assert_eq!(working.face_biases(), assembled.face_biases());
    assert_eq!(working.texture_triangles(), assembled.texture_triangles());
    assert_eq!(working.vertex_skins(), assembled.vertex_skins());
    assert_eq!(working.face_skins(), assembled.face_skins());
    assert_eq!(working.skeletal_vertices(), assembled.skeletal_vertices());

    let lit = light_model_data(&working, LightingParameters::for_loc(0, 0))?;

    assert_eq!(lit.identity, expected_identity);
    assert_eq!(lit.format, None);
    Ok(())
}

fn source_model(
    model_id: u32,
    x_offset: i32,
    format: ModelFormatIdentity,
) -> Result<SourceModel, Box<dyn Error>> {
    Ok(SourceModel::from_parts(SourceModelParts {
        identity: DefinitionIdentity::new(ModelId::new(model_id), provenance()?),
        format,
        vertices: vec![
            ModelPoint::new(x_offset, 0, 0),
            ModelPoint::new(x_offset + 128, 0, 0),
            ModelPoint::new(x_offset, 0, 128),
        ],
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

fn provenance() -> Result<TargetProvenance, Box<dyn Error>> {
    Ok(TargetProvenance::new(
        "osrs-live-build-241",
        ProfileDigest::from_lower_hex(PROFILE_DIGEST)?,
        CacheFingerprint::from_lower_hex(CACHE_FINGERPRINT)?,
        1,
    )?)
}
