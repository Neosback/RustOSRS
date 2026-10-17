//! Semantic repair planning for classified complex-roof section junctions.
//!
//! Classified junction contacts are converted into deterministic per-tile repair requirements.
//! Slope-continuation contacts need no repair. Valley, crest, ridge-intersection, and cross-slope
//! contacts produce requirements on both touching tiles. This layer remains asset-independent and
//! does not mutate WFC solutions, object placements, or map patches.

use crate::{
    CardinalDirection, RoofCell, RoofSection, RoofSectionJunctionClassification,
    RoofSectionJunctionContactKind,
};
use osrs_core::coords::MapTile;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RoofSectionJunctionRepairRole {
    Valley,
    Crest,
    RidgeIntersection,
    CrossSlopeIntersection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RoofSectionJunctionRepairRequirement {
    pub role: RoofSectionJunctionRepairRole,
    pub direction_to_other: CardinalDirection,
    pub other_tile: MapTile,
    pub other_section: RoofSection,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoofSectionJunctionRepairSite {
    section: RoofSection,
    cell: RoofCell,
    requirements: Vec<RoofSectionJunctionRepairRequirement>,
}

impl RoofSectionJunctionRepairSite {
    pub const fn section(&self) -> RoofSection {
        self.section
    }

    pub const fn cell(&self) -> RoofCell {
        self.cell
    }

    pub fn requirements(&self) -> &[RoofSectionJunctionRepairRequirement] {
        &self.requirements
    }
}

type SectionSortKey = (u8, u8, i32, i32, u16, u16);
type RepairSiteKey = (SectionSortKey, MapTile);
type RepairSiteValue = (
    RoofSection,
    RoofCell,
    Vec<RoofSectionJunctionRepairRequirement>,
);

/// Plan asset-independent repair requirements for classified section junctions.
///
/// Requirements are grouped by owning section and tile so a tile participating in multiple seams
/// remains one repair site with multiple local requirements. Duplicate classifications do not
/// duplicate requirements. Output order is deterministic and independent of input order.
pub fn plan_roof_section_junction_repairs(
    classifications: &[RoofSectionJunctionClassification],
) -> Vec<RoofSectionJunctionRepairSite> {
    let mut ordered = classifications.iter().collect::<Vec<_>>();
    ordered.sort_by_key(|classification| {
        (
            section_sort_key(classification.first_section()),
            section_sort_key(classification.second_section()),
        )
    });

    let mut sites = BTreeMap::<RepairSiteKey, RepairSiteValue>::new();
    for classification in ordered {
        for classified in classification.contacts() {
            let Some(role) = repair_role(classified.kind) else {
                continue;
            };
            let contact = classified.contact;
            let first_section = classification.first_section();
            let second_section = classification.second_section();

            push_requirement(
                &mut sites,
                first_section,
                contact.first,
                RoofSectionJunctionRepairRequirement {
                    role,
                    direction_to_other: contact.direction_from_first,
                    other_tile: contact.second.tile,
                    other_section: second_section,
                },
            );
            push_requirement(
                &mut sites,
                second_section,
                contact.second,
                RoofSectionJunctionRepairRequirement {
                    role,
                    direction_to_other: opposite(contact.direction_from_first),
                    other_tile: contact.first.tile,
                    other_section: first_section,
                },
            );
        }
    }

    sites
        .into_values()
        .map(|(section, cell, mut requirements)| {
            requirements.sort_by_key(requirement_sort_key);
            requirements
                .dedup_by(|left, right| requirement_sort_key(left) == requirement_sort_key(right));
            RoofSectionJunctionRepairSite {
                section,
                cell,
                requirements,
            }
        })
        .collect()
}

fn push_requirement(
    sites: &mut BTreeMap<RepairSiteKey, RepairSiteValue>,
    section: RoofSection,
    cell: RoofCell,
    requirement: RoofSectionJunctionRepairRequirement,
) {
    let entry = sites
        .entry((section_sort_key(section), cell.tile))
        .or_insert_with(|| (section, cell, Vec::new()));
    debug_assert_eq!(entry.0, section);
    debug_assert_eq!(entry.1, cell);
    entry.2.push(requirement);
}

const fn repair_role(
    kind: RoofSectionJunctionContactKind,
) -> Option<RoofSectionJunctionRepairRole> {
    match kind {
        RoofSectionJunctionContactKind::SlopeContinuation => None,
        RoofSectionJunctionContactKind::Valley => Some(RoofSectionJunctionRepairRole::Valley),
        RoofSectionJunctionContactKind::Crest => Some(RoofSectionJunctionRepairRole::Crest),
        RoofSectionJunctionContactKind::RidgeIntersection => {
            Some(RoofSectionJunctionRepairRole::RidgeIntersection)
        }
        RoofSectionJunctionContactKind::CrossSlopeIntersection => {
            Some(RoofSectionJunctionRepairRole::CrossSlopeIntersection)
        }
    }
}

const fn opposite(direction: CardinalDirection) -> CardinalDirection {
    match direction {
        CardinalDirection::North => CardinalDirection::South,
        CardinalDirection::East => CardinalDirection::West,
        CardinalDirection::South => CardinalDirection::North,
        CardinalDirection::West => CardinalDirection::East,
    }
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

fn requirement_sort_key(
    requirement: &RoofSectionJunctionRepairRequirement,
) -> (
    RoofSectionJunctionRepairRole,
    CardinalDirection,
    MapTile,
    SectionSortKey,
) {
    (
        requirement.role,
        requirement.direction_to_other,
        requirement.other_tile,
        section_sort_key(requirement.other_section),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        BuildingArchetype, BuildingConstraints, BuildingRequest, BuildingRoofPlan, BuildingStyle,
        RectSite, classify_roof_section_cells, classify_roof_section_junctions,
        decompose_roof_sections, detect_roof_section_junctions, plan_roof_coverage,
        plan_roof_section_shapes, plan_vertical_connectivity,
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

    fn classifications_for(floor: Vec<MapTile>) -> Vec<RoofSectionJunctionClassification> {
        let vertical = plan_vertical_connectivity(request(), &[floor])
            .unwrap_or_else(|error| unreachable!("valid vertical plan: {error:?}"));
        let roof: BuildingRoofPlan = plan_roof_coverage(&vertical)
            .unwrap_or_else(|error| unreachable!("valid roof plan: {error:?}"));
        let sections = decompose_roof_sections(&roof)
            .unwrap_or_else(|error| unreachable!("roof decomposes: {error:?}"));
        let shapes = plan_roof_section_shapes(&sections);
        let cells = classify_roof_section_cells(&roof, &shapes)
            .unwrap_or_else(|error| unreachable!("roof section cells classify: {error:?}"));
        let junctions = detect_roof_section_junctions(&cells)
            .unwrap_or_else(|error| unreachable!("valid roof junctions: {error:?}"));
        classify_roof_section_junctions(&junctions)
    }

    fn l_shape_classifications() -> Vec<RoofSectionJunctionClassification> {
        let mut floor = rectangle(MapTile::new(3200, 3200), 5, 1);
        floor.extend(rectangle(MapTile::new(3201, 3201), 3, 3));
        classifications_for(floor)
    }

    #[test]
    fn l_shape_repairs_both_sides_of_all_ridge_intersection_contacts() {
        let repairs = plan_roof_section_junction_repairs(&l_shape_classifications());

        assert_eq!(repairs.len(), 6);
        for repair in &repairs {
            assert_eq!(repair.requirements().len(), 1);
            assert_eq!(
                repair.requirements()[0].role,
                RoofSectionJunctionRepairRole::RidgeIntersection
            );
        }

        for (offset, repair) in repairs[..3].iter().enumerate() {
            let x = 3201 + i32::try_from(offset).unwrap_or_else(|_| unreachable!("small offset"));
            let requirement = repair.requirements()[0];
            assert_eq!(repair.cell().tile, MapTile::new(x, 3200));
            assert_eq!(requirement.direction_to_other, CardinalDirection::North);
            assert_eq!(requirement.other_tile, MapTile::new(x, 3201));
        }
        for (offset, repair) in repairs[3..].iter().enumerate() {
            let x = 3201 + i32::try_from(offset).unwrap_or_else(|_| unreachable!("small offset"));
            let requirement = repair.requirements()[0];
            assert_eq!(repair.cell().tile, MapTile::new(x, 3201));
            assert_eq!(requirement.direction_to_other, CardinalDirection::South);
            assert_eq!(requirement.other_tile, MapTile::new(x, 3200));
        }
    }

    #[test]
    fn continuation_needs_no_repair_and_other_contact_kinds_map_to_roles() {
        assert_eq!(
            repair_role(RoofSectionJunctionContactKind::SlopeContinuation),
            None
        );
        assert_eq!(
            repair_role(RoofSectionJunctionContactKind::Valley),
            Some(RoofSectionJunctionRepairRole::Valley)
        );
        assert_eq!(
            repair_role(RoofSectionJunctionContactKind::Crest),
            Some(RoofSectionJunctionRepairRole::Crest)
        );
        assert_eq!(
            repair_role(RoofSectionJunctionContactKind::RidgeIntersection),
            Some(RoofSectionJunctionRepairRole::RidgeIntersection)
        );
        assert_eq!(
            repair_role(RoofSectionJunctionContactKind::CrossSlopeIntersection),
            Some(RoofSectionJunctionRepairRole::CrossSlopeIntersection)
        );
    }

    #[test]
    fn duplicate_classifications_do_not_duplicate_requirements() {
        let classifications = l_shape_classifications();
        let expected = plan_roof_section_junction_repairs(&classifications);
        let duplicated = vec![classifications[0].clone(), classifications[0].clone()];

        assert_eq!(plan_roof_section_junction_repairs(&duplicated), expected);
    }

    #[test]
    fn repair_plan_is_independent_of_classification_input_order() {
        let mut floor = rectangle(MapTile::new(3200, 3200), 5, 1);
        floor.extend(rectangle(MapTile::new(3200, 3201), 1, 3));
        floor.extend(rectangle(MapTile::new(3204, 3201), 1, 3));
        let classifications = classifications_for(floor);
        assert_eq!(classifications.len(), 2);
        let mut reversed = classifications.clone();
        reversed.reverse();

        assert_eq!(
            plan_roof_section_junction_repairs(&classifications),
            plan_roof_section_junction_repairs(&reversed)
        );
    }

    #[test]
    fn one_tile_can_accumulate_multiple_seam_requirements() {
        let floor = vec![
            MapTile::new(3201, 3200),
            MapTile::new(3200, 3201),
            MapTile::new(3201, 3201),
            MapTile::new(3202, 3201),
            MapTile::new(3201, 3202),
        ];
        let repairs = plan_roof_section_junction_repairs(&classifications_for(floor));
        let center = repairs
            .iter()
            .find(|repair| {
                repair.cell().tile == MapTile::new(3201, 3201)
                    && repair.section().bounds().width() == 3
            })
            .unwrap_or_else(|| unreachable!("horizontal center tile must require repair"));

        assert_eq!(center.requirements().len(), 2);
        assert_eq!(
            center
                .requirements()
                .iter()
                .map(|requirement| requirement.direction_to_other)
                .collect::<Vec<_>>(),
            vec![CardinalDirection::North, CardinalDirection::South]
        );
        assert!(center.requirements().iter().all(|requirement| {
            requirement.role == RoofSectionJunctionRepairRole::RidgeIntersection
        }));
    }
}
