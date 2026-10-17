//! Deterministic WFC solving for decomposed semantic roof sections.
//!
//! Each rectangular section is solved as an independent local WFC grid, then the resulting
//! semantic pattern IDs are mapped back to world tiles. This layer deliberately does not join
//! valleys/intersections, resolve concrete assets, or build map edits.

use crate::{
    RoofPatternSelection, RoofSection, RoofSectionWfcPlan, WfcCatalog, WfcCoord, WfcSolveError,
    solve_wfc,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoofSectionWfcSolution {
    section: RoofSection,
    selections: Vec<RoofPatternSelection>,
    backtracks: u32,
}

impl RoofSectionWfcSolution {
    pub const fn section(&self) -> RoofSection {
        self.section
    }

    pub fn selections(&self) -> &[RoofPatternSelection] {
        &self.selections
    }

    pub const fn backtracks(&self) -> u32 {
        self.backtracks
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoofSectionWfcSolveError {
    Solve {
        section: RoofSection,
        error: WfcSolveError,
    },
    CoordinateOverflow {
        section: RoofSection,
        coord: WfcCoord,
    },
    MissingSolvedPattern {
        section: RoofSection,
        coord: WfcCoord,
    },
}

/// Solve every decomposed roof section independently with the generic WFC solver.
///
/// Section plans are normalized into deterministic world-space order before solving. Every local
/// `(x, y)` solution coordinate is then mapped back through that section's bounds origin. The
/// returned selections remain semantic pattern IDs only.
pub fn solve_roof_section_wfc(
    catalog: &WfcCatalog,
    section_plans: &[RoofSectionWfcPlan],
    seed: u64,
    max_backtracks: u32,
) -> Result<Vec<RoofSectionWfcSolution>, RoofSectionWfcSolveError> {
    let mut ordered = section_plans.iter().collect::<Vec<_>>();
    ordered.sort_by_key(|plan| section_sort_key(plan.section()));

    let mut result = Vec::with_capacity(ordered.len());
    for plan in ordered {
        let section = plan.section();
        let bounds = section.bounds();
        let solved = solve_wfc(
            catalog,
            bounds.width(),
            bounds.height(),
            seed,
            plan.constraints(),
            max_backtracks,
        )
        .map_err(|error| RoofSectionWfcSolveError::Solve { section, error })?;

        let origin = bounds.origin();
        let mut selections =
            Vec::with_capacity(usize::from(bounds.width()) * usize::from(bounds.height()));
        for y in 0..bounds.height() {
            for x in 0..bounds.width() {
                let coord = WfcCoord::new(x, y);
                let tile_x = origin
                    .x
                    .checked_add(i32::from(x))
                    .ok_or(RoofSectionWfcSolveError::CoordinateOverflow { section, coord })?;
                let tile_y = origin
                    .y
                    .checked_add(i32::from(y))
                    .ok_or(RoofSectionWfcSolveError::CoordinateOverflow { section, coord })?;
                let pattern = solved
                    .at(coord)
                    .ok_or(RoofSectionWfcSolveError::MissingSolvedPattern { section, coord })?;
                selections.push(RoofPatternSelection {
                    tile: osrs_core::coords::MapTile::new(tile_x, tile_y),
                    pattern,
                });
            }
        }
        selections.sort_by_key(|selection| selection.tile);
        result.push(RoofSectionWfcSolution {
            section,
            selections,
            backtracks: solved.backtracks(),
        });
    }

    Ok(result)
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
        BuildingArchetype, BuildingConstraints, BuildingRequest, BuildingRoofPlan, BuildingStyle,
        PatternDomain, RectSite, RoofPatternSet, RoofSectionCellPlan, WfcPatternId, WfcPatternSpec,
        WfcSocket, classify_roof_section_cells, decompose_roof_sections, plan_roof_coverage,
        plan_roof_section_shapes, plan_vertical_connectivity, roof_section_wfc_constraints,
    };
    use osrs_core::coords::{MapTile, SourcePlane};

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

    fn complex_roof() -> BuildingRoofPlan {
        let mut floor = rectangle(MapTile::new(3200, 3200), 5, 1);
        floor.extend(rectangle(MapTile::new(3201, 3201), 3, 3));
        roof_for(floor)
    }

    fn pattern(value: u8) -> WfcPatternId {
        WfcPatternId::new(value).unwrap_or_else(|| unreachable!("test pattern id is valid"))
    }

    fn domain(values: &[u8]) -> PatternDomain {
        PatternDomain::from_patterns(values.iter().copied().map(pattern))
    }

    fn patterns() -> RoofPatternSet {
        RoofPatternSet {
            ridge: domain(&[0, 1]),
            slopes: [domain(&[2]), domain(&[3]), domain(&[4]), domain(&[5])],
            eaves: [domain(&[6]), domain(&[7]), domain(&[8]), domain(&[9])],
        }
    }

    fn catalog(pattern_count: usize) -> WfcCatalog {
        WfcCatalog::new((0..pattern_count).map(|index| WfcPatternSpec {
            weight: u32::try_from(index + 1).unwrap_or_else(|_| unreachable!("small test catalog")),
            sockets: [WfcSocket(1); 4],
        }))
        .unwrap_or_else(|error| unreachable!("valid test catalog: {error:?}"))
    }

    fn section_cells(roof: &BuildingRoofPlan) -> Vec<RoofSectionCellPlan> {
        let sections = decompose_roof_sections(roof)
            .unwrap_or_else(|error| unreachable!("roof decomposes: {error:?}"));
        let shapes = plan_roof_section_shapes(&sections);
        classify_roof_section_cells(roof, &shapes)
            .unwrap_or_else(|error| unreachable!("roof section cells classify: {error:?}"))
    }

    fn section_plans(roof: &BuildingRoofPlan) -> Vec<RoofSectionWfcPlan> {
        roof_section_wfc_constraints(&section_cells(roof), patterns())
            .unwrap_or_else(|error| unreachable!("section constraints are valid: {error:?}"))
    }

    #[test]
    fn solve_returns_one_pattern_per_section_tile() {
        let roof = complex_roof();
        let plans = section_plans(&roof);
        let solved = solve_roof_section_wfc(&catalog(10), &plans, 91, 32)
            .unwrap_or_else(|error| unreachable!("roof sections solve: {error:?}"));

        assert_eq!(solved.len(), plans.len());
        for solution in &solved {
            let bounds = solution.section().bounds();
            assert_eq!(
                solution.selections().len(),
                usize::from(bounds.width()) * usize::from(bounds.height())
            );
            assert!(
                solution
                    .selections()
                    .iter()
                    .all(|selection| bounds.contains(selection.tile))
            );
        }
    }

    #[test]
    fn identical_seed_produces_identical_section_solutions() {
        let roof = complex_roof();
        let plans = section_plans(&roof);
        let catalog = catalog(10);

        assert_eq!(
            solve_roof_section_wfc(&catalog, &plans, 1234, 32),
            solve_roof_section_wfc(&catalog, &plans, 1234, 32)
        );
    }

    #[test]
    fn local_section_solutions_map_back_to_world_tiles() {
        let roof = complex_roof();
        let solved = solve_roof_section_wfc(&catalog(10), &section_plans(&roof), 7, 32)
            .unwrap_or_else(|error| unreachable!("roof sections solve: {error:?}"));

        assert_eq!(solved.len(), 2);
        assert_eq!(
            solved[0]
                .selections()
                .first()
                .map(|selection| selection.tile),
            Some(MapTile::new(3200, 3200))
        );
        assert_eq!(
            solved[0]
                .selections()
                .last()
                .map(|selection| selection.tile),
            Some(MapTile::new(3204, 3200))
        );
        assert_eq!(
            solved[1]
                .selections()
                .first()
                .map(|selection| selection.tile),
            Some(MapTile::new(3201, 3201))
        );
        assert_eq!(
            solved[1]
                .selections()
                .last()
                .map(|selection| selection.tile),
            Some(MapTile::new(3203, 3203))
        );
    }

    #[test]
    fn solve_error_reports_the_exact_section() {
        let roof = complex_roof();
        let plans = section_plans(&roof);
        let expected_section = plans[1].section();

        assert_eq!(
            solve_roof_section_wfc(&catalog(2), &plans, 7, 32),
            Err(RoofSectionWfcSolveError::Solve {
                section: expected_section,
                error: WfcSolveError::ConstraintHasNoValidPatterns {
                    coord: WfcCoord::new(0, 0),
                },
            })
        );
    }

    #[test]
    fn solution_order_is_independent_of_section_plan_input_order() {
        let roof = complex_roof();
        let plans = section_plans(&roof);
        let mut reversed = plans.clone();
        reversed.reverse();
        let catalog = catalog(10);

        assert_eq!(
            solve_roof_section_wfc(&catalog, &plans, 99, 32),
            solve_roof_section_wfc(&catalog, &reversed, 99, 32)
        );
    }
}
