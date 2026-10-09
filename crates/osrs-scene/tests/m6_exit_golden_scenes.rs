use osrs_core::{
    coords::{HALF_TILE, MapTile, SceneTile, SourcePlane, StoragePlane},
    definitions::LocType,
    ids::ObjectId,
};
use osrs_scene::{
    FlatTerrainSurface, PlacementHeightInput, PlacementInput, PlacementKind,
    REFERENCE_TERRAIN_SKIP_COLOR, SceneGrid, SceneLayer, SemanticTile, ShapedTerrainInput,
    ShapedTerrainSurface, TerrainCorners, TerrainSurface, flat_paint_is_reference_skipped,
    plan_placement, sample_placement_height,
};
use std::{error::Error, io};

fn storage_plane(value: u8) -> Result<StoragePlane, io::Error> {
    StoragePlane::new(value)
        .ok_or_else(|| io::Error::other(format!("storage plane {value} must be valid")))
}

fn source_plane(value: u8) -> Result<SourcePlane, io::Error> {
    SourcePlane::new(value)
        .ok_or_else(|| io::Error::other(format!("source plane {value} must be valid")))
}

#[test]
fn sloped_height_sampling_closes_initial_placement_center_contract() -> Result<(), Box<dyn Error>> {
    let heights: Vec<Vec<i32>> = (0..=4)
        .map(|x| (0..=4).map(|y| x * 100 + y * 10).collect())
        .collect();

    let sampled = sample_placement_height(
        &heights,
        4,
        4,
        PlacementHeightInput {
            tile: SceneTile::new(0, 0),
            size_x: 2,
            size_y: 3,
            orientation: 1,
        },
    )?;
    assert_eq!(sampled, 160);

    let plan = plan_placement(PlacementInput {
        loc_type: LocType::new(10),
        orientation: 1,
        tile: SceneTile::new(0, 0),
        size_x: 2,
        size_y: 3,
        sampled_height: sampled,
        existing_wall_displacement: None,
    })?;
    assert_eq!(plan.model_center.y.units(), 160);
    assert_eq!(plan.storage_center.y.units(), 160);

    let edge_sample = sample_placement_height(
        &heights,
        4,
        4,
        PlacementHeightInput {
            tile: SceneTile::new(3, 3),
            size_x: 2,
            size_y: 3,
            orientation: 0,
        },
    )?;
    assert_eq!(edge_sample, 385);
    Ok(())
}

#[test]
fn terrain_surface_choice_preserves_flat_diagonal_sentinel_and_shaped_identity()
-> Result<(), Box<dyn Error>> {
    let flat = FlatTerrainSurface::new(
        TerrainCorners::new(10, 20, 30, 40),
        TerrainCorners::new(101, 202, REFERENCE_TERRAIN_SKIP_COLOR, 404),
        Some(7),
    );
    assert!(flat_paint_is_reference_skipped(&flat));
    assert!(!flat.is_flat);
    assert_eq!(flat.heights, TerrainCorners::new(10, 20, 30, 40));
    let flat_surface = TerrainSurface::Flat(flat);
    assert!(matches!(flat_surface, TerrainSurface::Flat(_)));

    let shaped = ShapedTerrainSurface::build(ShapedTerrainInput {
        shape: 12,
        rotation: 3,
        texture_id: Some(9),
        tile: SceneTile::new(1, 1),
        heights: TerrainCorners::new(10, 20, 30, 40),
        underlay_colors: TerrainCorners::new(1, 2, 3, 4),
        overlay_colors: TerrainCorners::new(5, 6, 7, 8),
        underlay_rgb: 0x112233,
        overlay_rgb: 0x445566,
    })?;
    let shaped_surface = TerrainSurface::Shaped(shaped);
    assert!(matches!(shaped_surface, TerrainSurface::Shaped(_)));
    Ok(())
}

#[test]
fn golden_wall_and_decor_orientation_scene_survives_semantic_storage() -> Result<(), Box<dyn Error>>
{
    let plane = storage_plane(0)?;
    let mut grid = SceneGrid::new(36, 1, 1)?;

    for loc_type in 0_u8..=8 {
        for orientation in 0_u8..4 {
            let x = u32::from(loc_type) * 4 + u32::from(orientation);
            let tile = SceneTile::new(x, 0);
            let plan = plan_placement(PlacementInput {
                loc_type: LocType::new(loc_type),
                orientation,
                tile,
                size_x: 1,
                size_y: 1,
                sampled_height: 100 + i32::from(orientation),
                existing_wall_displacement: matches!(loc_type, 5 | 6 | 8).then_some(34),
            })?;
            assert!(grid.insert_placement(
                plane,
                ObjectId::new(1_000 + u32::from(loc_type) * 4 + u32::from(orientation)),
                plan,
            )?);

            match plan.kind.layer() {
                SceneLayer::Boundary => {
                    let stored = grid
                        .boundary(plane, tile)
                        .ok_or_else(|| io::Error::other("missing boundary in golden scene"))?;
                    assert_eq!(stored.placement(), plan);
                    assert!(matches!(
                        stored.placement().kind,
                        PlacementKind::Boundary(_)
                    ));
                }
                SceneLayer::WallDecoration => {
                    let stored = grid
                        .wall_decoration(plane, tile)
                        .ok_or_else(|| io::Error::other("missing wall decor in golden scene"))?;
                    assert_eq!(stored.placement(), plan);
                    assert!(matches!(
                        stored.placement().kind,
                        PlacementKind::WallDecoration(_)
                    ));
                }
                other => {
                    return Err(io::Error::other(format!(
                        "loc type {loc_type} unexpectedly mapped to {other:?}"
                    ))
                    .into());
                }
            }
        }
    }
    Ok(())
}

#[test]
fn golden_four_plane_bridge_column_relinks_structurally() -> Result<(), Box<dyn Error>> {
    let tile = SceneTile::new(0, 0);
    let mut grid = SceneGrid::new(1, 1, 4)?;

    for plane_index in 0_u8..4 {
        let storage = storage_plane(plane_index)?;
        grid.set_tile(
            storage,
            tile,
            SemanticTile::new(Some(source_plane(plane_index)?), storage),
        )?;
    }

    grid.set_link_below(tile)?;

    assert_eq!(
        grid.tile(storage_plane(0)?, tile)
            .and_then(SemanticTile::source_plane),
        Some(source_plane(1)?)
    );
    assert_eq!(
        grid.tile(storage_plane(1)?, tile)
            .and_then(SemanticTile::source_plane),
        Some(source_plane(2)?)
    );
    assert_eq!(
        grid.tile(storage_plane(2)?, tile)
            .and_then(SemanticTile::source_plane),
        Some(source_plane(3)?)
    );
    assert!(grid.tile(storage_plane(3)?, tile).is_none());

    let linked_below = grid
        .tile(storage_plane(0)?, tile)
        .and_then(SemanticTile::linked_below)
        .ok_or_else(|| io::Error::other("missing linked-below plane-zero tile"))?;
    assert_eq!(linked_below.source_plane(), Some(source_plane(0)?));
    Ok(())
}

#[test]
fn golden_region_border_fixture_preserves_world_scene_and_local_identity()
-> Result<(), Box<dyn Error>> {
    let scene_origin = MapTile::new(3_200, 3_200);
    let west_south = MapTile::new(3_263, 3_263);
    let east_north = MapTile::new(3_264, 3_264);

    let (region_a, within_a) = west_south.split_region();
    let (region_b, within_b) = east_north.split_region();
    assert_ne!(region_a, region_b);
    assert_eq!((within_a.x(), within_a.y()), (63, 63));
    assert_eq!((within_b.x(), within_b.y()), (0, 0));

    let scene_a = west_south
        .to_scene(scene_origin)
        .ok_or_else(|| io::Error::other("west/south border tile must map into scene"))?;
    let scene_b = east_north
        .to_scene(scene_origin)
        .ok_or_else(|| io::Error::other("east/north border tile must map into scene"))?;
    assert_eq!(scene_a, SceneTile::new(63, 63));
    assert_eq!(scene_b, SceneTile::new(64, 64));
    assert_eq!(scene_a.to_map(scene_origin), Some(west_south));
    assert_eq!(scene_b.to_map(scene_origin), Some(east_north));

    let plan_a = plan_placement(PlacementInput {
        loc_type: LocType::new(10),
        orientation: 0,
        tile: scene_a,
        size_x: 1,
        size_y: 1,
        sampled_height: 10,
        existing_wall_displacement: None,
    })?;
    let plan_b = plan_placement(PlacementInput {
        loc_type: LocType::new(10),
        orientation: 0,
        tile: scene_b,
        size_x: 1,
        size_y: 1,
        sampled_height: 20,
        existing_wall_displacement: None,
    })?;

    let origin_local = scene_origin
        .local_origin()
        .ok_or_else(|| io::Error::other("scene origin local conversion overflow"))?;
    let world_a_local = west_south
        .local_origin()
        .ok_or_else(|| io::Error::other("world A local conversion overflow"))?;
    let world_b_local = east_north
        .local_origin()
        .ok_or_else(|| io::Error::other("world B local conversion overflow"))?;

    assert_eq!(
        origin_local.x.units() + plan_a.model_center.x.units(),
        world_a_local.x.units() + HALF_TILE
    );
    assert_eq!(
        origin_local.z.units() + plan_a.model_center.z.units(),
        world_a_local.z.units() + HALF_TILE
    );
    assert_eq!(
        origin_local.x.units() + plan_b.model_center.x.units(),
        world_b_local.x.units() + HALF_TILE
    );
    assert_eq!(
        origin_local.z.units() + plan_b.model_center.z.units(),
        world_b_local.z.units() + HALF_TILE
    );

    let mut grid = SceneGrid::new(65, 65, 1)?;
    let plane = storage_plane(0)?;
    assert!(grid.insert_placement(plane, ObjectId::new(2001), plan_a)?);
    assert!(grid.insert_placement(plane, ObjectId::new(2002), plan_b)?);
    assert_eq!(
        grid.game_object(plane, scene_a)
            .map(|value| value.object_id()),
        Some(ObjectId::new(2001))
    );
    assert_eq!(
        grid.game_object(plane, scene_b)
            .map(|value| value.object_id()),
        Some(ObjectId::new(2002))
    );
    Ok(())
}
