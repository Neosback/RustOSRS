//! Semantic roof coverage planning for generated buildings.
//!
//! Roof planning starts from the already-validated vertical building stack. This module deliberately
//! stops at semantic coverage and exposed boundary edges so later roof-shape, WFC, and asset
//! realization stages can evolve without coupling the headless edit domain to concrete object IDs.

use crate::{BuildingFloorIndex, BuildingVerticalPlan, CardinalDirection};
use osrs_core::coords::{MapTile, SourcePlane};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RoofEdge {
    pub tile: MapTile,
    pub direction: CardinalDirection,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildingRoofPlan {
    covered_floor: BuildingFloorIndex,
    plane: SourcePlane,
    tiles: Vec<MapTile>,
    exposed_edges: Vec<RoofEdge>,
}

impl BuildingRoofPlan {
    pub const fn covered_floor(&self) -> BuildingFloorIndex {
        self.covered_floor
    }

    pub const fn plane(&self) -> SourcePlane {
        self.plane
    }

    pub fn tiles(&self) -> &[MapTile] {
        &self.tiles
    }

    pub fn exposed_edges(&self) -> &[RoofEdge] {
        &self.exposed_edges
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildingRoofError {
    MissingTopFloor,
    CoordinateOverflow {
        tile: MapTile,
        direction: CardinalDirection,
    },
}

/// Derive exact roof coverage from the highest solved building floor.
///
/// Lower floors never expand the roof footprint. Every top-floor tile is covered once, while each
/// cardinal edge whose neighboring tile is outside the top-floor footprint is recorded as exposed.
pub fn plan_roof_coverage(
    vertical: &BuildingVerticalPlan,
) -> Result<BuildingRoofPlan, BuildingRoofError> {
    let Some(top_floor) = vertical.floors().last() else {
        return Err(BuildingRoofError::MissingTopFloor);
    };

    let tiles = top_floor.tiles().to_vec();
    let tile_set = tiles.iter().copied().collect::<BTreeSet<_>>();
    let mut exposed_edges = Vec::new();

    for &tile in &tiles {
        for direction in CardinalDirection::ALL {
            let Some(neighbor) = direction.neighbor(tile) else {
                return Err(BuildingRoofError::CoordinateOverflow { tile, direction });
            };
            if !tile_set.contains(&neighbor) {
                exposed_edges.push(RoofEdge { tile, direction });
            }
        }
    }

    Ok(BuildingRoofPlan {
        covered_floor: top_floor.floor,
        plane: top_floor.plane,
        tiles,
        exposed_edges,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        BuildingArchetype, BuildingConstraints, BuildingRequest, BuildingStyle, RectSite,
        plan_vertical_connectivity,
    };

    fn plane(value: u8) -> SourcePlane {
        SourcePlane::new(value).unwrap_or_else(|| unreachable!("test plane is valid"))
    }

    fn request(floors: u8) -> BuildingRequest {
        BuildingRequest {
            seed: 77,
            site: RectSite::new(MapTile::new(3200, 3200), 8, 8)
                .unwrap_or_else(|| unreachable!("test site is valid")),
            base_plane: plane(0),
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

    #[test]
    fn roof_covers_only_the_highest_floor() {
        let lower = rectangle(MapTile::new(3201, 3201), 5, 5);
        let upper = rectangle(MapTile::new(3202, 3202), 3, 2);
        let vertical = plan_vertical_connectivity(request(2), &[lower, upper.clone()])
            .unwrap_or_else(|error| unreachable!("valid vertical plan: {error:?}"));
        let roof = plan_roof_coverage(&vertical)
            .unwrap_or_else(|error| unreachable!("valid roof plan: {error:?}"));
        let mut expected = upper;
        expected.sort();
        expected.dedup();

        assert_eq!(roof.covered_floor(), BuildingFloorIndex::new(1));
        assert_eq!(roof.plane(), plane(1));
        assert_eq!(roof.tiles(), expected.as_slice());
    }

    #[test]
    fn rectangular_roof_records_only_exterior_edges() {
        let origin = MapTile::new(3202, 3202);
        let floor = rectangle(origin, 2, 2);
        let vertical = plan_vertical_connectivity(request(1), &[floor])
            .unwrap_or_else(|error| unreachable!("valid vertical plan: {error:?}"));
        let roof = plan_roof_coverage(&vertical)
            .unwrap_or_else(|error| unreachable!("valid roof plan: {error:?}"));

        assert_eq!(roof.exposed_edges().len(), 8);
        assert!(roof.exposed_edges().contains(&RoofEdge {
            tile: origin,
            direction: CardinalDirection::South,
        }));
        assert!(roof.exposed_edges().contains(&RoofEdge {
            tile: origin,
            direction: CardinalDirection::West,
        }));
        assert!(!roof.exposed_edges().contains(&RoofEdge {
            tile: origin,
            direction: CardinalDirection::North,
        }));
        assert!(!roof.exposed_edges().contains(&RoofEdge {
            tile: origin,
            direction: CardinalDirection::East,
        }));
    }

    #[test]
    fn missing_neighbor_creates_an_exposed_roof_edge() {
        let south = MapTile::new(3202, 3202);
        let north = MapTile::new(3202, 3203);
        let east = MapTile::new(3203, 3202);
        let floor = vec![south, north, east];
        let vertical = plan_vertical_connectivity(request(1), &[floor])
            .unwrap_or_else(|error| unreachable!("valid vertical plan: {error:?}"));
        let roof = plan_roof_coverage(&vertical)
            .unwrap_or_else(|error| unreachable!("valid roof plan: {error:?}"));

        assert!(roof.exposed_edges().contains(&RoofEdge {
            tile: north,
            direction: CardinalDirection::East,
        }));
        assert!(roof.exposed_edges().contains(&RoofEdge {
            tile: east,
            direction: CardinalDirection::North,
        }));
    }

    #[test]
    fn identical_vertical_plan_produces_identical_roof_semantics() {
        let floor = rectangle(MapTile::new(3201, 3201), 4, 3);
        let vertical = plan_vertical_connectivity(request(1), &[floor])
            .unwrap_or_else(|error| unreachable!("valid vertical plan: {error:?}"));

        assert_eq!(
            plan_roof_coverage(&vertical),
            plan_roof_coverage(&vertical)
        );
    }
}
