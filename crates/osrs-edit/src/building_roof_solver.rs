//! Deterministic WFC solving for semantic roof patterns.
//!
//! This layer executes the generic WFC solver over a simple roof's exact rectangular footprint and
//! maps solved semantic pattern IDs back to world tiles. It deliberately stops before resolving
//! those pattern IDs into RuneScape object IDs or map edits.

use crate::{
    BuildingRoofPlan, RectSite, RoofConstraintError, RoofPatternSet, RoofShape, WfcCatalog,
    WfcCoord, WfcPatternId, WfcSolveError, roof_wfc_constraints, solve_wfc,
};
use osrs_core::coords::MapTile;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RoofPatternSelection {
    pub tile: MapTile,
    pub pattern: WfcPatternId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoofWfcSolution {
    bounds: RectSite,
    selections: Vec<RoofPatternSelection>,
    backtracks: u32,
}

impl RoofWfcSolution {
    pub const fn bounds(&self) -> RectSite {
        self.bounds
    }

    pub fn selections(&self) -> &[RoofPatternSelection] {
        &self.selections
    }

    pub const fn backtracks(&self) -> u32 {
        self.backtracks
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoofWfcSolveError {
    EmptyRoof,
    InvalidRoofBounds,
    NonRectangularRoofGrid,
    Constraint(RoofConstraintError),
    Solve(WfcSolveError),
    MissingSolvedPattern { tile: MapTile },
}

/// Solve semantic roof pattern choices over the roof's exact rectangular footprint.
///
/// The generic WFC grid is intentionally cropped to the roof bounds rather than the full building
/// site. This prevents unconstrained non-roof cells from participating in socket propagation. The
/// result contains semantic pattern IDs only; concrete object realization remains a later stage.
pub fn solve_roof_wfc(
    catalog: &WfcCatalog,
    roof: &BuildingRoofPlan,
    shape: RoofShape,
    patterns: RoofPatternSet,
    seed: u64,
    max_backtracks: u32,
) -> Result<RoofWfcSolution, RoofWfcSolveError> {
    let bounds = roof_bounds(roof)?;
    let expected_cells = usize::from(bounds.width()) * usize::from(bounds.height());
    if roof.tiles().len() != expected_cells {
        return Err(RoofWfcSolveError::NonRectangularRoofGrid);
    }

    let constraints = roof_wfc_constraints(bounds, roof, shape, patterns)
        .map_err(RoofWfcSolveError::Constraint)?;
    let solved = solve_wfc(
        catalog,
        bounds.width(),
        bounds.height(),
        seed,
        &constraints,
        max_backtracks,
    )
    .map_err(RoofWfcSolveError::Solve)?;

    let origin = bounds.origin();
    let mut selections = Vec::with_capacity(roof.tiles().len());
    for &tile in roof.tiles() {
        let x = tile
            .x
            .checked_sub(origin.x)
            .and_then(|value| u16::try_from(value).ok())
            .ok_or(RoofWfcSolveError::InvalidRoofBounds)?;
        let y = tile
            .y
            .checked_sub(origin.y)
            .and_then(|value| u16::try_from(value).ok())
            .ok_or(RoofWfcSolveError::InvalidRoofBounds)?;
        let pattern = solved
            .at(WfcCoord::new(x, y))
            .ok_or(RoofWfcSolveError::MissingSolvedPattern { tile })?;
        selections.push(RoofPatternSelection { tile, pattern });
    }
    selections.sort_by_key(|selection| selection.tile);

    Ok(RoofWfcSolution {
        bounds,
        selections,
        backtracks: solved.backtracks(),
    })
}

fn roof_bounds(roof: &BuildingRoofPlan) -> Result<RectSite, RoofWfcSolveError> {
    let Some(first) = roof.tiles().first().copied() else {
        return Err(RoofWfcSolveError::EmptyRoof);
    };

    let mut min_x = first.x;
    let mut max_x = first.x;
    let mut min_y = first.y;
    let mut max_y = first.y;
    for tile in roof.tiles().iter().copied().skip(1) {
        min_x = min_x.min(tile.x);
        max_x = max_x.max(tile.x);
        min_y = min_y.min(tile.y);
        max_y = max_y.max(tile.y);
    }

    let width = i64::from(max_x) - i64::from(min_x) + 1;
    let height = i64::from(max_y) - i64::from(min_y) + 1;
    let width = u16::try_from(width).map_err(|_| RoofWfcSolveError::InvalidRoofBounds)?;
    let height = u16::try_from(height).map_err(|_| RoofWfcSolveError::InvalidRoofBounds)?;
    RectSite::new(MapTile::new(min_x, min_y), width, height)
        .ok_or(RoofWfcSolveError::InvalidRoofBounds)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        BuildingArchetype, BuildingConstraints, BuildingRequest, BuildingStyle, PatternDomain,
        RoofCellKind, WfcPatternSpec, WfcSocket, classify_roof_cells, plan_roof_coverage,
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

    #[test]
    fn solve_returns_one_allowed_pattern_per_roof_tile() {
        let roof = roof_for(rectangle(MapTile::new(3200, 3200), 5, 5));
        let shape = plan_roof_shape(&roof);
        let pattern_set = patterns();
        let solved = solve_roof_wfc(&catalog(10), &roof, shape, pattern_set, 91, 32)
            .unwrap_or_else(|error| unreachable!("roof solves: {error:?}"));
        let cells = classify_roof_cells(&roof, shape)
            .unwrap_or_else(|error| unreachable!("roof classifies: {error:?}"));

        assert_eq!(solved.selections().len(), roof.tiles().len());
        for selection in solved.selections() {
            let kind = cells
                .iter()
                .find(|cell| cell.tile == selection.tile)
                .map(|cell| cell.kind)
                .unwrap_or_else(|| unreachable!("selection tile has semantic cell"));
            assert!(pattern_set.domain_for(kind).contains(selection.pattern));
        }
    }

    #[test]
    fn identical_seed_produces_identical_roof_solution() {
        let roof = roof_for(rectangle(MapTile::new(3200, 3200), 5, 5));
        let shape = plan_roof_shape(&roof);
        let catalog = catalog(10);

        assert_eq!(
            solve_roof_wfc(&catalog, &roof, shape, patterns(), 1234, 32),
            solve_roof_wfc(&catalog, &roof, shape, patterns(), 1234, 32)
        );
    }

    #[test]
    fn solve_grid_is_cropped_to_offset_roof_bounds() {
        let roof = roof_for(rectangle(MapTile::new(3202, 3203), 3, 3));
        let solved = solve_roof_wfc(
            &catalog(10),
            &roof,
            plan_roof_shape(&roof),
            patterns(),
            7,
            32,
        )
        .unwrap_or_else(|error| unreachable!("offset roof solves: {error:?}"));

        assert_eq!(solved.bounds().origin(), MapTile::new(3202, 3203));
        assert_eq!(solved.bounds().width(), 3);
        assert_eq!(solved.bounds().height(), 3);
        assert_eq!(
            solved.selections().first().map(|selection| selection.tile),
            Some(MapTile::new(3202, 3203))
        );
        assert_eq!(
            solved.selections().last().map(|selection| selection.tile),
            Some(MapTile::new(3204, 3205))
        );
    }

    #[test]
    fn catalog_missing_required_semantic_patterns_is_reported() {
        let roof = roof_for(rectangle(MapTile::new(3200, 3200), 5, 5));

        assert_eq!(
            solve_roof_wfc(
                &catalog(2),
                &roof,
                plan_roof_shape(&roof),
                patterns(),
                7,
                32,
            ),
            Err(RoofWfcSolveError::Solve(
                WfcSolveError::ConstraintHasNoValidPatterns {
                    coord: WfcCoord::new(0, 0),
                }
            ))
        );
    }

    #[test]
    fn irregular_grid_is_rejected_before_unconstrained_holes_can_participate() {
        let roof = roof_for(vec![
            MapTile::new(3200, 3200),
            MapTile::new(3201, 3200),
            MapTile::new(3200, 3201),
        ]);

        assert_eq!(
            solve_roof_wfc(
                &catalog(10),
                &roof,
                RoofShape::Gable {
                    ridge: crate::RoofRidge {
                        axis: crate::RoofRidgeAxis::EastWest,
                        cross_axis_coordinate_x2: 6402,
                    },
                },
                patterns(),
                7,
                32,
            ),
            Err(RoofWfcSolveError::NonRectangularRoofGrid)
        );
    }

    #[test]
    fn ridge_variation_remains_semantic_after_solving() {
        let roof = roof_for(rectangle(MapTile::new(3200, 3200), 5, 5));
        let solved = solve_roof_wfc(
            &catalog(10),
            &roof,
            plan_roof_shape(&roof),
            patterns(),
            91,
            32,
        )
        .unwrap_or_else(|error| unreachable!("roof solves: {error:?}"));

        let center = solved
            .selections()
            .iter()
            .find(|selection| selection.tile == MapTile::new(3202, 3202))
            .copied()
            .unwrap_or_else(|| unreachable!("center roof tile is selected"));
        assert!(matches!(center.pattern.get(), 0 | 1));
        assert_eq!(
            classify_roof_cells(&roof, plan_roof_shape(&roof))
                .unwrap_or_else(|error| unreachable!("roof classifies: {error:?}"))
                .iter()
                .find(|cell| cell.tile == center.tile)
                .map(|cell| cell.kind),
            Some(RoofCellKind::Ridge)
        );
    }
}
