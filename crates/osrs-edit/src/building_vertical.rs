//! Multi-floor support and deterministic vertical-connectivity planning.
//!
//! Each floor remains a two-dimensional semantic solve. This module validates how those solved
//! floor surfaces stack and chooses paired stair anchors between adjacent floors. It deliberately
//! avoids one monolithic 3D WFC domain: global vertical guarantees are fixed before local detail.

use crate::{
    BuildingRequest, CardinalDirection, LocKind, MapOp, MapPatch, MapPosition, ObjectOrientation,
    ObjectPlacement, PatternDomain, WfcConstraint, WfcCoord, WfcPatternId,
};
use osrs_core::coords::{MapTile, SourcePlane};
use osrs_core::ids::ObjectId;
use std::collections::{BTreeSet, HashSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BuildingFloorIndex(u8);

impl BuildingFloorIndex {
    pub const fn new(value: u8) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u8 {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildingFloorSurface {
    pub floor: BuildingFloorIndex,
    pub plane: SourcePlane,
    tiles: Vec<MapTile>,
}

impl BuildingFloorSurface {
    pub fn tiles(&self) -> &[MapTile] {
        &self.tiles
    }

    pub fn contains(&self, tile: MapTile) -> bool {
        self.tiles.binary_search(&tile).is_ok()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StairAnchor {
    pub tile: MapTile,
    pub facing: CardinalDirection,
    pub lower_floor: BuildingFloorIndex,
    pub upper_floor: BuildingFloorIndex,
}

impl StairAnchor {
    pub fn approach_tile(self) -> Option<MapTile> {
        self.facing.neighbor(self.tile)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildingVerticalPlan {
    pub request: BuildingRequest,
    floors: Vec<BuildingFloorSurface>,
    stairs: Vec<StairAnchor>,
}

impl BuildingVerticalPlan {
    pub fn floors(&self) -> &[BuildingFloorSurface] {
        &self.floors
    }

    pub fn stairs(&self) -> &[StairAnchor] {
        &self.stairs
    }

    pub fn floor(&self, index: BuildingFloorIndex) -> Option<&BuildingFloorSurface> {
        self.floors.iter().find(|floor| floor.floor == index)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildingVerticalError {
    FloorCountMustBePositive,
    FloorCountMismatch {
        requested: u8,
        provided: usize,
    },
    PlaneOverflow {
        floor: BuildingFloorIndex,
    },
    EmptyFloor {
        floor: BuildingFloorIndex,
    },
    TileOutsideSite {
        floor: BuildingFloorIndex,
        tile: MapTile,
    },
    UnsupportedUpperTile {
        floor: BuildingFloorIndex,
        tile: MapTile,
    },
    NoStairAnchor {
        lower_floor: BuildingFloorIndex,
        upper_floor: BuildingFloorIndex,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StairAsset {
    pub object_id: ObjectId,
    pub kind: LocKind,
    pub orientations: [ObjectOrientation; 4],
}

impl StairAsset {
    pub const fn orientation_for(self, facing: CardinalDirection) -> ObjectOrientation {
        self.orientations[facing.index()]
    }

    pub fn placement(
        self,
        tile: MapTile,
        facing: CardinalDirection,
        plane: SourcePlane,
    ) -> ObjectPlacement {
        ObjectPlacement {
            object_id: self.object_id,
            position: MapPosition::new(tile, plane),
            kind: self.kind,
            orientation: self.orientation_for(facing),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StairPalette {
    pub up: StairAsset,
    pub down: StairAsset,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StairRealizationError {
    MissingFloor { floor: BuildingFloorIndex },
    DuplicateObjectPlacement { placement: ObjectPlacement },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StairPatternSet {
    pub up: WfcPatternId,
    pub down: WfcPatternId,
    pub approach_allowed: PatternDomain,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StairConstraintError {
    MissingFloor { floor: BuildingFloorIndex },
    EmptyApproachDomain,
    InvalidAnchorTile { tile: MapTile },
    InvalidApproachTile { tile: MapTile },
}

/// Validate aligned solved floor surfaces and choose one deterministic stair anchor per transition.
///
/// The initial vertical policy is intentionally strict: every upper-floor tile must be supported by
/// the floor directly below it, and a stair needs two cardinally adjacent walkable tiles on both
/// floors so it has a deterministic approach direction. Stair footprints are reserved on both
/// floors, preventing adjacent vertical transitions from silently occupying the same tiles.
pub fn plan_vertical_connectivity(
    request: BuildingRequest,
    floor_tiles: &[Vec<MapTile>],
) -> Result<BuildingVerticalPlan, BuildingVerticalError> {
    if request.floors == 0 {
        return Err(BuildingVerticalError::FloorCountMustBePositive);
    }
    if usize::from(request.floors) != floor_tiles.len() {
        return Err(BuildingVerticalError::FloorCountMismatch {
            requested: request.floors,
            provided: floor_tiles.len(),
        });
    }

    let mut floors = Vec::with_capacity(floor_tiles.len());
    let mut floor_sets = Vec::with_capacity(floor_tiles.len());
    for (raw_floor, tiles) in floor_tiles.iter().enumerate() {
        let floor = BuildingFloorIndex::new(
            u8::try_from(raw_floor).unwrap_or_else(|_| unreachable!("request floor count is u8")),
        );
        let plane = source_plane_for_floor(request, floor)?;
        let set = tiles.iter().copied().collect::<BTreeSet<_>>();
        if set.is_empty() {
            return Err(BuildingVerticalError::EmptyFloor { floor });
        }
        for &tile in &set {
            if !request.site.contains(tile) {
                return Err(BuildingVerticalError::TileOutsideSite { floor, tile });
            }
        }
        floors.push(BuildingFloorSurface {
            floor,
            plane,
            tiles: set.iter().copied().collect(),
        });
        floor_sets.push(set);
    }

    for upper_index in 1..floor_sets.len() {
        let floor = BuildingFloorIndex::new(
            u8::try_from(upper_index).unwrap_or_else(|_| unreachable!("request floor count is u8")),
        );
        for &tile in &floor_sets[upper_index] {
            if !floor_sets[upper_index - 1].contains(&tile) {
                return Err(BuildingVerticalError::UnsupportedUpperTile { floor, tile });
            }
        }
    }

    let mut reserved = vec![BTreeSet::<MapTile>::new(); floor_sets.len()];
    let mut stairs = Vec::with_capacity(floor_sets.len().saturating_sub(1));
    for lower_index in 0..floor_sets.len().saturating_sub(1) {
        let lower_floor = BuildingFloorIndex::new(
            u8::try_from(lower_index).unwrap_or_else(|_| unreachable!("request floor count is u8")),
        );
        let upper_floor = BuildingFloorIndex::new(
            lower_floor
                .get()
                .checked_add(1)
                .unwrap_or_else(|| unreachable!("adjacent floor index remains u8")),
        );
        let candidates = stair_candidates(
            &floor_sets[lower_index],
            &floor_sets[lower_index + 1],
            &reserved[lower_index],
            &reserved[lower_index + 1],
        );
        let Some(anchor) = choose_stair_anchor(request.seed, lower_floor, upper_floor, &candidates)
        else {
            return Err(BuildingVerticalError::NoStairAnchor {
                lower_floor,
                upper_floor,
            });
        };
        let approach = anchor
            .approach_tile()
            .unwrap_or_else(|| unreachable!("candidate approach is valid"));
        reserved[lower_index].insert(anchor.tile);
        reserved[lower_index].insert(approach);
        reserved[lower_index + 1].insert(anchor.tile);
        reserved[lower_index + 1].insert(approach);
        stairs.push(anchor);
    }

    Ok(BuildingVerticalPlan {
        request,
        floors,
        stairs,
    })
}

/// Realize every vertical anchor as one up-stair object on the lower floor and one down-stair
/// object on the upper floor. The result uses the same `MapPatch` path as all other generated edits.
pub fn build_stair_patch(
    plan: &BuildingVerticalPlan,
    palette: StairPalette,
) -> Result<MapPatch, StairRealizationError> {
    let mut ops = Vec::with_capacity(plan.stairs.len() * 2);
    let mut placements = HashSet::new();

    for &anchor in &plan.stairs {
        let lower = plan
            .floor(anchor.lower_floor)
            .ok_or(StairRealizationError::MissingFloor {
                floor: anchor.lower_floor,
            })?;
        let upper = plan
            .floor(anchor.upper_floor)
            .ok_or(StairRealizationError::MissingFloor {
                floor: anchor.upper_floor,
            })?;
        for placement in [
            palette
                .up
                .placement(anchor.tile, anchor.facing, lower.plane),
            palette
                .down
                .placement(anchor.tile, anchor.facing, upper.plane),
        ] {
            if !placements.insert(placement) {
                return Err(StairRealizationError::DuplicateObjectPlacement { placement });
            }
            ops.push(MapOp::AddObject { placement });
        }
    }

    Ok(MapPatch::new(ops))
}

/// Convert vertical anchors touching one floor into fixed WFC constraints.
///
/// Lower-floor anchors are pinned to `patterns.up`; upper-floor anchors are pinned to
/// `patterns.down`. Their adjacent approach tiles are restricted to the caller's walkable domain.
pub fn stair_wfc_constraints(
    plan: &BuildingVerticalPlan,
    floor: BuildingFloorIndex,
    patterns: StairPatternSet,
) -> Result<Vec<WfcConstraint>, StairConstraintError> {
    if patterns.approach_allowed.is_empty() {
        return Err(StairConstraintError::EmptyApproachDomain);
    }
    if plan.floor(floor).is_none() {
        return Err(StairConstraintError::MissingFloor { floor });
    }

    let mut constraints = Vec::new();
    for &anchor in &plan.stairs {
        let pattern = if anchor.lower_floor == floor {
            Some(patterns.up)
        } else if anchor.upper_floor == floor {
            Some(patterns.down)
        } else {
            None
        };
        let Some(pattern) = pattern else {
            continue;
        };
        let anchor_coord = local_wfc_coord(plan.request, anchor.tile)
            .ok_or(StairConstraintError::InvalidAnchorTile { tile: anchor.tile })?;
        let approach = anchor
            .approach_tile()
            .ok_or(StairConstraintError::InvalidApproachTile { tile: anchor.tile })?;
        let approach_coord = local_wfc_coord(plan.request, approach)
            .ok_or(StairConstraintError::InvalidApproachTile { tile: approach })?;
        constraints.push(WfcConstraint {
            coord: anchor_coord,
            allowed: PatternDomain::singleton(pattern),
        });
        constraints.push(WfcConstraint {
            coord: approach_coord,
            allowed: patterns.approach_allowed,
        });
    }
    constraints.sort_by_key(|constraint| constraint.coord);
    Ok(constraints)
}

fn source_plane_for_floor(
    request: BuildingRequest,
    floor: BuildingFloorIndex,
) -> Result<SourcePlane, BuildingVerticalError> {
    let Some(value) = request.base_plane.index().get().checked_add(floor.get()) else {
        return Err(BuildingVerticalError::PlaneOverflow { floor });
    };
    SourcePlane::new(value).ok_or(BuildingVerticalError::PlaneOverflow { floor })
}

fn stair_candidates(
    lower: &BTreeSet<MapTile>,
    upper: &BTreeSet<MapTile>,
    reserved_lower: &BTreeSet<MapTile>,
    reserved_upper: &BTreeSet<MapTile>,
) -> Vec<(MapTile, CardinalDirection)> {
    let mut candidates = Vec::new();
    for &tile in lower.intersection(upper) {
        if reserved_lower.contains(&tile) || reserved_upper.contains(&tile) {
            continue;
        }
        for direction in CardinalDirection::ALL {
            let Some(approach) = direction.neighbor(tile) else {
                continue;
            };
            if lower.contains(&approach)
                && upper.contains(&approach)
                && !reserved_lower.contains(&approach)
                && !reserved_upper.contains(&approach)
            {
                candidates.push((tile, direction));
            }
        }
    }
    candidates.sort();
    candidates.dedup();
    candidates
}

fn choose_stair_anchor(
    seed: u64,
    lower_floor: BuildingFloorIndex,
    upper_floor: BuildingFloorIndex,
    candidates: &[(MapTile, CardinalDirection)],
) -> Option<StairAnchor> {
    if candidates.is_empty() {
        return None;
    }
    let mixed = mix64(seed ^ u64::from(lower_floor.get()).rotate_left(19));
    let index = usize::try_from(mixed % u64::try_from(candidates.len()).ok()?).ok()?;
    let (tile, facing) = candidates[index];
    Some(StairAnchor {
        tile,
        facing,
        lower_floor,
        upper_floor,
    })
}

fn local_wfc_coord(request: BuildingRequest, tile: MapTile) -> Option<WfcCoord> {
    if !request.site.contains(tile) {
        return None;
    }
    let x = u16::try_from(tile.x.checked_sub(request.site.origin().x)?).ok()?;
    let y = u16::try_from(tile.y.checked_sub(request.site.origin().y)?).ok()?;
    Some(WfcCoord::new(x, y))
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
    use crate::{BuildingArchetype, BuildingConstraints, BuildingStyle, MapDocument, RectSite};

    fn plane(value: u8) -> SourcePlane {
        SourcePlane::new(value).unwrap_or_else(|| unreachable!("test plane is valid"))
    }

    fn request(base_plane: u8, floors: u8) -> BuildingRequest {
        BuildingRequest {
            seed: 42,
            site: RectSite::new(MapTile::new(3200, 3200), 8, 8)
                .unwrap_or_else(|| unreachable!("test site is valid")),
            base_plane: plane(base_plane),
            floors,
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

    fn orientations() -> [ObjectOrientation; 4] {
        [0, 1, 2, 3].map(|value| {
            ObjectOrientation::new(value)
                .unwrap_or_else(|| unreachable!("test orientation is valid"))
        })
    }

    fn stair_asset(object_id: u32) -> StairAsset {
        StairAsset {
            object_id: ObjectId::new(object_id),
            kind: LocKind::new(10).unwrap_or_else(|| unreachable!("test loc kind is valid")),
            orientations: orientations(),
        }
    }

    fn pattern(value: u8) -> WfcPatternId {
        WfcPatternId::new(u16::from(value)).unwrap_or_else(|| unreachable!("test pattern is valid"))
    }

    #[test]
    fn same_stack_produces_identical_stair_anchors() {
        let request = request(0, 3);
        let floor = rectangle(MapTile::new(3201, 3201), 5, 5);
        let first =
            plan_vertical_connectivity(request, &[floor.clone(), floor.clone(), floor.clone()])
                .unwrap_or_else(|error| unreachable!("valid vertical plan: {error:?}"));
        let second = plan_vertical_connectivity(request, &[floor.clone(), floor.clone(), floor])
            .unwrap_or_else(|error| unreachable!("valid vertical plan: {error:?}"));

        assert_eq!(first, second);
        assert_eq!(first.stairs().len(), 2);
        assert_eq!(first.floors()[0].plane, plane(0));
        assert_eq!(first.floors()[1].plane, plane(1));
        assert_eq!(first.floors()[2].plane, plane(2));
        assert_ne!(first.stairs()[0].tile, first.stairs()[1].tile);
    }

    #[test]
    fn upper_floor_must_be_supported_by_floor_below() {
        let request = request(0, 2);
        let lower = rectangle(MapTile::new(3201, 3201), 4, 4);
        let mut upper = lower.clone();
        let unsupported = MapTile::new(3205, 3205);
        upper.push(unsupported);

        assert_eq!(
            plan_vertical_connectivity(request, &[lower, upper]),
            Err(BuildingVerticalError::UnsupportedUpperTile {
                floor: BuildingFloorIndex::new(1),
                tile: unsupported,
            })
        );
    }

    #[test]
    fn stair_requires_shared_approach_clearance() {
        let request = request(0, 2);
        let only_tile = MapTile::new(3202, 3202);

        assert_eq!(
            plan_vertical_connectivity(request, &[vec![only_tile], vec![only_tile]]),
            Err(BuildingVerticalError::NoStairAnchor {
                lower_floor: BuildingFloorIndex::new(0),
                upper_floor: BuildingFloorIndex::new(1),
            })
        );
    }

    #[test]
    fn plane_overflow_is_rejected_before_stair_planning() {
        let request = request(3, 2);
        let floor = rectangle(MapTile::new(3201, 3201), 3, 3);

        assert_eq!(
            plan_vertical_connectivity(request, &[floor.clone(), floor]),
            Err(BuildingVerticalError::PlaneOverflow {
                floor: BuildingFloorIndex::new(1),
            })
        );
    }

    #[test]
    fn floor_tiles_are_normalized_for_deterministic_queries() {
        let request = request(0, 1);
        let a = MapTile::new(3202, 3202);
        let b = MapTile::new(3201, 3201);
        let plan = plan_vertical_connectivity(request, &[vec![a, b, a]])
            .unwrap_or_else(|error| unreachable!("valid vertical plan: {error:?}"));

        assert_eq!(plan.floors()[0].tiles(), &[b, a]);
        assert!(plan.floors()[0].contains(a));
    }

    #[test]
    fn stair_realization_is_paired_and_exactly_reversible() {
        let request = request(0, 2);
        let floor = rectangle(MapTile::new(3201, 3201), 4, 4);
        let vertical = plan_vertical_connectivity(request, &[floor.clone(), floor])
            .unwrap_or_else(|error| unreachable!("valid vertical plan: {error:?}"));
        let patch = build_stair_patch(
            &vertical,
            StairPalette {
                up: stair_asset(500),
                down: stair_asset(501),
            },
        )
        .unwrap_or_else(|error| unreachable!("valid stair realization: {error:?}"));

        assert_eq!(patch.len(), 2);
        let mut planes = patch
            .ops()
            .iter()
            .filter_map(|op| match op {
                MapOp::AddObject { placement } => Some(placement.position.plane),
                _ => None,
            })
            .collect::<Vec<_>>();
        planes.sort();
        assert_eq!(planes, vec![plane(0), plane(1)]);

        let mut document = MapDocument::new();
        let before = document.clone();
        document
            .apply_patch(&patch)
            .unwrap_or_else(|error| unreachable!("valid stair patch: {error:?}"));
        assert_ne!(document, before);
        document
            .apply_patch(&patch.inverse())
            .unwrap_or_else(|error| unreachable!("valid stair inverse: {error:?}"));
        assert_eq!(document, before);
    }

    #[test]
    fn stair_constraints_pin_vertical_cells_and_preserve_walkable_approaches() {
        let request = request(0, 3);
        let floor = rectangle(MapTile::new(3201, 3201), 5, 5);
        let vertical = plan_vertical_connectivity(request, &[floor.clone(), floor.clone(), floor])
            .unwrap_or_else(|error| unreachable!("valid vertical plan: {error:?}"));
        let approach_allowed = PatternDomain::from_patterns([pattern(2), pattern(3)]);
        let patterns = StairPatternSet {
            up: pattern(5),
            down: pattern(6),
            approach_allowed,
        };

        let middle = stair_wfc_constraints(&vertical, BuildingFloorIndex::new(1), patterns)
            .unwrap_or_else(|error| unreachable!("valid stair constraints: {error:?}"));
        assert_eq!(middle.len(), 4);
        assert_eq!(
            middle
                .iter()
                .filter(|constraint| constraint.allowed == PatternDomain::singleton(patterns.down))
                .count(),
            1
        );
        assert_eq!(
            middle
                .iter()
                .filter(|constraint| constraint.allowed == PatternDomain::singleton(patterns.up))
                .count(),
            1
        );
        assert_eq!(
            middle
                .iter()
                .filter(|constraint| constraint.allowed == approach_allowed)
                .count(),
            2
        );
    }

    #[test]
    fn empty_stair_approach_domain_is_rejected() {
        let request = request(0, 1);
        let floor = rectangle(MapTile::new(3201, 3201), 4, 4);
        let vertical = plan_vertical_connectivity(request, &[floor])
            .unwrap_or_else(|error| unreachable!("valid vertical plan: {error:?}"));
        let result = stair_wfc_constraints(
            &vertical,
            BuildingFloorIndex::new(0),
            StairPatternSet {
                up: pattern(5),
                down: pattern(6),
                approach_allowed: PatternDomain::empty(),
            },
        );

        assert_eq!(result, Err(StairConstraintError::EmptyApproachDomain));
    }
}
