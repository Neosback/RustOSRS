use osrs_core::{
    coords::{SceneTile, StoragePlane},
    definitions::LocType,
    ids::ObjectId,
};
use osrs_scene::{
    PlacementInput, PlacementKind, SceneGrid, SceneLayer, ScenePlacedLoc, plan_placement,
};
use std::{error::Error, io};

fn plane0() -> Result<StoragePlane, io::Error> {
    StoragePlane::new(0).ok_or_else(|| io::Error::other("plane zero must be valid"))
}

fn placement(
    loc_type: u8,
    orientation: u8,
    tile: SceneTile,
    size_x: u16,
    size_y: u16,
    sampled_height: i32,
) -> Result<osrs_scene::PlacementPlan, osrs_scene::PlacementError> {
    plan_placement(PlacementInput {
        loc_type: LocType::new(loc_type),
        orientation,
        tile,
        size_x,
        size_y,
        sampled_height,
        existing_wall_displacement: None,
    })
}

fn fixed_layer_entry<'a>(
    grid: &'a SceneGrid,
    plane: StoragePlane,
    tile: SceneTile,
    layer: SceneLayer,
) -> Option<&'a ScenePlacedLoc> {
    match layer {
        SceneLayer::FloorDecoration => grid.floor_decoration(plane, tile),
        SceneLayer::Boundary => grid.boundary(plane, tile),
        SceneLayer::WallDecoration => grid.wall_decoration(plane, tile),
        SceneLayer::GameObject => None,
    }
}

#[test]
fn every_loc_dispatch_path_lands_in_its_semantic_scene_layer() -> Result<(), Box<dyn Error>> {
    let mut grid = SceneGrid::new(64, 2, 1)?;
    let plane = plane0()?;
    let mut loc_types: Vec<u8> = (0..=22).collect();
    loc_types.push(42);

    for loc_type in loc_types {
        let tile = SceneTile::new(u32::from(loc_type), 0);
        let plan = placement(loc_type, 0, tile, 1, 1, 100)?;
        let expected_layer = plan.kind.layer();
        assert!(grid.insert_placement(plane, ObjectId::new(u32::from(loc_type)), plan)?);

        match expected_layer {
            SceneLayer::GameObject => {
                let stored = grid.game_object(plane, tile).ok_or_else(|| {
                    io::Error::other(format!("missing game object type {loc_type}"))
                })?;
                assert_eq!(stored.object_id(), ObjectId::new(u32::from(loc_type)));
                assert_eq!(stored.placement(), Some(plan));
            }
            layer => {
                let stored = fixed_layer_entry(&grid, plane, tile, layer).ok_or_else(|| {
                    io::Error::other(format!("missing fixed layer for type {loc_type}"))
                })?;
                assert_eq!(stored.object_id(), ObjectId::new(u32::from(loc_type)));
                assert_eq!(stored.placement(), plan);
            }
        }
    }
    Ok(())
}

#[test]
fn dual_boundary_keeps_both_semantic_arms_after_storage() -> Result<(), Box<dyn Error>> {
    let mut grid = SceneGrid::new(4, 4, 1)?;
    let plane = plane0()?;
    let tile = SceneTile::new(2, 2);
    let plan = placement(2, 3, tile, 1, 1, 55)?;
    assert!(grid.insert_placement(plane, ObjectId::new(200), plan)?);

    let stored = grid
        .boundary(plane, tile)
        .ok_or_else(|| io::Error::other("type-2 boundary was not stored"))?;
    let PlacementKind::Boundary(boundary) = stored.placement().kind else {
        return Err(io::Error::other("stored type-2 placement lost boundary kind").into());
    };
    assert_eq!(boundary.primary.loc_type.get(), 2);
    assert_eq!(boundary.primary.orientation, 7);
    assert_eq!(
        boundary.secondary.map(|request| request.orientation),
        Some(0)
    );
    assert_eq!(boundary.primary_flag, 8);
    assert_eq!(boundary.secondary_flag, 1);
    Ok(())
}

#[test]
fn rotated_non_square_game_object_occupies_exact_transposed_footprint() -> Result<(), Box<dyn Error>>
{
    let mut grid = SceneGrid::new(8, 8, 1)?;
    let plane = plane0()?;
    let start = SceneTile::new(1, 1);
    let plan = placement(10, 1, start, 2, 3, 75)?;
    assert!(grid.insert_placement(plane, ObjectId::new(10), plan)?);

    let anchor = grid
        .game_object(plane, start)
        .ok_or_else(|| io::Error::other("missing rotated game object"))?;
    assert_eq!(anchor.start(), start);
    assert_eq!(anchor.end(), SceneTile::new(3, 2));
    assert_eq!(anchor.edge_mask(), 6);
    assert_eq!(anchor.placement(), Some(plan));

    let opposite = grid
        .tile(plane, SceneTile::new(3, 2))
        .and_then(|tile| tile.game_objects().first())
        .ok_or_else(|| io::Error::other("missing opposite footprint occupancy"))?;
    assert_eq!(opposite.edge_mask(), 9);
    assert_eq!(opposite.instance_id(), anchor.instance_id());

    for x in 1..=3 {
        for y in 1..=2 {
            assert_eq!(
                grid.tile(plane, SceneTile::new(x, y))
                    .map(|tile| tile.game_objects().len()),
                Some(1)
            );
        }
    }
    assert!(grid.tile(plane, SceneTile::new(4, 2)).is_none());
    assert!(grid.tile(plane, SceneTile::new(3, 3)).is_none());
    Ok(())
}

#[test]
fn floor_decoration_preserves_supplied_flat_and_slope_height_without_lift()
-> Result<(), Box<dyn Error>> {
    let mut grid = SceneGrid::new(2, 1, 1)?;
    let plane = plane0()?;

    for (x, height) in [(0_u32, 100_i32), (1, -28)] {
        let tile = SceneTile::new(x, 0);
        let plan = placement(22, 0, tile, 1, 1, height)?;
        assert!(grid.insert_placement(plane, ObjectId::new(22 + x), plan)?);
        let stored = grid
            .floor_decoration(plane, tile)
            .ok_or_else(|| io::Error::other("missing floor decoration"))?;
        assert_eq!(stored.placement().storage_center.y.units(), height);
        assert_eq!(stored.placement().model_center.y.units(), height);
    }
    Ok(())
}

#[test]
fn fixed_layer_queries_return_only_the_requested_slot() -> Result<(), Box<dyn Error>> {
    let mut grid = SceneGrid::new(3, 1, 1)?;
    let plane = plane0()?;
    let boundary_tile = SceneTile::new(0, 0);
    let decor_tile = SceneTile::new(1, 0);
    let floor_tile = SceneTile::new(2, 0);

    assert!(grid.insert_placement(
        plane,
        ObjectId::new(1),
        placement(0, 0, boundary_tile, 1, 1, 0)?,
    )?);
    assert!(grid.insert_placement(
        plane,
        ObjectId::new(2),
        placement(5, 0, decor_tile, 1, 1, 0)?,
    )?);
    assert!(grid.insert_placement(
        plane,
        ObjectId::new(3),
        placement(22, 0, floor_tile, 1, 1, 0)?,
    )?);

    assert!(grid.boundary(plane, boundary_tile).is_some());
    assert!(grid.wall_decoration(plane, boundary_tile).is_none());
    assert!(grid.floor_decoration(plane, boundary_tile).is_none());
    assert!(grid.wall_decoration(plane, decor_tile).is_some());
    assert!(grid.boundary(plane, decor_tile).is_none());
    assert!(grid.floor_decoration(plane, floor_tile).is_some());
    assert!(grid.game_object(plane, floor_tile).is_none());
    Ok(())
}
