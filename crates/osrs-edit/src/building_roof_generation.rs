//! End-to-end headless roof generation from vertical semantics to a reversible map patch.
//!
//! This module owns the single roof-generation entry point. Rectangular roofs retain the existing
//! whole-roof WFC path, while complex roofs decompose into sections, solve locally, classify and
//! repair section junctions, then build the same reversible `MapPatch` transaction. It deliberately
//! contains no editor, renderer, cache, or runtime integration.

use crate::{
    BuildingRoofError, BuildingVerticalPlan, MapPatch, RoofJunctionPatternPalette, RoofPalette,
    RoofPatchError, RoofPatternSet, RoofPlacementDescriptionError, RoofSectionCellError,
    RoofSectionConstraintError, RoofSectionError, RoofSectionJunctionError,
    RoofSectionJunctionRealizationError, RoofSectionPlacementDescriptionError,
    RoofSectionWfcSolveError, RoofShape, RoofWfcSolveError, WfcCatalog, build_roof_patch,
    classify_roof_section_cells, classify_roof_section_junctions, decompose_roof_sections,
    describe_repaired_roof_section_placements, describe_roof_placements,
    describe_roof_section_placements, detect_roof_section_junctions, plan_roof_coverage,
    plan_roof_section_junction_repairs, plan_roof_section_shapes, plan_roof_shape,
    roof_section_wfc_constraints, solve_roof_section_wfc, solve_roof_wfc,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RoofGenerationError {
    Coverage(BuildingRoofError),
    Solve(RoofWfcSolveError),
    Description(RoofPlacementDescriptionError),
    SectionDecomposition(RoofSectionError),
    SectionCells(RoofSectionCellError),
    SectionConstraints(RoofSectionConstraintError),
    SectionSolve(RoofSectionWfcSolveError),
    SectionDescription(RoofSectionPlacementDescriptionError),
    JunctionDetection(RoofSectionJunctionError),
    JunctionRealization(RoofSectionJunctionRealizationError),
    Patch(RoofPatchError),
}

/// Generate one exact reversible roof patch from an already-validated vertical building plan.
///
/// Rectangular roofs use the original whole-roof WFC flow unchanged. Complex roofs use deterministic
/// rectangular decomposition, section-local WFC solves, semantic junction repair, and the same final
/// patch builder. The junction pattern palette is consulted only for complex roofs.
pub fn generate_roof_patch(
    vertical: &BuildingVerticalPlan,
    catalog: &WfcCatalog,
    patterns: RoofPatternSet,
    palette: &RoofPalette,
    junction_patterns: &RoofJunctionPatternPalette,
    seed: u64,
    max_backtracks: u32,
) -> Result<MapPatch, RoofGenerationError> {
    let roof = plan_roof_coverage(vertical).map_err(RoofGenerationError::Coverage)?;
    let shape = plan_roof_shape(&roof);

    if shape != RoofShape::Complex {
        let solution = solve_roof_wfc(catalog, &roof, shape, patterns, seed, max_backtracks)
            .map_err(RoofGenerationError::Solve)?;
        let descriptions = describe_roof_placements(&roof, &solution, palette)
            .map_err(RoofGenerationError::Description)?;
        return build_roof_patch(&descriptions).map_err(RoofGenerationError::Patch);
    }

    let sections =
        decompose_roof_sections(&roof).map_err(RoofGenerationError::SectionDecomposition)?;
    let section_shapes = plan_roof_section_shapes(&sections);
    let section_cells = classify_roof_section_cells(&roof, &section_shapes)
        .map_err(RoofGenerationError::SectionCells)?;
    let section_wfc = roof_section_wfc_constraints(&section_cells, patterns)
        .map_err(RoofGenerationError::SectionConstraints)?;
    let solutions = solve_roof_section_wfc(catalog, &section_wfc, seed, max_backtracks)
        .map_err(RoofGenerationError::SectionSolve)?;
    let placements = describe_roof_section_placements(&solutions, palette)
        .map_err(RoofGenerationError::SectionDescription)?;
    let junctions = detect_roof_section_junctions(&section_cells)
        .map_err(RoofGenerationError::JunctionDetection)?;
    let classifications = classify_roof_section_junctions(&junctions);
    let repairs = plan_roof_section_junction_repairs(&classifications);
    let repaired = describe_repaired_roof_section_placements(
        &placements,
        &repairs,
        junction_patterns,
        palette,
    )
    .map_err(RoofGenerationError::JunctionRealization)?;

    build_roof_patch(&repaired).map_err(RoofGenerationError::Patch)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        BuildingArchetype, BuildingConstraints, BuildingRequest, BuildingStyle, CardinalDirection,
        LocKind, MapDocument, MapOp, ObjectOrientation, PatternDomain, RectSite, RoofAsset,
        RoofJunctionPatternKey, RoofJunctionPatternPaletteEntry, RoofJunctionPatternRequirement,
        RoofPaletteEntry, RoofSectionJunctionRepairRole, WfcPatternId, WfcPatternSpec, WfcSocket,
        plan_vertical_connectivity,
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

    fn rectangular_vertical() -> BuildingVerticalPlan {
        plan_vertical_connectivity(request(), &[rectangle(MapTile::new(3200, 3200), 5, 5)])
            .unwrap_or_else(|error| unreachable!("valid vertical plan: {error:?}"))
    }

    fn complex_vertical() -> BuildingVerticalPlan {
        let mut floor = rectangle(MapTile::new(3200, 3200), 5, 1);
        floor.extend(rectangle(MapTile::new(3201, 3201), 3, 3));
        plan_vertical_connectivity(request(), &[floor])
            .unwrap_or_else(|error| unreachable!("valid vertical plan: {error:?}"))
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

    fn palette() -> RoofPalette {
        RoofPalette::new((0_u8..12).map(|value| {
            RoofPaletteEntry {
                pattern: pattern(value),
                asset: RoofAsset {
                    object_id: ObjectId::new(20_000 + u32::from(value)),
                    kind: LocKind::new(10)
                        .unwrap_or_else(|| unreachable!("test loc kind is valid")),
                    orientation: ObjectOrientation::new(value % 4)
                        .unwrap_or_else(|| unreachable!("test orientation is valid")),
                },
            }
        }))
        .unwrap_or_else(|error| unreachable!("valid roof palette: {error:?}"))
    }

    fn junction_key(direction_to_other: CardinalDirection) -> RoofJunctionPatternKey {
        RoofJunctionPatternKey::new([RoofJunctionPatternRequirement {
            role: RoofSectionJunctionRepairRole::RidgeIntersection,
            direction_to_other,
        }])
    }

    fn junction_patterns() -> RoofJunctionPatternPalette {
        RoofJunctionPatternPalette::new([
            RoofJunctionPatternPaletteEntry {
                key: junction_key(CardinalDirection::North),
                pattern: pattern(10),
            },
            RoofJunctionPatternPaletteEntry {
                key: junction_key(CardinalDirection::South),
                pattern: pattern(11),
            },
        ])
        .unwrap_or_else(|error| unreachable!("valid junction palette: {error:?}"))
    }

    #[test]
    fn rectangular_generation_preserves_original_simple_pipeline() {
        let vertical = rectangular_vertical();
        let catalog = catalog();
        let palette = palette();
        let junction_patterns = junction_patterns();
        let roof = plan_roof_coverage(&vertical)
            .unwrap_or_else(|error| unreachable!("valid roof coverage: {error:?}"));
        let shape = plan_roof_shape(&roof);
        let solution = solve_roof_wfc(&catalog, &roof, shape, patterns(), 91, 32)
            .unwrap_or_else(|error| unreachable!("simple roof solves: {error:?}"));
        let descriptions = describe_roof_placements(&roof, &solution, &palette)
            .unwrap_or_else(|error| unreachable!("simple roof descriptions resolve: {error:?}"));
        let expected = build_roof_patch(&descriptions)
            .unwrap_or_else(|error| unreachable!("simple roof patch builds: {error:?}"));

        assert_eq!(
            generate_roof_patch(
                &vertical,
                &catalog,
                patterns(),
                &palette,
                &junction_patterns,
                91,
                32,
            ),
            Ok(expected)
        );
    }

    #[test]
    fn complex_generation_returns_one_repaired_add_object_per_roof_tile() {
        let vertical = complex_vertical();
        let patch = generate_roof_patch(
            &vertical,
            &catalog(),
            patterns(),
            &palette(),
            &junction_patterns(),
            91,
            32,
        )
        .unwrap_or_else(|error| unreachable!("complex roof generation succeeds: {error:?}"));
        let roof = plan_roof_coverage(&vertical)
            .unwrap_or_else(|error| unreachable!("valid roof coverage: {error:?}"));

        assert_eq!(patch.len(), roof.tiles().len());
        assert!(
            patch
                .ops()
                .iter()
                .all(|op| matches!(op, MapOp::AddObject { .. }))
        );
        let junction_ids = [ObjectId::new(20_010), ObjectId::new(20_011)];
        assert_eq!(
            patch
                .ops()
                .iter()
                .filter(|op| match op {
                    MapOp::AddObject { placement } => junction_ids.contains(&placement.object_id),
                    _ => false,
                })
                .count(),
            6
        );
    }

    #[test]
    fn unified_generation_is_deterministic_for_simple_and_complex_roofs() {
        let catalog = catalog();
        let palette = palette();
        let junction_patterns = junction_patterns();

        for vertical in [rectangular_vertical(), complex_vertical()] {
            assert_eq!(
                generate_roof_patch(
                    &vertical,
                    &catalog,
                    patterns(),
                    &palette,
                    &junction_patterns,
                    1234,
                    32,
                ),
                generate_roof_patch(
                    &vertical,
                    &catalog,
                    patterns(),
                    &palette,
                    &junction_patterns,
                    1234,
                    32,
                )
            );
        }
    }

    #[test]
    fn complex_generated_patch_apply_and_inverse_restore_exact_document() {
        let patch = generate_roof_patch(
            &complex_vertical(),
            &catalog(),
            patterns(),
            &palette(),
            &junction_patterns(),
            91,
            32,
        )
        .unwrap_or_else(|error| unreachable!("complex roof generation succeeds: {error:?}"));
        let original = MapDocument::new();
        let mut document = original.clone();

        document
            .apply_patch(&patch)
            .unwrap_or_else(|error| unreachable!("generated roof patch applies: {error:?}"));
        assert_eq!(document.object_count(), patch.len());

        document
            .apply_patch(&patch.inverse())
            .unwrap_or_else(|error| unreachable!("generated roof inverse applies: {error:?}"));
        assert_eq!(document, original);
    }

    #[test]
    fn complex_generation_reports_missing_junction_profile() {
        let empty = RoofJunctionPatternPalette::new([])
            .unwrap_or_else(|error| unreachable!("empty junction palette is valid: {error:?}"));
        let error = match generate_roof_patch(
            &complex_vertical(),
            &catalog(),
            patterns(),
            &palette(),
            &empty,
            91,
            32,
        ) {
            Ok(_) => unreachable!("complex roof needs junction profile bindings"),
            Err(error) => error,
        };

        assert!(matches!(
            error,
            RoofGenerationError::JunctionRealization(
                RoofSectionJunctionRealizationError::MissingJunctionPattern { .. }
            )
        ));
    }
}
