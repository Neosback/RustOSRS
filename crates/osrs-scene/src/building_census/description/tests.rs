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

#[test]
fn open_shell_candidate_is_bounded_and_explicitly_incomplete() {
    let (mut area, mut placements, table) = house();
    for x in 0..10 {
        for y in 0..10 {
            area.set(
                0,
                (x, y),
                crate::building_census::Ground {
                    height_known: true,
                    overlay: 5,
                    ..Default::default()
                },
            );
        }
    }
    placements.retain(|p| p.tile.0 != 2 || p.kind != 0);
    assert!(pick_candidates(&area, &placements, &table, MapTile::new(4, 4), 0, None).is_empty());
    let candidate = incomplete_candidate(&area, &placements, &table, MapTile::new(4, 4), 0, None)
        .unwrap_or_else(|| panic!("expected bounded incomplete candidate"));
    assert!(
        candidate
            .description
            .review
            .iter()
            .any(|r| r.starts_with("INCOMPLETE"))
    );
    assert!(candidate.description.floors[&0].len() <= 17 * 17);
    assert!(candidate.description.floors[&0].contains(&MapTile::new(4, 4)));
    let empty = CensusArea::new((0, 0), 10, 10);
    assert!(
        incomplete_candidate(&empty, &placements, &table, MapTile::new(4, 4), 0, None).is_none()
    );
}

#[test]
fn wall_pick_and_capture_share_exact_document_membership() {
    let (area, mut placements, table) = house();
    let wall = object(10, 2, 4, 0, 2, 0);
    let candidates = pick_candidates(
        &area,
        &placements,
        &table,
        MapTile::new(2, 4),
        0,
        Some(wall),
    );
    assert_eq!(candidates.len(), 1);
    let picked = &candidates[0];
    assert_eq!(
        Some(picked.description.clone()),
        describe(&area, &placements, &table, picked.seed, picked.plane)
    );
    // An edit removes an enclosure: the query must not reuse a renderer's old building.
    placements.retain(|p| !(p.tile.0 == 2 && p.kind == 0));
    assert!(
        pick_candidates(
            &area,
            &placements,
            &table,
            MapTile::new(2, 4),
            0,
            Some(wall)
        )
        .is_empty()
    );
}

#[test]
fn shared_wall_exposes_both_disconnected_interiors_and_roof_click_uses_lower_plane() {
    let (_, left, table) = house();
    let mut placements = left.clone();
    placements.extend(left.into_iter().map(|mut p| {
        p.tile.0 += 3;
        p
    }));
    let area = CensusArea::new((0, 0), 13, 10);
    let wall = object(10, 6, 4, 0, 0, 0);
    let sides = pick_candidates(
        &area,
        &placements,
        &table,
        MapTile::new(6, 4),
        0,
        Some(wall),
    );
    assert_eq!(sides.len(), 2, "a party wall needs a side choice");
    assert_ne!(sides[0].description.floors, sides[1].description.floors);
    let roof = pick_candidates(&area, &placements, &table, MapTile::new(4, 4), 1, None);
    assert!(!roof.is_empty());
    assert_eq!(roof[0].plane, 0);
}
