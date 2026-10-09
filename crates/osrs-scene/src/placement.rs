//! Exact semantic location-placement planning for M6.
//!
//! This module owns deterministic type dispatch, orientation tables, footprint
//! rotation, center calculation, and wall-decoration displacement. It does not
//! build models, mutate collision, or perform bridge/link-below processing.

use osrs_core::{
    coords::{HALF_TILE, LOCAL_UNITS_PER_TILE, LocalCoord, LocalPoint, SceneTile},
    definitions::LocType,
};
use std::{error::Error, fmt};

pub const STRAIGHT_WALL_FLAGS: [u16; 4] = [1, 2, 4, 8];
pub const DIAGONAL_WALL_FLAGS: [u16; 4] = [16, 32, 64, 128];
pub const CARDINAL_OFFSET_X: [i32; 4] = [1, 0, -1, 0];
pub const CARDINAL_OFFSET_Z: [i32; 4] = [0, -1, 0, 1];
pub const DIAGONAL_OFFSET_X: [i32; 4] = [1, -1, -1, 1];
pub const DIAGONAL_OFFSET_Z: [i32; 4] = [-1, -1, 1, 1];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SceneLayer {
    FloorDecoration,
    Boundary,
    WallDecoration,
    GameObject,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Footprint {
    pub width: u16,
    pub depth: u16,
}

impl Footprint {
    pub const ONE_BY_ONE: Self = Self { width: 1, depth: 1 };

    pub const fn new(width: u16, depth: u16) -> Self {
        Self { width, depth }
    }

    pub const fn rotated(size_x: u16, size_y: u16, orientation: u8) -> Self {
        if orientation == 1 || orientation == 3 {
            Self::new(size_y, size_x)
        } else {
            Self::new(size_x, size_y)
        }
    }
}

/// Exact geometry request passed to the model-construction layer.
///
/// No fallback model type is represented here. If this request cannot produce a
/// model, the caller must leave the placement absent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModelRequest {
    pub loc_type: LocType,
    pub orientation: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BoundaryPlan {
    pub primary: ModelRequest,
    pub secondary: Option<ModelRequest>,
    pub primary_flag: u16,
    pub secondary_flag: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WallDecorationPlan {
    pub primary: ModelRequest,
    pub secondary: Option<ModelRequest>,
    pub orientation_flag: u16,
    pub orientation_parameter: u8,
    pub offset_x: i32,
    pub offset_z: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GameObjectPlan {
    pub model: ModelRequest,
    pub storage_footprint: Footprint,
    pub insertion_flag: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FloorDecorationPlan {
    pub model: ModelRequest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlacementKind {
    FloorDecoration(FloorDecorationPlan),
    Boundary(BoundaryPlan),
    WallDecoration(WallDecorationPlan),
    GameObject(GameObjectPlan),
}

impl PlacementKind {
    pub const fn layer(self) -> SceneLayer {
        match self {
            Self::FloorDecoration(_) => SceneLayer::FloorDecoration,
            Self::Boundary(_) => SceneLayer::Boundary,
            Self::WallDecoration(_) => SceneLayer::WallDecoration,
            Self::GameObject(_) => SceneLayer::GameObject,
        }
    }

    pub const fn storage_footprint(self) -> Footprint {
        match self {
            Self::GameObject(plan) => plan.storage_footprint,
            Self::FloorDecoration(_) | Self::Boundary(_) | Self::WallDecoration(_) => {
                Footprint::ONE_BY_ONE
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlacementInput {
    pub loc_type: LocType,
    pub orientation: u8,
    pub tile: SceneTile,
    pub size_x: u16,
    pub size_y: u16,
    /// Exact sampled semantic terrain height. No generic decoration lift is applied.
    pub sampled_height: i32,
    /// Existing boundary definition displacement. `None` means reference defaults.
    pub existing_wall_displacement: Option<u16>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlacementPlan {
    pub source_loc_type: LocType,
    pub source_orientation: u8,
    pub rotated_definition_footprint: Footprint,
    /// Center supplied to model/entity construction from the rotated definition footprint.
    pub model_center: LocalPoint,
    /// Center implied by the scene storage footprint/layer.
    pub storage_center: LocalPoint,
    pub kind: PlacementKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlacementError {
    InvalidOrientation(u8),
    ZeroFootprint,
    CoordinateOverflow,
}

impl fmt::Display for PlacementError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidOrientation(value) => {
                write!(formatter, "loc orientation {value} is outside 0..=3")
            }
            Self::ZeroFootprint => formatter.write_str("loc definition footprint must be non-zero"),
            Self::CoordinateOverflow => formatter.write_str("loc placement coordinate overflow"),
        }
    }
}

impl Error for PlacementError {}

pub fn plan_placement(input: PlacementInput) -> Result<PlacementPlan, PlacementError> {
    if input.orientation > 3 {
        return Err(PlacementError::InvalidOrientation(input.orientation));
    }
    if input.size_x == 0 || input.size_y == 0 {
        return Err(PlacementError::ZeroFootprint);
    }

    let definition_footprint = Footprint::rotated(input.size_x, input.size_y, input.orientation);
    let kind = placement_kind(input, definition_footprint);
    let model_center = footprint_center(input.tile, definition_footprint, input.sampled_height)?;
    let storage_center = footprint_center(input.tile, kind.storage_footprint(), input.sampled_height)?;

    Ok(PlacementPlan {
        source_loc_type: input.loc_type,
        source_orientation: input.orientation,
        rotated_definition_footprint: definition_footprint,
        model_center,
        storage_center,
        kind,
    })
}

fn placement_kind(input: PlacementInput, definition_footprint: Footprint) -> PlacementKind {
    let raw = input.loc_type.get();
    let orientation = input.orientation;
    let index = usize::from(orientation);
    let request = |loc_type: u8, model_orientation: u8| ModelRequest {
        loc_type: LocType::new(loc_type),
        orientation: model_orientation,
    };

    match raw {
        22 => PlacementKind::FloorDecoration(FloorDecorationPlan {
            model: request(22, orientation),
        }),
        10 | 11 => PlacementKind::GameObject(GameObjectPlan {
            model: request(10, orientation),
            storage_footprint: definition_footprint,
            insertion_flag: if raw == 11 { 256 } else { 0 },
        }),
        0 => PlacementKind::Boundary(BoundaryPlan {
            primary: request(0, orientation),
            secondary: None,
            primary_flag: STRAIGHT_WALL_FLAGS[index],
            secondary_flag: 0,
        }),
        1 => PlacementKind::Boundary(BoundaryPlan {
            primary: request(1, orientation),
            secondary: None,
            primary_flag: DIAGONAL_WALL_FLAGS[index],
            secondary_flag: 0,
        }),
        2 => {
            let next = (orientation + 1) & 3;
            PlacementKind::Boundary(BoundaryPlan {
                primary: request(2, orientation + 4),
                secondary: Some(request(2, next)),
                primary_flag: STRAIGHT_WALL_FLAGS[index],
                secondary_flag: STRAIGHT_WALL_FLAGS[usize::from(next)],
            })
        }
        3 => PlacementKind::Boundary(BoundaryPlan {
            primary: request(3, orientation),
            secondary: None,
            primary_flag: DIAGONAL_WALL_FLAGS[index],
            secondary_flag: 0,
        }),
        4 => PlacementKind::WallDecoration(WallDecorationPlan {
            primary: request(4, orientation),
            secondary: None,
            orientation_flag: STRAIGHT_WALL_FLAGS[index],
            orientation_parameter: 0,
            offset_x: 0,
            offset_z: 0,
        }),
        5 => {
            let displacement = i32::from(input.existing_wall_displacement.unwrap_or(16));
            PlacementKind::WallDecoration(WallDecorationPlan {
                primary: request(4, orientation),
                secondary: None,
                orientation_flag: STRAIGHT_WALL_FLAGS[index],
                orientation_parameter: 0,
                offset_x: displacement * CARDINAL_OFFSET_X[index],
                offset_z: displacement * CARDINAL_OFFSET_Z[index],
            })
        }
        6 => {
            let displacement = input
                .existing_wall_displacement
                .map_or(8, |value| i32::from(value / 2));
            PlacementKind::WallDecoration(WallDecorationPlan {
                primary: request(4, orientation + 4),
                secondary: None,
                orientation_flag: 256,
                orientation_parameter: orientation,
                offset_x: displacement * DIAGONAL_OFFSET_X[index],
                offset_z: displacement * DIAGONAL_OFFSET_Z[index],
            })
        }
        7 => {
            let opposite = (orientation + 2) & 3;
            PlacementKind::WallDecoration(WallDecorationPlan {
                primary: request(4, opposite + 4),
                secondary: None,
                orientation_flag: 256,
                orientation_parameter: opposite,
                offset_x: 0,
                offset_z: 0,
            })
        }
        8 => {
            let opposite = (orientation + 2) & 3;
            let displacement = input
                .existing_wall_displacement
                .map_or(8, |value| i32::from(value / 2));
            PlacementKind::WallDecoration(WallDecorationPlan {
                primary: request(4, orientation + 4),
                secondary: Some(request(4, opposite + 4)),
                orientation_flag: 256,
                orientation_parameter: orientation,
                offset_x: displacement * DIAGONAL_OFFSET_X[index],
                offset_z: displacement * DIAGONAL_OFFSET_Z[index],
            })
        }
        9 => PlacementKind::GameObject(GameObjectPlan {
            model: request(9, orientation),
            storage_footprint: Footprint::ONE_BY_ONE,
            insertion_flag: 0,
        }),
        12..=u8::MAX => PlacementKind::GameObject(GameObjectPlan {
            model: request(raw, orientation),
            storage_footprint: Footprint::ONE_BY_ONE,
            insertion_flag: 0,
        }),
    }
}

fn footprint_center(
    tile: SceneTile,
    footprint: Footprint,
    sampled_height: i32,
) -> Result<LocalPoint, PlacementError> {
    let tile_x = i32::try_from(tile.x).map_err(|_| PlacementError::CoordinateOverflow)?;
    let tile_z = i32::try_from(tile.y).map_err(|_| PlacementError::CoordinateOverflow)?;
    let base_x = tile_x
        .checked_mul(LOCAL_UNITS_PER_TILE)
        .ok_or(PlacementError::CoordinateOverflow)?;
    let base_z = tile_z
        .checked_mul(LOCAL_UNITS_PER_TILE)
        .ok_or(PlacementError::CoordinateOverflow)?;
    let offset_x = i32::from(footprint.width)
        .checked_mul(HALF_TILE)
        .ok_or(PlacementError::CoordinateOverflow)?;
    let offset_z = i32::from(footprint.depth)
        .checked_mul(HALF_TILE)
        .ok_or(PlacementError::CoordinateOverflow)?;
    let x = base_x
        .checked_add(offset_x)
        .ok_or(PlacementError::CoordinateOverflow)?;
    let z = base_z
        .checked_add(offset_z)
        .ok_or(PlacementError::CoordinateOverflow)?;

    Ok(LocalPoint::new(
        LocalCoord::from_units(x),
        LocalCoord::from_units(sampled_height),
        LocalCoord::from_units(z),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(loc_type: u8, orientation: u8) -> PlacementInput {
        PlacementInput {
            loc_type: LocType::new(loc_type),
            orientation,
            tile: SceneTile::new(10, 20),
            size_x: 2,
            size_y: 3,
            sampled_height: 100,
            existing_wall_displacement: None,
        }
    }

    #[test]
    fn orientation_tables_match_pinned_reference() {
        assert_eq!(STRAIGHT_WALL_FLAGS, [1, 2, 4, 8]);
        assert_eq!(DIAGONAL_WALL_FLAGS, [16, 32, 64, 128]);
        assert_eq!(CARDINAL_OFFSET_X, [1, 0, -1, 0]);
        assert_eq!(CARDINAL_OFFSET_Z, [0, -1, 0, 1]);
        assert_eq!(DIAGONAL_OFFSET_X, [1, -1, -1, 1]);
        assert_eq!(DIAGONAL_OFFSET_Z, [-1, -1, 1, 1]);
    }

    #[test]
    fn non_square_definition_footprint_rotates_without_corner_drift() -> Result<(), PlacementError> {
        let expected = [
            (Footprint::new(2, 3), 1408, 2752),
            (Footprint::new(3, 2), 1472, 2688),
            (Footprint::new(2, 3), 1408, 2752),
            (Footprint::new(3, 2), 1472, 2688),
        ];
        for (orientation, (footprint, x, z)) in expected.into_iter().enumerate() {
            let plan = plan_placement(input(10, orientation as u8))?;
            assert_eq!(plan.rotated_definition_footprint, footprint);
            assert_eq!(plan.model_center.x.units(), x);
            assert_eq!(plan.model_center.z.units(), z);
            assert_eq!(plan.storage_center, plan.model_center);
        }
        Ok(())
    }

    #[test]
    fn type_11_uses_type_10_geometry_and_flag_256() -> Result<(), PlacementError> {
        let plan = plan_placement(input(11, 3))?;
        let PlacementKind::GameObject(game) = plan.kind else {
            panic!("type 11 must dispatch to game-object storage");
        };
        assert_eq!(game.model.loc_type.get(), 10);
        assert_eq!(game.model.orientation, 3);
        assert_eq!(game.insertion_flag, 256);
        assert_eq!(game.storage_footprint, Footprint::new(3, 2));
        Ok(())
    }

    #[test]
    fn one_by_one_storage_paths_keep_definition_model_center_distinct() -> Result<(), PlacementError> {
        for loc_type in [9, 12, 14, 21] {
            let plan = plan_placement(input(loc_type, 0))?;
            let PlacementKind::GameObject(game) = plan.kind else {
                panic!("type {loc_type} must dispatch to game-object storage");
            };
            assert_eq!(game.storage_footprint, Footprint::ONE_BY_ONE);
            assert_eq!(plan.model_center.x.units(), 1408);
            assert_eq!(plan.model_center.z.units(), 2752);
            assert_eq!(plan.storage_center.x.units(), 1344);
            assert_eq!(plan.storage_center.z.units(), 2624);
        }
        Ok(())
    }

    #[test]
    fn floor_decoration_has_exact_tile_center_and_no_height_lift() -> Result<(), PlacementError> {
        let plan = plan_placement(input(22, 0))?;
        assert_eq!(plan.kind.layer(), SceneLayer::FloorDecoration);
        assert_eq!(plan.storage_center.x.units(), 1344);
        assert_eq!(plan.storage_center.y.units(), 100);
        assert_eq!(plan.storage_center.z.units(), 2624);
        Ok(())
    }

    #[test]
    fn all_initial_loc_types_dispatch_to_expected_scene_layer() -> Result<(), PlacementError> {
        for loc_type in 0_u8..=22 {
            let layer = plan_placement(input(loc_type, 0))?.kind.layer();
            let expected = match loc_type {
                0..=3 => SceneLayer::Boundary,
                4..=8 => SceneLayer::WallDecoration,
                22 => SceneLayer::FloorDecoration,
                _ => SceneLayer::GameObject,
            };
            assert_eq!(layer, expected, "loc type {loc_type}");
        }
        assert_eq!(
            plan_placement(input(42, 0))?.kind.layer(),
            SceneLayer::GameObject
        );
        Ok(())
    }

    #[test]
    fn wall_decoration_displacement_uses_exact_full_half_and_opposite_rules(
    ) -> Result<(), PlacementError> {
        let mut full = input(5, 0);
        full.existing_wall_displacement = Some(34);
        let PlacementKind::WallDecoration(type5) = plan_placement(full)?.kind else {
            panic!("type 5 must be a wall decoration");
        };
        assert_eq!((type5.offset_x, type5.offset_z), (34, 0));

        let mut half = input(6, 1);
        half.existing_wall_displacement = Some(34);
        let PlacementKind::WallDecoration(type6) = plan_placement(half)?.kind else {
            panic!("type 6 must be a wall decoration");
        };
        assert_eq!((type6.offset_x, type6.offset_z), (-17, -17));
        assert_eq!(type6.primary.orientation, 5);
        assert_eq!(type6.orientation_flag, 256);
        assert_eq!(type6.orientation_parameter, 1);

        let PlacementKind::WallDecoration(type7) = plan_placement(input(7, 1))?.kind else {
            panic!("type 7 must be a wall decoration");
        };
        assert_eq!(type7.primary.orientation, 7);
        assert_eq!(type7.orientation_parameter, 3);
        assert_eq!((type7.offset_x, type7.offset_z), (0, 0));
        Ok(())
    }
}
