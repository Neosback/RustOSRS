use osrs_cache::{capability::TargetCapability, profile::TargetProfile};
use osrs_core::{
    coords::{LocalCoord, LocalPoint, MapTile, SceneTile, SourcePlane, StoragePlane},
    definitions::{LocType, ObjectPlacementFlags},
    ids::ObjectId,
};
use osrs_scene::{
    CollisionSideEffect, DefinitionSideEffectInputs, FlatTerrainSurface, Footprint, PlacementInput,
    SceneGrid, SemanticTile, ShapedTerrainInput, ShapedTerrainSurface, TerrainCorners,
    TerrainSurface, plan_placement, plan_side_effects, semantic_scene_hash_hex_v1,
};
use std::{error::Error, io};

const TARGET_PROFILE_YAML: &str =
    include_str!("../../../profiles/osrs-live-241-2026-09-30-openrs2-2727.yaml");
const M9_COMPOSED_SCENE_HASH_V1: &str =
    "50974f0232192dbd97c623d31bdbe208432ef64a13121c2852230afcc262f4a6";
const M9_BRIDGE_SCENE_HASH_V1: &str =
    "bf7a7876864142f75a29c47836f331da6cfb97b512d500274fd3c8035c3a296c";

fn storage_plane(value: u8) -> Result<StoragePlane, io::Error> {
    StoragePlane::new(value)
        .ok_or_else(|| io::Error::other(format!("storage plane {value} must be valid")))
}

fn source_plane(value: u8) -> Result<SourcePlane, io::Error> {
    SourcePlane::new(value)
        .ok_or_else(|| io::Error::other(format!("source plane {value} must be valid")))
}

fn build_m9_composed_scene() -> Result<SceneGrid, Box<dyn Error>> {
    let plane = storage_plane(0)?;
    let mut scene = SceneGrid::new(3, 3, 1)?;
    scene.set_terrain(
        plane,
        SceneTile::new(0, 0),
        TerrainSurface::Flat(FlatTerrainSurface::new(
            TerrainCorners::new(1, 2, 3, 4),
            TerrainCorners::new(10, 20, 30, 40),
            Some(7),
        )),
    )?;

    let placements = [
        (
            ObjectId::new(1001),
            PlacementInput {
                loc_type: LocType::new(22),
                orientation: 0,
                tile: SceneTile::new(0, 1),
                size_x: 1,
                size_y: 1,
                sampled_height: 11,
                existing_wall_displacement: None,
            },
        ),
        (
            ObjectId::new(1002),
            PlacementInput {
                loc_type: LocType::new(2),
                orientation: 3,
                tile: SceneTile::new(1, 1),
                size_x: 1,
                size_y: 1,
                sampled_height: 22,
                existing_wall_displacement: None,
            },
        ),
        (
            ObjectId::new(1003),
            PlacementInput {
                loc_type: LocType::new(6),
                orientation: 1,
                tile: SceneTile::new(2, 1),
                size_x: 1,
                size_y: 1,
                sampled_height: 33,
                existing_wall_displacement: Some(34),
            },
        ),
        (
            ObjectId::new(1004),
            PlacementInput {
                loc_type: LocType::new(10),
                orientation: 0,
                tile: SceneTile::new(2, 2),
                size_x: 1,
                size_y: 1,
                sampled_height: 44,
                existing_wall_displacement: None,
            },
        ),
    ];

    for (object_id, input) in placements {
        assert!(scene.insert_placement(plane, object_id, plan_placement(input)?)?);
    }
    Ok(scene)
}

fn build_m9_bridge_scene() -> Result<SceneGrid, Box<dyn Error>> {
    let tile = SceneTile::new(0, 0);
    let mut scene = SceneGrid::new(1, 1, 4)?;
    for plane_index in 0_u8..4 {
        let storage = storage_plane(plane_index)?;
        scene.set_tile(
            storage,
            tile,
            SemanticTile::new(Some(source_plane(plane_index)?), storage),
        )?;
    }
    scene.set_link_below(tile)?;
    Ok(scene)
}

#[test]
fn coord_001_spans_world_local_footprint_and_terrain_positions() -> Result<(), Box<dyn Error>> {
    let world = MapTile::new(-1, 64);
    let (region, within) = world.split_region();
    assert_eq!((region.x, region.y), (-1, 1));
    assert_eq!((within.x(), within.y()), (63, 0));
    assert_eq!(MapTile::from_region(region, within), Some(world));

    let local_origin = world
        .local_origin()
        .ok_or_else(|| io::Error::other("world tile must fit semantic local coordinates"))?;
    assert_eq!(local_origin.x, LocalCoord::from_units(-128));
    assert_eq!(local_origin.z, LocalCoord::from_units(8_192));

    let placement = plan_placement(PlacementInput {
        loc_type: LocType::new(10),
        orientation: 1,
        tile: SceneTile::new(10, 20),
        size_x: 2,
        size_y: 3,
        sampled_height: 75,
        existing_wall_displacement: None,
    })?;
    assert_eq!(placement.rotated_definition_footprint, Footprint::new(3, 2));
    assert_eq!(
        placement.model_center,
        LocalPoint::new(
            LocalCoord::from_units(1_472),
            LocalCoord::from_units(75),
            LocalCoord::from_units(2_688),
        )
    );
    assert_eq!(placement.storage_center, placement.model_center);

    let terrain = ShapedTerrainSurface::build(ShapedTerrainInput {
        shape: 0,
        rotation: 0,
        texture_id: None,
        tile: SceneTile::new(10, 20),
        heights: TerrainCorners::new(1, 2, 3, 4),
        underlay_colors: TerrainCorners::new(10, 20, 30, 40),
        overlay_colors: TerrainCorners::new(50, 60, 70, 80),
        underlay_rgb: 0,
        overlay_rgb: 0,
    })?;
    let positions: Vec<_> = terrain
        .vertices
        .iter()
        .map(|vertex| vertex.position)
        .collect();
    assert_eq!(
        positions,
        vec![
            LocalPoint::new(
                LocalCoord::from_units(1_280),
                LocalCoord::from_units(1),
                LocalCoord::from_units(2_560),
            ),
            LocalPoint::new(
                LocalCoord::from_units(1_408),
                LocalCoord::from_units(2),
                LocalCoord::from_units(2_560),
            ),
            LocalPoint::new(
                LocalCoord::from_units(1_408),
                LocalCoord::from_units(3),
                LocalCoord::from_units(2_688),
            ),
            LocalPoint::new(
                LocalCoord::from_units(1_280),
                LocalCoord::from_units(4),
                LocalCoord::from_units(2_688),
            ),
        ]
    );
    Ok(())
}

#[test]
fn loc_placement_005_preserves_deterministic_rebuild_inputs() -> Result<(), Box<dyn Error>> {
    let boundary = plan_placement(PlacementInput {
        loc_type: LocType::new(2),
        orientation: 3,
        tile: SceneTile::new(10, 20),
        size_x: 1,
        size_y: 1,
        sampled_height: 0,
        existing_wall_displacement: None,
    })?;
    let definition = DefinitionSideEffectInputs::new(
        ObjectPlacementFlags {
            interact_type: 2,
            blocks_projectiles: true,
            clipped: false,
            model_clipped: true,
            obstructs_ground: true,
            solid: true,
        },
        37,
    );

    let first = plan_side_effects(definition, boundary);
    let second = plan_side_effects(definition, boundary);
    assert_eq!(first, second);
    assert_eq!(first.definition, definition);
    assert_eq!(
        first.collision,
        CollisionSideEffect::Boundary {
            loc_type: LocType::new(2),
            orientation: 3,
            blocks_projectiles: true,
        }
    );
    assert_eq!(first.wall_displacement, Some(37));

    let game_object = plan_placement(PlacementInput {
        loc_type: LocType::new(9),
        orientation: 1,
        tile: SceneTile::new(30, 40),
        size_x: 2,
        size_y: 3,
        sampled_height: 0,
        existing_wall_displacement: None,
    })?;
    let game_plan = plan_side_effects(definition, game_object);
    assert_eq!(
        game_plan.collision,
        CollisionSideEffect::GameObject {
            footprint: Footprint::new(3, 2),
            blocks_projectiles: true,
        }
    );
    assert_eq!(game_plan.definition, definition);
    assert_eq!(game_plan.wall_displacement, Some(37));
    Ok(())
}

#[test]
fn target_profile_exposes_source_verified_terrain_color_builder_capability()
-> Result<(), Box<dyn Error>> {
    let profile = TargetProfile::from_yaml_str(TARGET_PROFILE_YAML)?;
    let terrain = profile
        .capability_diagnostic(TargetCapability::TerrainColorBuilder)
        .ok_or_else(|| io::Error::other("terrain color capability gate must exist"))?;

    assert!(!terrain.is_blocked());
    assert_eq!(terrain.gate_name, "terrain_color_builder");
    assert_eq!(terrain.state, "source_verified");
    assert_eq!(terrain.spec, Some("TERRAIN-004"));
    assert!(
        terrain
            .note
            .is_some_and(|note| note.contains("class470.method9712"))
    );

    let blocked: Vec<_> = profile
        .blocked_capabilities()
        .map(|diagnostic| diagnostic.capability)
        .collect();
    assert!(blocked.is_empty());

    let extended_ids = profile
        .capability_diagnostic(TargetCapability::ExtendedObjectModelIds)
        .ok_or_else(|| io::Error::other("extended model id capability gate must exist"))?;
    assert_eq!(extended_ids.state, "required");
    assert!(!extended_ids.is_blocked());
    Ok(())
}

#[test]
fn semantic_golden_scene_hashes_are_exact_and_rebuild_deterministically()
-> Result<(), Box<dyn Error>> {
    let first = build_m9_composed_scene()?;
    let rebuilt = build_m9_composed_scene()?;
    assert_eq!(first, rebuilt);
    assert_eq!(
        semantic_scene_hash_hex_v1(&first),
        M9_COMPOSED_SCENE_HASH_V1
    );
    assert_eq!(
        semantic_scene_hash_hex_v1(&rebuilt),
        M9_COMPOSED_SCENE_HASH_V1
    );

    let bridge = build_m9_bridge_scene()?;
    let rebuilt_bridge = build_m9_bridge_scene()?;
    assert_eq!(bridge, rebuilt_bridge);
    assert_eq!(semantic_scene_hash_hex_v1(&bridge), M9_BRIDGE_SCENE_HASH_V1);
    assert_eq!(
        semantic_scene_hash_hex_v1(&rebuilt_bridge),
        M9_BRIDGE_SCENE_HASH_V1
    );
    Ok(())
}
