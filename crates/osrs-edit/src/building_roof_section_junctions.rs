//! Semantic junction detection and classification between decomposed roof sections.
//!
//! This layer finds exact cardinal cell contacts between independently classified rectangular roof
//! sections, then classifies the local slope relationship at those contacts. It preserves a
//! deterministic section orientation for later asset repair, but does not modify WFC solutions,
//! choose concrete junction assets, or build map edits.

use crate::{CardinalDirection, RoofCell, RoofCellKind, RoofSection, RoofSectionCellPlan};
use osrs_core::coords::MapTile;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RoofSectionJunctionContact {
    pub first: RoofCell,
    pub second: RoofCell,
    pub direction_from_first: CardinalDirection,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoofSectionJunction {
    first_section: RoofSection,
    second_section: RoofSection,
    contacts: Vec<RoofSectionJunctionContact>,
}

impl RoofSectionJunction {
    pub const fn first_section(&self) -> RoofSection {
        self.first_section
    }

    pub const fn second_section(&self) -> RoofSection {
        self.second_section
    }

    pub fn contacts(&self) -> &[RoofSectionJunctionContact] {
        &self.contacts
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RoofSectionJunctionContactKind {
    /// Both touching cells descend in the same world direction, so their local slopes can continue.
    SlopeContinuation,
    /// Both touching cells descend toward the shared seam.
    Valley,
    /// Both touching cells descend away from the shared seam.
    Crest,
    /// At least one touching cell is itself a ridge cell.
    RidgeIntersection,
    /// The touching slopes disagree without forming a direct valley or crest across the seam.
    CrossSlopeIntersection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RoofSectionJunctionKind {
    SlopeContinuation,
    Valley,
    Crest,
    RidgeIntersection,
    CrossSlopeIntersection,
    /// Different contacts along the same section pair require different local junction semantics.
    Mixed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RoofSectionJunctionContactClassification {
    pub contact: RoofSectionJunctionContact,
    pub kind: RoofSectionJunctionContactKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoofSectionJunctionClassification {
    first_section: RoofSection,
    second_section: RoofSection,
    kind: RoofSectionJunctionKind,
    contacts: Vec<RoofSectionJunctionContactClassification>,
}

impl RoofSectionJunctionClassification {
    pub const fn first_section(&self) -> RoofSection {
        self.first_section
    }

    pub const fn second_section(&self) -> RoofSection {
        self.second_section
    }

    pub const fn kind(&self) -> RoofSectionJunctionKind {
        self.kind
    }

    pub fn contacts(&self) -> &[RoofSectionJunctionContactClassification] {
        &self.contacts
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoofSectionJunctionError {
    SectionMetadataMismatch {
        section: RoofSection,
    },
    DuplicateTile {
        tile: MapTile,
        first_section: RoofSection,
        second_section: RoofSection,
    },
}

type SectionSortKey = (i32, i32, u16, u16);
type JunctionSortKey = (SectionSortKey, SectionSortKey);

/// Detect exact cardinal contacts between distinct roof sections.
///
/// All input plans must belong to the same covered floor and plane. Sections are canonicalized by
/// their deterministic geometric sort key, so input order does not affect junction orientation or
/// output. Each shared edge is recorded once by scanning only north/east neighbors. Corner-only
/// contact is intentionally not considered a roof junction.
pub fn detect_roof_section_junctions(
    plans: &[RoofSectionCellPlan],
) -> Result<Vec<RoofSectionJunction>, RoofSectionJunctionError> {
    let Some(reference) = plans.first().map(RoofSectionCellPlan::section) else {
        return Ok(Vec::new());
    };

    let mut ownership = BTreeMap::<MapTile, (RoofSection, RoofCell)>::new();
    for plan in plans {
        let section = plan.section();
        if section.covered_floor() != reference.covered_floor()
            || section.plane() != reference.plane()
        {
            return Err(RoofSectionJunctionError::SectionMetadataMismatch { section });
        }
        for &cell in plan.cells() {
            if let Some((first_section, _)) = ownership.insert(cell.tile, (section, cell)) {
                return Err(RoofSectionJunctionError::DuplicateTile {
                    tile: cell.tile,
                    first_section,
                    second_section: section,
                });
            }
        }
    }

    let mut grouped = BTreeMap::<
        JunctionSortKey,
        (RoofSection, RoofSection, Vec<RoofSectionJunctionContact>),
    >::new();

    for (&tile, &(section, cell)) in &ownership {
        for direction in [CardinalDirection::North, CardinalDirection::East] {
            let Some(neighbor_tile) = direction.neighbor(tile) else {
                continue;
            };
            let Some(&(other_section, other_cell)) = ownership.get(&neighbor_tile) else {
                continue;
            };
            if section == other_section {
                continue;
            }

            let section_key = section_sort_key(section);
            let other_key = section_sort_key(other_section);
            let (key, first_section, second_section, contact) = if section_key < other_key {
                (
                    (section_key, other_key),
                    section,
                    other_section,
                    RoofSectionJunctionContact {
                        first: cell,
                        second: other_cell,
                        direction_from_first: direction,
                    },
                )
            } else {
                (
                    (other_key, section_key),
                    other_section,
                    section,
                    RoofSectionJunctionContact {
                        first: other_cell,
                        second: cell,
                        direction_from_first: opposite(direction),
                    },
                )
            };

            grouped
                .entry(key)
                .or_insert_with(|| (first_section, second_section, Vec::new()))
                .2
                .push(contact);
        }
    }

    let mut junctions = Vec::with_capacity(grouped.len());
    for (_, (first_section, second_section, mut contacts)) in grouped {
        contacts.sort_by_key(contact_sort_key);
        junctions.push(RoofSectionJunction {
            first_section,
            second_section,
            contacts,
        });
    }
    Ok(junctions)
}

/// Classify each detected section junction from the local roof-cell slope relationship.
///
/// The classifier is deliberately conservative. It identifies direct slope continuation, valleys,
/// crests, ridge intersections, and other cross-slope intersections. If different contacts along a
/// section pair require different local semantics, the whole junction is labeled `Mixed` while each
/// contact retains its exact classification.
pub fn classify_roof_section_junctions(
    junctions: &[RoofSectionJunction],
) -> Vec<RoofSectionJunctionClassification> {
    let mut ordered = junctions.iter().collect::<Vec<_>>();
    ordered.sort_by_key(|junction| {
        (
            section_sort_key(junction.first_section()),
            section_sort_key(junction.second_section()),
        )
    });

    ordered
        .into_iter()
        .map(|junction| {
            let mut contacts = junction
                .contacts()
                .iter()
                .copied()
                .map(|contact| RoofSectionJunctionContactClassification {
                    contact,
                    kind: classify_contact(contact),
                })
                .collect::<Vec<_>>();
            contacts.sort_by_key(|classified| contact_sort_key(&classified.contact));
            let kind = classify_junction_kind(&contacts);

            RoofSectionJunctionClassification {
                first_section: junction.first_section(),
                second_section: junction.second_section(),
                kind,
                contacts,
            }
        })
        .collect()
}

fn classify_contact(contact: RoofSectionJunctionContact) -> RoofSectionJunctionContactKind {
    if matches!(contact.first.kind, RoofCellKind::Ridge)
        || matches!(contact.second.kind, RoofCellKind::Ridge)
    {
        return RoofSectionJunctionContactKind::RidgeIntersection;
    }

    let first_downhill = downhill_direction(contact.first.kind)
        .unwrap_or_else(|| unreachable!("non-ridge roof cells always have downhill direction"));
    let second_downhill = downhill_direction(contact.second.kind)
        .unwrap_or_else(|| unreachable!("non-ridge roof cells always have downhill direction"));

    if first_downhill == second_downhill {
        return RoofSectionJunctionContactKind::SlopeContinuation;
    }

    let toward_second = contact.direction_from_first;
    let toward_first = opposite(toward_second);
    if first_downhill == toward_second && second_downhill == toward_first {
        RoofSectionJunctionContactKind::Valley
    } else if first_downhill == toward_first && second_downhill == toward_second {
        RoofSectionJunctionContactKind::Crest
    } else {
        RoofSectionJunctionContactKind::CrossSlopeIntersection
    }
}

fn classify_junction_kind(
    contacts: &[RoofSectionJunctionContactClassification],
) -> RoofSectionJunctionKind {
    let Some(first) = contacts.first().map(|classified| classified.kind) else {
        return RoofSectionJunctionKind::Mixed;
    };
    if contacts.iter().all(|classified| classified.kind == first) {
        match first {
            RoofSectionJunctionContactKind::SlopeContinuation => {
                RoofSectionJunctionKind::SlopeContinuation
            }
            RoofSectionJunctionContactKind::Valley => RoofSectionJunctionKind::Valley,
            RoofSectionJunctionContactKind::Crest => RoofSectionJunctionKind::Crest,
            RoofSectionJunctionContactKind::RidgeIntersection => {
                RoofSectionJunctionKind::RidgeIntersection
            }
            RoofSectionJunctionContactKind::CrossSlopeIntersection => {
                RoofSectionJunctionKind::CrossSlopeIntersection
            }
        }
    } else {
        RoofSectionJunctionKind::Mixed
    }
}

const fn downhill_direction(kind: RoofCellKind) -> Option<CardinalDirection> {
    match kind {
        RoofCellKind::Ridge => None,
        RoofCellKind::Slope { downhill } | RoofCellKind::Eave { downhill } => Some(downhill),
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
        bounds.origin().y,
        bounds.origin().x,
        bounds.height(),
        bounds.width(),
    )
}

fn contact_sort_key(contact: &RoofSectionJunctionContact) -> (MapTile, MapTile, CardinalDirection) {
    (
        contact.first.tile,
        contact.second.tile,
        contact.direction_from_first,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        BuildingArchetype, BuildingConstraints, BuildingRequest, BuildingRoofPlan, BuildingStyle,
        RectSite, classify_roof_section_cells, decompose_roof_sections, plan_roof_coverage,
        plan_roof_section_shapes, plan_vertical_connectivity,
    };
    use osrs_core::coords::SourcePlane;

    fn plane(value: u8) -> SourcePlane {
        SourcePlane::new(value).unwrap_or_else(|| unreachable!("test plane is valid"))
    }

    fn request(base_plane: SourcePlane) -> BuildingRequest {
        BuildingRequest {
            seed: 77,
            site: RectSite::new(MapTile::new(3200, 3200), 8, 8)
                .unwrap_or_else(|| unreachable!("test site is valid")),
            base_plane,
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

    fn roof_for(floor: Vec<MapTile>, base_plane: SourcePlane) -> BuildingRoofPlan {
        let vertical = plan_vertical_connectivity(request(base_plane), &[floor])
            .unwrap_or_else(|error| unreachable!("valid vertical plan: {error:?}"));
        plan_roof_coverage(&vertical)
            .unwrap_or_else(|error| unreachable!("valid roof plan: {error:?}"))
    }

    fn section_plans_for(floor: Vec<MapTile>, base_plane: SourcePlane) -> Vec<RoofSectionCellPlan> {
        let roof = roof_for(floor, base_plane);
        let sections = decompose_roof_sections(&roof)
            .unwrap_or_else(|error| unreachable!("roof decomposes: {error:?}"));
        let shapes = plan_roof_section_shapes(&sections);
        classify_roof_section_cells(&roof, &shapes)
            .unwrap_or_else(|error| unreachable!("roof section cells classify: {error:?}"))
    }

    fn l_shaped_plans() -> Vec<RoofSectionCellPlan> {
        let mut floor = rectangle(MapTile::new(3200, 3200), 5, 1);
        floor.extend(rectangle(MapTile::new(3201, 3201), 3, 3));
        section_plans_for(floor, plane(0))
    }

    fn one_contact_junction(
        first_kind: RoofCellKind,
        second_kind: RoofCellKind,
    ) -> RoofSectionJunction {
        let plans = l_shaped_plans();
        RoofSectionJunction {
            first_section: plans[0].section(),
            second_section: plans[1].section(),
            contacts: vec![RoofSectionJunctionContact {
                first: RoofCell {
                    tile: MapTile::new(3201, 3200),
                    kind: first_kind,
                },
                second: RoofCell {
                    tile: MapTile::new(3201, 3201),
                    kind: second_kind,
                },
                direction_from_first: CardinalDirection::North,
            }],
        }
    }

    fn classified_kind(junction: RoofSectionJunction) -> RoofSectionJunctionKind {
        classify_roof_section_junctions(&[junction])[0].kind()
    }

    #[test]
    fn l_shape_reports_exact_three_cell_section_junction() {
        let plans = l_shaped_plans();
        let junctions = detect_roof_section_junctions(&plans)
            .unwrap_or_else(|error| unreachable!("valid junctions: {error:?}"));

        assert_eq!(junctions.len(), 1);
        let junction = &junctions[0];
        assert_eq!(junction.first_section(), plans[0].section());
        assert_eq!(junction.second_section(), plans[1].section());
        assert_eq!(junction.contacts().len(), 3);

        for (offset, contact) in junction.contacts().iter().enumerate() {
            let x = 3201 + i32::try_from(offset).unwrap_or_else(|_| unreachable!("small offset"));
            assert_eq!(contact.first.tile, MapTile::new(x, 3200));
            assert_eq!(contact.second.tile, MapTile::new(x, 3201));
            assert_eq!(contact.direction_from_first, CardinalDirection::North);
            assert_eq!(contact.first.kind, RoofCellKind::Ridge);
            assert_eq!(
                contact.second.kind,
                RoofCellKind::Slope {
                    downhill: CardinalDirection::South,
                }
            );
        }
    }

    #[test]
    fn one_rectangular_section_has_no_junctions() {
        let plans = section_plans_for(rectangle(MapTile::new(3201, 3201), 5, 3), plane(0));

        assert_eq!(detect_roof_section_junctions(&plans), Ok(Vec::new()));
    }

    #[test]
    fn corner_only_contact_is_not_a_junction() {
        let floor = vec![MapTile::new(3200, 3200), MapTile::new(3201, 3201)];
        let plans = section_plans_for(floor, plane(0));
        assert_eq!(plans.len(), 2);

        assert_eq!(detect_roof_section_junctions(&plans), Ok(Vec::new()));
    }

    #[test]
    fn junction_detection_is_independent_of_plan_input_order() {
        let plans = l_shaped_plans();
        let mut reversed = plans.clone();
        reversed.reverse();

        assert_eq!(
            detect_roof_section_junctions(&plans),
            detect_roof_section_junctions(&reversed)
        );
    }

    #[test]
    fn duplicate_section_tiles_are_rejected() {
        let plans = l_shaped_plans();
        let duplicate = plans[0].clone();
        let tile = duplicate.cells()[0].tile;
        let section = duplicate.section();
        let duplicated = vec![duplicate.clone(), duplicate];

        assert_eq!(
            detect_roof_section_junctions(&duplicated),
            Err(RoofSectionJunctionError::DuplicateTile {
                tile,
                first_section: section,
                second_section: section,
            })
        );
    }

    #[test]
    fn mixed_roof_planes_are_rejected_before_contact_detection() {
        let floor = rectangle(MapTile::new(3201, 3201), 3, 3);
        let first = section_plans_for(floor.clone(), plane(0));
        let second = section_plans_for(floor, plane(1));
        let mixed = vec![first[0].clone(), second[0].clone()];

        assert_eq!(
            detect_roof_section_junctions(&mixed),
            Err(RoofSectionJunctionError::SectionMetadataMismatch {
                section: second[0].section(),
            })
        );
    }

    #[test]
    fn actual_l_shape_classifies_as_ridge_intersection() {
        let plans = l_shaped_plans();
        let junctions = detect_roof_section_junctions(&plans)
            .unwrap_or_else(|error| unreachable!("valid junctions: {error:?}"));
        let classified = classify_roof_section_junctions(&junctions);

        assert_eq!(classified.len(), 1);
        assert_eq!(
            classified[0].kind(),
            RoofSectionJunctionKind::RidgeIntersection
        );
        assert!(
            classified[0]
                .contacts()
                .iter()
                .all(|contact| contact.kind == RoofSectionJunctionContactKind::RidgeIntersection)
        );
    }

    #[test]
    fn same_downhill_direction_is_slope_continuation() {
        let kind = RoofCellKind::Slope {
            downhill: CardinalDirection::North,
        };
        assert_eq!(
            classified_kind(one_contact_junction(kind, kind)),
            RoofSectionJunctionKind::SlopeContinuation
        );
    }

    #[test]
    fn slopes_descending_into_seam_form_valley() {
        let junction = one_contact_junction(
            RoofCellKind::Slope {
                downhill: CardinalDirection::North,
            },
            RoofCellKind::Slope {
                downhill: CardinalDirection::South,
            },
        );
        assert_eq!(classified_kind(junction), RoofSectionJunctionKind::Valley);
    }

    #[test]
    fn slopes_descending_away_from_seam_form_crest() {
        let junction = one_contact_junction(
            RoofCellKind::Slope {
                downhill: CardinalDirection::South,
            },
            RoofCellKind::Slope {
                downhill: CardinalDirection::North,
            },
        );
        assert_eq!(classified_kind(junction), RoofSectionJunctionKind::Crest);
    }

    #[test]
    fn perpendicular_conflicting_slopes_form_cross_slope_intersection() {
        let junction = one_contact_junction(
            RoofCellKind::Slope {
                downhill: CardinalDirection::East,
            },
            RoofCellKind::Slope {
                downhill: CardinalDirection::West,
            },
        );
        assert_eq!(
            classified_kind(junction),
            RoofSectionJunctionKind::CrossSlopeIntersection
        );
    }

    #[test]
    fn mixed_contact_semantics_do_not_force_one_junction_type() {
        let plans = l_shaped_plans();
        let junction = RoofSectionJunction {
            first_section: plans[0].section(),
            second_section: plans[1].section(),
            contacts: vec![
                RoofSectionJunctionContact {
                    first: RoofCell {
                        tile: MapTile::new(3201, 3200),
                        kind: RoofCellKind::Ridge,
                    },
                    second: RoofCell {
                        tile: MapTile::new(3201, 3201),
                        kind: RoofCellKind::Slope {
                            downhill: CardinalDirection::South,
                        },
                    },
                    direction_from_first: CardinalDirection::North,
                },
                RoofSectionJunctionContact {
                    first: RoofCell {
                        tile: MapTile::new(3202, 3200),
                        kind: RoofCellKind::Slope {
                            downhill: CardinalDirection::North,
                        },
                    },
                    second: RoofCell {
                        tile: MapTile::new(3202, 3201),
                        kind: RoofCellKind::Slope {
                            downhill: CardinalDirection::South,
                        },
                    },
                    direction_from_first: CardinalDirection::North,
                },
            ],
        };
        let classified = classify_roof_section_junctions(&[junction]);

        assert_eq!(classified[0].kind(), RoofSectionJunctionKind::Mixed);
        assert_eq!(
            classified[0].contacts()[0].kind,
            RoofSectionJunctionContactKind::RidgeIntersection
        );
        assert_eq!(
            classified[0].contacts()[1].kind,
            RoofSectionJunctionContactKind::Valley
        );
    }
}
