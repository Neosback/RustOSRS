use super::*;
use crate::MapDocument;
use osrs_core::ids::{FloorOverlayId, FloorUnderlayId, ObjectId};

fn plane(value: u8) -> SourcePlane {
    SourcePlane::new(value).unwrap_or_else(|| unreachable!("test plane is valid"))
}

fn object(id: u32, x: i32, y: i32, p: u8, kind: u8, orientation: u8) -> ObjectPlacement {
    ObjectPlacement {
        object_id: ObjectId::new(id),
        position: MapPosition::new(MapTile::new(x, y), plane(p)),
        kind: LocKind::new(kind).unwrap_or_else(|| unreachable!("test kind is valid")),
        orientation: ObjectOrientation::new(orientation)
            .unwrap_or_else(|| unreachable!("test orientation is valid")),
    }
}

/// A 4x3 patch of ground with distinct heights, a diagonal overlay, a wall on the east edge of
/// (1,1), a pillar-like scenery object (2x1) and a one-tile table.
fn world(origin: (i32, i32)) -> MapDocument {
    let (ox, oy) = origin;
    let mut terrain = Vec::new();
    let mut heights = Vec::new();
    for y in 0..3 {
        for x in 0..4 {
            let overlay = (x == 2 && y == 1).then(|| TerrainOverlay {
                id: FloorOverlayId::new(5),
                shape: OverlayShape::new(2).unwrap_or_else(|| unreachable!()),
                rotation: OverlayRotation::new(1).unwrap_or_else(|| unreachable!()),
            });
            terrain.push((
                MapPosition::new(MapTile::new(ox + x, oy + y), plane(0)),
                TerrainTile {
                    underlay: Some(FloorUnderlayId::new(3)),
                    overlay,
                    flags: u8::from(x == 0 && y == 0),
                },
            ));
        }
    }
    for y in 0..=3 {
        for x in 0..=4 {
            heights.push((
                HeightVertexCoord::new(MapTile::new(ox + x, oy + y), plane(0)),
                -(x * 10 + y),
            ));
        }
    }
    MapDocument::from_parts(
        terrain,
        heights,
        [
            object(100, ox + 1, oy + 1, 0, 0, 2),
            object(200, ox, oy + 2, 0, 10, 0),
            object(300, ox + 3, oy, 0, 10, 1),
        ],
    )
}

fn empty_ground(origin: (i32, i32), w: i32, h: i32) -> MapDocument {
    let mut heights = Vec::new();
    for y in 0..=h {
        for x in 0..=w {
            heights.push((
                HeightVertexCoord::new(MapTile::new(origin.0 + x, origin.1 + y), plane(0)),
                0,
            ));
        }
    }
    MapDocument::from_parts([], heights, [])
}

/// Footprints: object 200 is 2x1, everything else one tile.
fn sizes(id: ObjectId, _: LocKind) -> (u8, u8) {
    if id.get() == 200 { (2, 1) } else { (1, 1) }
}

fn spec(origin: (i32, i32), turns: u8) -> PlaceSpec {
    PlaceSpec {
        origin: MapTile::new(origin.0, origin.1),
        base_plane: plane(0),
        quarter_turns: turns,
        terrain: true,
        heights: true,
        objects: true,
        replace_objects: false,
    }
}

fn capture_of(doc: &MapDocument, origin: (i32, i32), w: u16, h: u16) -> Prefab {
    capture_prefab(
        doc,
        CaptureSpec {
            origin: MapTile::new(origin.0, origin.1),
            width: w,
            height: h,
            base_plane: plane(0),
            planes: 1,
            terrain: true,
            heights: true,
            objects: true,
        },
        "test",
        PrefabSource::Authored,
    )
    .unwrap_or_else(|error| unreachable!("capture: {error}"))
}

#[test]
fn capture_then_place_unturned_reproduces_the_area_elsewhere() {
    let source = world((3200, 3200));
    let prefab = capture_of(&source, (3200, 3200), 4, 3);
    assert_eq!(prefab.tiles.len(), 12);
    assert_eq!(prefab.heights.len(), 20);
    assert_eq!(prefab.objects.len(), 3);

    let mut target = empty_ground((3300, 3250), 4, 3);
    let placed = plan_place_prefab(&target, &prefab, spec((3300, 3250), 0), &sizes)
        .unwrap_or_else(|error| unreachable!("{error}"));
    assert_eq!((placed.width, placed.height), (4, 3));
    target
        .apply_patch(&placed.patch)
        .unwrap_or_else(|error| unreachable!("{error:?}"));
    assert_eq!(capture_of(&target, (3300, 3250), 4, 3), prefab);
}

#[test]
fn four_quarter_turns_are_the_identity_and_each_turn_changes_the_shape() {
    let source = world((3200, 3200));
    let original = capture_of(&source, (3200, 3200), 4, 3);
    let mut current = original.clone();
    let mut origin = (3400, 3400);
    for turn in 1..=4u8 {
        let target = empty_ground(origin, 4, 4);
        let placed = plan_place_prefab(&target, &current, spec(origin, 1), &sizes)
            .unwrap_or_else(|error| unreachable!("turn {turn}: {error}"));
        let mut target = target;
        target
            .apply_patch(&placed.patch)
            .unwrap_or_else(|error| unreachable!("{error:?}"));
        current = capture_of(&target, origin, placed.width, placed.height);
        if turn < 4 {
            assert_ne!(current, original, "turn {turn} left the prefab unchanged");
        }
        origin.0 += 20;
    }
    assert_eq!(current.tiles, original.tiles);
    assert_eq!(current.heights, original.heights);
    assert_eq!(current.objects, original.objects);
}

#[test]
fn one_turn_moves_things_where_a_clockwise_quarter_turn_puts_them() {
    let source = world((3200, 3200));
    let prefab = capture_of(&source, (3200, 3200), 4, 3);
    let target = empty_ground((3300, 3300), 3, 4);
    let placed = plan_place_prefab(&target, &prefab, spec((3300, 3300), 1), &sizes)
        .unwrap_or_else(|error| unreachable!("{error}"));
    assert_eq!((placed.width, placed.height), (3, 4));
    let objects: Vec<_> = placed
        .patch
        .ops()
        .iter()
        .filter_map(|op| match op {
            MapOp::AddObject { placement } => Some(*placement),
            _ => None,
        })
        .collect();
    // The wall at (1,1) facing east (2) becomes (1, 4-1-1=2) facing south (3).
    assert!(objects.contains(&object(100, 3301, 3302, 0, 0, 3)));
    // The 2x1 object at (0,2) (west of the north row) becomes a 1x2 footprint whose south-west
    // tile is (2, 4-2-0=2)... turned: x' = y = 2, y' = w - fw - x = 4 - 2 - 0 = 2, orientation 1.
    assert!(objects.contains(&object(200, 3302, 3302, 0, 10, 1)));
    // The table at (3,0) (south-east corner) goes to the south-west corner (0,0).
    assert!(objects.contains(&object(300, 3300, 3300, 0, 10, 2)));
}

#[test]
fn replacing_removes_what_stands_there_and_identical_objects_are_kept() {
    let source = world((3200, 3200));
    let prefab = capture_of(&source, (3200, 3200), 4, 3);
    let mut target = world((3200, 3200));
    // An extra object inside the box, and the prefab placed over its own source.
    let extra = object(999, 3201, 3200, 0, 10, 0);
    let mut patch = MapPatch::new([MapOp::AddObject { placement: extra }]);
    target
        .apply_patch(&patch)
        .unwrap_or_else(|error| unreachable!("{error:?}"));
    let keep = plan_place_prefab(&target, &prefab, spec((3200, 3200), 0), &sizes)
        .unwrap_or_else(|error| unreachable!("{error}"));
    assert!(
        keep.patch.is_empty(),
        "same map, same prefab: nothing to do"
    );
    assert_eq!(keep.already_present, 3);

    let mut replace = spec((3200, 3200), 0);
    replace.replace_objects = true;
    let placed = plan_place_prefab(&target, &prefab, replace, &sizes)
        .unwrap_or_else(|error| unreachable!("{error}"));
    patch = placed.patch;
    assert!(
        patch
            .ops()
            .contains(&MapOp::RemoveObject { placement: extra })
    );
    target
        .apply_patch(&patch)
        .unwrap_or_else(|error| unreachable!("{error:?}"));
    assert!(!target.contains_object(extra));
    assert_eq!(capture_of(&target, (3200, 3200), 4, 3), prefab);
}

#[test]
fn placement_is_one_patch_that_undoes_exactly() {
    let source = world((3200, 3200));
    let prefab = capture_of(&source, (3200, 3200), 4, 3);
    let mut target = empty_ground((3500, 3500), 4, 3);
    let before = capture_prefab(
        &target,
        CaptureSpec {
            origin: MapTile::new(3500, 3500),
            width: 4,
            height: 3,
            base_plane: plane(0),
            planes: 1,
            terrain: false,
            heights: true,
            objects: false,
        },
        "before",
        PrefabSource::Authored,
    )
    .unwrap_or_else(|error| unreachable!("{error}"));
    let placed = plan_place_prefab(&target, &prefab, spec((3500, 3500), 0), &sizes)
        .unwrap_or_else(|error| unreachable!("{error}"));
    target
        .apply_patch(&placed.patch)
        .unwrap_or_else(|error| unreachable!("{error:?}"));
    let inverse = MapPatch::new(placed.patch.ops().iter().rev().map(|op| op.inverse()));
    target
        .apply_patch(&inverse)
        .unwrap_or_else(|error| unreachable!("{error:?}"));
    let after = capture_prefab(
        &target,
        CaptureSpec {
            origin: MapTile::new(3500, 3500),
            width: 4,
            height: 3,
            base_plane: plane(0),
            planes: 1,
            terrain: false,
            heights: true,
            objects: false,
        },
        "after",
        PrefabSource::Authored,
    )
    .unwrap_or_else(|error| unreachable!("{error}"));
    assert_eq!(before.heights, after.heights);
    assert_eq!(target.object_count(), 0);
    assert_eq!(target.terrain_entry_count(), 0);
}

#[test]
fn heights_the_map_lacks_are_skipped_not_invented() {
    let source = world((3200, 3200));
    let prefab = capture_of(&source, (3200, 3200), 4, 3);
    let target = MapDocument::new();
    let placed = plan_place_prefab(&target, &prefab, spec((3600, 3600), 0), &sizes)
        .unwrap_or_else(|error| unreachable!("{error}"));
    assert_eq!(placed.skipped_heights, 20);
    assert!(
        !placed
            .patch
            .ops()
            .iter()
            .any(|op| matches!(op, MapOp::SetHeight { .. }))
    );
}

#[test]
fn files_that_lie_are_rejected_before_use() {
    let source = world((3200, 3200));
    let good = capture_of(&source, (3200, 3200), 4, 3);

    let mut bad = good.clone();
    bad.format = "other".into();
    assert!(matches!(bad.validate(), Err(PrefabError::Format(_))));

    let mut bad = good.clone();
    bad.version = 9;
    assert_eq!(bad.validate(), Err(PrefabError::Version(9)));

    let mut bad = good.clone();
    bad.objects[0].x = 40;
    assert!(matches!(bad.validate(), Err(PrefabError::OutOfBox { .. })));

    let mut bad = good.clone();
    bad.objects[0].kind = 23;
    assert!(matches!(bad.validate(), Err(PrefabError::BadValue { .. })));

    let mut bad = good.clone();
    bad.tiles.push(bad.tiles[0].clone());
    assert!(matches!(bad.validate(), Err(PrefabError::Duplicate { .. })));

    let mut bad = good;
    bad.name = "  ".into();
    assert_eq!(bad.validate(), Err(PrefabError::EmptyName));
}

#[test]
fn prefab_from_a_patch_matches_capturing_the_patched_map() {
    let mut document = empty_ground((3700, 3700), 4, 3);
    let source = world((3700, 3700));
    let placed = plan_place_prefab(
        &document,
        &capture_of(&source, (3700, 3700), 4, 3),
        spec((3700, 3700), 0),
        &sizes,
    )
    .unwrap_or_else(|error| unreachable!("{error}"));
    let from_patch = prefab_from_patch(
        &placed.patch,
        MapTile::new(3700, 3700),
        plane(0),
        4,
        3,
        "test",
        PrefabSource::Authored,
    )
    .unwrap_or_else(|error| unreachable!("{error}"));
    document
        .apply_patch(&placed.patch)
        .unwrap_or_else(|error| unreachable!("{error:?}"));
    let captured = capture_of(&document, (3700, 3700), 4, 3);
    assert_eq!(from_patch.tiles, captured.tiles);
    assert_eq!(from_patch.objects, captured.objects);
    // The patch only sets heights that changed; the capture also holds the unchanged zeros.
    assert!(from_patch.heights.len() <= captured.heights.len());
}

#[test]
fn replacing_also_clears_big_objects_that_reach_into_the_area_from_outside() {
    // A 3x3 object stored two tiles west of the area reaches its first column; a 1x1 object
    // one tile outside does not; both are on the same plane.
    let reaching = object(777, 3198, 3201, 0, 10, 0);
    let outside = object(778, 3199, 3201, 0, 10, 0);
    let mut target = world((3200, 3200));
    target
        .apply_patch(&MapPatch::new([
            MapOp::AddObject {
                placement: reaching,
            },
            MapOp::AddObject { placement: outside },
        ]))
        .unwrap_or_else(|e| unreachable!("{e:?}"));
    let prefab = capture_of(&world((3200, 3200)), (3200, 3200), 4, 3);
    let big = |id: ObjectId, _: LocKind| if id.get() == 777 { (3, 3) } else { (1, 1) };
    let mut replace = spec((3200, 3200), 0);
    replace.replace_objects = true;
    let placed =
        plan_place_prefab(&target, &prefab, replace, &big).unwrap_or_else(|e| unreachable!("{e}"));
    let removed: Vec<_> = placed
        .patch
        .ops()
        .iter()
        .filter_map(|op| match op {
            MapOp::RemoveObject { placement } => Some(placement.object_id.get()),
            _ => None,
        })
        .collect();
    assert!(
        removed.contains(&777),
        "the big object reaching in is cleared"
    );
    assert!(
        !removed.contains(&778),
        "a neighbour that only touches the edge stays"
    );
}

#[test]
fn non_scenery_rotation_uses_scene_storage_footprints_even_with_large_definitions() {
    let source = MapDocument::from_parts([], [], [object(900, 3201, 3201, 0, 12, 0)]);
    let p = capture_of(&source, (3200, 3200), 4, 3);
    let placed = plan_place_prefab(
        &empty_ground((3300, 3300), 3, 4),
        &p,
        spec((3300, 3300), 1),
        &|_, _| (3, 2),
    )
    .unwrap_or_else(|e| panic!("{e}"));
    assert!(placed.patch.ops().contains(&MapOp::AddObject {
        placement: object(900, 3301, 3302, 0, 12, 1)
    }));
}
