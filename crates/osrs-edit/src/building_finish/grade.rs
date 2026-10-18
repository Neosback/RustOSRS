//! Grading the site: one level under the building and a blend across the margin, so walls stand
//! on flat ground and the ground meets the map without a step.

use crate::{BspBuildingPlan, GenerationSnapshot, HeightVertexCoord, MapOp};
use osrs_core::coords::MapTile;

/// Client heights are stored in steps of 8.
fn step8(value: i32) -> i32 {
    value.div_euclid(8) * 8
}

pub(super) fn grade(building: &BspBuildingPlan, snapshot: &GenerationSnapshot) -> Vec<MapOp> {
    let plan = &building.plan;
    let site = snapshot.site();
    let (mut min_x, mut min_y, mut max_x, mut max_y) = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
    for room in &plan.rooms {
        let r = room.rect;
        min_x = min_x.min(r.origin.x);
        min_y = min_y.min(r.origin.y);
        max_x = max_x.max(r.origin.x + i32::from(r.width));
        max_y = max_y.max(r.origin.y + i32::from(r.height));
    }
    if min_x > max_x {
        return Vec::new();
    }
    let plane = plan.request.base_plane;
    let height_at =
        |x: i32, y: i32| snapshot.height_at(HeightVertexCoord::new(MapTile::new(x, y), plane));
    // The level: the median of the heights under the footprint corners.
    let mut under: Vec<i32> = (min_x..=max_x)
        .flat_map(|x| (min_y..=max_y).map(move |y| (x, y)))
        .filter_map(|(x, y)| height_at(x, y))
        .collect();
    if under.is_empty() {
        return Vec::new();
    }
    under.sort_unstable();
    let level = step8(under[under.len() / 2]);
    let (sx0, sy0) = (site.origin().x, site.origin().y);
    let (sx1, sy1) = (
        sx0 + i32::from(site.width()),
        sy0 + i32::from(site.height()),
    );
    let mut ops = Vec::new();
    for x in sx0..=sx1 {
        for y in sy0..=sy1 {
            let Some(before) = height_at(x, y) else {
                continue;
            };
            // Distance to the footprint (0 under it) and to the edge of the site.
            let dx = (min_x - x).max(x - max_x).max(0);
            let dy = (min_y - y).max(y - max_y).max(0);
            let toward_footprint = dx.max(dy);
            let after = if toward_footprint == 0 {
                level
            } else {
                let to_edge = (x - sx0).min(sx1 - x).min(y - sy0).min(sy1 - y);
                // Full level next to the footprint, the original height at the site's edge.
                let total = toward_footprint + to_edge;
                let keep = i64::from(toward_footprint);
                let blended = i64::from(level) * i64::from(total - toward_footprint)
                    + i64::from(before) * keep;
                step8(i32::try_from(blended / i64::from(total.max(1))).unwrap_or(before))
            };
            if after != before {
                ops.push(MapOp::SetHeight {
                    coord: HeightVertexCoord::new(MapTile::new(x, y), plane),
                    before,
                    after,
                });
            }
        }
    }
    ops
}
