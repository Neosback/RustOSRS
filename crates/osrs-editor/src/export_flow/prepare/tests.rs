use super::*;
use crate::export_flow::test_support::*;
use osrs_core::coords::MapTile;
use osrs_core::ids::FloorUnderlayId;
use osrs_edit::{HeightVertexCoord, MapOp, MapPatch, TerrainTile};

#[test]
fn unloaded_pending_regions_and_live_cross_border_edits_are_included_without_mutation() {
    let f = fixture();
    let mut snapshot = snapshot(&f);
    snapshot.authored = [f.regions[0]].into_iter().collect();
    snapshot.document = baseline(&[f.regions[0]]);
    paint(&mut snapshot.document, position(3263, 3201, 0), 5);
    snapshot.pending.push(RegionDiff {
        region: f.regions[1],
        tiles: vec![(
            position(3264, 3201, 1),
            TerrainTile {
                underlay: Some(FloorUnderlayId::new(7)),
                ..TerrainTile::default()
            },
        )],
        heights: vec![(
            HeightVertexCoord::new(MapTile::new(3264, 3201), plane(1)),
            -176,
        )],
        objects_added: Vec::new(),
        objects_removed: Vec::new(),
    });
    let before = snapshot.document.clone();
    let prepared = prepare_with(snapshot.clone(), "fixture".into(), |r| Ok(Some(square(r))));
    assert!(prepared.can_publish(), "{:?}", prepared.issues);
    assert_eq!(prepared.regions.len(), 2);
    // Changing plane 1 requires a plane-2 encoding compensation to retain its final height.
    assert_eq!(prepared.regions[1].changes.heights, 2);
    assert_eq!(
        prepared.regions[1]
            .expected
            .height_at(HeightVertexCoord::new(MapTile::new(3264, 3201), plane(2))),
        Some(-240)
    );
    assert_eq!(snapshot.document, before);
    assert_eq!(prepared.stamp, snapshot.stamp);
}

#[test]
fn missing_authored_region_blocks_the_entire_export() {
    let f = fixture();
    let prepared = prepare_with(snapshot(&f), "fixture".into(), |_| Ok(None));
    assert!(!prepared.can_publish());
    assert_eq!(prepared.issues.len(), 4);
    assert!(
        prepared
            .issues
            .iter()
            .all(|i| i.code == "missing-region" && i.region.is_some())
    );
}

#[test]
fn unstorable_height_reports_the_exact_location() {
    let f = fixture();
    let mut snapshot = snapshot(&f);
    let p = position(3202, 3203, 0);
    let coord = HeightVertexCoord::new(p.tile, p.plane);
    snapshot
        .document
        .apply_patch(&MapPatch::new(vec![MapOp::SetHeight {
            coord,
            before: -80,
            after: 1000,
        }]))
        .unwrap_or_else(|e| unreachable!("{e:?}"));
    let prepared = prepare_with(snapshot, "fixture".into(), |r| Ok(Some(square(r))));
    assert!(!prepared.can_publish());
    assert_eq!(prepared.issues[0].code, "height-not-storable");
    assert_eq!(prepared.issues[0].location, Some(p));
}

#[test]
fn pending_records_cannot_write_outside_their_region() {
    let f = fixture();
    let mut snapshot = snapshot(&f);
    snapshot.authored.clear();
    snapshot.pending.push(RegionDiff {
        region: f.regions[0],
        tiles: vec![(position(3264, 3200, 0), TerrainTile::default())],
        heights: Vec::new(),
        objects_added: Vec::new(),
        objects_removed: Vec::new(),
    });
    let prepared = prepare_with(snapshot, "fixture".into(), |r| Ok(Some(square(r))));
    assert!(!prepared.can_publish());
    assert_eq!(prepared.issues.len(), 1);
}

#[test]
fn live_authority_supersedes_a_pending_projection_retry() {
    let f = fixture();
    let mut snapshot = snapshot(&f);
    let p = position(3202, 3202, 0);
    paint(&mut snapshot.document, p, 9);
    snapshot.pending.push(RegionDiff {
        region: f.regions[0],
        tiles: vec![(p, TerrainTile::default())],
        heights: Vec::new(),
        objects_added: Vec::new(),
        objects_removed: Vec::new(),
    });
    let prepared = prepare_with(snapshot, "fixture".into(), |r| Ok(Some(square(r))));
    assert!(prepared.can_publish(), "{:?}", prepared.issues);
    assert_eq!(
        prepared.regions[0].expected.terrain_at(p).underlay,
        Some(FloorUnderlayId::new(9))
    );
}

#[test]
fn floor_id_truncation_is_a_located_blocker_before_encoding() {
    let f = fixture();
    let mut snapshot = snapshot(&f);
    let p = position(3202, 3202, 0);
    paint(&mut snapshot.document, p, 65_541);
    let prepared = prepare_with(snapshot, "fixture".into(), |r| Ok(Some(square(r))));
    assert!(!prepared.can_publish());
    assert_eq!(prepared.issues[0].code, "terrain-not-storable");
    assert_eq!(prepared.issues[0].location, Some(p));
}

#[test]
fn off_grid_heights_are_reported_and_verified_without_rounding_the_project() {
    let f = fixture();
    let mut snapshot = snapshot(&f);
    let p = position(3202, 3203, 0);
    paint(&mut snapshot.document, p, 5);
    let coord = HeightVertexCoord::new(p.tile, p.plane);
    snapshot
        .document
        .apply_patch(&MapPatch::new([MapOp::SetHeight {
            coord,
            before: -80,
            after: -83,
        }]))
        .unwrap_or_else(|_| unreachable!("valid fixture"));
    let original = snapshot.document.clone();
    let prepared = prepare_with(snapshot.clone(), "fixture".into(), |r| Ok(Some(square(r))));
    assert!(prepared.can_publish(), "{:?}", prepared.issues);
    let region = prepared
        .regions
        .iter()
        .find(|r| r.region == p.tile.split_region().0)
        .unwrap_or_else(|| unreachable!("valid fixture"));
    assert_eq!(region.expected.height_at(coord), Some(-80));
    assert!(
        prepared
            .issues
            .iter()
            .any(|i| i.code == "height-conversion" && i.location == Some(p))
    );
    assert_eq!(snapshot.document, original);
}
