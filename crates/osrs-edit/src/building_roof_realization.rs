//! Asset mapping and reversible patch construction for solved semantic roof patterns.
//!
//! This layer resolves WFC pattern IDs into concrete object-placement descriptions, then can turn
//! those validated descriptions into the same headless `MapPatch` transaction used by other map
//! edits. It contains no editor, renderer, cache, or runtime integration.

use crate::{
    BuildingRoofPlan, LocKind, MapOp, MapPatch, MapPosition, ObjectOrientation, ObjectPlacement,
    RoofWfcSolution, WfcPatternId,
};
use osrs_core::coords::{MapTile, SourcePlane};
use osrs_core::ids::ObjectId;
use std::collections::{BTreeMap, HashSet};

/// Concrete object metadata for one already-oriented semantic roof pattern.
///
/// Roof WFC pattern IDs are direction-specific, so orientation belongs to the palette entry rather
/// than being inferred again during realization.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RoofAsset {
    pub object_id: ObjectId,
    pub kind: LocKind,
    pub orientation: ObjectOrientation,
}

impl RoofAsset {
    pub fn placement(self, tile: MapTile, plane: SourcePlane) -> ObjectPlacement {
        ObjectPlacement {
            object_id: self.object_id,
            position: MapPosition::new(tile, plane),
            kind: self.kind,
            orientation: self.orientation,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RoofPaletteEntry {
    pub pattern: WfcPatternId,
    pub asset: RoofAsset,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoofPalette {
    assets: BTreeMap<WfcPatternId, RoofAsset>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoofPaletteError {
    DuplicatePattern { pattern: WfcPatternId },
}

impl RoofPalette {
    pub fn new(
        entries: impl IntoIterator<Item = RoofPaletteEntry>,
    ) -> Result<Self, RoofPaletteError> {
        let mut assets = BTreeMap::new();
        for entry in entries {
            if assets.insert(entry.pattern, entry.asset).is_some() {
                return Err(RoofPaletteError::DuplicatePattern {
                    pattern: entry.pattern,
                });
            }
        }
        Ok(Self { assets })
    }

    pub fn asset(&self, pattern: WfcPatternId) -> Option<RoofAsset> {
        self.assets.get(&pattern).copied()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RoofPlacementDescription {
    pub pattern: WfcPatternId,
    pub placement: ObjectPlacement,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoofPlacementDescriptionError {
    SelectionOutsideRoof { tile: MapTile },
    MissingPatternAsset { pattern: WfcPatternId },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoofPatchError {
    DuplicateObjectPlacement { placement: ObjectPlacement },
}

/// Resolve a solved semantic roof into concrete placement descriptions without mutating a map.
pub fn describe_roof_placements(
    roof: &BuildingRoofPlan,
    solution: &RoofWfcSolution,
    palette: &RoofPalette,
) -> Result<Vec<RoofPlacementDescription>, RoofPlacementDescriptionError> {
    let mut descriptions = Vec::with_capacity(solution.selections().len());
    for &selection in solution.selections() {
        if roof.tiles().binary_search(&selection.tile).is_err() {
            return Err(RoofPlacementDescriptionError::SelectionOutsideRoof {
                tile: selection.tile,
            });
        }
        let asset = palette.asset(selection.pattern).ok_or(
            RoofPlacementDescriptionError::MissingPatternAsset {
                pattern: selection.pattern,
            },
        )?;
        descriptions.push(RoofPlacementDescription {
            pattern: selection.pattern,
            placement: asset.placement(selection.tile, roof.plane()),
        });
    }
    Ok(descriptions)
}

/// Build one exact reversible object patch from validated roof placement descriptions.
///
/// Descriptions retain their solved ordering. Duplicate exact placements are rejected before any
/// patch is returned so a generated roof cannot contain a transaction that adds the same semantic
/// object twice.
pub fn build_roof_patch(
    descriptions: &[RoofPlacementDescription],
) -> Result<MapPatch, RoofPatchError> {
    let mut placements = HashSet::with_capacity(descriptions.len());
    let mut ops = Vec::with_capacity(descriptions.len());

    for description in descriptions {
        let placement = description.placement;
        if !placements.insert(placement) {
            return Err(RoofPatchError::DuplicateObjectPlacement { placement });
        }
        ops.push(MapOp::AddObject { placement });
    }

    Ok(MapPatch::new(ops))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        BuildingArchetype, BuildingConstraints, BuildingRequest, BuildingStyle, MapDocument,
        PatternDomain, RectSite, RoofPatternSet, WfcCatalog, WfcPatternSpec, WfcSocket,
        plan_roof_coverage, plan_roof_shape, plan_vertical_connectivity, solve_roof_wfc,
    };

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

    fn pattern(value: u8) -> WfcPatternId {
        WfcPatternId::new(u16::from(value))
            .unwrap_or_else(|| unreachable!("test pattern id is valid"))
    }

    fn domain(values: &[u8]) -> PatternDomain {
        PatternDomain::from_patterns(values.iter().copied().map(pattern))
    }

    fn pattern_set() -> RoofPatternSet {
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

    fn solved_roof() -> (BuildingRoofPlan, RoofWfcSolution) {
        let vertical =
            plan_vertical_connectivity(request(), &[rectangle(MapTile::new(3200, 3200), 5, 5)])
                .unwrap_or_else(|error| unreachable!("valid vertical plan: {error:?}"));
        let roof = plan_roof_coverage(&vertical)
            .unwrap_or_else(|error| unreachable!("valid roof plan: {error:?}"));
        let solution = solve_roof_wfc(
            &catalog(),
            &roof,
            plan_roof_shape(&roof),
            pattern_set(),
            91,
            32,
        )
        .unwrap_or_else(|error| unreachable!("roof solves: {error:?}"));
        (roof, solution)
    }

    fn solved_descriptions() -> Vec<RoofPlacementDescription> {
        let (roof, solution) = solved_roof();
        describe_roof_placements(&roof, &solution, &full_palette())
            .unwrap_or_else(|error| unreachable!("roof descriptions resolve: {error:?}"))
    }

    #[test]
    fn palette_rejects_duplicate_pattern_bindings() {
        let duplicate = pattern(2);
        assert_eq!(
            RoofPalette::new([
                RoofPaletteEntry {
                    pattern: duplicate,
                    asset: asset(100, 0),
                },
                RoofPaletteEntry {
                    pattern: duplicate,
                    asset: asset(101, 1),
                },
            ]),
            Err(RoofPaletteError::DuplicatePattern { pattern: duplicate })
        );
    }

    #[test]
    fn descriptions_preserve_solution_tiles_plane_and_assets() {
        let (roof, solution) = solved_roof();
        let palette = full_palette();
        let descriptions = describe_roof_placements(&roof, &solution, &palette)
            .unwrap_or_else(|error| unreachable!("roof descriptions resolve: {error:?}"));

        assert_eq!(descriptions.len(), solution.selections().len());
        for (description, selection) in descriptions.iter().zip(solution.selections()) {
            let expected_asset = palette
                .asset(selection.pattern)
                .unwrap_or_else(|| unreachable!("full palette contains solved pattern"));
            assert_eq!(description.pattern, selection.pattern);
            assert_eq!(description.placement.position.tile, selection.tile);
            assert_eq!(description.placement.position.plane, roof.plane());
            assert_eq!(description.placement.object_id, expected_asset.object_id);
            assert_eq!(description.placement.kind, expected_asset.kind);
            assert_eq!(
                description.placement.orientation,
                expected_asset.orientation
            );
        }
    }

    #[test]
    fn missing_pattern_asset_is_reported_without_partial_map_edits() {
        let (roof, solution) = solved_roof();
        let first_pattern = solution
            .selections()
            .first()
            .map(|selection| selection.pattern)
            .unwrap_or_else(|| unreachable!("solved roof is non-empty"));
        let palette = RoofPalette::new(
            (0_u8..10)
                .filter(|&value| pattern(value) != first_pattern)
                .map(|value| RoofPaletteEntry {
                    pattern: pattern(value),
                    asset: asset(10_000 + u32::from(value), value % 4),
                }),
        )
        .unwrap_or_else(|error| unreachable!("valid incomplete palette: {error:?}"));

        assert_eq!(
            describe_roof_placements(&roof, &solution, &palette),
            Err(RoofPlacementDescriptionError::MissingPatternAsset {
                pattern: first_pattern,
            })
        );
    }

    #[test]
    fn descriptions_remain_sorted_in_solution_order() {
        let (roof, solution) = solved_roof();
        let descriptions = describe_roof_placements(&roof, &solution, &full_palette())
            .unwrap_or_else(|error| unreachable!("roof descriptions resolve: {error:?}"));

        let described_tiles = descriptions
            .iter()
            .map(|description| description.placement.position.tile)
            .collect::<Vec<_>>();
        let solved_tiles = solution
            .selections()
            .iter()
            .map(|selection| selection.tile)
            .collect::<Vec<_>>();
        assert_eq!(described_tiles, solved_tiles);
    }

    #[test]
    fn roof_patch_preserves_description_order_as_add_object_ops() {
        let descriptions = solved_descriptions();
        let patch = build_roof_patch(&descriptions)
            .unwrap_or_else(|error| unreachable!("valid roof patch: {error:?}"));
        let expected = descriptions
            .iter()
            .map(|description| MapOp::AddObject {
                placement: description.placement,
            })
            .collect::<Vec<_>>();

        assert_eq!(patch.ops(), expected);
    }

    #[test]
    fn roof_patch_apply_and_inverse_restore_exact_document() {
        let descriptions = solved_descriptions();
        let patch = build_roof_patch(&descriptions)
            .unwrap_or_else(|error| unreachable!("valid roof patch: {error:?}"));
        let original = MapDocument::new();
        let mut document = original.clone();

        document
            .apply_patch(&patch)
            .unwrap_or_else(|error| unreachable!("roof patch applies: {error:?}"));
        assert_eq!(document.object_count(), descriptions.len());
        for description in &descriptions {
            assert!(document.contains_object(description.placement));
        }

        document
            .apply_patch(&patch.inverse())
            .unwrap_or_else(|error| unreachable!("roof inverse applies: {error:?}"));
        assert_eq!(document, original);
    }

    #[test]
    fn roof_patch_rejects_duplicate_exact_placements() {
        let description = RoofPlacementDescription {
            pattern: pattern(0),
            placement: asset(77, 2).placement(MapTile::new(3200, 3200), plane(1)),
        };

        assert_eq!(
            build_roof_patch(&[description, description]),
            Err(RoofPatchError::DuplicateObjectPlacement {
                placement: description.placement,
            })
        );
    }
}
