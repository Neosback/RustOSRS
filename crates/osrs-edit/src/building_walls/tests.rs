use super::*;
use crate::LocKind;
use osrs_core::{coords::SourcePlane, ids::ObjectId};
use std::collections::HashSet;

fn wall(x: i32, y: i32, orientation: u8) -> MapOp {
    MapOp::AddObject {
        placement: ObjectPlacement {
            object_id: ObjectId::new(1902),
            position: MapPosition::new(
                MapTile::new(x, y),
                SourcePlane::new(0).unwrap_or_else(|| unreachable!()),
            ),
            kind: LocKind::new(0).unwrap_or_else(|| unreachable!()),
            orientation: ObjectOrientation::new(orientation).unwrap_or_else(|| unreachable!()),
        },
    }
}

/// The set of edges a patch's walls make, as `(tile, orientation)` seen from both tiles.
fn edges(patch: &MapPatch) -> HashSet<(MapTile, u8)> {
    let mut set = HashSet::new();
    for op in patch.ops() {
        if let MapOp::AddObject { placement } = op {
            let o = placement.orientation.get();
            set.insert((placement.position.tile, o));
            let (tile, back) = across(placement.position.tile, o);
            set.insert((tile, back));
        }
    }
    set
}

fn per_tile(patch: &MapPatch) -> HashMap<MapTile, usize> {
    let mut counts = HashMap::new();
    for op in patch.ops() {
        if let MapOp::AddObject { placement } = op {
            *counts.entry(placement.position.tile).or_default() += 1;
        }
    }
    counts
}

#[test]
fn a_corner_tile_with_two_walls_keeps_both_walls_on_separate_tiles() {
    // West and north edges of one tile: only one piece fits on it.
    let patch = MapPatch::new([wall(10, 10, 0), wall(10, 10, 1)]);
    let (fixed, unplaced) = resolve_wall_slots(&patch);
    assert_eq!(unplaced, 0);
    assert_eq!(
        edges(&fixed),
        edges(&patch),
        "the same two edges are still walled"
    );
    assert!(per_tile(&fixed).values().all(|n| *n == 1));
}

#[test]
fn a_one_wide_corridor_with_walls_on_both_sides_is_split_across_tiles() {
    // A corridor tile walled on its west and east sides.
    let patch = MapPatch::new([wall(5, 5, 0), wall(5, 5, 2)]);
    let (fixed, unplaced) = resolve_wall_slots(&patch);
    assert_eq!(unplaced, 0);
    assert_eq!(edges(&fixed), edges(&patch));
    assert!(per_tile(&fixed).values().all(|n| *n == 1));
}

#[test]
fn a_whole_room_ring_resolves_with_one_piece_per_tile_and_the_same_edges() {
    // Walls on every outside edge of a 5x4 room, owned by the inside tile, so the four corner
    // tiles carry two pieces each.
    let mut ops = Vec::new();
    for x in 0..5 {
        ops.push(wall(x, 0, 3));
        ops.push(wall(x, 3, 1));
    }
    for y in 0..4 {
        ops.push(wall(0, y, 0));
        ops.push(wall(4, y, 2));
    }
    let patch = MapPatch::new(ops);
    assert!(per_tile(&patch).values().any(|n| *n > 1));
    let (fixed, unplaced) = resolve_wall_slots(&patch);
    assert_eq!(unplaced, 0);
    assert_eq!(edges(&fixed), edges(&patch));
    assert!(per_tile(&fixed).values().all(|n| *n == 1));
    assert_eq!(
        fixed.ops().len(),
        patch.ops().len(),
        "no wall is lost or added"
    );
}

#[test]
fn pieces_stay_on_their_own_tile_when_nothing_needs_moving() {
    let patch = MapPatch::new([wall(1, 1, 3), wall(2, 1, 3), wall(3, 1, 3)]);
    let (fixed, unplaced) = resolve_wall_slots(&patch);
    assert_eq!(unplaced, 0);
    assert_eq!(fixed, patch);
}

#[test]
fn resolution_is_deterministic_and_leaves_other_objects_alone() {
    let mut ops = vec![wall(10, 10, 0), wall(10, 10, 1)];
    ops.push(MapOp::AddObject {
        placement: ObjectPlacement {
            object_id: ObjectId::new(604),
            position: MapPosition::new(
                MapTile::new(11, 11),
                SourcePlane::new(0).unwrap_or_else(|| unreachable!()),
            ),
            kind: LocKind::new(10).unwrap_or_else(|| unreachable!()),
            orientation: ObjectOrientation::new(0).unwrap_or_else(|| unreachable!()),
        },
    });
    let patch = MapPatch::new(ops);
    assert_eq!(resolve_wall_slots(&patch), resolve_wall_slots(&patch));
    let (fixed, _) = resolve_wall_slots(&patch);
    assert!(fixed.ops().iter().any(
        |op| matches!(op, MapOp::AddObject { placement } if placement.object_id.get() == 604)
    ));
}

#[test]
fn identical_wall_tiles_on_different_planes_do_not_displace_each_other() {
    let lower = wall(10, 10, 0);
    let MapOp::AddObject { mut placement } = lower else {
        panic!("wall")
    };
    placement.position.plane = SourcePlane::new(1).unwrap_or_else(|| panic!("plane"));
    let upper = MapOp::AddObject { placement };
    let patch = MapPatch::new([lower, upper]);
    assert_eq!(resolve_wall_slots(&patch), (patch, 0));
}

#[test]
fn straight_walls_respect_fixed_corner_slots_and_unresolved_conflicts_are_reported() {
    let MapOp::AddObject { mut placement } = wall(10, 10, 0) else {
        panic!("wall")
    };
    placement.kind = LocKind::new(2).unwrap_or_else(|| panic!("corner"));
    let corner = MapOp::AddObject { placement };
    let patch = MapPatch::new([corner, wall(10, 10, 0)]);
    let (resolved, count) = resolve_wall_slots(&patch);
    assert_eq!(count, 0);
    assert!(per_tile(&resolved).values().all(|n| *n == 1));
    assert!(resolved.ops().contains(&corner));
    // A fixed corner cannot move out of another fixed boundary slot.
    placement.object_id = ObjectId::new(200);
    let duplicate = MapOp::AddObject { placement };
    let patch = MapPatch::new([corner, duplicate]);
    assert_eq!(resolve_wall_slots(&patch), (patch, 1));
}
