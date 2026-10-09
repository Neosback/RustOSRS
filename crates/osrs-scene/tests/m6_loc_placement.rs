use osrs_core::{coords::SceneTile, definitions::LocType};
use osrs_scene::{PlacementInput, PlacementKind, plan_placement};
use std::{error::Error, io};

const HISTORICAL_GOLDEN: &str = include_str!("../../../reference-fixtures/deob_golden.txt");

fn value(line: &str, key: &str) -> Result<i32, Box<dyn Error>> {
    let prefix = format!("{key}=");
    let token = line
        .split_whitespace()
        .find_map(|part| part.strip_prefix(&prefix))
        .ok_or_else(|| io::Error::other(format!("missing {key} in {line}")))?;
    Ok(token.parse()?)
}

fn input(loc_type: u8, orientation: u8, wall_displacement: Option<u16>) -> PlacementInput {
    PlacementInput {
        loc_type: LocType::new(loc_type),
        orientation,
        tile: SceneTile::new(10, 20),
        size_x: 1,
        size_y: 1,
        sampled_height: 100,
        existing_wall_displacement: wall_displacement,
    }
}

#[test]
fn historical_wall_and_decor_orientation_matrix_matches_planner() -> Result<(), Box<dyn Error>> {
    let wall_lines: Vec<&str> = HISTORICAL_GOLDEN
        .lines()
        .filter(|line| line.starts_with("wall t="))
        .collect();
    let decor_lines: Vec<&str> = HISTORICAL_GOLDEN
        .lines()
        .filter(|line| line.starts_with("decor t="))
        .collect();
    assert_eq!(wall_lines.len(), 16);
    assert_eq!(decor_lines.len(), 20);

    for line in wall_lines {
        let loc_type = u8::try_from(value(line, "t")?)?;
        let orientation = u8::try_from(value(line, "o")?)?;
        let PlacementKind::Boundary(plan) =
            plan_placement(input(loc_type, orientation, None))?.kind
        else {
            return Err(io::Error::other(format!("{line} did not produce boundary plan")).into());
        };
        assert_eq!(i32::from(plan.primary_flag), value(line, "oA")?, "{line}");
        assert_eq!(i32::from(plan.secondary_flag), value(line, "oB")?, "{line}");
    }

    for line in decor_lines {
        let loc_type = u8::try_from(value(line, "t")?)?;
        let orientation = u8::try_from(value(line, "o")?)?;
        let PlacementKind::WallDecoration(plan) =
            plan_placement(input(loc_type, orientation, None))?.kind
        else {
            return Err(io::Error::other(format!("{line} did not produce decor plan")).into());
        };
        assert_eq!(
            i32::from(plan.orientation_flag),
            value(line, "o")?,
            "{line}"
        );
        assert_eq!(
            i32::from(plan.orientation_parameter),
            value(line, "o2")?,
            "{line}"
        );

        if matches!(loc_type, 6..=8) {
            assert_eq!(plan.offset_x, value(line, "xOff")?, "{line}");
            assert_eq!(plan.offset_z, value(line, "f3196")?, "{line}");
        }
    }

    Ok(())
}

#[test]
fn all_decor_orientations_apply_full_half_default_and_existing_wall_displacement()
-> Result<(), Box<dyn Error>> {
    let cardinal = [(1, 0), (0, -1), (-1, 0), (0, 1)];
    let diagonal = [(1, -1), (-1, -1), (-1, 1), (1, 1)];

    for orientation in 0_u8..4 {
        let index = usize::from(orientation);

        let PlacementKind::WallDecoration(default_full) =
            plan_placement(input(5, orientation, None))?.kind
        else {
            return Err(io::Error::other("type 5 did not produce decor plan").into());
        };
        assert_eq!(
            (default_full.offset_x, default_full.offset_z),
            (16 * cardinal[index].0, 16 * cardinal[index].1)
        );

        let PlacementKind::WallDecoration(existing_full) =
            plan_placement(input(5, orientation, Some(34)))?.kind
        else {
            return Err(io::Error::other("type 5 did not produce decor plan").into());
        };
        assert_eq!(
            (existing_full.offset_x, existing_full.offset_z),
            (34 * cardinal[index].0, 34 * cardinal[index].1)
        );

        for loc_type in [6_u8, 8] {
            let PlacementKind::WallDecoration(default_half) =
                plan_placement(input(loc_type, orientation, None))?.kind
            else {
                return Err(
                    io::Error::other("half-displacement type did not produce decor plan").into(),
                );
            };
            assert_eq!(
                (default_half.offset_x, default_half.offset_z),
                (8 * diagonal[index].0, 8 * diagonal[index].1)
            );

            let PlacementKind::WallDecoration(existing_half) =
                plan_placement(input(loc_type, orientation, Some(34)))?.kind
            else {
                return Err(
                    io::Error::other("half-displacement type did not produce decor plan").into(),
                );
            };
            assert_eq!(
                (existing_half.offset_x, existing_half.offset_z),
                (17 * diagonal[index].0, 17 * diagonal[index].1)
            );
        }
    }

    Ok(())
}
