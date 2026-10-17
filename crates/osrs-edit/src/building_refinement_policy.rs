//! Deterministic retry and reroll policy for local building refinement.
//!
//! Macro building geometry is already fixed before this layer runs. A variation changes only the
//! local refinement seed. Each WFC attempt is allowed to exhaust its own bounded backtracking
//! budget first; only a true contradiction is eligible for a deterministic retry seed.

use crate::{
    BuildingArchitecture, BuildingPalette, BuildingPlan, FacadeRefinementError, FacadeWfcConfig,
    GenerationSnapshot, MapPatch, RefinedBuildingArchitecture, WfcSolveError,
    build_wfc_refined_building_patch, refine_exterior_walls_with_wfc,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FacadeRefinementPolicy {
    /// Number of alternate deterministic seeds attempted after the initial WFC solve contradicts.
    pub retry_attempts: u8,
}

impl Default for FacadeRefinementPolicy {
    fn default() -> Self {
        Self { retry_attempts: 2 }
    }
}

/// Derive a local facade seed without changing the macro-layout seed stored in `BuildingRequest`.
pub fn facade_variation_seed(building_seed: u64, variation: u32) -> u64 {
    mix64(
        building_seed
            ^ u64::from(variation)
                .wrapping_mul(0xd6e8_feb8_6659_fd93)
                .rotate_left(23),
    )
}

/// Refine one already-planned architecture with deterministic contradiction repair.
pub fn refine_exterior_walls_for_variation(
    architecture: &BuildingArchitecture,
    building_seed: u64,
    variation: u32,
    config: FacadeWfcConfig,
    policy: FacadeRefinementPolicy,
) -> Result<RefinedBuildingArchitecture, FacadeRefinementError> {
    let seed = facade_variation_seed(building_seed, variation);
    retry_contradictions(seed, policy.retry_attempts, |attempt_seed| {
        refine_exterior_walls_with_wfc(architecture, attempt_seed, config)
    })
}

/// Compile a refinement variation through the normal semantic `MapPatch` realization path.
pub fn build_wfc_building_variation_patch(
    snapshot: &GenerationSnapshot,
    plan: &BuildingPlan,
    palette: BuildingPalette,
    variation: u32,
    config: FacadeWfcConfig,
    policy: FacadeRefinementPolicy,
) -> Result<MapPatch, FacadeRefinementError> {
    let seed = facade_variation_seed(plan.request.seed, variation);
    retry_contradictions(seed, policy.retry_attempts, |attempt_seed| {
        build_wfc_refined_building_patch(snapshot, plan, palette, attempt_seed, config)
    })
}

fn retry_contradictions<T>(
    base_seed: u64,
    retry_attempts: u8,
    mut solve: impl FnMut(u64) -> Result<T, FacadeRefinementError>,
) -> Result<T, FacadeRefinementError> {
    for attempt in 0..=retry_attempts {
        let seed = repair_seed(base_seed, attempt);
        match solve(seed) {
            Ok(value) => return Ok(value),
            Err(
                error @ FacadeRefinementError::Solve {
                    error: WfcSolveError::Contradiction { .. },
                },
            ) => {
                if attempt == retry_attempts {
                    return Err(error);
                }
            }
            Err(error) => return Err(error),
        }
    }
    unreachable!("inclusive retry loop always returns on its final attempt")
}

fn repair_seed(base_seed: u64, attempt: u8) -> u64 {
    if attempt == 0 {
        base_seed
    } else {
        mix64(
            base_seed
                ^ u64::from(attempt)
                    .wrapping_mul(0x9e37_79b9_7f4a_7c15)
                    .rotate_left(17),
        )
    }
}

fn mix64(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9e37_79b9_7f4a_7c15);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        BuildingArchetype, BuildingConstraints, BuildingRequest, BuildingStyle, EdgeFeature,
        RectSite, WfcCoord, derive_building_architecture, plan_basic_house,
    };
    use osrs_core::coords::{MapTile, SourcePlane};

    fn plane() -> SourcePlane {
        SourcePlane::new(0).unwrap_or_else(|| unreachable!("plane zero is valid"))
    }

    fn architecture(seed: u64) -> BuildingArchitecture {
        let request = BuildingRequest {
            seed,
            site: RectSite::new(MapTile::new(3200, 3200), 12, 10)
                .unwrap_or_else(|| unreachable!("test site is valid")),
            base_plane: plane(),
            floors: 1,
            archetype: BuildingArchetype::House,
            style: BuildingStyle::Generic,
            constraints: BuildingConstraints::default(),
        };
        let plan = plan_basic_house(request)
            .unwrap_or_else(|error| unreachable!("valid house plan: {error:?}"));
        derive_building_architecture(&plan)
            .unwrap_or_else(|error| unreachable!("valid architecture: {error:?}"))
    }

    #[test]
    fn retry_policy_uses_alternate_seed_only_after_contradiction() {
        let mut calls = Vec::new();
        let result = retry_contradictions(77, 2, |seed| {
            calls.push(seed);
            if calls.len() == 1 {
                Err(FacadeRefinementError::Solve {
                    error: WfcSolveError::Contradiction {
                        coord: WfcCoord::new(2, 0),
                        backtracks: 4,
                    },
                })
            } else {
                Ok(seed)
            }
        })
        .unwrap_or_else(|error| unreachable!("second deterministic attempt succeeds: {error:?}"));

        assert_eq!(calls, vec![77, repair_seed(77, 1)]);
        assert_eq!(result, repair_seed(77, 1));
    }

    #[test]
    fn non_contradiction_errors_are_not_retried() {
        let mut calls = 0;
        let result = retry_contradictions::<()>(99, 8, |_| {
            calls += 1;
            Err(FacadeRefinementError::InvalidConfig)
        });

        assert_eq!(calls, 1);
        assert_eq!(result, Err(FacadeRefinementError::InvalidConfig));
    }

    #[test]
    fn variation_changes_only_refinement_not_macro_geometry() {
        let architecture = architecture(42);
        let config = FacadeWfcConfig::default();
        let policy = FacadeRefinementPolicy::default();
        let first = refine_exterior_walls_for_variation(&architecture, 42, 0, config, policy)
            .unwrap_or_else(|error| unreachable!("valid refinement: {error:?}"));
        let second = refine_exterior_walls_for_variation(&architecture, 42, 1, config, policy)
            .unwrap_or_else(|error| unreachable!("valid refinement: {error:?}"));

        assert_eq!(first.floor_tiles(), second.floor_tiles());
        assert_eq!(first.edges().len(), second.edges().len());
        for (left, right) in first.edges().iter().zip(second.edges()) {
            assert_eq!(left.tile, right.tile);
            assert_eq!(left.direction, right.direction);
            if left.feature != right.feature {
                assert!(matches!(
                    (left.feature, right.feature),
                    (EdgeFeature::ExteriorWall, EdgeFeature::Window)
                        | (EdgeFeature::Window, EdgeFeature::ExteriorWall)
                ));
            }
        }
    }

    #[test]
    fn variation_seed_is_repeatable_and_separate_from_building_seed() {
        assert_eq!(
            facade_variation_seed(1234, 7),
            facade_variation_seed(1234, 7)
        );
        assert_ne!(
            facade_variation_seed(1234, 7),
            facade_variation_seed(1234, 8)
        );
        assert_ne!(
            facade_variation_seed(1234, 7),
            facade_variation_seed(1235, 7)
        );
    }
}
