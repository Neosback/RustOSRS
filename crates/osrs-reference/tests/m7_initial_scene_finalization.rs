use osrs_core::{
    coords::{ModelPoint, SceneTile, StoragePlane},
    definitions::{
        DefinitionIdentity, LocType, ModelScale, ModelTranslation, ObjectDefinition, ObjectModels,
        ObjectPlacementFlags, TypedObjectModel,
    },
    ids::{ModelId, ObjectId},
    lighting::light_model_data,
    model::{
        FacePriority, ModelEncoding, ModelFormatIdentity, SourceModel, SourceModelParts, Triangle,
    },
    provenance::{CacheFingerprint, ProfileDigest, TargetProvenance},
    static_entity::{InitialStaticEntity, InitialStaticEntityCache, SceneLocalModelDataEntity},
};
use osrs_scene::{SceneReferenceFinalizationError, SceneReferenceFinalizer};
use std::error::Error;

const PROFILE_DIGEST: &str = "cfdefa9ef99eff799fcef4fdf0ec78d9fdcd72d8e5be78e1c154d018ab4575b7";
const CACHE_FINGERPRINT: &str = "ae76dad78b4990d1b404e68e77a85ed2c96cf4a56c16f7b017cb97d1e92fdb38";
const BASE_TRIANGLE: [ModelPoint; 3] = [
    ModelPoint::new(0, 0, 0),
    ModelPoint::new(128, 0, 0),
    ModelPoint::new(0, 0, 128),
];
const EAST_SHIFTED_TRIANGLE: [ModelPoint; 3] = [
    ModelPoint::new(128, 0, 0),
    ModelPoint::new(256, 0, 0),
    ModelPoint::new(128, 0, 128),
];

#[test]
fn initial_non_flat_floor_neighbors_reconcile_before_final_lighting()
-> Result<(), Box<dyn Error>> {
    let west_source = source_model(10_001, EAST_SHIFTED_TRIANGLE, 0x1234)?;
    let east_source = source_model(10_002, BASE_TRIANGLE, 0x1234)?;
    let west_snapshot = west_source.clone();
    let east_snapshot = east_source.clone();

    let west = initial_non_flat_entity(1_001, &west_source, 0, 0)?;
    let east = initial_non_flat_entity(1_002, &east_source, 0, 0)?;

    let mut scene = SceneReferenceFinalizer::new(2, 1, 1)?;
    let west_id = scene.add_initial_model_data(west);
    let east_id = scene.add_initial_model_data(east);
    scene.set_floor_decoration(plane(0), SceneTile::new(0, 0), west_id)?;
    scene.set_floor_decoration(plane(0), SceneTile::new(1, 0), east_id)?;

    assert_eq!(scene.lit_model(west_id), None);
    assert_eq!(scene.lit_model(east_id), None);
    assert_eq!(scene.model_data().is_pending_model_data(west_id), Some(true));
    assert_eq!(scene.model_data().is_pending_model_data(east_id), Some(true));

    let report = scene.reconcile_and_light()?;

    assert_eq!(report.merge_calls(), 1);
    assert_eq!(report.matched_vertex_pairs(), 3);
    assert_eq!(report.hidden_faces(), 2);
    assert_eq!(report.closed_models(), 2);
    assert_eq!(scene.model_data().is_pending_model_data(west_id), Some(false));
    assert_eq!(scene.model_data().is_pending_model_data(east_id), Some(false));
    assert_eq!(scene.lit_model(west_id).ok_or("west model was not lit")?.face_colors[0].c, -2);
    assert_eq!(scene.lit_model(east_id).ok_or("east model was not lit")?.face_colors[0].c, -2);
    assert_eq!(west_source, west_snapshot);
    assert_eq!(east_source, east_snapshot);
    Ok(())
}

#[test]
fn retained_loc_lighting_parameters_are_used_after_reconciliation()
-> Result<(), Box<dyn Error>> {
    let source = source_model(10_101, BASE_TRIANGLE, 0x2345)?;
    let entity = initial_non_flat_entity(1_101, &source, 7, -11)?;
    let expected = light_model_data(entity.model(), entity.lighting())?;

    let mut scene = SceneReferenceFinalizer::new(1, 1, 1)?;
    let id = scene.add_initial_model_data(entity);
    scene.set_floor_decoration(plane(0), SceneTile::new(0, 0), id)?;

    let report = scene.reconcile_and_light()?;

    assert_eq!(report.matched_vertex_pairs(), 0);
    assert_eq!(report.closed_models(), 1);
    assert_eq!(scene.lit_model(id), Some(&expected));
    Ok(())
}

#[test]
fn unplaced_initial_model_data_is_not_prematurely_lit() -> Result<(), Box<dyn Error>> {
    let source = source_model(10_201, BASE_TRIANGLE, 0x3456)?;
    let entity = initial_non_flat_entity(1_201, &source, 0, 0)?;
    let mut scene = SceneReferenceFinalizer::new(1, 1, 1)?;
    let id = scene.add_initial_model_data(entity);

    let error = scene
        .reconcile_and_light()
        .expect_err("unplaced ModelData must remain pending");

    assert_eq!(error, SceneReferenceFinalizationError::ModelStillPending(id.index()));
    assert_eq!(scene.lit_model(id), None);
    assert_eq!(scene.model_data().is_pending_model_data(id), Some(true));
    Ok(())
}

fn initial_non_flat_entity(
    object_id: u32,
    source: &SourceModel,
    ambient: i16,
    contrast: i16,
) -> Result<SceneLocalModelDataEntity, Box<dyn Error>> {
    let definition = object_definition(object_id, source.identity().id, ambient, contrast)?;
    let mut cache = InitialStaticEntityCache::new();
    let entity = cache
        .get_or_build(&definition, LocType::new(10), 0, || {
            Some(source.to_working_copy())
        })?
        .ok_or("initial non-flat entity unexpectedly absent")?;
    let InitialStaticEntity::ModelData(model_data) = entity else {
        return Err("initial non-flat entity was prematurely lit".into());
    };
    Ok(model_data)
}

fn object_definition(
    object_id: u32,
    model_id: ModelId,
    ambient: i16,
    contrast: i16,
) -> Result<ObjectDefinition, Box<dyn Error>> {
    Ok(ObjectDefinition {
        identity: DefinitionIdentity::new(ObjectId::new(object_id), provenance()?),
        name: Some("M7 scene finalization fixture".to_owned()),
        models: Some(ObjectModels::Typed(vec![TypedObjectModel {
            loc_type: LocType::new(10),
            model_id,
        }])),
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

fn source_model(
    model_id: u32,
    vertices: [ModelPoint; 3],
    color: u16,
) -> Result<SourceModel, Box<dyn Error>> {
    Ok(SourceModel::from_parts(SourceModelParts {
        identity: DefinitionIdentity::new(ModelId::new(model_id), provenance()?),
        format: ModelFormatIdentity {
            encoding: ModelEncoding::TrailerFfFd,
            version: None,
        },
        vertices: vertices.to_vec(),
        faces: vec![Triangle::new(0, 1, 2)],
        face_colors: vec![color],
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

fn plane(value: u8) -> StoragePlane {
    let Some(plane) = StoragePlane::new(value) else {
        unreachable!("test uses only valid storage planes");
    };
    plane
}

fn provenance() -> Result<TargetProvenance, Box<dyn Error>> {
    Ok(TargetProvenance::new(
        "osrs-live-build-241",
        ProfileDigest::from_lower_hex(PROFILE_DIGEST)?,
        CacheFingerprint::from_lower_hex(CACHE_FINGERPRINT)?,
        1,
    )?)
}
