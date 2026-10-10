use osrs_core::{
    coords::ModelPoint,
    definitions::{
        DefinitionIdentity, LocType, ModelScale, ModelTranslation, ObjectDefinition,
        ObjectPlacementFlags,
    },
    ids::{ModelId, ObjectId},
    lighting::{LightingParameters, light_model_data},
    model::{
        FacePriority, ModelEncoding, ModelFormatIdentity, SourceModel, SourceModelParts, Triangle,
    },
    model_identity::ModelSemanticIdentity,
    provenance::{CacheFingerprint, ProfileDigest, TargetProvenance},
    static_entity::{InitialStaticEntity, InitialStaticEntityCache},
};
use std::error::Error;

const PROFILE_DIGEST: &str = "cfdefa9ef99eff799fcef4fdf0ec78d9fdcd72d8e5be78e1c154d018ab4575b7";
const CACHE_FINGERPRINT: &str =
    "ae76dad78b4990d1b404e68e77a85ed2c96cf4a56c16f7b017cb97d1e92fdb38";

#[test]
fn working_and_lit_models_retain_exact_single_source_identity() -> Result<(), Box<dyn Error>> {
    let source = source_model(8_801)?;
    let expected_identity = ModelSemanticIdentity::from_source(&source);
    let working = source.to_working_copy();

    assert_eq!(working.identity(), &expected_identity);
    assert_eq!(working.format(), Some(source.format()));

    let lit = light_model_data(&working, LightingParameters::for_loc(0, 0))?;
    assert_eq!(lit.identity, expected_identity);
    assert_eq!(lit.format, Some(source.format()));
    Ok(())
}

#[test]
fn flat_and_nonflat_entity_cache_paths_preserve_semantic_identity() -> Result<(), Box<dyn Error>> {
    let source = source_model(8_802)?;
    let expected_identity = ModelSemanticIdentity::from_source(&source);
    let flat_definition = object_definition(802, false)?;
    let nonflat_definition = object_definition(803, true)?;
    let mut flat_cache = InitialStaticEntityCache::new();
    let mut nonflat_cache = InitialStaticEntityCache::new();

    let flat = flat_cache
        .get_or_build(&flat_definition, LocType::new(10), 0, || {
            Some(source.to_working_copy())
        })?
        .ok_or("flat identity fixture unexpectedly absent")?;
    let InitialStaticEntity::Lit(flat) = flat else {
        return Err("flat identity fixture was not lit".into());
    };
    assert_eq!(flat.identity, expected_identity);
    assert_eq!(flat.format, Some(source.format()));

    let nonflat = nonflat_cache
        .get_or_build(&nonflat_definition, LocType::new(10), 0, || {
            Some(source.to_working_copy())
        })?
        .ok_or("non-flat identity fixture unexpectedly absent")?;
    let InitialStaticEntity::ModelData(nonflat) = nonflat else {
        return Err("non-flat identity fixture was prematurely lit".into());
    };
    assert_eq!(nonflat.model().identity(), &expected_identity);
    assert_eq!(nonflat.model().format(), Some(source.format()));
    Ok(())
}

fn source_model(model_id: u32) -> Result<SourceModel, Box<dyn Error>> {
    Ok(SourceModel::from_parts(SourceModelParts {
        identity: DefinitionIdentity::new(ModelId::new(model_id), provenance()?),
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

fn object_definition(
    object_id: u32,
    non_flat_shading: bool,
) -> Result<ObjectDefinition, Box<dyn Error>> {
    Ok(ObjectDefinition {
        identity: DefinitionIdentity::new(ObjectId::new(object_id), provenance()?),
        name: Some("M7 identity threading fixture".to_owned()),
        models: None,
        size_x: 1,
        size_y: 1,
        placement: ObjectPlacementFlags {
            interact_type: 2,
            blocks_projectiles: true,
            clipped: true,
            model_clipped: false,
            obstructs_ground: false,
            solid: false,
        },
        decoration_displacement: 16,
        support_items: None,
        is_rotated: false,
        non_flat_shading,
        contour_clip: None,
        animation: None,
        ambient: 0,
        contrast: 0,
        scale: ModelScale::IDENTITY,
        translation: ModelTranslation::ZERO,
        recolors: Vec::new(),
        retextures: Vec::new(),
        morphs: None,
        map_scene: None,
        map_icon: None,
        category: None,
        actions: [None, None, None, None, None],
    })
}

fn provenance() -> Result<TargetProvenance, Box<dyn Error>> {
    Ok(TargetProvenance::new(
        "osrs-live-build-241",
        ProfileDigest::from_lower_hex(PROFILE_DIGEST)?,
        CacheFingerprint::from_lower_hex(CACHE_FINGERPRINT)?,
        1,
    )?)
}
