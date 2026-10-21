use super::*;
use std::collections::BTreeMap;

fn plane() -> SourcePlane {
    SourcePlane::new(1).unwrap_or_else(|| unreachable!("plane one is valid"))
}

const ASSETS: HipRoofAssets = HipRoofAssets::new(ObjectId::new(1924), ObjectId::new(1790));

/// `(x, y) -> (object, kind, orientation)` for a footprint.
fn roof_of(tiles: &[(i32, i32)]) -> BTreeMap<(i32, i32), (u32, u8, u8)> {
    let footprint: Vec<MapTile> = tiles.iter().map(|&(x, y)| MapTile::new(x, y)).collect();
    hip_roof_placements(&footprint, ASSETS, plane())
        .into_iter()
        .map(|p| {
            (
                (p.position.tile.x, p.position.tile.y),
                (p.object_id.get(), p.kind.get(), p.orientation.get()),
            )
        })
        .collect()
}

fn rect(w: i32, h: i32) -> Vec<(i32, i32)> {
    (0..h).flat_map(|y| (0..w).map(move |x| (x, y))).collect()
}

#[test]
fn a_rectangle_gets_edge_ring_slope_ring_and_flat_top_oriented_like_the_real_client() {
    let roof = roof_of(&rect(8, 6));
    assert_eq!(roof.len(), 48, "one piece per tile");
    let kinds: BTreeMap<u8, usize> = roof.values().fold(BTreeMap::new(), |mut m, (_, k, _)| {
        *m.entry(*k).or_default() += 1;
        m
    });
    // Edge ring 24 (4 corners), slope ring 16 (4 corners), flat top 8.
    assert_eq!(
        kinds,
        BTreeMap::from([(12, 12), (16, 4), (17, 8), (18, 20), (19, 4)])
    );
    // Straight pieces: the orientation names the outside (south 1, north 3, west 2, east 0).
    assert_eq!(roof[&(3, 0)], (1790, 18, 1));
    assert_eq!(roof[&(3, 5)], (1790, 18, 3));
    assert_eq!(roof[&(0, 2)], (1790, 18, 2));
    assert_eq!(roof[&(7, 2)], (1790, 18, 0));
    // Corners: the outside diagonal (south-east 0, south-west 1, north-west 2, north-east 3).
    assert_eq!(roof[&(7, 0)], (1790, 19, 0));
    assert_eq!(roof[&(0, 0)], (1790, 19, 1));
    assert_eq!(roof[&(0, 5)], (1790, 19, 2));
    assert_eq!(roof[&(7, 5)], (1790, 19, 3));
    // The slope ring sits just inside, in the other object.
    assert_eq!(roof[&(3, 1)], (1924, 12, 1));
    assert_eq!(roof[&(1, 1)], (1924, 16, 1));
    assert_eq!(roof[&(6, 4)], (1924, 16, 3));
    assert_eq!(roof[&(3, 3)], (1924, 17, 1));
}

/// A real Lumbridge roof, read off the cache: rows from the north (y 3243) down to y 3235, columns
/// from x 25 to x 35. Each entry is `kind/orientation`; `.` is outside the roof.
const LUMBRIDGE: [&str; 9] = [
    "19/2 18/3 18/3 18/3 18/3 18/3 18/3 18/3 18/3 19/3",
    "18/2 16/2 12/3 12/3 12/3 12/3 12/3 12/3 16/3 18/0",
    "18/2 12/2 17/1 17/1 17/1 17/1 17/1 17/1 12/0 18/0",
    "18/2 12/2 17/1 17/1 17/1 17/1 17/1 17/1 12/0 18/0",
    "18/2 16/1 12/1 15/1 17/1 17/1 17/1 17/1 12/0 18/0",
    "19/1 18/1 20/1 12/2 17/1 17/1 17/1 17/1 12/0 18/0",
    "  .   .  18/2 12/2 17/1 17/1 17/1 17/1 12/0 18/0",
    "  .   .  18/2 16/1 12/1 12/1 12/1 12/1 16/0 18/0",
    "  .   .  19/1 18/1 18/1 18/1 18/1 18/1 18/1 19/0",
];

#[test]
fn the_roof_of_a_real_building_is_reproduced_piece_for_piece() {
    let mut footprint = Vec::new();
    let mut expected = BTreeMap::new();
    for (row, line) in LUMBRIDGE.iter().enumerate() {
        let y = 8 - row as i32;
        for (column, cell) in line.split_whitespace().enumerate() {
            if cell == "." {
                continue;
            }
            let (kind, orientation) = cell.split_once('/').unwrap_or_else(|| unreachable!());
            let parse = |text: &str| text.parse::<u8>().unwrap_or_else(|_| unreachable!());
            footprint.push((column as i32, y));
            expected.insert((column as i32, y), (parse(kind), parse(orientation)));
        }
    }
    let roof = roof_of(&footprint);
    assert_eq!(roof.len(), footprint.len());
    for (tile, (kind, orientation)) in expected {
        let (_, got_kind, got_orientation) = roof[&tile];
        assert_eq!(
            (got_kind, got_orientation),
            (kind, orientation),
            "tile {tile:?}"
        );
    }
}

#[test]
fn every_shape_gets_exactly_one_piece_per_tile_even_when_thin() {
    for (w, h) in [(1, 1), (1, 5), (2, 2), (2, 7), (3, 3), (3, 9), (9, 3)] {
        let tiles = rect(w, h);
        assert_eq!(roof_of(&tiles).len(), tiles.len(), "{w}x{h}");
    }
    // An L and a T: inner corners appear and nothing is skipped.
    let l: Vec<_> = rect(9, 9)
        .into_iter()
        .filter(|&(x, y)| !(x >= 5 && y >= 5))
        .collect();
    let roof = roof_of(&l);
    assert_eq!(roof.len(), l.len());
    assert!(
        roof.values().any(|(_, kind, _)| *kind == 20),
        "an edge inner corner"
    );
    assert!(
        roof.values().any(|(_, kind, _)| *kind == 15),
        "a slope inner corner"
    );
    assert!(hip_roof_placements(&[], ASSETS, plane()).is_empty());
}
