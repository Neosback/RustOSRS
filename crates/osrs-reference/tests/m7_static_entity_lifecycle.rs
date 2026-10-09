use osrs_core::{
    coords::ModelPoint,
    definitions::{
        DefinitionIdentity, LocType, ModelScale, ModelTranslation, ObjectDefinition, ObjectModels,
        ObjectPlacementFlags, TypedObjectModel,
    },
    ids::{ModelId, ObjectId},
    lighting::{LightingParameters, LitFaceColors},
    model::{
        FacePriority, ModelEncoding, ModelFormatIdentity, ModelNormalState, SourceModel,
        SourceModelParts, Triangle,
    },
    provenance::{CacheFingerprint, ProfileDigest, TargetProvenance},
    static_entity::{InitialStaticEntity, InitialStaticEntityCache, InitialStaticEntityKey},
};
use std::{cell::Cell, error::Error};

const PROFILE_DIGEST: &str = "cfdefa9ef99eff799fcef4fdf0ec78d9fdcd72d8e5be78e1c154d018ab4575b7";
const CACHE_FINGERPRINT: &str =
    "ae76dad78b4990d1b404e68e77a85ed2c96cf4a56c16f7b017cb97d1e92fdb38";
const BASE_TRIANGLE: [ModelPoint; 3] = [
    ModelPoint::new(0, 0, 0),
    ModelPoint::new(128, 0, 0),
    ModelPoint::new(0, 0, 128),
];

#[test]
fn initial_entity_key_matches_typed_and_untyped_reference_formulas()
-> Result<(), Box<dyn Error>> {
    let untyped = object_definition(
        123,
        Some(ObjectModels::Untyped(vec![ModelId::new(7)])),
        false,
        0,
        0,
    )?;
    let typed = object_definition(
        123,
        Some(ObjectModels::Typed(vec![TypedObjectModel {
            loc_type: LocType::new(4),
            model_id: ModelId::new(7),
        }])),
        false,
        0,
        0,
    )?;
    let absent_models = object_definition(123, None, false, 0, 0)?;

    let untyped_key = InitialStaticEntityKey::for_object(&untyped, LocType::new(10), 6);
    let absent_key = InitialStaticEntityKey::for_object(&absent_models, LocType::new(22), 6);
    let typed_key = InitialStaticEntityKey::for_object(&typed, LocType::new(4), 6);

    assert_eq!(untyped_key.raw(), i64::from((123_i32 << 10) + 6));
    assert_eq!(absent_key.raw(), untyped_key.raw());
    assert_eq!(
        typed_key.raw(),
        i64::from((123_i32 << 10) + (4_i32 << 3) + 6)
    );
    assert_ne!(typed_key, untyped_key);
    Ok(())
}

#[test]
fn flat_initial_entity_is_lit_once_and_reused_from_cache() -> Result<(), Box<dyn Error>> {
    let source = source_model(9_501)?;
    let source_snapshot = source.clone();
    let definition = object_definition(
        500,
        Some(ObjectModels::Typed(vec![TypedObjectModel {
            loc_type: LocType::new(10),
            model_id: ModelId::new(9_501),
        }])),
        false,
        0,
        0,
    )?;
    let mut cache = InitialStaticEntityCache::new();
    let builds = Cell::new(0_u32);

    let first = cache
        .get_or_build(&definition, LocType::new(10), 0, || {
            builds.set(builds.get() + 1);
            Some(source.to_working_copy())
        })?
        .ok_or("flat initial entity unexpectedly absent")?;
    let second = cache
        .get_or_build(&definition, LocType::new(10), 0, || {
            builds.set(builds.get() + 1);
            None
        })?
        .ok_or("cached flat initial entity unexpectedly absent")?;

    assert_eq!(builds.get(), 1);
    assert_eq!(cache.len(), 1);
    assert!(first.is_lit());
    assert!(second.is_lit());
    let InitialStaticEntity::Lit(first) = first else {
        return Err("flat entity did not return a lit model".into());
    };
    let InitialStaticEntity::Lit(second) = second else {
        return Err("flat cache hit did not return a lit model".into());
    };
    assert_eq!(first, second);
    assert_eq!(
        first.face_colors,
        vec![LitFaceColors {
            a: 4_638,
            b: 4_638,
            c: 4_638,
        }]
    );
    assert_eq!(source, source_snapshot);
    Ok(())
}

#[test]
fn non_flat_initial_entity_caches_normalized_model_data_and_returns_scene_local_copy()
-> Result<(), Box<dyn Error>> {
    let source = source_model(9_601)?;
    let source_snapshot = source.clone();
    let definition = object_definition(
        600,
        Some(ObjectModels::Typed(vec![TypedObjectModel {
            loc_type: LocType::new(10),
            model_id: ModelId::new(9_601),
        }])),
        true,
        7,
        -11,
    )?;
    let expected_lighting = LightingParameters::for_loc(7, -11);
    let mut cache = InitialStaticEntityCache::new();
    let builds = Cell::new(0_u32);

    let first = cache
        .get_or_build(&definition, LocType::new(10), 2, || {
            builds.set(builds.get() + 1);
            Some(source.to_working_copy())
        })?
        .ok_or("non-flat initial entity unexpectedly absent")?;

    let InitialStaticEntity::ModelData(mut first) = first else {
        return Err("non-flat entity was prematurely lit".into());
    };
    assert_eq!(first.lighting(), expected_lighting);
    let ModelNormalState::Computed(first_normals) = first.model().normal_state() else {
        return Err("non-flat cached ModelData did not calculate base normals".into());
    };
    assert_eq!(first_normals.merged_vertex_normals, None);
    assert_eq!(first_normals.base_vertex_normals.len(), 3);

    first.model_mut().vertices_mut()[0].x = 9_999;

    let second = cache
        .get_or_build(&definition, LocType::new(10), 2, || {
            builds.set(builds.get() + 1);
            None
        })?
        .ok_or("cached non-flat initial entity unexpectedly absent")?;
    let InitialStaticEntity::ModelData(second) = second else {
        return Err("non-flat cache hit was prematurely lit".into());
    };

    assert_eq!(builds.get(), 1);
    assert_eq!(cache.len(), 1);
    assert_eq!(second.lighting(), expected_lighting);
    assert_eq!(second.model().vertices()[0], BASE_TRIANGLE[0]);
    assert!(matches!(
        second.model().normal_state(),
        ModelNormalState::Computed(_)
    ));
    assert_eq!(source, source_snapshot);
    Ok(())
}

#[test]
fn missing_model_data_remains_absent_and_is_not_cached() -> Result<(), Box<dyn Error>> {
    let definition = object_definition(700, None, false, 0, 0)?;
    let mut cache = InitialStaticEntityCache::new();
    let builds = Cell::new(0_u32);

    for _ in 0..2 {
        let entity = cache.get_or_build(&definition, LocType::new(10), 0, || {
            builds.set(builds.get() + 1);
            None
        })?;
        assert_eq!(entity, None);
    }

    assert_eq!(builds.get(), 2);
    assert!(cache.is_empty());
    Ok(())
}

#[test]
fn flat_and_non_flat_branches_preserve_distinct_semantic_representations()
-> Result<(), Box<dyn Error>> {
    let source = source_model(9_701)?;
    let flat_definition = object_definition(
        701,
        Some(ObjectModels::Untyped(vec![ModelId::new(9_701)])),
        false,
        0,
        0,
    )?;
    let non_flat_definition = ObjectDefinition {
        non_flat_shading: true,
        ..flat_definition.clone()
    };
    let mut flat_cache = InitialStaticEntityCache::new();
    let mut non_flat_cache = InitialStaticEntityCache::new();

    let flat = flat_cache
        .get_or_build(&flat_definition, LocType::new(10), 1, || {
            Some(source.to_working_copy())
        })?
        .ok_or("flat branch unexpectedly absent")?;
    let non_flat = non_flat_cache
        .get_or_build(&non_flat_definition, LocType::new(10), 1, || {
            Some(source.to_working_copy())
        })?
        .ok_or("non-flat branch unexpectedly absent")?;

    assert!(flat.is_lit());
    assert!(non_flat.is_model_data());
    Ok(())
}

fn source_model(model_id: u32) -> Result<SourceModel, Box<dyn Error>> {
    Ok(SourceModel::from_parts(SourceModelParts {
        identity: DefinitionIdentity::new(ModelId::new(model_id), provenance()?),
        format: ModelFormatIdentity {
            encoding: ModelEncoding::TrailerFfFd,
            version: None,
        },
        vertices: BASE_TRIANGLE.to_vec(),
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
    models: Option<ObjectModels>,
    non_flat_shading: bool,
    ambient: i16,
    contrast: i16,
) -> Result<ObjectDefinition, Box<dyn Error>> {
    Ok(ObjectDefinition {
        identity: DefinitionIdentity::new(ObjectId::new(object_id), provenance()?),
        name: Some("M7 static entity fixture".to_owned()),
        models,
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
        ambient,
        contrast,
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
