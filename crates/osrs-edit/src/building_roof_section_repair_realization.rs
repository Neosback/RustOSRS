//! Concrete realization of semantic complex-roof junction repair requirements.
//!
//! Junction repair sites are normalized into semantic profile keys. A junction pattern palette maps
//! each profile to a replacement `WfcPatternId`, then the existing `RoofPalette` resolves that
//! pattern into the concrete roof asset. Normal section placements are replaced before patch build.

use crate::{
    CardinalDirection, RoofPalette, RoofPlacementDescription, RoofSection,
    RoofSectionJunctionRepairRole, RoofSectionJunctionRepairSite, RoofSectionPlacementPlan,
    WfcPatternId,
};
use osrs_core::coords::MapTile;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RoofJunctionPatternRequirement {
    pub role: RoofSectionJunctionRepairRole,
    pub direction_to_other: CardinalDirection,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RoofJunctionPatternKey {
    requirements: Vec<RoofJunctionPatternRequirement>,
}

impl RoofJunctionPatternKey {
    pub fn new(requirements: impl IntoIterator<Item = RoofJunctionPatternRequirement>) -> Self {
        let mut requirements = requirements.into_iter().collect::<Vec<_>>();
        requirements.sort_unstable();
        requirements.dedup();
        Self { requirements }
    }

    pub fn requirements(&self) -> &[RoofJunctionPatternRequirement] {
        &self.requirements
    }

    fn from_repair_site(site: &RoofSectionJunctionRepairSite) -> Self {
        Self::new(
            site.requirements()
                .iter()
                .map(|requirement| RoofJunctionPatternRequirement {
                    role: requirement.role,
                    direction_to_other: requirement.direction_to_other,
                }),
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoofJunctionPatternPaletteEntry {
    pub key: RoofJunctionPatternKey,
    pub pattern: WfcPatternId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoofJunctionPatternPalette {
    patterns: BTreeMap<RoofJunctionPatternKey, WfcPatternId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RoofJunctionPatternPaletteError {
    DuplicateKey { key: RoofJunctionPatternKey },
}

impl RoofJunctionPatternPalette {
    pub fn new(
        entries: impl IntoIterator<Item = RoofJunctionPatternPaletteEntry>,
    ) -> Result<Self, RoofJunctionPatternPaletteError> {
        let mut patterns = BTreeMap::new();
        for entry in entries {
            if patterns.insert(entry.key.clone(), entry.pattern).is_some() {
                return Err(RoofJunctionPatternPaletteError::DuplicateKey { key: entry.key });
            }
        }
        Ok(Self { patterns })
    }

    pub fn pattern(&self, key: &RoofJunctionPatternKey) -> Option<WfcPatternId> {
        self.patterns.get(key).copied()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RoofSectionJunctionRealizationError {
    DuplicateBasePlacementTile {
        section: RoofSection,
        tile: MapTile,
    },
    DuplicateRepairSite {
        section: RoofSection,
        tile: MapTile,
    },
    MissingBasePlacement {
        section: RoofSection,
        tile: MapTile,
    },
    MissingJunctionPattern {
        section: RoofSection,
        tile: MapTile,
        key: RoofJunctionPatternKey,
    },
    MissingPatternAsset {
        section: RoofSection,
        tile: MapTile,
        pattern: WfcPatternId,
    },
}

type SectionSortKey = (u8, u8, i32, i32, u16, u16);
type PlacementKey = (SectionSortKey, MapTile);

/// Replace normal section roof placements at semantic repair sites with junction-specific assets.
///
/// Output is flattened in deterministic section/world-tile order and is ready for `build_roof_patch`.
/// The order of section plans and repair sites does not affect the result.
pub fn describe_repaired_roof_section_placements(
    plans: &[RoofSectionPlacementPlan],
    repairs: &[RoofSectionJunctionRepairSite],
    junction_patterns: &RoofJunctionPatternPalette,
    roof_palette: &RoofPalette,
) -> Result<Vec<RoofPlacementDescription>, RoofSectionJunctionRealizationError> {
    let mut placements = BTreeMap::<PlacementKey, (RoofSection, RoofPlacementDescription)>::new();
    for plan in plans {
        let section = plan.section();
        for &description in plan.descriptions() {
            let tile = description.placement.position.tile;
            let key = (section_sort_key(section), tile);
            if placements.insert(key, (section, description)).is_some() {
                return Err(
                    RoofSectionJunctionRealizationError::DuplicateBasePlacementTile {
                        section,
                        tile,
                    },
                );
            }
        }
    }

    let mut repair_sites = BTreeMap::<PlacementKey, &RoofSectionJunctionRepairSite>::new();
    for repair in repairs {
        let section = repair.section();
        let tile = repair.cell().tile;
        let key = (section_sort_key(section), tile);
        if repair_sites.insert(key, repair).is_some() {
            return Err(RoofSectionJunctionRealizationError::DuplicateRepairSite { section, tile });
        }
    }

    for (placement_key, repair) in repair_sites {
        let section = repair.section();
        let tile = repair.cell().tile;
        let Some((_, description)) = placements.get_mut(&placement_key) else {
            return Err(RoofSectionJunctionRealizationError::MissingBasePlacement {
                section,
                tile,
            });
        };

        let key = RoofJunctionPatternKey::from_repair_site(repair);
        let pattern = junction_patterns.pattern(&key).ok_or_else(|| {
            RoofSectionJunctionRealizationError::MissingJunctionPattern {
                section,
                tile,
                key: key.clone(),
            }
        })?;
        let asset = roof_palette.asset(pattern).ok_or(
            RoofSectionJunctionRealizationError::MissingPatternAsset {
                section,
                tile,
                pattern,
            },
        )?;

        description.pattern = pattern;
        description.placement = asset.placement(tile, section.plane());
    }

    Ok(placements
        .into_values()
        .map(|(_, description)| description)
        .collect())
}

fn section_sort_key(section: RoofSection) -> SectionSortKey {
    let bounds = section.bounds();
    (
        section.covered_floor().get(),
        section.plane().index().get(),
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
        LocKind, MapDocument, ObjectOrientation, PatternDomain, RectSite, RoofAsset,
        RoofPaletteEntry, RoofPatternSet, RoofSectionCellPlan, RoofSectionWfcPlan, WfcCatalog,
        WfcPatternSpec, WfcSocket, build_roof_patch, classify_roof_section_cells,
        classify_roof_section_junctions, decompose_roof_sections, describe_roof_section_placements,
        detect_roof_section_junctions, plan_roof_coverage, plan_roof_section_junction_repairs,
        plan_roof_section_shapes, plan_vertical_connectivity, roof_section_wfc_constraints,
        solve_roof_section_wfc,
    };
    use osrs_core::coords::SourcePlane;
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

    fn roof_palette(include_junctions: bool) -> RoofPalette {
        let end = if include_junctions { 12 } else { 10 };
        RoofPalette::new((0_u8..end).map(|value| RoofPaletteEntry {
            pattern: pattern(value),
            asset: asset(10_000 + u32::from(value), value % 4),
        }))
        .unwrap_or_else(|error| unreachable!("valid roof palette: {error:?}"))
    }

    fn key(
        requirements: impl IntoIterator<Item = (RoofSectionJunctionRepairRole, CardinalDirection)>,
    ) -> RoofJunctionPatternKey {
        RoofJunctionPatternKey::new(requirements.into_iter().map(|(role, direction_to_other)| {
            RoofJunctionPatternRequirement {
                role,
                direction_to_other,
            }
        }))
    }

    fn l_shape_junction_patterns() -> RoofJunctionPatternPalette {
        RoofJunctionPatternPalette::new([
            RoofJunctionPatternPaletteEntry {
                key: key([(
                    RoofSectionJunctionRepairRole::RidgeIntersection,
                    CardinalDirection::North,
                )]),
                pattern: pattern(10),
            },
            RoofJunctionPatternPaletteEntry {
                key: key([(
                    RoofSectionJunctionRepairRole::RidgeIntersection,
                    CardinalDirection::South,
                )]),
                pattern: pattern(11),
            },
        ])
        .unwrap_or_else(|error| unreachable!("valid junction palette: {error:?}"))
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

    fn fixture() -> (
        Vec<RoofSectionPlacementPlan>,
        Vec<RoofSectionJunctionRepairSite>,
    ) {
        let mut floor = rectangle(MapTile::new(3200, 3200), 5, 1);
        floor.extend(rectangle(MapTile::new(3201, 3201), 3, 3));
        let roof = roof_for(floor);
        let cells = section_cells(&roof);
        let wfc_plans: Vec<RoofSectionWfcPlan> = roof_section_wfc_constraints(&cells, patterns())
            .unwrap_or_else(|error| unreachable!("section constraints are valid: {error:?}"));
        let solutions = solve_roof_section_wfc(&catalog(), &wfc_plans, 91, 32)
            .unwrap_or_else(|error| unreachable!("roof sections solve: {error:?}"));
        let placements = describe_roof_section_placements(&solutions, &roof_palette(true))
            .unwrap_or_else(|error| unreachable!("section descriptions resolve: {error:?}"));
        let junctions = detect_roof_section_junctions(&cells)
            .unwrap_or_else(|error| unreachable!("valid junctions: {error:?}"));
        let classifications = classify_roof_section_junctions(&junctions);
        let repairs = plan_roof_section_junction_repairs(&classifications);
        (placements, repairs)
    }

    #[test]
    fn key_normalizes_requirement_order_and_duplicates() {
        let normalized = key([
            (
                RoofSectionJunctionRepairRole::RidgeIntersection,
                CardinalDirection::South,
            ),
            (
                RoofSectionJunctionRepairRole::RidgeIntersection,
                CardinalDirection::North,
            ),
            (
                RoofSectionJunctionRepairRole::RidgeIntersection,
                CardinalDirection::North,
            ),
        ]);

        assert_eq!(normalized.requirements().len(), 2);
        assert_eq!(
            normalized.requirements()[0].direction_to_other,
            CardinalDirection::North
        );
        assert_eq!(
            normalized.requirements()[1].direction_to_other,
            CardinalDirection::South
        );
    }

    #[test]
    fn palette_rejects_duplicate_normalized_profiles() {
        let duplicate = key([(
            RoofSectionJunctionRepairRole::Valley,
            CardinalDirection::East,
        )]);
        assert_eq!(
            RoofJunctionPatternPalette::new([
                RoofJunctionPatternPaletteEntry {
                    key: duplicate.clone(),
                    pattern: pattern(10),
                },
                RoofJunctionPatternPaletteEntry {
                    key: duplicate.clone(),
                    pattern: pattern(11),
                },
            ]),
            Err(RoofJunctionPatternPaletteError::DuplicateKey { key: duplicate })
        );
    }

    #[test]
    fn l_shape_replaces_all_six_junction_tiles_with_bound_assets() {
        let (plans, repairs) = fixture();
        let repaired = describe_repaired_roof_section_placements(
            &plans,
            &repairs,
            &l_shape_junction_patterns(),
            &roof_palette(true),
        )
        .unwrap_or_else(|error| unreachable!("junction repairs resolve: {error:?}"));
        let repaired_by_tile = repaired
            .iter()
            .map(|description| (description.placement.position.tile, description))
            .collect::<BTreeMap<_, _>>();

        assert_eq!(repairs.len(), 6);
        assert_eq!(
            repaired.len(),
            plans
                .iter()
                .map(|plan| plan.descriptions().len())
                .sum::<usize>()
        );
        for repair in &repairs {
            let expected_pattern = match repair.requirements()[0].direction_to_other {
                CardinalDirection::North => pattern(10),
                CardinalDirection::South => pattern(11),
                _ => unreachable!("L-shape seam is north/south"),
            };
            let description = repaired_by_tile
                .get(&repair.cell().tile)
                .unwrap_or_else(|| unreachable!("repair tile remains in output"));
            let expected_asset = roof_palette(true)
                .asset(expected_pattern)
                .unwrap_or_else(|| unreachable!("junction pattern has asset"));
            assert_eq!(description.pattern, expected_pattern);
            assert_eq!(description.placement.object_id, expected_asset.object_id);
        }
    }

    #[test]
    fn realization_is_independent_of_plan_and_repair_input_order() {
        let (plans, repairs) = fixture();
        let mut reversed_plans = plans.clone();
        reversed_plans.reverse();
        let mut reversed_repairs = repairs.clone();
        reversed_repairs.reverse();
        let junction_patterns = l_shape_junction_patterns();
        let roof_palette = roof_palette(true);

        assert_eq!(
            describe_repaired_roof_section_placements(
                &plans,
                &repairs,
                &junction_patterns,
                &roof_palette,
            ),
            describe_repaired_roof_section_placements(
                &reversed_plans,
                &reversed_repairs,
                &junction_patterns,
                &roof_palette,
            )
        );
    }

    #[test]
    fn missing_profile_and_asset_report_exact_first_repair_site() {
        let (plans, repairs) = fixture();
        let first = &repairs[0];
        let expected_key = RoofJunctionPatternKey::from_repair_site(first);
        let empty = RoofJunctionPatternPalette::new([])
            .unwrap_or_else(|error| unreachable!("empty palette is valid: {error:?}"));

        assert_eq!(
            describe_repaired_roof_section_placements(
                &plans,
                &repairs,
                &empty,
                &roof_palette(true),
            ),
            Err(
                RoofSectionJunctionRealizationError::MissingJunctionPattern {
                    section: first.section(),
                    tile: first.cell().tile,
                    key: expected_key.clone(),
                }
            )
        );

        let expected_pattern = l_shape_junction_patterns()
            .pattern(&expected_key)
            .unwrap_or_else(|| unreachable!("first repair profile is bound"));
        assert_eq!(
            describe_repaired_roof_section_placements(
                &plans,
                &repairs,
                &l_shape_junction_patterns(),
                &roof_palette(false),
            ),
            Err(RoofSectionJunctionRealizationError::MissingPatternAsset {
                section: first.section(),
                tile: first.cell().tile,
                pattern: expected_pattern,
            })
        );
    }

    #[test]
    fn repaired_patch_apply_and_inverse_restore_exact_document() {
        let (plans, repairs) = fixture();
        let repaired = describe_repaired_roof_section_placements(
            &plans,
            &repairs,
            &l_shape_junction_patterns(),
            &roof_palette(true),
        )
        .unwrap_or_else(|error| unreachable!("junction repairs resolve: {error:?}"));
        let patch = build_roof_patch(&repaired)
            .unwrap_or_else(|error| unreachable!("repaired roof patch builds: {error:?}"));
        let original = MapDocument::new();
        let mut document = original.clone();

        document
            .apply_patch(&patch)
            .unwrap_or_else(|error| unreachable!("repaired roof patch applies: {error:?}"));
        assert_eq!(document.object_count(), repaired.len());

        document
            .apply_patch(&patch.inverse())
            .unwrap_or_else(|error| unreachable!("repaired roof inverse applies: {error:?}"));
        assert_eq!(document, original);
    }
}
