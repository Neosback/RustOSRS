//! Section-local WFC constraints for decomposed semantic roofs.
//!
//! Each rectangular roof section becomes an independent local WFC grid. The already-classified
//! ridge, slope, and true exterior-eave semantics are translated through the existing
//! `RoofPatternSet`; this layer does not solve WFC, join valleys/intersections, resolve assets, or
//! build map edits.

use crate::{
    RoofCell, RoofPatternSet, RoofSection, RoofSectionCellPlan, WfcConstraint, WfcCoord,
};
use osrs_core::coords::MapTile;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoofSectionWfcPlan {
    section: RoofSection,
    constraints: Vec<WfcConstraint>,
}

impl RoofSectionWfcPlan {
    pub const fn section(&self) -> RoofSection {
        self.section
    }

    pub fn constraints(&self) -> &[WfcConstraint] {
        &self.constraints
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoofSectionConstraintError {
    EmptyPatternDomain {
        section: RoofSection,
        cell: RoofCell,
    },
    InvalidSectionTile {
        section: RoofSection,
        tile: MapTile,
    },
}

/// Translate classified section cells into constraints over each section's own local WFC grid.
///
/// Every section is independent: `(0, 0)` always means that section's bounds origin. Domains are
/// validated lazily per used semantic cell, preserving the same behavior as simple-roof WFC
/// constraints for even spans that contain no ridge cells.
pub fn roof_section_wfc_constraints(
    section_plans: &[RoofSectionCellPlan],
    patterns: RoofPatternSet,
) -> Result<Vec<RoofSectionWfcPlan>, RoofSectionConstraintError> {
    let mut ordered = section_plans.iter().collect::<Vec<_>>();
    ordered.sort_by_key(|plan| section_sort_key(plan.section()));

    let mut result = Vec::with_capacity(ordered.len());
    for plan in ordered {
        let section = plan.section();
        let mut constraints = Vec::with_capacity(plan.cells().len());
        for &cell in plan.cells() {
            let allowed = patterns.domain_for(cell.kind);
            if allowed.is_empty() {
                return Err(RoofSectionConstraintError::EmptyPatternDomain { section, cell });
            }
            let coord = local_section_coord(section, cell.tile).ok_or(
                RoofSectionConstraintError::InvalidSectionTile {
                    section,
                    tile: cell.tile,
                },
            )?;
            constraints.push(WfcConstraint { coord, allowed });
        }
        constraints.sort_by_key(|constraint| constraint.coord);
        result.push(RoofSectionWfcPlan {
            section,
            constraints,
        });
    }

    Ok(result)
}

fn local_section_coord(section: RoofSection, tile: MapTile) -> Option<WfcCoord> {
    let bounds = section.bounds();
    if !bounds.contains(tile) {
        return None;
    }
    let origin = bounds.origin();
    let x = u16::try_from(tile.x.checked_sub(origin.x)?).ok()?;
    let y = u16::try_from(tile.y.checked_sub(origin.y)?).ok()?;
    Some(WfcCoord::new(x, y))
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
        PatternDomain, RectSite, RoofCellKind, WfcPatternId, classify_roof_section_cells,
        decompose_roof_sections, plan_roof_coverage, plan_roof_section_shapes,
        plan_vertical_connectivity,
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

    fn section_cells(roof: &BuildingRoofPlan) -> Vec<RoofSectionCellPlan> {
        let sections = decompose_roof_sections(roof)
            .unwrap_or_else(|error| unreachable!("roof decomposes: {error:?}"));
        let shapes = plan_roof_section_shapes(&sections);
        classify_roof_section_cells(roof, &shapes)
            .unwrap_or_else(|error| unreachable!("roof section cells classify: {error:?}"))
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

    fn complex_roof() -> BuildingRoofPlan {
        let mut floor = rectangle(MapTile::new(3200, 3200), 5, 1);
        floor.extend(rectangle(MapTile::new(3201, 3201), 3, 3));
        roof_for(floor)
    }

    fn constraint_at(constraints: &[WfcConstraint], coord: WfcCoord) -> WfcConstraint {
        constraints
            .iter()
            .find(|constraint| constraint.coord == coord)
            .copied()
            .unwrap_or_else(|| unreachable!("section tile has a constraint"))
    }

    #[test]
    fn each_section_uses_its_own_local_grid() {
        let roof = complex_roof();
        let plans = roof_section_wfc_constraints(&section_cells(&roof), patterns())
            .unwrap_or_else(|error| unreachable!("section constraints are valid: {error:?}"));

        assert_eq!(plans.len(), 2);
        assert_eq!(
            plans[0].section().bounds().origin(),
            MapTile::new(3200, 3200)
        );
        assert_eq!(
            plans[1].section().bounds().origin(),
            MapTile::new(3201, 3201)
        );
        assert_eq!(
            plans[0]
                .constraints()
                .first()
                .map(|constraint| constraint.coord),
            Some(WfcCoord::new(0, 0))
        );
        assert_eq!(
            plans[1]
                .constraints()
                .first()
                .map(|constraint| constraint.coord),
            Some(WfcCoord::new(0, 0))
        );
        assert_eq!(
            plans[1]
                .constraints()
                .last()
                .map(|constraint| constraint.coord),
            Some(WfcCoord::new(2, 2))
        );
    }

    #[test]
    fn section_ridge_preserves_multi_pattern_domain() {
        let roof = complex_roof();
        let plans = roof_section_wfc_constraints(&section_cells(&roof), patterns())
            .unwrap_or_else(|error| unreachable!("section constraints are valid: {error:?}"));
        let ridge = constraint_at(plans[1].constraints(), WfcCoord::new(1, 1));

        assert_eq!(ridge.allowed.len(), 2);
        assert!(ridge.allowed.contains(pattern(0)));
        assert!(ridge.allowed.contains(pattern(1)));
    }

    #[test]
    fn internal_decomposition_seam_uses_slope_domain() {
        let roof = complex_roof();
        let plans = roof_section_wfc_constraints(&section_cells(&roof), patterns())
            .unwrap_or_else(|error| unreachable!("section constraints are valid: {error:?}"));

        assert_eq!(
            constraint_at(plans[1].constraints(), WfcCoord::new(0, 0)).allowed,
            domain(&[4])
        );
    }

    #[test]
    fn true_exterior_boundary_uses_eave_domain() {
        let roof = complex_roof();
        let plans = roof_section_wfc_constraints(&section_cells(&roof), patterns())
            .unwrap_or_else(|error| unreachable!("section constraints are valid: {error:?}"));

        assert_eq!(
            constraint_at(plans[1].constraints(), WfcCoord::new(0, 2)).allowed,
            domain(&[6])
        );
    }

    #[test]
    fn empty_used_domain_is_rejected_with_section_and_cell() {
        let roof = roof_for(rectangle(MapTile::new(3200, 3200), 3, 3));
        let section_cells = section_cells(&roof);
        let section = section_cells[0].section();
        let mut patterns = patterns();
        patterns.ridge = PatternDomain::empty();

        assert_eq!(
            roof_section_wfc_constraints(&section_cells, patterns),
            Err(RoofSectionConstraintError::EmptyPatternDomain {
                section,
                cell: RoofCell {
                    tile: MapTile::new(3200, 3201),
                    kind: RoofCellKind::Ridge,
                },
            })
        );
    }

    #[test]
    fn output_order_is_independent_of_section_plan_input_order() {
        let roof = complex_roof();
        let plans = section_cells(&roof);
        let mut reversed = plans.clone();
        reversed.reverse();

        assert_eq!(
            roof_section_wfc_constraints(&plans, patterns()),
            roof_section_wfc_constraints(&reversed, patterns())
        );
    }
}
