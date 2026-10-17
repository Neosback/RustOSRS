//! Asset mapping for independently solved complex-roof sections.
//!
//! Each solved rectangular roof section is kept as a distinct realization plan while its semantic
//! WFC pattern IDs are resolved through the existing `RoofPalette`. This layer stops before
//! valley/intersection repair or `MapPatch` construction.

use crate::{
    RoofPalette, RoofPlacementDescription, RoofSection, RoofSectionWfcSolution, WfcPatternId,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoofSectionPlacementPlan {
    section: RoofSection,
    descriptions: Vec<RoofPlacementDescription>,
}

impl RoofSectionPlacementPlan {
    pub const fn section(&self) -> RoofSection {
        self.section
    }

    pub fn descriptions(&self) -> &[RoofPlacementDescription] {
        &self.descriptions
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoofSectionPlacementDescriptionError {
    MissingPatternAsset {
        section: RoofSection,
        pattern: WfcPatternId,
    },
}

/// Resolve solved semantic section patterns into concrete object placement descriptions.
///
/// Input solutions are normalized into deterministic section order. Section identity is preserved
/// so a later junction/valley stage can reason about boundaries before any final map patch is built.
pub fn describe_roof_section_placements(
    solutions: &[RoofSectionWfcSolution],
    palette: &RoofPalette,
) -> Result<Vec<RoofSectionPlacementPlan>, RoofSectionPlacementDescriptionError> {
    let mut ordered = solutions.iter().collect::<Vec<_>>();
    ordered.sort_by_key(|solution| section_sort_key(solution.section()));

    let mut plans = Vec::with_capacity(ordered.len());
    for solution in ordered {
        let section = solution.section();
        let mut descriptions = Vec::with_capacity(solution.selections().len());
        for &selection in solution.selections() {
            let asset = palette.asset(selection.pattern).ok_or(
                RoofSectionPlacementDescriptionError::MissingPatternAsset {
                    section,
                    pattern: selection.pattern,
                },
            )?;
            descriptions.push(RoofPlacementDescription {
                pattern: selection.pattern,
                placement: asset.placement(selection.tile, section.plane()),
            });
        }
        descriptions.sort_by_key(|description| description.placement.position.tile);
        plans.push(RoofSectionPlacementPlan {
            section,
            descriptions,
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
        BuildingArchetype, BuildingConstraints, BuildingRequest, BuildingRoofPlan, BuildingStyle,
        LocKind, ObjectOrientation, PatternDomain, RectSite, RoofAsset, RoofPaletteEntry,
        RoofPatternSet, RoofSectionCellPlan, RoofSectionWfcPlan, WfcCatalog, WfcPatternId,
        WfcPatternSpec, WfcSocket, classify_roof_section_cells, decompose_roof_sections,
        plan_roof_coverage, plan_roof_section_shapes, plan_vertical_connectivity,
        roof_section_wfc_constraints, solve_roof_section_wfc,
    };
    use osrs_core::coords::{MapTile, SourcePlane};
    use osrs_core::ids::ObjectId;

    fn plane(value: u8) -> SourcePlane {
        SourcePlane::new(value).unwrap_or_else(|| unreachable!("test plane is valid"))
    }

    fn request() -> BuildingRequest {
        BuildingRequest {
            seed: 77,
            site: RectSite::new(MapTile::new(3200, 3200), 8, 8)
                .unwrap_or_else(|| unreachable!("test site is valid")),
            base_plane: plane(1),
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

    fn complex_roof() -> BuildingRoofPlan {
        let mut floor = rectangle(MapTile::new(3200, 3200), 5, 1);
        floor.extend(rectangle(MapTile::new(3201, 3201), 3, 3));
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

    fn catalog() -> WfcCatalog {
        WfcCatalog::new((0..10).map(|index| WfcPatternSpec {
            weight: u32::try_from(index + 1).unwrap_or_else(|_| unreachable!("small test catalog")),
            sockets: [WfcSocket(1); 4],
        }))
        .unwrap_or_else(|error| unreachable!("valid test catalog: {error:?}"))
    }

    fn asset(value: u32, orientation: u8) -> RoofAsset {
        RoofAsset {
            object_id: ObjectId::new(value),
            kind: LocKind::new(10).unwrap_or_else(|| unreachable!("test loc kind is valid")),
            orientation: ObjectOrientation::new(orientation)
                .unwrap_or_else(|| unreachable!("test orientation is valid")),
        }
    }

    fn full_palette() -> RoofPalette {
        RoofPalette::new((0_u8..10).map(|value| RoofPaletteEntry {
            pattern: pattern(value),
            asset: asset(10_000 + u32::from(value), value % 4),
        }))
        .unwrap_or_else(|error| unreachable!("valid roof palette: {error:?}"))
    }

    fn section_cells(roof: &BuildingRoofPlan) -> Vec<RoofSectionCellPlan> {
        let sections = decompose_roof_sections(roof)
            .unwrap_or_else(|error| unreachable!("roof decomposes: {error:?}"));
        let shapes = plan_roof_section_shapes(&sections);
        classify_roof_section_cells(roof, &shapes)
            .unwrap_or_else(|error| unreachable!("roof section cells classify: {error:?}"))
    }

    fn section_wfc_plans(roof: &BuildingRoofPlan) -> Vec<RoofSectionWfcPlan> {
        roof_section_wfc_constraints(&section_cells(roof), patterns())
            .unwrap_or_else(|error| unreachable!("section constraints are valid: {error:?}"))
    }

    fn solved_sections() -> Vec<RoofSectionWfcSolution> {
        let roof = complex_roof();
        solve_roof_section_wfc(&catalog(), &section_wfc_plans(&roof), 91, 32)
            .unwrap_or_else(|error| unreachable!("roof sections solve: {error:?}"))
    }

    #[test]
    fn section_descriptions_preserve_section_tiles_plane_and_assets() {
        let solutions = solved_sections();
        let palette = full_palette();
        let plans = describe_roof_section_placements(&solutions, &palette)
            .unwrap_or_else(|error| unreachable!("section descriptions resolve: {error:?}"));

        assert_eq!(plans.len(), solutions.len());
        for (plan, solution) in plans.iter().zip(&solutions) {
            assert_eq!(plan.section(), solution.section());
            assert_eq!(plan.descriptions().len(), solution.selections().len());
            for (description, selection) in plan.descriptions().iter().zip(solution.selections()) {
                let expected_asset = palette
                    .asset(selection.pattern)
                    .unwrap_or_else(|| unreachable!("full palette contains solved pattern"));
                assert_eq!(description.pattern, selection.pattern);
                assert_eq!(description.placement.position.tile, selection.tile);
                assert_eq!(description.placement.position.plane, solution.section().plane());
                assert_eq!(description.placement.object_id, expected_asset.object_id);
                assert_eq!(description.placement.kind, expected_asset.kind);
                assert_eq!(description.placement.orientation, expected_asset.orientation);
            }
        }
    }

    #[test]
    fn descriptions_remain_sorted_by_world_tile_within_each_section() {
        let plans = describe_roof_section_placements(&solved_sections(), &full_palette())
            .unwrap_or_else(|error| unreachable!("section descriptions resolve: {error:?}"));

        for plan in plans {
            let tiles = plan
                .descriptions()
                .iter()
                .map(|description| description.placement.position.tile)
                .collect::<Vec<_>>();
            let mut sorted = tiles.clone();
            sorted.sort();
            assert_eq!(tiles, sorted);
        }
    }

    #[test]
    fn missing_pattern_asset_reports_exact_section_and_pattern() {
        let solutions = solved_sections();
        let first = solutions[0]
            .selections()
            .first()
            .copied()
            .unwrap_or_else(|| unreachable!("solved section is non-empty"));
        let expected_section = solutions[0].section();
        let palette = RoofPalette::new(
            (0_u8..10)
                .filter(|&value| pattern(value) != first.pattern)
                .map(|value| RoofPaletteEntry {
                    pattern: pattern(value),
                    asset: asset(10_000 + u32::from(value), value % 4),
                }),
        )
        .unwrap_or_else(|error| unreachable!("valid incomplete palette: {error:?}"));

        assert_eq!(
            describe_roof_section_placements(&solutions, &palette),
            Err(RoofSectionPlacementDescriptionError::MissingPatternAsset {
                section: expected_section,
                pattern: first.pattern,
            })
        );
    }

    #[test]
    fn plan_order_is_independent_of_solution_input_order() {
        let solutions = solved_sections();
        let mut reversed = solutions.clone();
        reversed.reverse();
        let palette = full_palette();

        assert_eq!(
            describe_roof_section_placements(&solutions, &palette),
            describe_roof_section_placements(&reversed, &palette)
        );
    }
}
