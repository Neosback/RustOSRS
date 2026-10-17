//! Semantic cell classification for decomposed roof sections.
//!
//! This layer applies each section-local ridge to the exact tiles in that rectangular section.
//! Ridge and slope direction are section-local, while eaves remain authoritative to the original
//! whole-roof exposed edges so decomposition seams are never mislabeled as exterior eaves. It does
//! not run WFC, join valleys, resolve assets, or build map edits.

use crate::{
    BuildingRoofPlan, CardinalDirection, RoofCell, RoofCellKind, RoofEdge, RoofRidgeAxis,
    RoofSection, RoofSectionShape, RoofShape,
};
use osrs_core::coords::MapTile;
use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoofSectionCellPlan {
    section: RoofSection,
    shape: RoofShape,
    cells: Vec<RoofCell>,
}

impl RoofSectionCellPlan {
    pub const fn section(&self) -> RoofSection {
        self.section
    }

    pub const fn shape(&self) -> RoofShape {
        self.shape
    }

    pub fn cells(&self) -> &[RoofCell] {
        &self.cells
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoofSectionCellError {
    SectionMetadataMismatch {
        section: RoofSection,
    },
    SectionTileOutsideRoof {
        section: RoofSection,
        tile: MapTile,
    },
    CoordinateOverflow {
        section: RoofSection,
        x: u16,
        y: u16,
    },
    UnsupportedShape {
        section: RoofSection,
        shape: RoofShape,
    },
}

/// Classify ridge, slope, and exterior-eave semantics inside each rectangular roof section.
///
/// Section-local shapes determine ridge position and downhill direction. A downhill boundary is an
/// `Eave` only when the same edge is exposed on the original whole roof; a boundary introduced only
/// by rectangular decomposition remains a `Slope` for later valley/intersection planning.
pub fn classify_roof_section_cells(
    roof: &BuildingRoofPlan,
    section_shapes: &[RoofSectionShape],
) -> Result<Vec<RoofSectionCellPlan>, RoofSectionCellError> {
    let roof_tiles = roof.tiles().iter().copied().collect::<BTreeSet<_>>();
    let exposed_edges = roof
        .exposed_edges()
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let mut ordered = section_shapes.to_vec();
    ordered.sort_by_key(|planned| section_sort_key(planned.section()));

    let mut plans = Vec::with_capacity(ordered.len());
    for planned in ordered {
        let section = planned.section();
        if section.covered_floor() != roof.covered_floor() || section.plane() != roof.plane() {
            return Err(RoofSectionCellError::SectionMetadataMismatch { section });
        }

        let ridge = match planned.shape() {
            RoofShape::Gable { ridge } => ridge,
            shape => {
                return Err(RoofSectionCellError::UnsupportedShape { section, shape });
            }
        };
        let bounds = section.bounds();
        let origin = bounds.origin();
        let mut cells = Vec::with_capacity(
            usize::from(bounds.width()).saturating_mul(usize::from(bounds.height())),
        );

        for y in 0..bounds.height() {
            let tile_y = origin
                .y
                .checked_add(i32::from(y))
                .ok_or(RoofSectionCellError::CoordinateOverflow { section, x: 0, y })?;
            for x in 0..bounds.width() {
                let tile_x = origin
                    .x
                    .checked_add(i32::from(x))
                    .ok_or(RoofSectionCellError::CoordinateOverflow { section, x, y })?;
                let tile = MapTile::new(tile_x, tile_y);
                if !roof_tiles.contains(&tile) {
                    return Err(RoofSectionCellError::SectionTileOutsideRoof { section, tile });
                }

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
        }
        cells.sort_by_key(|cell| cell.tile);
        plans.push(RoofSectionCellPlan {
            section,
            shape: planned.shape(),
            cells,
        });
    }

    Ok(plans)
}

fn section_sort_key(section: RoofSection) -> (i32, i32, u16, u16) {
    let bounds = section.bounds();
    (
        bounds.origin().y,
        bounds.origin().x,
        bounds.height(),
        bounds.width(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        BuildingArchetype, BuildingConstraints, BuildingRequest, BuildingStyle, RectSite,
        classify_roof_cells, decompose_roof_sections, plan_roof_coverage, plan_roof_section_shapes,
        plan_roof_shape, plan_vertical_connectivity,
    };
    use osrs_core::coords::SourcePlane;

    fn plane(value: u8) -> SourcePlane {
        SourcePlane::new(value).unwrap_or_else(|| unreachable!("test plane is valid"))
    }

    fn request() -> BuildingRequest {
        BuildingRequest {
            seed: 77,
            site: RectSite::new(MapTile::new(3200, 3200), 8, 8)
                .unwrap_or_else(|| unreachable!("test site is valid")),
            base_plane: plane(0),
            floors: 1,
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
        let vertical = plan_vertical_connectivity(request(), &[floor])
            .unwrap_or_else(|error| unreachable!("valid vertical plan: {error:?}"));
        plan_roof_coverage(&vertical)
            .unwrap_or_else(|error| unreachable!("valid roof plan: {error:?}"))
    }

    fn section_plans(roof: &BuildingRoofPlan) -> Vec<RoofSectionCellPlan> {
        let sections = decompose_roof_sections(roof)
            .unwrap_or_else(|error| unreachable!("roof decomposes: {error:?}"));
        let shapes = plan_roof_section_shapes(&sections);
        classify_roof_section_cells(roof, &shapes)
            .unwrap_or_else(|error| unreachable!("roof section cells classify: {error:?}"))
    }

    fn kind_at(plans: &[RoofSectionCellPlan], tile: MapTile) -> RoofCellKind {
        plans
            .iter()
            .flat_map(|plan| plan.cells())
            .find(|cell| cell.tile == tile)
            .map(|cell| cell.kind)
            .unwrap_or_else(|| unreachable!("requested tile belongs to section plans"))
    }

    #[test]
    fn one_section_classification_matches_simple_whole_roof_semantics() {
        let roof = roof_for(rectangle(MapTile::new(3201, 3201), 5, 3));
        let plans = section_plans(&roof);
        let mut section_cells = plans[0].cells().to_vec();
        let mut whole_cells = classify_roof_cells(&roof, plan_roof_shape(&roof))
            .unwrap_or_else(|error| unreachable!("simple roof classifies: {error:?}"));
        section_cells.sort();
        whole_cells.sort();

        assert_eq!(plans.len(), 1);
        assert_eq!(section_cells, whole_cells);
    }

    #[test]
    fn internal_section_boundary_is_slope_not_fake_eave() {
        let mut floor = rectangle(MapTile::new(3200, 3200), 5, 1);
        floor.extend(rectangle(MapTile::new(3201, 3201), 3, 3));
        let roof = roof_for(floor);
        assert_eq!(plan_roof_shape(&roof), RoofShape::Complex);
        let plans = section_plans(&roof);

        assert_eq!(
            kind_at(&plans, MapTile::new(3201, 3201)),
            RoofCellKind::Slope {
                downhill: CardinalDirection::South,
            }
        );
    }

    #[test]
    fn true_whole_roof_boundary_remains_eave() {
        let mut floor = rectangle(MapTile::new(3200, 3200), 5, 1);
        floor.extend(rectangle(MapTile::new(3201, 3201), 3, 3));
        let roof = roof_for(floor);
        let plans = section_plans(&roof);

        assert_eq!(
            kind_at(&plans, MapTile::new(3201, 3203)),
            RoofCellKind::Eave {
                downhill: CardinalDirection::North,
            }
        );
    }

    #[test]
    fn classification_is_independent_of_section_shape_input_order() {
        let mut floor = rectangle(MapTile::new(3200, 3200), 5, 1);
        floor.extend(rectangle(MapTile::new(3201, 3201), 3, 3));
        let roof = roof_for(floor);
        let sections = decompose_roof_sections(&roof)
            .unwrap_or_else(|error| unreachable!("roof decomposes: {error:?}"));
        let shapes = plan_roof_section_shapes(&sections);
        let mut reversed = shapes.clone();
        reversed.reverse();

        assert_eq!(
            classify_roof_section_cells(&roof, &shapes),
            classify_roof_section_cells(&roof, &reversed)
        );
    }

    #[test]
    fn section_from_another_roof_is_rejected_before_classification() {
        let roof_a = roof_for(rectangle(MapTile::new(3200, 3200), 3, 3));
        let roof_b = roof_for(rectangle(MapTile::new(3204, 3204), 3, 3));
        let sections = decompose_roof_sections(&roof_a)
            .unwrap_or_else(|error| unreachable!("roof decomposes: {error:?}"));
        let shapes = plan_roof_section_shapes(&sections);

        assert_eq!(
            classify_roof_section_cells(&roof_b, &shapes),
            Err(RoofSectionCellError::SectionTileOutsideRoof {
                section: sections[0],
                tile: MapTile::new(3200, 3200),
            })
        );
    }
}
