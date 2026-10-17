//! End-to-end headless roof generation from vertical semantics to a reversible map patch.
//!
//! This module composes the already-separated roof planning, WFC solving, asset description, and
//! patch construction stages. It deliberately contains no editor, renderer, cache, or runtime
//! integration and introduces no new roof-shape policy.

use crate::{
    BuildingRoofError, BuildingVerticalPlan, MapPatch, RoofPalette, RoofPatchError, RoofPatternSet,
    RoofPlacementDescriptionError, RoofWfcSolveError, WfcCatalog, build_roof_patch,
    describe_roof_placements, plan_roof_coverage, plan_roof_shape, solve_roof_wfc,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoofGenerationError {
    Coverage(BuildingRoofError),
    Solve(RoofWfcSolveError),
    Description(RoofPlacementDescriptionError),
    Patch(RoofPatchError),
}

/// Generate one exact reversible roof patch from an already-validated vertical building plan.
///
/// The pipeline is intentionally explicit and side-effect free:
/// coverage -> shape -> semantic WFC solve -> placement descriptions -> `MapPatch`.
pub fn generate_roof_patch(
    vertical: &BuildingVerticalPlan,
    catalog: &WfcCatalog,
    patterns: RoofPatternSet,
    palette: &RoofPalette,
    seed: u64,
    max_backtracks: u32,
) -> Result<MapPatch, RoofGenerationError> {
    let roof = plan_roof_coverage(vertical).map_err(RoofGenerationError::Coverage)?;
    let shape = plan_roof_shape(&roof);
    let solution = solve_roof_wfc(catalog, &roof, shape, patterns, seed, max_backtracks)
        .map_err(RoofGenerationError::Solve)?;
    let descriptions = describe_roof_placements(&roof, &solution, palette)
        .map_err(RoofGenerationError::Description)?;
    build_roof_patch(&descriptions).map_err(RoofGenerationError::Patch)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        BuildingArchetype, BuildingConstraints, BuildingRequest, BuildingStyle, LocKind, MapDocument,
        MapOp, ObjectOrientation, PatternDomain, RectSite, RoofAsset, RoofPaletteEntry,
        WfcPatternId, WfcPatternSpec, WfcSocket, plan_vertical_connectivity,
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

    fn vertical() -> BuildingVerticalPlan {
        plan_vertical_connectivity(
            request(),
            &[rectangle(MapTile::new(3200, 3200), 5, 5)],
        )
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
        RoofPalette::new((0_u8..10).map(|value| RoofPaletteEntry {
            pattern: pattern(value),
            asset: RoofAsset {
                object_id: ObjectId::new(20_000 + u32::from(value)),
                kind: LocKind::new(10).unwrap_or_else(|| unreachable!("test loc kind is valid")),
                orientation: ObjectOrientation::new(value % 4)
                    .unwrap_or_else(|| unreachable!("test orientation is valid")),
            },
        }))
        .unwrap_or_else(|error| unreachable!("valid roof palette: {error:?}"))
    }

    #[test]
    fn end_to_end_generation_returns_one_add_object_per_roof_tile() {
        let vertical = vertical();
        let patch = generate_roof_patch(&vertical, &catalog(), patterns(), &palette(), 91, 32)
            .unwrap_or_else(|error| unreachable!("roof generation succeeds: {error:?}"));
        let roof = plan_roof_coverage(&vertical)
            .unwrap_or_else(|error| unreachable!("valid roof coverage: {error:?}"));

        assert_eq!(patch.len(), roof.tiles().len());
        assert!(
            patch
                .ops()
                .iter()
                .all(|op| matches!(op, MapOp::AddObject { .. }))
        );
    }

    #[test]
    fn end_to_end_generation_is_deterministic_for_identical_seed() {
        let vertical = vertical();
        let catalog = catalog();
        let palette = palette();

        assert_eq!(
            generate_roof_patch(&vertical, &catalog, patterns(), &palette, 1234, 32),
            generate_roof_patch(&vertical, &catalog, patterns(), &palette, 1234, 32)
        );
    }

    #[test]
    fn generated_patch_apply_and_inverse_restore_exact_document() {
        let patch = generate_roof_patch(&vertical(), &catalog(), patterns(), &palette(), 91, 32)
            .unwrap_or_else(|error| unreachable!("roof generation succeeds: {error:?}"));
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
}
