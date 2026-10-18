//! Symbolic structural and placement metrics, independent of renderer appearance.
use osrs_core::coords::{MapTile, SourcePlane};
use osrs_edit::{MapDocument, MapPosition};
use osrs_scene::building_census::{Dir, description::canonical_edge};
use serde::Serialize;
use std::collections::BTreeSet;
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Score {
    pub terrain: f64,
    pub height: f64,
    pub objects: f64,
    pub edges: f64,
    pub placement: f64,
    pub total: f64,
    pub exact: bool,
}
fn f1(matches: usize, a: usize, b: usize) -> f64 {
    if a + b == 0 {
        1.0
    } else {
        2.0 * matches as f64 / (a + b) as f64
    }
}
fn edges(document: &MapDocument) -> BTreeSet<(u8, MapTile, u8)> {
    let mut result = BTreeSet::new();
    for p in document.object_entries() {
        let o = p.orientation.get();
        let sides = match p.kind.get() {
            0 => vec![o],
            2 => vec![o, (o + 1) & 3],
            _ => vec![],
        };
        for side in sides {
            let (tile, dir) = canonical_edge(p.position.tile, Dir::from_orientation(side));
            result.insert((
                p.position.plane.index().get(),
                tile,
                match dir {
                    Dir::West => 0,
                    Dir::North => 1,
                    Dir::East => 2,
                    Dir::South => 3,
                },
            ));
        }
    }
    result
}
pub(crate) fn measure(current: &MapDocument, target: &MapDocument, task: &crate::LabTask) -> Score {
    let (width, height, planes) = (task.width, task.height, task.planes);
    let dont_care_tiles: std::collections::HashSet<(u16, u16, u8)> = task
        .dont_care_tiles
        .iter()
        .map(|c| (c.x, c.y, c.plane))
        .collect();
    let dont_care_heights: std::collections::HashSet<(u16, u16, u8)> = task
        .dont_care_heights
        .iter()
        .map(|c| (c.x, c.y, c.plane))
        .collect();

    let mut terrain_sum = 0.0;
    let mut terrain_count = 0usize;
    let mut terrain_exact = true;
    for p in 0..planes {
        let Some(plane) = SourcePlane::new(p) else {
            continue;
        };
        for x in 0..width {
            for y in 0..height {
                if dont_care_tiles.contains(&(x, y, p)) {
                    continue;
                }
                let c = MapPosition::new(MapTile::new(i32::from(x), i32::from(y)), plane);
                let cur = current.terrain_at(c);
                let tgt = target.terrain_at(c);
                terrain_count += 1;
                if cur == tgt {
                    terrain_sum += 1.0;
                } else {
                    terrain_exact = false;
                    let mut credit = 0.0;
                    if cur.underlay == tgt.underlay {
                        credit += 0.4;
                    }
                    if cur.overlay == tgt.overlay {
                        credit += 0.5;
                    } else if cur.overlay.map(|o| o.id) == tgt.overlay.map(|o| o.id) {
                        credit += 0.35;
                    }
                    if cur.flags == tgt.flags {
                        credit += 0.1;
                    }
                    terrain_sum += credit;
                }
            }
        }
    }
    let mut l1 = 0u64;
    let mut heights = 0;
    let mut height_exact = true;
    for (coord, value) in target.height_entries() {
        let cx = u16::try_from(coord.corner.x).unwrap_or(u16::MAX);
        let cy = u16::try_from(coord.corner.y).unwrap_or(u16::MAX);
        let cp = coord.plane.index().get();
        if dont_care_heights.contains(&(cx, cy, cp)) {
            continue;
        }
        heights += 1;
        let diff = current
            .height_at(coord)
            .map_or(32768, |h| i64::from(h).abs_diff(i64::from(value)));
        if diff != 0 {
            height_exact = false;
        }
        l1 += diff;
    }
    let height_score = 1.0 / (1.0 + l1 as f64 / (8.0 * (heights.max(1) as f64)));
    let a = current.object_entries();
    let b = target.object_entries();
    let matches = a.iter().filter(|&&p| target.contains_object(p)).count();
    let ae = edges(current);
    let be = edges(target);
    let edge_score = f1(ae.intersection(&be).count(), ae.len(), be.len());
    let objects = f1(matches, a.len(), b.len());
    // Greedy deterministic occurrence matching gives useful distance/facing credit without
    // collapsing repeated IDs. Exact object F1 remains a separate metric.
    let mut candidates = Vec::new();
    for (i, p) in a.iter().enumerate() {
        for (j, q) in b.iter().enumerate() {
            if p.object_id == q.object_id && p.kind == q.kind {
                let turn = p.orientation.get().abs_diff(q.orientation.get());
                let cost = p.position.tile.x.abs_diff(q.position.tile.x)
                    + p.position.tile.y.abs_diff(q.position.tile.y)
                    + 4 * u32::from(
                        p.position
                            .plane
                            .index()
                            .get()
                            .abs_diff(q.position.plane.index().get()),
                    )
                    + u32::from(turn.min(4 - turn));
                candidates.push((cost, i, j));
            }
        }
    }
    candidates.sort_unstable();
    let (mut used_a, mut used_b) = (BTreeSet::new(), BTreeSet::new());
    let mut credit = 0.0;
    for (cost, i, j) in candidates {
        if !used_a.contains(&i) && !used_b.contains(&j) {
            used_a.insert(i);
            used_b.insert(j);
            credit += 1.0 / (1.0 + f64::from(cost));
        }
    }
    let placement = if a.is_empty() && b.is_empty() {
        1.0
    } else {
        2.0 * credit / (a.len() + b.len()) as f64
    };
    let terrain = if terrain_count == 0 {
        1.0
    } else {
        terrain_sum / terrain_count as f64
    };
    let objects_exact = matches == a.len() && matches == b.len();
    let exact = terrain_exact && height_exact && objects_exact;
    let total = if exact {
        1.0
    } else if a.is_empty() && b.is_empty() {
        (0.65 * terrain + 0.35 * height_score).min(0.999999)
    } else {
        ((terrain + height_score + objects + placement + 2.0 * edge_score) / 6.0).min(0.999999)
    };
    Score {
        terrain,
        height: height_score,
        objects,
        edges: edge_score,
        placement,
        total,
        exact,
    }
}
