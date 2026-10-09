use osrs_core::{
    coords::{SceneTile, StoragePlane},
    definitions::LocType,
    ids::ObjectId,
};
use osrs_scene::{
    CARDINAL_OFFSET_X, CARDINAL_OFFSET_Z, DIAGONAL_OFFSET_X, DIAGONAL_OFFSET_Z,
    DIAGONAL_WALL_FLAGS, Footprint, PlacementInput, PlacementKind, SceneGrid,
    STRAIGHT_WALL_FLAGS, plan_placement,
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
    existing_wall_displacement: Option<u16>,
) -> Result<osrs_scene::PlacementPlan, osrs_scene::PlacementError> {
    plan_placement(PlacementInput {
        loc_type: LocType::new(loc_type),
        orientation,
        tile,
        size_x,
        size_y,
        sampled_height,
        existing_wall_displacement,
    })
}

#[test]
fn boundary_types_cover_all_orientations_with_exact_arms_and_flags() -> Result<(), Box<dyn Error>> {
    let mut grid = SceneGrid::new(16, 1, 1)?;
    let plane = plane0()?;

    for loc_type in 0_u8..=3 {
        for orientation in 0_u8..=3 {
            let x = u32::from(loc_type) * 4 + u32::from(orientation);
            let tile = SceneTile::new(x, 0);
            let plan = placement(loc_type, orientation, tile, 1, 1, 0, None)?;
            assert!(grid.insert_placement(plane, ObjectId::new(x), plan)?);

            let stored = grid
                .boundary(plane, tile)
                .ok_or_else(|| io::Error::other("boundary placement was not stored"))?;
            assert_eq!(stored.placement(), plan);

            let PlacementKind::Boundary(boundary) = stored.placement().kind else {
                return Err(io::Error::other("stored placement lost boundary kind").into());
            };
            let index = usize::from(orientation);
            match loc_type {
                0 => {
                    assert_eq!(boundary.primary.loc_type.get(), 0);
                    assert_eq!(boundary.primary.orientation, orientation);
                    assert_eq!(boundary.secondary, None);
                    assert_eq!(boundary.primary_flag, STRAIGHT_WALL_FLAGS[index]);
                    assert_eq!(boundary.secondary_flag, 0);
                }
                1 => {
                    assert_eq!(boundary.primary.loc_type.get(), 1);
                    assert_eq!(boundary.primary.orientation, orientation);
                    assert_eq!(boundary.secondary, None);
                    assert_eq!(boundary.primary_flag, DIAGONAL_WALL_FLAGS[index]);
                    assert_eq!(boundary.secondary_flag, 0);
                }
                2 => {
                    let next = (orientation + 1) & 3;
                    assert_eq!(boundary.primary.loc_type.get(), 2);
                    assert_eq!(boundary.primary.orientation, orientation + 4);
                    assert_eq!(
                        boundary.secondary.map(|request| request.loc_type.get()),
                        Some(2)
                    );
                    assert_eq!(
                        boundary.secondary.map(|request| request.orientation),
                        Some(next)
                    );
                    assert_eq!(boundary.primary_flag, STRAIGHT_WALL_FLAGS[index]);
                    assert_eq!(
                        boundary.secondary_flag,
                        STRAIGHT_WALL_FLAGS[usize::from(next)]
                    );
                }
                3 => {
                    assert_eq!(boundary.primary.loc_type.get(), 3);
                    assert_eq!(boundary.primary.orientation, orientation);
                    assert_eq!(boundary.secondary, None);
                    assert_eq!(boundary.primary_flag, DIAGONAL_WALL_FLAGS[index]);
                    assert_eq!(boundary.secondary_flag, 0);
                }
                _ => return Err(io::Error::other("unexpected boundary loc type").into()),
            }
        }
    }

    Ok(())
}

#[test]
fn wall_decoration_types_cover_all_orientations_and_displacement_sources()
-> Result<(), Box<dyn Error>> {
    let mut grid = SceneGrid::new(40, 1, 1)?;
    let plane = plane0()?;
    let mut x = 0_u32;

    for wall_displacement in [None, Some(34)] {
        for loc_type in 4_u8..=8 {
            for orientation in 0_u8..=3 {
                let tile = SceneTile::new(x, 0);
                let plan = placement(
                    loc_type,
                    orientation,
                    tile,
                    1,
                    1,
                    0,
                    wall_displacement,
                )?;
                assert!(grid.insert_placement(plane, ObjectId::new(x), plan)?);

                let stored = grid.wall_decoration(plane, tile).ok_or_else(|| {
                    io::Error::other("wall-decoration placement was not stored")
                })?;
                assert_eq!(stored.placement(), plan);

                let PlacementKind::WallDecoration(decor) = stored.placement().kind else {
                    return Err(io::Error::other("stored placement lost wall-decoration kind").into());
                };
                let index = usize::from(orientation);
                let full_displacement = i32::from(wall_displacement.unwrap_or(16));
                let half_displacement = wall_displacement.map_or(8, |value| i32::from(value / 2));
                let opposite = (orientation + 2) & 3;

                match loc_type {
                    4 => {
                        assert_eq!(decor.primary.loc_type.get(), 4);
                        assert_eq!(decor.primary.orientation, orientation);
                        assert_eq!(decor.secondary, None);
                        assert_eq!(decor.orientation_flag, STRAIGHT_WALL_FLAGS[index]);
                        assert_eq!(decor.orientation_parameter, 0);
                        assert_eq!((decor.offset_x, decor.offset_z), (0, 0));
                    }
                    5 => {
                        assert_eq!(decor.primary.loc_type.get(), 4);
                        assert_eq!(decor.primary.orientation, orientation);
                        assert_eq!(decor.secondary, None);
                        assert_eq!(decor.orientation_flag, STRAIGHT_WALL_FLAGS[index]);
                        assert_eq!(decor.orientation_parameter, 0);
                        assert_eq!(
                            (decor.offset_x, decor.offset_z),
                            (
                                full_displacement * CARDINAL_OFFSET_X[index],
                                full_displacement * CARDINAL_OFFSET_Z[index],
                            )
                        );
                    }
                    6 => {
                        assert_eq!(decor.primary.loc_type.get(), 4);
                        assert_eq!(decor.primary.orientation, orientation + 4);
                        assert_eq!(decor.secondary, None);
                        assert_eq!(decor.orientation_flag, 256);
                        assert_eq!(decor.orientation_parameter, orientation);
                        assert_eq!(
                            (decor.offset_x, decor.offset_z),
                            (
                                half_displacement * DIAGONAL_OFFSET_X[index],
                                half_displacement * DIAGONAL_OFFSET_Z[index],
                            )
                        );
                    }
                    7 => {
                        assert_eq!(decor.primary.loc_type.get(), 4);
                        assert_eq!(decor.primary.orientation, opposite + 4);
                        assert_eq!(decor.secondary, None);
                        assert_eq!(decor.orientation_flag, 256);
                        assert_eq!(decor.orientation_parameter, opposite);
                        assert_eq!((decor.offset_x, decor.offset_z), (0, 0));
                    }
                    8 => {
                        assert_eq!(decor.primary.loc_type.get(), 4);
                        assert_eq!(decor.primary.orientation, orientation + 4);
                        assert_eq!(
                            decor.secondary.map(|request| request.loc_type.get()),
                            Some(4)
                        );
                        assert_eq!(
                            decor.secondary.map(|request| request.orientation),
                            Some(opposite + 4)
                        );
                        assert_eq!(decor.orientation_flag, 256);
                        assert_eq!(decor.orientation_parameter, orientation);
                        assert_eq!(
                            (decor.offset_x, decor.offset_z),
                            (
                                half_displacement * DIAGONAL_OFFSET_X[index],
                                half_displacement * DIAGONAL_OFFSET_Z[index],
                            )
                        );
                    }
                    _ => {
                        return Err(io::Error::other("unexpected wall-decoration loc type").into());
                    }
                }

                x += 1;
            }
        }
    }

    assert_eq!(x, 40);
    Ok(())
}

#[test]
fn square_and_non_square_game_objects_cover_all_orientations_and_sampled_heights()
-> Result<(), Box<dyn Error>> {
    let mut grid = SceneGrid::new(32, 5, 1)?;
    let plane = plane0()?;
    let heights = [-96, 37, 211, -305, 512, -1, 88, 999];

    for (shape_index, (size_x, size_y)) in [(2_u16, 2_u16), (2_u16, 3_u16)]
        .into_iter()
        .enumerate()
    {
        for orientation in 0_u8..=3 {
            let sample_index = shape_index * 4 + usize::from(orientation);
            let start_x = u32::try_from(sample_index * 4)?;
            let start = SceneTile::new(start_x, 1);
            let sampled_height = heights[sample_index];
            let plan = placement(
                10,
                orientation,
                start,
                size_x,
                size_y,
                sampled_height,
                None,
            )?;
            let expected_footprint = Footprint::rotated(size_x, size_y, orientation);

            assert_eq!(plan.rotated_definition_footprint, expected_footprint);
            assert_eq!(plan.model_center.y.units(), sampled_height);
            assert_eq!(plan.storage_center.y.units(), sampled_height);
            assert!(grid.insert_placement(plane, ObjectId::new(start_x), plan)?);

            let stored = grid
                .game_object(plane, start)
                .ok_or_else(|| io::Error::other("game-object placement was not stored"))?;
            let expected_end = SceneTile::new(
                start.x + u32::from(expected_footprint.width) - 1,
                start.y + u32::from(expected_footprint.depth) - 1,
            );
            assert_eq!(stored.start(), start);
            assert_eq!(stored.end(), expected_end);
            assert_eq!(stored.placement(), Some(plan));

            for x in start.x..=expected_end.x {
                for y in start.y..=expected_end.y {
                    let occupancy = grid
                        .tile(plane, SceneTile::new(x, y))
                        .map(|tile| tile.game_objects().len());
                    assert_eq!(occupancy, Some(1));
                }
            }
        }
    }

    Ok(())
}
