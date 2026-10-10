use osrs_core::{
    coords::{ModelPoint, SceneTile, StoragePlane},
    definitions::DefinitionIdentity,
    ids::ModelId,
    model::{
        FacePriority, ModelEncoding, ModelFormatIdentity, ModelNormalState, SourceModel,
        SourceModelParts, Triangle, VertexNormal, WorkingModel,
    },
    provenance::{CacheFingerprint, ProfileDigest, TargetProvenance},
};
use osrs_scene::{Footprint, SceneModelDataGrid};
use std::error::Error;

const PROFILE_DIGEST: &str = "cfdefa9ef99eff799fcef4fdf0ec78d9fdcd72d8e5be78e1c154d018ab4575b7";
const CACHE_FINGERPRINT: &str = "ae76dad78b4990d1b404e68e77a85ed2c96cf4a56c16f7b017cb97d1e92fdb38";
const BASE_TRIANGLE: [ModelPoint; 3] = [
    ModelPoint::new(0, 0, 0),
    ModelPoint::new(128, 0, 0),
    ModelPoint::new(0, 0, 128),
];

#[test]
fn dual_arm_boundary_reconciles_arms_after_neighbor_scan() -> Result<(), Box<dyn Error>> {
    let mut scene = SceneModelDataGrid::new(3, 3, 1)?;
    let primary = scene.add_model(working_triangle(8_001, BASE_TRIANGLE)?);
    let secondary = scene.add_model(working_triangle(8_002, BASE_TRIANGLE)?);
    scene.set_boundary(plane(0), SceneTile::new(1, 1), primary, Some(secondary))?;

    let report = scene.reconcile_normals()?;

    assert_eq!(report.merge_calls(), 1);
    assert_eq!(report.matched_vertex_pairs(), 3);
    assert_eq!(report.hidden_faces(), 0);
    assert_eq!(report.closed_models(), 2);
    assert_eq!(merged_at(model(&scene, primary)?, 0).magnitude, 2);
    assert_eq!(merged_at(model(&scene, secondary)?, 0).magnitude, 2);
    assert_eq!(model(&scene, primary)?.face_render_types(), None);
    assert_eq!(model(&scene, secondary)?.face_render_types(), None);
    assert_eq!(scene.is_pending_model_data(primary), Some(false));
    assert_eq!(scene.is_pending_model_data(secondary), Some(false));
    Ok(())
}

#[test]
fn boundary_to_game_neighbor_uses_exact_footprint_translation() -> Result<(), Box<dyn Error>> {
    let mut scene = SceneModelDataGrid::new(4, 3, 1)?;
    let boundary = scene.add_model(working_triangle(
        8_101,
        [
            ModelPoint::new(128, 0, 0),
            ModelPoint::new(256, 0, 0),
            ModelPoint::new(128, 0, 128),
        ],
    )?);
    let game = scene.add_model(working_triangle(8_102, BASE_TRIANGLE)?);
    scene.set_boundary(plane(0), SceneTile::new(1, 1), boundary, None)?;
    scene.insert_game_object(plane(0), SceneTile::new(2, 1), Footprint::ONE_BY_ONE, game)?;

    let report = scene.reconcile_normals()?;

    assert_eq!(report.matched_vertex_pairs(), 3);
    assert_eq!(merged_at(model(&scene, boundary)?, 0).magnitude, 2);
    assert_eq!(merged_at(model(&scene, game)?, 0).magnitude, 2);
    assert_eq!(model(&scene, boundary)?.face_render_types(), Some(&[2][..]));
    assert_eq!(model(&scene, game)?.face_render_types(), Some(&[2][..]));
    Ok(())
}

#[test]
fn floor_decoration_neighbor_path_hides_fully_matched_faces() -> Result<(), Box<dyn Error>> {
    let mut scene = SceneModelDataGrid::new(4, 3, 1)?;
    let west = scene.add_model(working_triangle(
        8_201,
        [
            ModelPoint::new(128, 0, 0),
            ModelPoint::new(256, 0, 0),
            ModelPoint::new(128, 0, 128),
        ],
    )?);
    let east = scene.add_model(working_triangle(8_202, BASE_TRIANGLE)?);
    scene.set_floor_decoration(plane(0), SceneTile::new(1, 1), west)?;
    scene.set_floor_decoration(plane(0), SceneTile::new(2, 1), east)?;

    let report = scene.reconcile_normals()?;

    assert_eq!(report.matched_vertex_pairs(), 3);
    assert_eq!(report.hidden_faces(), 2);
    assert_eq!(model(&scene, west)?.face_render_types(), Some(&[2][..]));
    assert_eq!(model(&scene, east)?.face_render_types(), Some(&[2][..]));
    Ok(())
}

#[test]
fn plane_above_neighbor_uses_average_height_delta_without_face_hiding() -> Result<(), Box<dyn Error>>
{
    let mut scene = SceneModelDataGrid::new(3, 3, 2)?;
    for x in 0..=scene.width() {
        for y in 0..=scene.height() {
            scene.set_height_corner(plane(1), x, y, 32)?;
        }
    }

    let below = scene.add_model(working_triangle(
        8_301,
        [
            ModelPoint::new(0, 32, 0),
            ModelPoint::new(128, 32, 0),
            ModelPoint::new(0, 32, 128),
        ],
    )?);
    let above = scene.add_model(working_triangle(8_302, BASE_TRIANGLE)?);
    scene.set_boundary(plane(0), SceneTile::new(1, 1), below, None)?;
    scene.set_boundary(plane(1), SceneTile::new(1, 1), above, None)?;

    let report = scene.reconcile_normals()?;

    assert_eq!(report.matched_vertex_pairs(), 3);
    assert_eq!(report.hidden_faces(), 0);
    assert_eq!(merged_at(model(&scene, below)?, 0).magnitude, 2);
    assert_eq!(merged_at(model(&scene, above)?, 0).magnitude, 2);
    assert_eq!(model(&scene, below)?.face_render_types(), None);
    assert_eq!(model(&scene, above)?.face_render_types(), None);
    Ok(())
}

fn plane(value: u8) -> StoragePlane {
    let Some(plane) = StoragePlane::new(value) else {
        unreachable!("test uses only valid storage planes");
    };
    plane
}

fn model(
    scene: &SceneModelDataGrid,
    id: osrs_scene::SceneModelDataId,
) -> Result<&WorkingModel, Box<dyn Error>> {
    scene
        .model(id)
        .ok_or_else(|| std::io::Error::other("missing scene model").into())
}

fn merged_at(model: &WorkingModel, vertex: usize) -> VertexNormal {
    let ModelNormalState::Computed(normals) = model.normal_state() else {
        unreachable!("scene normal pass must compute normal state");
    };
    normals
        .merged_vertex_normals
        .as_ref()
        .and_then(|values| values[vertex])
        .unwrap_or_default()
}

fn working_triangle(
    model_id: u32,
    vertices: [ModelPoint; 3],
) -> Result<WorkingModel, Box<dyn Error>> {
    Ok(SourceModel::from_parts(SourceModelParts {
        identity: DefinitionIdentity::new(ModelId::new(model_id), provenance()?),
        format: ModelFormatIdentity {
            encoding: ModelEncoding::TrailerFfFd,
            version: None,
        },
        vertices: vertices.to_vec(),
        faces: vec![Triangle::new(0, 1, 2)],
        face_colors: vec![0],
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
