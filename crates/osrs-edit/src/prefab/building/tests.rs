use super::*;
fn plane(n: u8) -> SourcePlane {
    SourcePlane::new(n).unwrap_or_else(|| unreachable!())
}
fn source() -> (MapDocument, BuildingCaptureSpec) {
    let floors = (1..=3)
        .flat_map(|x| (1..=3).map(move |y| MapPosition::new(MapTile::new(x, y), plane(0))))
        .collect::<BTreeSet<_>>();
    let heights = (0..2)
        .flat_map(|p| {
            (0..=5).flat_map(move |x| {
                (0..=5).map(move |y| {
                    (
                        HeightVertexCoord::new(MapTile::new(x, y), plane(p)),
                        -i32::from(p) * 240,
                    )
                })
            })
        })
        .collect::<Vec<_>>();
    let wall = ObjectPlacement {
        object_id: ObjectId::new(10),
        position: MapPosition::new(MapTile::new(0, 2), plane(0)),
        kind: LocKind::new(0).unwrap_or_else(|| unreachable!()),
        orientation: ObjectOrientation::new(2).unwrap_or_else(|| unreachable!()),
    };
    let mut roof = wall;
    roof.position = MapPosition::new(MapTile::new(2, 2), plane(1));
    roof.kind = LocKind::new(12).unwrap_or_else(|| unreachable!());
    let support = heights.iter().map(|(c, _)| *c).collect();
    let terrain = (0..6).flat_map(|x| {
        (0..6).map(move |y| {
            (
                MapPosition::new(MapTile::new(x, y), plane(0)),
                TerrainTile {
                    underlay: Some(FloorUnderlayId::new(if x == 0 { 99 } else { 3 })),
                    ..TerrainTile::default()
                },
            )
        })
    });
    (
        MapDocument::from_parts(terrain, heights, [wall, roof]),
        BuildingCaptureSpec {
            floors,
            support,
            objects: vec![wall, roof],
            object_sizes: std::collections::BTreeMap::from([(ObjectId::new(10), (1, 1))]),
            provenance: "fixture cache".into(),
            relations: vec![],
            reviewed: true,
            labels: Default::default(),
        },
    )
}
#[test]
fn outside_anchors_do_not_copy_outside_ground_and_review_is_required() {
    let (doc, mut spec) = source();
    let p = capture_building_prefab(&doc, &spec, "house").unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(p.objects.len(), 2);
    assert_eq!(p.tiles.len(), 9);
    assert!(p.tiles.iter().all(|t| t.underlay == Some(3)));
    assert!(p.objects.iter().any(|o| o.x == 0));
    spec.reviewed = false;
    assert_eq!(
        capture_building_prefab(&doc, &spec, "house"),
        Err(PrefabError::Building(BuildingPrefabError::Unreviewed))
    );
}
#[test]
fn foundations_rebase_roofs_and_undo_restores_every_channel() {
    let (doc, spec) = source();
    let p = capture_building_prefab(&doc, &spec, "house").unwrap_or_else(|e| panic!("{e}"));
    let heights = (0..2).flat_map(|p| {
        (25..45).flat_map(move |x| {
            (25..45).map(move |y| {
                (
                    HeightVertexCoord::new(MapTile::new(x, y), plane(p)),
                    -800 - i32::from(p) * 240 + (x - 30) * 8,
                )
            })
        })
    });
    let mut destination = MapDocument::from_parts([], heights, []);
    let before = destination.clone();
    let placed = plan_place_prefab(
        &destination,
        &p,
        PlaceSpec {
            origin: MapTile::new(30, 30),
            base_plane: plane(0),
            quarter_turns: 1,
            terrain: true,
            heights: true,
            objects: true,
            replace_objects: false,
        },
        &|_, _| (1, 1),
    )
    .unwrap_or_else(|e| panic!("{e}"));
    destination
        .apply_patch(&placed.patch)
        .unwrap_or_else(|e| panic!("{e:?}"));
    let roof = destination
        .height_at(HeightVertexCoord::new(MapTile::new(32, 34), plane(1)))
        .unwrap_or_else(|| unreachable!());
    let base = destination
        .height_at(HeightVertexCoord::new(MapTile::new(32, 34), plane(0)))
        .unwrap_or_else(|| unreachable!());
    assert_eq!(roof - base, -240);
    destination
        .apply_patch(&placed.patch.inverse())
        .unwrap_or_else(|e| panic!("{e:?}"));
    assert_eq!(destination, before);
}
#[test]
fn rectangle_prefab_v1_still_loads_without_building_metadata() {
    let (doc, _) = source();
    let mut p = capture_prefab(
        &doc,
        CaptureSpec {
            origin: MapTile::new(0, 0),
            width: 4,
            height: 4,
            base_plane: plane(0),
            planes: 2,
            terrain: true,
            heights: true,
            objects: true,
        },
        "legacy",
        PrefabSource::Authored,
    )
    .unwrap_or_else(|e| panic!("{e}"));
    p.version = 1;
    let json = serde_json::to_string(&p).unwrap_or_else(|e| panic!("{e}"));
    let decoded: Prefab = serde_json::from_str(&json).unwrap_or_else(|e| panic!("{e}"));
    assert!(decoded.validate().is_ok());
    assert!(decoded.building.is_none());
}

#[test]
fn sparse_courtyard_capture_and_studio_roundtrip_exclude_pad_and_keep_default_floor() {
    let (doc, mut spec) = source();
    spec.floors
        .remove(&MapPosition::new(MapTile::new(2, 2), plane(0)));
    let p = capture_building_prefab(&doc, &spec, "courtyard").unwrap_or_else(|e| panic!("{e}"));
    assert_eq!((p.width, p.height), (5, 5));
    assert!(!p.tiles.iter().any(|t| (t.x, t.y) == (2, 2)));
    let pad = MapDocument::from_parts([], doc.height_entries(), []);
    let mut scene = pad.clone();
    let place = PlaceSpec {
        origin: MapTile::new(0, 0),
        base_plane: plane(0),
        quarter_turns: 0,
        terrain: true,
        heights: true,
        objects: true,
        replace_objects: false,
    };
    let patch =
        plan_place_exact(&scene, &p, place, &|_, _| (1, 1)).unwrap_or_else(|e| panic!("{e}"));
    scene
        .apply_patch(&patch.patch)
        .unwrap_or_else(|e| panic!("{e:?}"));
    let capture = CaptureSpec {
        origin: MapTile::new(0, 0),
        width: p.width,
        height: p.height,
        base_plane: plane(0),
        planes: p.planes,
        terrain: true,
        heights: true,
        objects: true,
    };
    let round = capture_prefab_over(&scene, &pad, capture, &p).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(round, p);
    let c = MapPosition::new(MapTile::new(1, 1), plane(0));
    scene
        .apply_patch(&MapPatch::new(vec![MapOp::SetTerrain {
            coord: c,
            before: scene.terrain_at(c),
            after: TerrainTile::default(),
        }]))
        .unwrap_or_else(|e| panic!("{e:?}"));
    let edited = capture_prefab_over(&scene, &pad, capture, &p).unwrap_or_else(|e| panic!("{e}"));
    assert!(
        edited
            .tiles
            .iter()
            .any(|t| (t.x, t.y) == (1, 1) && t.underlay.is_none())
    );
}

#[test]
fn fitted_rotations_preserve_upper_elevation_and_reject_unreviewed_neighbors() {
    let (doc, spec) = source();
    let p = capture_building_prefab(&doc, &spec, "house").unwrap_or_else(|e| panic!("{e}"));
    for turns in 0..4 {
        let heights = (0..2).flat_map(|p| {
            (20..45).flat_map(move |x| {
                (20..45).map(move |y| {
                    (
                        HeightVertexCoord::new(MapTile::new(x, y), plane(p)),
                        -800 - i32::from(p) * 240 + (x - 30) * 8,
                    )
                })
            })
        });
        let mut destination = MapDocument::from_parts([], heights, []);
        let before = destination.clone();
        let placement = PlaceSpec {
            origin: MapTile::new(30, 30),
            base_plane: plane(0),
            quarter_turns: turns,
            terrain: true,
            heights: true,
            objects: true,
            replace_objects: false,
        };
        let plan = plan_place_prefab(&destination, &p, placement, &|_, _| (1, 1))
            .unwrap_or_else(|e| panic!("{e}"));
        destination
            .apply_patch(&plan.patch)
            .unwrap_or_else(|e| panic!("{e:?}"));
        let (x, y) = super::super::foundation::turn(2, 2, &p, turns, true);
        let vertex = MapTile::new(30 + x, 30 + y);
        let base = destination
            .height_at(HeightVertexCoord::new(vertex, plane(0)))
            .unwrap_or_default();
        let roof = destination
            .height_at(HeightVertexCoord::new(vertex, plane(1)))
            .unwrap_or_default();
        assert_eq!(roof - base, -240);
        destination
            .apply_patch(&plan.patch.inverse())
            .unwrap_or_else(|e| panic!("{e:?}"));
        assert_eq!(destination, before);
        let mut unrelated = spec.objects[0];
        unrelated.position = MapPosition::new(MapTile::new(32, 32), plane(0));
        destination
            .apply_patch(&MapPatch::new(vec![MapOp::AddObject {
                placement: unrelated,
            }]))
            .unwrap_or_else(|e| panic!("{e:?}"));
        assert!(matches!(
            plan_place_prefab(&destination, &p, placement, &|_, _| (1, 1)),
            Err(PrefabError::Building(BuildingPrefabError::Occupied(_)))
        ));
    }
}

#[test]
fn incomplete_floor_support_cannot_be_claimed_complete() {
    let (doc, mut spec) = source();
    spec.support
        .remove(&HeightVertexCoord::new(MapTile::new(1, 1), plane(0)));
    assert!(matches!(
        capture_building_prefab(&doc, &spec, "broken"),
        Err(PrefabError::Building(BuildingPrefabError::MissingHeight(_)))
    ));
}
