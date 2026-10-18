use super::*;
use osrs_core::ids::ObjectId;
fn object(id: u32, x: i32, y: i32, kind: u8, orientation: u8, plane: u8) -> Placed {
    Placed {
        id: ObjectId::new(id),
        tile: (x, y),
        kind,
        orientation,
        plane,
    }
}
fn house() -> (CensusArea, Vec<Placed>, ObjectTable) {
    let mut placements = Vec::new();
    for n in 3..=5 {
        // Each wall is stored on the outside tile, facing inward.
        placements.extend([
            object(10, 2, n, 0, 2, 0),
            object(10, 6, n, 0, 0, 0),
            object(10, n, 2, 0, 1, 0),
            object(10, n, 6, 0, 3, 0),
        ]);
    }
    placements.push(object(11, 2, 4, 5, 2, 0)); // Same-anchor exterior decoration.
    placements.push(object(12, 4, 4, 10, 1, 0));
    placements.push(object(13, 4, 4, 12, 2, 1));
    placements.push(object(99, 1, 4, 10, 0, 0)); // Neighbor scenery must stay out.
    let mut table = ObjectTable::new();
    for (id, name) in [
        (10, "wall"),
        (11, "torch"),
        (12, "table"),
        (13, "roof"),
        (99, "tree"),
    ] {
        table.insert(
            ObjectId::new(id),
            crate::building_census::ObjectInfo {
                name: name.into(),
                size: (1, 1),
                ..Default::default()
            },
        );
    }
    (CensusArea::new((0, 0), 10, 10), placements, table)
}
#[test]
fn outside_wall_anchors_keep_exact_occurrences_and_do_not_own_ground() {
    let (area, p, table) = house();
    let b = describe(&area, &p, &table, MapTile::new(4, 4), 0).unwrap_or_else(|| unreachable!());
    assert_eq!(b.floors[&0].len(), 9);
    assert!(!b.floors[&0].contains(&MapTile::new(2, 4)));
    assert_eq!(b.members.len(), p.len() - 1);
    assert!(
        b.members
            .iter()
            .any(|m| m.placement.id.get() == 11 && m.role == MemberRole::Decoration)
    );
    assert!(
        b.members
            .iter()
            .any(|m| m.placement.id.get() == 13 && m.role == MemberRole::Roof)
    );
    assert!(
        b.connections
            .iter()
            .any(|c| c.kind == ConnectionKind::Decorates)
    );
}
#[test]
fn canonical_edges_match_both_anchor_representations_and_all_rotations() {
    for d in Dir::ALL {
        let a = MapTile::new(4, 4);
        let (x, y) = d.from((a.x, a.y));
        assert_eq!(
            canonical_edge(a, d),
            canonical_edge(MapTile::new(x, y), d.opposite())
        );
    }
}
#[test]
fn repeated_ids_are_distinct_placements_and_input_order_does_not_change_links() {
    let (area, mut p, table) = house();
    let a = describe(&area, &p, &table, MapTile::new(4, 4), 0);
    p.reverse();
    assert_eq!(a, describe(&area, &p, &table, MapTile::new(4, 4), 0));
    let a = a.unwrap_or_else(|| unreachable!());
    assert_eq!(
        a.members
            .iter()
            .filter(|m| m.placement.id.get() == 10)
            .count(),
        12
    );
}
