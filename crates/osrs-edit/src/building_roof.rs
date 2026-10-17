//! Semantic roof coverage, shape, and cell planning for generated buildings.
//!
//! Roof planning starts from the already-validated vertical building stack. This module owns exact
//! top-floor coverage, exposed boundary edges, deterministic high-level roof shape semantics, and
//! semantic ridge/slope/eave cell classification. It deliberately stops before WFC patterns and
//! concrete object IDs.

use crate::{BuildingFloorIndex, BuildingVerticalPlan, CardinalDirection};
use osrs_core::coords::{MapTile, SourcePlane};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RoofEdge {
    pub tile: MapTile,
    pub direction: CardinalDirection,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildingRoofPlan {
    covered_floor: BuildingFloorIndex,
    plane: SourcePlane,
    tiles: Vec<MapTile>,
    exposed_edges: Vec<RoofEdge>,
}

impl BuildingRoofPlan {
    pub const fn covered_floor(&self) -> BuildingFloorIndex {
        self.covered_floor
    }

    pub const fn plane(&self) -> SourcePlane {
        self.plane
    }

    pub fn tiles(&self) -> &[MapTile] {
        &self.tiles
    }

    pub fn exposed_edges(&self) -> &[RoofEdge] {
        &self.exposed_edges
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildingRoofError {
    MissingTopFloor,
    CoordinateOverflow {
        tile: MapTile,
        direction: CardinalDirection,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RoofRidgeAxis {
    NorthSouth,
    EastWest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RoofRidge {
    pub axis: RoofRidgeAxis,
    /// Cross-axis ridge coordinate in half-tile world units.
    ///
    /// A tile centered at integer map coordinate `n` has center coordinate `2 * n + 1`. An even
    /// value therefore places the ridge between tile-center rows/columns, while an odd value passes
    /// through one row/column of tile centers.
    pub cross_axis_coordinate_x2: i64,
}

impl RoofRidge {
    pub const fn passes_through_tile_centers(self) -> bool {
        self.cross_axis_coordinate_x2 & 1 != 0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RoofShape {
    /// Reserved for archetype/style overrides that intentionally use a flat roof.
    Flat,
    /// One ridge spanning the longer axis of a rectangular footprint.
    Gable { ridge: RoofRidge },
    /// The footprint requires later decomposition into multiple roof sections.
    Complex,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RoofCellKind {
    /// The geometric ridge passes directly through this tile's center.
    Ridge,
    /// An interior sloped tile. `downhill` points away from the ridge toward the eave.
    Slope { downhill: CardinalDirection },
    /// The final sloped tile at an exposed downhill roof boundary.
    Eave { downhill: CardinalDirection },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RoofCell {
    pub tile: MapTile,
    pub kind: RoofCellKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoofCellClassificationError {
    FlatRoofUnsupported,
    ComplexRoofUnsupported,
}

/// Derive exact roof coverage from the highest solved building floor.
///
/// Lower floors never expand the roof footprint. Every top-floor tile is covered once, while each
/// cardinal edge whose neighboring tile is outside the top-floor footprint is recorded as exposed.
pub fn plan_roof_coverage(
    vertical: &BuildingVerticalPlan,
) -> Result<BuildingRoofPlan, BuildingRoofError> {
    let Some(top_floor) = vertical.floors().last() else {
        return Err(BuildingRoofError::MissingTopFloor);
    };

    let tiles = top_floor.tiles().to_vec();
    let tile_set = tiles.iter().copied().collect::<BTreeSet<_>>();
    let mut exposed_edges = Vec::new();

    for &tile in &tiles {
        for direction in CardinalDirection::ALL {
            let Some(neighbor) = direction.neighbor(tile) else {
                return Err(BuildingRoofError::CoordinateOverflow { tile, direction });
            };
            if !tile_set.contains(&neighbor) {
                exposed_edges.push(RoofEdge { tile, direction });
            }
        }
    }

    Ok(BuildingRoofPlan {
        covered_floor: top_floor.floor,
        plane: top_floor.plane,
        tiles,
        exposed_edges,
    })
}

/// Classify the covered roof footprint and derive a deterministic primary ridge when possible.
///
/// Rectangles receive one gable ridge along their longer axis so slope travel is minimized. Square
/// footprints use an east-west ridge as a stable tie-break. Non-rectangular footprints are marked
/// complex instead of inventing an invalid single-ridge roof; a later checkpoint can decompose them
/// into compatible roof sections.
pub fn plan_roof_shape(roof: &BuildingRoofPlan) -> RoofShape {
    let Some(first) = roof.tiles.first().copied() else {
        return RoofShape::Complex;
    };

    let mut min_x = first.x;
    let mut max_x = first.x;
    let mut min_y = first.y;
    let mut max_y = first.y;
    for tile in roof.tiles.iter().copied().skip(1) {
        min_x = min_x.min(tile.x);
        max_x = max_x.max(tile.x);
        min_y = min_y.min(tile.y);
        max_y = max_y.max(tile.y);
    }

    let width = i64::from(max_x) - i64::from(min_x) + 1;
    let height = i64::from(max_y) - i64::from(min_y) + 1;
    let Ok(width_usize) = usize::try_from(width) else {
        return RoofShape::Complex;
    };
    let Ok(height_usize) = usize::try_from(height) else {
        return RoofShape::Complex;
    };
    if width_usize.checked_mul(height_usize) != Some(roof.tiles.len()) {
        return RoofShape::Complex;
    }

    let axis = if width >= height {
        RoofRidgeAxis::EastWest
    } else {
        RoofRidgeAxis::NorthSouth
    };
    let cross_axis_coordinate_x2 = match axis {
        RoofRidgeAxis::EastWest => i64::from(min_y) + i64::from(max_y) + 1,
        RoofRidgeAxis::NorthSouth => i64::from(min_x) + i64::from(max_x) + 1,
    };

    RoofShape::Gable {
        ridge: RoofRidge {
            axis,
            cross_axis_coordinate_x2,
        },
    }
}

/// Classify every covered tile of a simple gable roof relative to its geometric ridge.
///
/// Ridge cells exist only when the ridge actually passes through tile centers. For an even cross
/// span the ridge lies between two rows/columns, so neither adjacent row is mislabeled as a ridge.
/// Remaining tiles slope away from the ridge; the final tile whose downhill edge is exposed is an
/// eave. Complex and explicitly flat roofs are deferred to later specialized solvers.
pub fn classify_roof_cells(
    roof: &BuildingRoofPlan,
    shape: RoofShape,
) -> Result<Vec<RoofCell>, RoofCellClassificationError> {
    let ridge = match shape {
        RoofShape::Gable { ridge } => ridge,
        RoofShape::Flat => return Err(RoofCellClassificationError::FlatRoofUnsupported),
        RoofShape::Complex => return Err(RoofCellClassificationError::ComplexRoofUnsupported),
    };
    let exposed_edges = roof.exposed_edges.iter().copied().collect::<BTreeSet<_>>();
    let mut cells = Vec::with_capacity(roof.tiles.len());

    for &tile in &roof.tiles {
        let cross_axis_center_x2 = match ridge.axis {
            RoofRidgeAxis::EastWest => i64::from(tile.y) * 2 + 1,
            RoofRidgeAxis::NorthSouth => i64::from(tile.x) * 2 + 1,
        };
        let kind = if cross_axis_center_x2 == ridge.cross_axis_coordinate_x2 {
            RoofCellKind::Ridge
        } else {
            let downhill = match ridge.axis {
                RoofRidgeAxis::EastWest => {
                    if cross_axis_center_x2 < ridge.cross_axis_coordinate_x2 {
                        CardinalDirection::South
                    } else {
                        CardinalDirection::North
                    }
                }
                RoofRidgeAxis::NorthSouth => {
                    if cross_axis_center_x2 < ridge.cross_axis_coordinate_x2 {
                        CardinalDirection::West
                    } else {
                        CardinalDirection::East
                    }
                }
            };
            let edge = RoofEdge {
                tile,
                direction: downhill,
            };
            if exposed_edges.contains(&edge) {
                RoofCellKind::Eave { downhill }
            } else {
                RoofCellKind::Slope { downhill }
            }
        };
        cells.push(RoofCell { tile, kind });
    }

    Ok(cells)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        BuildingArchetype, BuildingConstraints, BuildingRequest, BuildingStyle, RectSite,
        plan_vertical_connectivity,
    };

    fn plane(value: u8) -> SourcePlane {
        SourcePlane::new(value).unwrap_or_else(|| unreachable!("test plane is valid"))
    }

    fn request(floors: u8) -> BuildingRequest {
        BuildingRequest {
            seed: 77,
            site: RectSite::new(MapTile::new(3200, 3200), 8, 8)
                .unwrap_or_else(|| unreachable!("test site is valid")),
            base_plane: plane(0),
            floors,
            archetype: BuildingArchetype::House,
            style: BuildingStyle::Generic,
            constraints: BuildingConstraints::default(),
        }
    }

    fn rectangle(origin: MapTile, width: i32, height: i32) -> Vec<MapTile> {
        let mut tiles = Vec::new();
        for y in 0..height {
            for x in 0..width {
                tiles.push(MapTile::new(origin.x + x, origin.y + y));
            }
        }
        tiles
    }

    fn roof_for(floor: Vec<MapTile>) -> BuildingRoofPlan {
        let vertical = plan_vertical_connectivity(request(1), &[floor])
            .unwrap_or_else(|error| unreachable!("valid vertical plan: {error:?}"));
        plan_roof_coverage(&vertical)
            .unwrap_or_else(|error| unreachable!("valid roof plan: {error:?}"))
    }

    fn cell_kind(cells: &[RoofCell], tile: MapTile) -> RoofCellKind {
        cells
            .iter()
            .find(|cell| cell.tile == tile)
            .map(|cell| cell.kind)
            .unwrap_or_else(|| unreachable!("requested tile is covered by roof"))
    }

    #[test]
    fn roof_covers_only_the_highest_floor() {
        let lower = rectangle(MapTile::new(3201, 3201), 5, 5);
        let upper = rectangle(MapTile::new(3202, 3202), 3, 2);
        let vertical = plan_vertical_connectivity(request(2), &[lower, upper.clone()])
            .unwrap_or_else(|error| unreachable!("valid vertical plan: {error:?}"));
        let roof = plan_roof_coverage(&vertical)
            .unwrap_or_else(|error| unreachable!("valid roof plan: {error:?}"));
        let mut expected = upper;
        expected.sort();
        expected.dedup();

        assert_eq!(roof.covered_floor(), BuildingFloorIndex::new(1));
        assert_eq!(roof.plane(), plane(1));
        assert_eq!(roof.tiles(), expected.as_slice());
    }

    #[test]
    fn rectangular_roof_records_only_exterior_edges() {
        let origin = MapTile::new(3202, 3202);
        let roof = roof_for(rectangle(origin, 2, 2));

        assert_eq!(roof.exposed_edges().len(), 8);
        assert!(roof.exposed_edges().contains(&RoofEdge {
            tile: origin,
            direction: CardinalDirection::South,
        }));
        assert!(roof.exposed_edges().contains(&RoofEdge {
            tile: origin,
            direction: CardinalDirection::West,
        }));
        assert!(!roof.exposed_edges().contains(&RoofEdge {
            tile: origin,
            direction: CardinalDirection::North,
        }));
        assert!(!roof.exposed_edges().contains(&RoofEdge {
            tile: origin,
            direction: CardinalDirection::East,
        }));
    }

    #[test]
    fn missing_neighbor_creates_an_exposed_roof_edge() {
        let south = MapTile::new(3202, 3202);
        let north = MapTile::new(3202, 3203);
        let east = MapTile::new(3203, 3202);
        let roof = roof_for(vec![south, north, east]);

        assert!(roof.exposed_edges().contains(&RoofEdge {
            tile: north,
            direction: CardinalDirection::East,
        }));
        assert!(roof.exposed_edges().contains(&RoofEdge {
            tile: east,
            direction: CardinalDirection::North,
        }));
    }

    #[test]
    fn identical_vertical_plan_produces_identical_roof_semantics() {
        let floor = rectangle(MapTile::new(3201, 3201), 4, 3);
        let vertical = plan_vertical_connectivity(request(1), &[floor])
            .unwrap_or_else(|error| unreachable!("valid vertical plan: {error:?}"));

        assert_eq!(plan_roof_coverage(&vertical), plan_roof_coverage(&vertical));
    }

    #[test]
    fn wide_rectangle_uses_east_west_ridge() {
        let roof = roof_for(rectangle(MapTile::new(3200, 3200), 5, 3));
        let RoofShape::Gable { ridge } = plan_roof_shape(&roof) else {
            unreachable!("rectangular footprint should have a gable ridge");
        };

        assert_eq!(ridge.axis, RoofRidgeAxis::EastWest);
        assert!(ridge.passes_through_tile_centers());
    }

    #[test]
    fn tall_rectangle_uses_north_south_ridge() {
        let roof = roof_for(rectangle(MapTile::new(3200, 3200), 3, 5));
        let RoofShape::Gable { ridge } = plan_roof_shape(&roof) else {
            unreachable!("rectangular footprint should have a gable ridge");
        };

        assert_eq!(ridge.axis, RoofRidgeAxis::NorthSouth);
        assert!(ridge.passes_through_tile_centers());
    }

    #[test]
    fn even_cross_span_places_ridge_between_tile_centers() {
        let roof = roof_for(rectangle(MapTile::new(3200, 3200), 4, 2));
        let RoofShape::Gable { ridge } = plan_roof_shape(&roof) else {
            unreachable!("rectangular footprint should have a gable ridge");
        };

        assert_eq!(ridge.axis, RoofRidgeAxis::EastWest);
        assert_eq!(ridge.cross_axis_coordinate_x2, 6402);
        assert!(!ridge.passes_through_tile_centers());
    }

    #[test]
    fn non_rectangular_footprint_is_marked_complex() {
        let floor = vec![
            MapTile::new(3200, 3200),
            MapTile::new(3201, 3200),
            MapTile::new(3200, 3201),
        ];
        let roof = roof_for(floor);

        assert_eq!(plan_roof_shape(&roof), RoofShape::Complex);
    }

    #[test]
    fn odd_span_gable_classifies_ridge_slopes_and_eaves() {
        let roof = roof_for(rectangle(MapTile::new(3200, 3200), 5, 5));
        let shape = plan_roof_shape(&roof);
        let cells = classify_roof_cells(&roof, shape)
            .unwrap_or_else(|error| unreachable!("simple gable classifies: {error:?}"));

        assert_eq!(
            cell_kind(&cells, MapTile::new(3202, 3202)),
            RoofCellKind::Ridge
        );
        assert_eq!(
            cell_kind(&cells, MapTile::new(3202, 3201)),
            RoofCellKind::Slope {
                downhill: CardinalDirection::South,
            }
        );
        assert_eq!(
            cell_kind(&cells, MapTile::new(3202, 3200)),
            RoofCellKind::Eave {
                downhill: CardinalDirection::South,
            }
        );
        assert_eq!(
            cell_kind(&cells, MapTile::new(3202, 3203)),
            RoofCellKind::Slope {
                downhill: CardinalDirection::North,
            }
        );
        assert_eq!(
            cell_kind(&cells, MapTile::new(3202, 3204)),
            RoofCellKind::Eave {
                downhill: CardinalDirection::North,
            }
        );
    }

    #[test]
    fn even_span_gable_does_not_invent_ridge_cells() {
        let roof = roof_for(rectangle(MapTile::new(3200, 3200), 4, 4));
        let shape = plan_roof_shape(&roof);
        let cells = classify_roof_cells(&roof, shape)
            .unwrap_or_else(|error| unreachable!("simple gable classifies: {error:?}"));

        assert!(!cells.iter().any(|cell| cell.kind == RoofCellKind::Ridge));
        assert_eq!(
            cell_kind(&cells, MapTile::new(3201, 3201)),
            RoofCellKind::Slope {
                downhill: CardinalDirection::South,
            }
        );
        assert_eq!(
            cell_kind(&cells, MapTile::new(3201, 3202)),
            RoofCellKind::Slope {
                downhill: CardinalDirection::North,
            }
        );
        assert_eq!(
            cell_kind(&cells, MapTile::new(3201, 3200)),
            RoofCellKind::Eave {
                downhill: CardinalDirection::South,
            }
        );
        assert_eq!(
            cell_kind(&cells, MapTile::new(3201, 3203)),
            RoofCellKind::Eave {
                downhill: CardinalDirection::North,
            }
        );
    }

    #[test]
    fn north_south_ridge_assigns_east_and_west_downhill_directions() {
        let roof = roof_for(rectangle(MapTile::new(3200, 3200), 5, 7));
        let shape = plan_roof_shape(&roof);
        let cells = classify_roof_cells(&roof, shape)
            .unwrap_or_else(|error| unreachable!("simple gable classifies: {error:?}"));

        assert_eq!(
            cell_kind(&cells, MapTile::new(3202, 3203)),
            RoofCellKind::Ridge
        );
        assert_eq!(
            cell_kind(&cells, MapTile::new(3201, 3203)),
            RoofCellKind::Slope {
                downhill: CardinalDirection::West,
            }
        );
        assert_eq!(
            cell_kind(&cells, MapTile::new(3200, 3203)),
            RoofCellKind::Eave {
                downhill: CardinalDirection::West,
            }
        );
        assert_eq!(
            cell_kind(&cells, MapTile::new(3203, 3203)),
            RoofCellKind::Slope {
                downhill: CardinalDirection::East,
            }
        );
        assert_eq!(
            cell_kind(&cells, MapTile::new(3204, 3203)),
            RoofCellKind::Eave {
                downhill: CardinalDirection::East,
            }
        );
    }

    #[test]
    fn complex_roof_defers_cell_classification() {
        let roof = roof_for(vec![
            MapTile::new(3200, 3200),
            MapTile::new(3201, 3200),
            MapTile::new(3200, 3201),
        ]);

        assert_eq!(
            classify_roof_cells(&roof, plan_roof_shape(&roof)),
            Err(RoofCellClassificationError::ComplexRoofUnsupported)
        );
    }
}
