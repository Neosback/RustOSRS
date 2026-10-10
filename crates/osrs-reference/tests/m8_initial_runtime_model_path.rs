use osrs_core::{
    coords::ModelPoint,
    definitions::{
        DefinitionIdentity, LocType, ModelScale, ModelTranslation, ObjectDefinition, ObjectModels,
        ObjectPlacementFlags,
    },
    dynamic_model::DynamicModelCache,
    ids::{ModelId, ObjectId},
    model::{
        FacePriority, ModelEncoding, ModelFormatIdentity, SourceModel, SourceModelParts, Triangle,
    },
    provenance::{CacheFingerprint, ProfileDigest, TargetProvenance},
    static_entity::{InitialStaticEntity, InitialStaticEntityCache},
};
use std::{borrow::Cow, error::Error};

const PROFILE_DIGEST: &str = "cfdefa9ef99eff799fcef4fdf0ec78d9fdcd72d8e5be78e1c154d018ab4575b7";
const CACHE_FINGERPRINT: &str = "ae76dad78b4990d1b404e68e77a85ed2c96cf4a56c16f7b017cb97d1e92fdb38";

#[test]
fn same_non_flat_definition_uses_model_data_initially_and_lit_runtime_model()
-> Result<(), Box<dyn Error>> {
    let source = source_model()?;
    let source_snapshot = source.clone();
    let definition = object_definition()?;
    let loc_type = LocType::new(10);
    let orientation = 1;

    let mut initial_cache = InitialStaticEntityCache::new();
    let initial = initial_cache
        .get_or_build(&definition, loc_type, orientation, || {
            Some(source.to_working_copy())
        })?
        .ok_or("initial non-flat entity unexpectedly absent")?;

    let InitialStaticEntity::ModelData(mut initial_model_data) = initial else {
        return Err("initial non-flat path did not retain ModelData".into());
    };
    initial_model_data.model_mut().vertices_mut()[0].x = 9_999;

    let mut runtime_cache = DynamicModelCache::new();
    let runtime = runtime_cache
        .get_or_build_legacy(&definition, loc_type, orientation, None, None, || {
            Some(source.to_working_copy())
        })?
        .ok_or("runtime dynamic model unexpectedly absent")?;

    let Cow::Borrowed(runtime_lit) = runtime else {
        return Err(
            "runtime path without pose or contour did not borrow its lit cached base".into(),
        );
    };

    assert_eq!(runtime_cache.len(), 1);
    assert_eq!(runtime_lit.vertices[0], ModelPoint::new(0, 0, 0));
    assert_ne!(
        initial_model_data.model().vertices()[0],
        runtime_lit.vertices[0]
    );
    assert_eq!(
        source, source_snapshot,
        "shared raw source must remain immutable"
    );
    Ok(())
}

fn source_model() -> Result<SourceModel, Box<dyn Error>> {
    Ok(SourceModel::from_parts(SourceModelParts {
        identity: DefinitionIdentity::new(ModelId::new(9_801), provenance()?),
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

fn object_definition() -> Result<ObjectDefinition, Box<dyn Error>> {
    Ok(ObjectDefinition {
        identity: DefinitionIdentity::new(ObjectId::new(980), provenance()?),
        name: Some("M8 initial/runtime path fixture".to_owned()),
        models: Some(ObjectModels::Untyped(vec![ModelId::new(9_801)])),
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
        non_flat_shading: true,
        contour_clip: None,
        animation: None,
        ambient: 3,
        contrast: -4,
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
        "osrs-live-241-2026-09-30-openrs2-2727",
        ProfileDigest::from_lower_hex(PROFILE_DIGEST)?,
        CacheFingerprint::from_lower_hex(CACHE_FINGERPRINT)?,
        1,
    )?)
}
