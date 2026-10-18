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
pub(crate) fn measure(
    current: &MapDocument,
    target: &MapDocument,
    width: u16,
    height: u16,
    planes: u8,
) -> Score {
    let mut equal = 0usize;
    let count = usize::from(width) * usize::from(height) * usize::from(planes);
    for p in 0..planes {
        let Some(plane) = SourcePlane::new(p) else {
            continue;
        };
        for x in 0..width {
            for y in 0..height {
                let c = MapPosition::new(MapTile::new(i32::from(x), i32::from(y)), plane);
                equal += usize::from(current.terrain_at(c) == target.terrain_at(c));
            }
        }
    }
    let mut l1 = 0u64;
    let mut heights = 0;
    for (coord, value) in target.height_entries() {
        heights += 1;
        l1 += current
            .height_at(coord)
            .map_or(32768, |h| i64::from(h).abs_diff(i64::from(value)));
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
    let terrain = equal as f64 / count.max(1) as f64;
    let exact = current == target;
    Score {
        terrain,
        height: height_score,
        objects,
        edges: edge_score,
        placement,
        total: if exact {
            1.0
        } else {
            ((terrain + height_score + objects + placement + 2.0 * edge_score) / 6.0).min(0.999999)
        },
        exact,
    }
}
