use osrs_cache::{
    capability::TargetCapability,
    profile::TargetProfile,
};
use osrs_core::{
    coords::{LocalCoord, LocalPoint, MapTile, SceneTile},
    definitions::{LocType, ObjectPlacementFlags},
};
use osrs_scene::{
    CollisionSideEffect, DefinitionSideEffectInputs, Footprint, PlacementInput,
    ShapedTerrainInput, ShapedTerrainSurface, TerrainCorners, plan_placement, plan_side_effects,
};
use std::{error::Error, io};

const TARGET_PROFILE_YAML: &str = include_str!(
    "../../../profiles/osrs-live-241-2026-09-30-openrs2-2727.yaml"
);

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
fn target_profile_exposes_blocked_terrain_color_builder_capability() -> Result<(), Box<dyn Error>> {
    let profile = TargetProfile::from_yaml_str(TARGET_PROFILE_YAML)?;
    let terrain = profile
        .capability_diagnostic(TargetCapability::TerrainColorBuilder)
        .ok_or_else(|| io::Error::other("terrain color capability gate must exist"))?;

    assert!(terrain.is_blocked());
    assert_eq!(terrain.gate_name, "terrain_color_builder");
    assert_eq!(terrain.state, "blocked");
    assert_eq!(terrain.spec, Some("TERRAIN-004"));
    assert!(
        terrain
            .note
            .is_some_and(|note| note.contains("no guessed implementation"))
    );

    let blocked: Vec<_> = profile
        .blocked_capabilities()
        .map(|diagnostic| diagnostic.capability)
        .collect();
    assert_eq!(blocked, vec![TargetCapability::TerrainColorBuilder]);

    let extended_ids = profile
        .capability_diagnostic(TargetCapability::ExtendedObjectModelIds)
        .ok_or_else(|| io::Error::other("extended model id capability gate must exist"))?;
    assert_eq!(extended_ids.state, "required");
    assert!(!extended_ids.is_blocked());
    Ok(())
}
