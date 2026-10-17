//! Deterministic rectangular decomposition for complex semantic roof footprints.
//!
//! This layer partitions an already-planned roof footprint into exact, non-overlapping rectangular
//! sections. It does not choose per-section roof shapes, run WFC, resolve assets, or build map edits.

use crate::{BuildingFloorIndex, BuildingRoofPlan, RectSite};
use osrs_core::coords::{MapTile, SourcePlane};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RoofSection {
    bounds: RectSite,
    covered_floor: BuildingFloorIndex,
    plane: SourcePlane,
}

impl RoofSection {
    pub const fn bounds(self) -> RectSite {
        self.bounds
    }

    pub const fn covered_floor(self) -> BuildingFloorIndex {
        self.covered_floor
    }

    pub const fn plane(self) -> SourcePlane {
        self.plane
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoofSectionError {
    EmptyRoof,
    InvalidBounds {
        origin: MapTile,
        width: i64,
        height: i64,
    },
}

/// Partition a roof footprint into exact deterministic rectangular sections.
///
/// Each row is first reduced to contiguous horizontal runs. Runs with the same x-span are then
/// merged only across consecutive rows. This guarantees disjoint rectangular coverage while
/// avoiding any guess about ridges, valleys, pitch, WFC patterns, or concrete roof assets.
pub fn decompose_roof_sections(
    roof: &BuildingRoofPlan,
) -> Result<Vec<RoofSection>, RoofSectionError> {
    if roof.tiles().is_empty() {
        return Err(RoofSectionError::EmptyRoof);
    }

    let mut rows = BTreeMap::<i32, Vec<i32>>::new();
    for tile in roof.tiles() {
        rows.entry(tile.y).or_default().push(tile.x);
    }

    let mut span_rows = BTreeMap::<(i32, i32), Vec<i32>>::new();
    for (y, xs) in &mut rows {
        xs.sort_unstable();
        xs.dedup();

        let mut start = xs[0];
        let mut end = xs[0];
        for &x in &xs[1..] {
            if end.checked_add(1) == Some(x) {
                end = x;
            } else {
                span_rows.entry((start, end)).or_default().push(*y);
                start = x;
                end = x;
            }
        }
        span_rows.entry((start, end)).or_default().push(*y);
    }

    let mut sections = Vec::new();
    for ((min_x, max_x), ys) in span_rows {
        let mut start_y = ys[0];
        let mut end_y = ys[0];
        for y in ys.into_iter().skip(1) {
            if end_y.checked_add(1) == Some(y) {
                end_y = y;
            } else {
                sections.push(make_section(roof, min_x, max_x, start_y, end_y)?);
                start_y = y;
                end_y = y;
            }
        }
        sections.push(make_section(roof, min_x, max_x, start_y, end_y)?);
    }

    sections.sort_by_key(|section| {
        let bounds = section.bounds();
        (
            bounds.origin().y,
            bounds.origin().x,
            bounds.height(),
            bounds.width(),
        )
    });
    Ok(sections)
}

fn make_section(
    roof: &BuildingRoofPlan,
    min_x: i32,
    max_x: i32,
    min_y: i32,
    max_y: i32,
) -> Result<RoofSection, RoofSectionError> {
    let origin = MapTile::new(min_x, min_y);
    let width = i64::from(max_x) - i64::from(min_x) + 1;
    let height = i64::from(max_y) - i64::from(min_y) + 1;
    let Ok(width_u16) = u16::try_from(width) else {
        return Err(RoofSectionError::InvalidBounds {
            origin,
            width,
            height,
        });
    };
    let Ok(height_u16) = u16::try_from(height) else {
        return Err(RoofSectionError::InvalidBounds {
            origin,
            width,
            height,
        });
    };
    let Some(bounds) = RectSite::new(origin, width_u16, height_u16) else {
        return Err(RoofSectionError::InvalidBounds {
            origin,
            width,
            height,
        });
    };

    Ok(RoofSection {
        bounds,
        covered_floor: roof.covered_floor(),
        plane: roof.plane(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        BuildingArchetype, BuildingConstraints, BuildingRequest, BuildingStyle, RoofShape,
        plan_roof_coverage, plan_roof_shape, plan_vertical_connectivity,
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

    fn roof_for(floor: Vec<MapTile>) -> BuildingRoofPlan {
        let vertical = plan_vertical_connectivity(request(), &[floor])
            .unwrap_or_else(|error| unreachable!("valid vertical plan: {error:?}"));
        plan_roof_coverage(&vertical)
            .unwrap_or_else(|error| unreachable!("valid roof plan: {error:?}"))
    }

    fn section_tiles(section: RoofSection) -> Vec<MapTile> {
        let bounds = section.bounds();
        let mut tiles = Vec::new();
        for y in 0..i32::from(bounds.height()) {
            for x in 0..i32::from(bounds.width()) {
                tiles.push(MapTile::new(bounds.origin().x + x, bounds.origin().y + y));
            }
        }
        tiles
    }

    #[test]
    fn rectangular_roof_decomposes_to_one_section() {
        let roof = roof_for(rectangle(MapTile::new(3201, 3202), 4, 3));
        let sections = decompose_roof_sections(&roof)
            .unwrap_or_else(|error| unreachable!("roof decomposes: {error:?}"));

        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].bounds().origin(), MapTile::new(3201, 3202));
        assert_eq!(sections[0].bounds().width(), 4);
        assert_eq!(sections[0].bounds().height(), 3);
        assert_eq!(sections[0].covered_floor(), roof.covered_floor());
        assert_eq!(sections[0].plane(), roof.plane());
    }

    #[test]
    fn l_shaped_complex_roof_decomposes_deterministically() {
        let roof = roof_for(vec![
            MapTile::new(3200, 3200),
            MapTile::new(3201, 3200),
            MapTile::new(3202, 3200),
            MapTile::new(3200, 3201),
            MapTile::new(3200, 3202),
        ]);
        assert_eq!(plan_roof_shape(&roof), RoofShape::Complex);

        let sections = decompose_roof_sections(&roof)
            .unwrap_or_else(|error| unreachable!("complex roof decomposes: {error:?}"));

        assert_eq!(sections.len(), 2);
        assert_eq!(sections[0].bounds().origin(), MapTile::new(3200, 3200));
        assert_eq!(sections[0].bounds().width(), 3);
        assert_eq!(sections[0].bounds().height(), 1);
        assert_eq!(sections[1].bounds().origin(), MapTile::new(3200, 3201));
        assert_eq!(sections[1].bounds().width(), 1);
        assert_eq!(sections[1].bounds().height(), 2);
    }

    #[test]
    fn decomposed_sections_cover_exact_roof_once() {
        let roof = roof_for(vec![
            MapTile::new(3200, 3200),
            MapTile::new(3201, 3200),
            MapTile::new(3202, 3200),
            MapTile::new(3203, 3200),
            MapTile::new(3204, 3200),
            MapTile::new(3202, 3201),
            MapTile::new(3202, 3202),
        ]);
        let sections = decompose_roof_sections(&roof)
            .unwrap_or_else(|error| unreachable!("complex roof decomposes: {error:?}"));

        let mut covered = sections
            .iter()
            .copied()
            .flat_map(section_tiles)
            .collect::<Vec<_>>();
        let covered_count = covered.len();
        covered.sort();
        covered.dedup();

        assert_eq!(covered.len(), covered_count);
        assert_eq!(covered, roof.tiles());
    }

    #[test]
    fn decomposition_is_independent_of_input_tile_order() {
        let mut floor = vec![
            MapTile::new(3200, 3200),
            MapTile::new(3201, 3200),
            MapTile::new(3202, 3200),
            MapTile::new(3200, 3201),
            MapTile::new(3200, 3202),
        ];
        let roof_a = roof_for(floor.clone());
        floor.reverse();
        let roof_b = roof_for(floor);

        assert_eq!(
            decompose_roof_sections(&roof_a),
            decompose_roof_sections(&roof_b)
        );
    }
}
