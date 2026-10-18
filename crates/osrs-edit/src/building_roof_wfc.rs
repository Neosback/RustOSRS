//! WFC-domain constraints for semantic roof cells.
//!
//! This layer translates already-planned roof semantics into site-local WFC constraints. It does
//! not choose concrete RuneScape objects and deliberately allows each semantic role to map to a
//! multi-pattern domain so WFC can still choose weighted visual variants later.

use crate::{
    BuildingRoofPlan, PatternDomain, RectSite, RoofCell, RoofCellClassificationError, RoofCellKind,
    RoofShape, WfcConstraint, WfcCoord, classify_roof_cells,
};
use osrs_core::coords::MapTile;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RoofPatternSet {
    pub ridge: PatternDomain,
    /// Direction-indexed domains for interior slopes. The index is the downhill direction.
    pub slopes: [PatternDomain; 4],
    /// Direction-indexed domains for exposed eaves. The index is the downhill direction.
    pub eaves: [PatternDomain; 4],
}

impl RoofPatternSet {
    pub const fn domain_for(self, kind: RoofCellKind) -> PatternDomain {
        match kind {
            RoofCellKind::Ridge => self.ridge,
            RoofCellKind::Slope { downhill } => self.slopes[downhill.index()],
            RoofCellKind::Eave { downhill } => self.eaves[downhill.index()],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoofConstraintError {
    Classification(RoofCellClassificationError),
    EmptyPatternDomain { cell: RoofCell },
    InvalidRoofTile { tile: MapTile },
}

/// Translate semantic roof cells into fixed WFC domains over the request site's local grid.
///
/// Each covered roof tile contributes exactly one constraint. A domain can contain several pattern
/// IDs, preserving local WFC variation inside a semantic role. Domains are validated lazily per
/// used cell, so an even-span gable does not require a ridge domain when it has no ridge cells.
pub fn roof_wfc_constraints(
    site: RectSite,
    roof: &BuildingRoofPlan,
    shape: RoofShape,
    patterns: RoofPatternSet,
) -> Result<Vec<WfcConstraint>, RoofConstraintError> {
    let cells = classify_roof_cells(roof, shape).map_err(RoofConstraintError::Classification)?;
    let mut constraints = Vec::with_capacity(cells.len());

    for cell in cells {
        let allowed = patterns.domain_for(cell.kind);
        if allowed.is_empty() {
            return Err(RoofConstraintError::EmptyPatternDomain { cell });
        }
        let coord = local_wfc_coord(site, cell.tile)
            .ok_or(RoofConstraintError::InvalidRoofTile { tile: cell.tile })?;
        constraints.push(WfcConstraint { coord, allowed });
    }

    constraints.sort_by_key(|constraint| constraint.coord);
    Ok(constraints)
}

fn local_wfc_coord(site: RectSite, tile: MapTile) -> Option<WfcCoord> {
    if !site.contains(tile) {
        return None;
    }
    let origin = site.origin();
    let x = u16::try_from(tile.x.checked_sub(origin.x)?).ok()?;
    let y = u16::try_from(tile.y.checked_sub(origin.y)?).ok()?;
    Some(WfcCoord::new(x, y))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        BuildingArchetype, BuildingConstraints, BuildingRequest, BuildingStyle, RoofCellKind,
        WfcPatternId, plan_roof_coverage, plan_roof_shape, plan_vertical_connectivity,
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
        let request = request();
        let vertical = plan_vertical_connectivity(request, &[floor])
            .unwrap_or_else(|error| unreachable!("valid vertical plan: {error:?}"));
        plan_roof_coverage(&vertical)
            .unwrap_or_else(|error| unreachable!("valid roof plan: {error:?}"))
    }

    fn pattern(value: u8) -> WfcPatternId {
        WfcPatternId::new(u16::from(value))
            .unwrap_or_else(|| unreachable!("test pattern id is valid"))
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

    fn constraint_at(constraints: &[WfcConstraint], coord: WfcCoord) -> WfcConstraint {
        constraints
            .iter()
            .find(|constraint| constraint.coord == coord)
            .copied()
            .unwrap_or_else(|| unreachable!("roof tile has a constraint"))
    }

    #[test]
    fn ridge_constraint_preserves_multi_pattern_domain() {
        let roof = roof_for(rectangle(MapTile::new(3200, 3200), 5, 5));
        let constraints =
            roof_wfc_constraints(request().site, &roof, plan_roof_shape(&roof), patterns())
                .unwrap_or_else(|error| unreachable!("valid roof constraints: {error:?}"));
        let ridge = constraint_at(&constraints, WfcCoord::new(2, 2));

        assert_eq!(ridge.allowed.len(), 2);
        assert!(ridge.allowed.contains(pattern(0)));
        assert!(ridge.allowed.contains(pattern(1)));
    }

    #[test]
    fn slope_and_eave_constraints_use_downhill_direction_domains() {
        let roof = roof_for(rectangle(MapTile::new(3200, 3200), 5, 5));
        let constraints =
            roof_wfc_constraints(request().site, &roof, plan_roof_shape(&roof), patterns())
                .unwrap_or_else(|error| unreachable!("valid roof constraints: {error:?}"));

        assert_eq!(
            constraint_at(&constraints, WfcCoord::new(2, 1)).allowed,
            domain(&[4])
        );
        assert_eq!(
            constraint_at(&constraints, WfcCoord::new(2, 0)).allowed,
            domain(&[8])
        );
        assert_eq!(
            constraint_at(&constraints, WfcCoord::new(2, 3)).allowed,
            domain(&[2])
        );
        assert_eq!(
            constraint_at(&constraints, WfcCoord::new(2, 4)).allowed,
            domain(&[6])
        );
    }

    #[test]
    fn even_span_gable_does_not_require_unused_ridge_domain() {
        let roof = roof_for(rectangle(MapTile::new(3200, 3200), 4, 4));
        let mut patterns = patterns();
        patterns.ridge = PatternDomain::empty();

        let constraints =
            roof_wfc_constraints(request().site, &roof, plan_roof_shape(&roof), patterns)
                .unwrap_or_else(|error| unreachable!("even gable has no ridge cells: {error:?}"));

        assert_eq!(constraints.len(), 16);
    }

    #[test]
    fn empty_used_domain_is_rejected_at_semantic_cell() {
        let roof = roof_for(rectangle(MapTile::new(3200, 3200), 5, 5));
        let mut patterns = patterns();
        patterns.ridge = PatternDomain::empty();

        assert_eq!(
            roof_wfc_constraints(request().site, &roof, plan_roof_shape(&roof), patterns),
            Err(RoofConstraintError::EmptyPatternDomain {
                cell: RoofCell {
                    tile: MapTile::new(3200, 3202),
                    kind: RoofCellKind::Ridge,
                },
            })
        );
    }

    #[test]
    fn constraints_are_site_local_and_sorted() {
        let roof = roof_for(rectangle(MapTile::new(3202, 3203), 3, 3));
        let constraints =
            roof_wfc_constraints(request().site, &roof, plan_roof_shape(&roof), patterns())
                .unwrap_or_else(|error| unreachable!("valid roof constraints: {error:?}"));

        assert_eq!(constraints.len(), 9);
        assert_eq!(
            constraints.first().map(|constraint| constraint.coord),
            Some(WfcCoord::new(2, 3))
        );
        assert_eq!(
            constraints.last().map(|constraint| constraint.coord),
            Some(WfcCoord::new(4, 5))
        );
    }

    #[test]
    fn complex_roof_defers_wfc_constraints() {
        let roof = roof_for(vec![
            MapTile::new(3200, 3200),
            MapTile::new(3201, 3200),
            MapTile::new(3200, 3201),
        ]);

        assert_eq!(
            roof_wfc_constraints(request().site, &roof, plan_roof_shape(&roof), patterns()),
            Err(RoofConstraintError::Classification(
                RoofCellClassificationError::ComplexRoofUnsupported,
            ))
        );
    }
}
