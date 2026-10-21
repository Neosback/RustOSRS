use super::*;
use crate::{
    BuildingArchetype, BuildingConstraints, BuildingRequest, BuildingStyle, HeightVertexCoord,
    MapDocument, MapOp, RectSite, capture_generation_snapshot,
};
use osrs_core::coords::{MapTile, SourcePlane};

fn asset(id: u32, kind: u8) -> KitAsset {
    KitAsset {
        id,
        kind,
        // Semantic N/E/S/W maps to client orientation W/N/E/S.
        orientations: [1, 2, 3, 0],
    }
}

/// A kit using the same stand-in objects the generator's own tests use (ids are arbitrary).
fn kit() -> KitFile {
    KitFile {
        format: KIT_FORMAT.into(),
        version: KIT_VERSION,
        name: "Test cottage".into(),
        notes: String::new(),
        floor: KitFloor {
            underlay: Some(7),
            overlay: None,
        },
        exterior_wall: asset(100, 0),
        interior_wall: asset(101, 0),
        door: asset(102, 0),
        window: Some(asset(103, 0)),
        corner: Some(asset(200, 2)),
        roof_plane_offset: 0,
        door_variants: vec![
            KitWeightedAsset {
                asset: asset(300, 0),
                weight: 1,
            },
            KitWeightedAsset {
                asset: asset(301, 0),
                weight: 3,
            },
        ],
        roof_hip: None,
        finish: None,
        stairs: None,
        roof: Some(KitRoof {
            learned: None,
            patterns: (0..12u32)
                .map(|index| KitRoofPattern {
                    id: 20_000 + index,
                    kind: 10,
                    orientation: (index % 4) as u8,
                    weight: index + 1,
                    sockets: [1; 4],
                })
                .collect(),
            ridge: vec![0, 1],
            slopes: [vec![2], vec![3], vec![4], vec![5]],
            eaves: [vec![6], vec![7], vec![8], vec![9]],
            junctions: vec![
                KitJunction {
                    requires: vec![("ridge_intersection".into(), "north".into())],
                    pattern: 10,
                },
                KitJunction {
                    requires: vec![("ridge_intersection".into(), "south".into())],
                    pattern: 11,
                },
            ],
        }),
    }
}

#[test]
fn footprint_requirements_cover_all_asset_sources_and_reject_ambiguous_hints() {
    use crate::{KitDecor, KitFurniture, KitPlace, KitWindowDecor};
    let mut file = kit();
    for pattern in &mut file
        .roof
        .as_mut()
        .unwrap_or_else(|| panic!("roof"))
        .patterns
    {
        pattern.kind = 12;
    }
    assert!(
        file.footprint_requirements()
            .unwrap_or_else(|e| panic!("{e}"))
            .is_empty()
    );
    file.exterior_wall = asset(1, 10);
    file.interior_wall = asset(2, 11);
    file.door = asset(3, 10);
    file.window = Some(asset(4, 10));
    file.corner = Some(asset(5, 11));
    file.door_variants[0].asset = asset(6, 10);
    file.roof
        .as_mut()
        .unwrap_or_else(|| panic!("roof"))
        .patterns[0]
        .id = 7;
    file.roof
        .as_mut()
        .unwrap_or_else(|| panic!("roof"))
        .patterns[0]
        .kind = 10;
    let furniture = KitFurniture {
        id: 8,
        kind: 10,
        place: KitPlace::AgainstWall,
        relative: 0,
        min: 1,
        max: 1,
        size: [2, 3],
    };
    file.finish = Some(KitFinish {
        furniture: [("living".into(), vec![furniture.clone()])].into(),
        wall_decor: vec![KitDecor {
            id: 9,
            kind: 10,
            weight: 1,
        }],
        window_decor: Some(KitWindowDecor {
            outside: Some(KitDecor {
                id: 10,
                kind: 11,
                weight: 1,
            }),
            inside: Some(KitDecor {
                id: 11,
                kind: 10,
                weight: 1,
            }),
        }),
        ..Default::default()
    });
    let facts = file
        .footprint_requirements()
        .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(
        facts.iter().map(|f| f.object.get()).collect::<Vec<_>>(),
        (1..=11).collect::<Vec<_>>()
    );
    assert_eq!(facts[7].declared_size, Some((2, 3)));
    assert!(
        facts
            .iter()
            .filter(|f| f.object.get() != 8)
            .all(|f| f.declared_size.is_none())
    );
    let finish = file.finish.as_mut().unwrap_or_else(|| panic!("finish"));
    let mut conflicting = furniture;
    conflicting.size = [1, 1];
    finish.furniture.insert("bedroom".into(), vec![conflicting]);
    assert!(matches!(
        file.footprint_requirements(),
        Err(KitError::Value { value: 8, .. })
    ));
    let finish = file.finish.as_mut().unwrap_or_else(|| panic!("finish"));
    finish
        .furniture
        .get_mut("living")
        .unwrap_or_else(|| panic!("living"))[0]
        .size = [0, 3];
    finish
        .furniture
        .get_mut("bedroom")
        .unwrap_or_else(|| panic!("bedroom"))[0]
        .size = [1, 3];
    assert_eq!(
        file.footprint_requirements()
            .unwrap_or_else(|e| panic!("{e}"))[7]
            .declared_size,
        Some((1, 3))
    );
}

fn snapshot(width: u16, height: u16) -> (MapDocument, crate::GenerationSnapshot) {
    let plane = SourcePlane::new(1).unwrap_or_else(|| unreachable!());
    let request = BuildingRequest {
        seed: 77,
        site: RectSite::new(MapTile::new(3200, 3200), width, height)
            .unwrap_or_else(|| unreachable!()),
        base_plane: plane,
        floors: 1,
        archetype: BuildingArchetype::Bank,
        style: BuildingStyle::Generic,
        constraints: BuildingConstraints::default(),
    };
    let mut heights = Vec::new();
    for y in 0..=i32::from(height) {
        for x in 0..=i32::from(width) {
            heights.push((
                HeightVertexCoord::new(MapTile::new(3200 + x, 3200 + y), plane),
                -64,
            ));
        }
    }
    let document = MapDocument::from_parts([], heights, []);
    let snapshot = capture_generation_snapshot(&document, request, []);
    (document, snapshot)
}

#[test]
fn a_kit_file_round_trips_through_json() {
    let file = kit();
    let text = serde_json::to_string_pretty(&file).unwrap_or_else(|e| unreachable!("{e}"));
    let back: KitFile = serde_json::from_str(&text).unwrap_or_else(|e| unreachable!("{e}"));
    assert_eq!(back, file);
}

#[test]
fn a_compiled_kit_generates_the_same_building_every_time_with_its_own_objects() {
    let compiled = kit().compile().unwrap_or_else(|e| unreachable!("{e}"));
    assert_eq!(compiled.name(), "Test cottage");
    let (mut document, snapshot) = snapshot(28, 22);
    let first = compiled
        .generate(&snapshot, 2)
        .unwrap_or_else(|e| unreachable!("{e:?}"));
    assert_eq!(
        first,
        compiled
            .generate(&snapshot, 2)
            .unwrap_or_else(|e| unreachable!("{e:?}"))
    );
    let objects: Vec<u32> = first
        .ops()
        .iter()
        .filter_map(|op| match op {
            MapOp::AddObject { placement } => Some(placement.object_id.get()),
            _ => None,
        })
        .collect();
    assert!(
        objects.contains(&100) || objects.contains(&103),
        "walls and windows come from the kit"
    );
    assert!(
        objects.iter().any(|id| *id >= 20_000),
        "the roof comes from the kit"
    );
    assert!(
        objects
            .iter()
            .all(|id| [100, 101, 102, 103, 200, 300, 301].contains(id) || *id >= 20_000)
    );
    // A different facade variation changes appearance but still applies and undoes exactly.
    let other = compiled
        .generate(&snapshot, 5)
        .unwrap_or_else(|e| unreachable!("{e:?}"));
    let original = document.clone();
    document
        .apply_patch(&other)
        .unwrap_or_else(|e| unreachable!("{e:?}"));
    document
        .apply_patch(&other.inverse())
        .unwrap_or_else(|e| unreachable!("{e:?}"));
    assert_eq!(document, original);
}

#[test]
fn the_roof_goes_up_by_the_kits_plane_offset_and_walls_stay_put() {
    let mut file = kit();
    file.roof_plane_offset = 1;
    let lifted = file.compile().unwrap_or_else(|e| unreachable!("{e}"));
    let (_, snapshot) = snapshot(28, 22);
    let patch = lifted
        .generate(&snapshot, 2)
        .unwrap_or_else(|e| unreachable!("{e:?}"));
    // The test fixtures' roof pieces use kind 10, so only a kind 12 piece moves: use real kinds.
    let mut real = kit();
    real.roof_plane_offset = 1;
    for pattern in &mut real
        .roof
        .as_mut()
        .unwrap_or_else(|| unreachable!())
        .patterns
    {
        pattern.kind = 12;
    }
    let real = real.compile().unwrap_or_else(|e| unreachable!("{e}"));
    let moved = real
        .generate(&snapshot, 2)
        .unwrap_or_else(|e| unreachable!("{e:?}"));
    let planes = |patch: &MapPatch, roof: bool| -> std::collections::BTreeSet<u8> {
        patch
            .ops()
            .iter()
            .filter_map(|op| match op {
                MapOp::AddObject { placement } if (placement.object_id.get() >= 20_000) == roof => {
                    Some(placement.position.plane.index().get())
                }
                _ => None,
            })
            .collect()
    };
    assert_eq!(
        planes(&patch, true),
        [1].into(),
        "kind 10 pieces are not roofs and stay on the base plane"
    );
    assert_eq!(
        planes(&moved, true),
        [2].into(),
        "roof kinds go one plane up"
    );
    assert_eq!(planes(&moved, false), [1].into(), "walls do not move");
}

#[test]
fn bad_kits_are_refused_with_a_reason() {
    let mut file = kit();
    file.format = "x".into();
    assert!(matches!(file.compile(), Err(KitError::Format(_))));

    let mut file = kit();
    file.version = KIT_VERSION + 1;
    assert!(matches!(file.compile(), Err(KitError::Version(v)) if v == KIT_VERSION + 1));

    let mut file = kit();
    file.name = " ".into();
    assert!(matches!(file.compile(), Err(KitError::EmptyName)));

    let mut file = kit();
    file.door.orientations[2] = 9;
    assert!(matches!(
        file.compile(),
        Err(KitError::Value {
            what: "orientation",
            ..
        })
    ));

    let mut file = kit();
    file.exterior_wall.kind = 40;
    assert!(matches!(
        file.compile(),
        Err(KitError::Value {
            what: "object kind",
            ..
        })
    ));

    let mut file = kit();
    file.roof.as_mut().unwrap_or_else(|| unreachable!()).ridge = vec![0, 99];
    assert!(matches!(file.compile(), Err(KitError::Roof(_))));

    let mut file = kit();
    file.roof
        .as_mut()
        .unwrap_or_else(|| unreachable!())
        .junctions[0]
        .requires = vec![("sideways".into(), "north".into())];
    assert!(matches!(file.compile(), Err(KitError::Roof(_))));

    let mut file = kit();
    file.roof
        .as_mut()
        .unwrap_or_else(|| unreachable!())
        .patterns
        .clear();
    assert!(matches!(file.compile(), Err(KitError::Roof(_))));

    let mut file = kit();
    file.roof = None;
    assert!(
        matches!(file.compile(), Err(KitError::Roof(_))),
        "a kit needs some roof"
    );

    let text = serde_json::to_string(&kit())
        .unwrap_or_else(|e| unreachable!("{e}"))
        .replace("\"notes\"", "\"surprise\":1,\"notes\"");
    assert!(
        serde_json::from_str::<KitFile>(&text).is_err(),
        "unknown fields are rejected"
    );
}

fn hip_kit() -> KitFile {
    let mut file = kit();
    file.roof = None;
    file.roof_hip = Some(KitHipRoof {
        slope_object: 1924,
        edge_object: 1790,
        style: Default::default(),
    });
    file.roof_plane_offset = 1;
    file
}

#[test]
fn a_hip_kit_roofs_the_footprint_with_an_overhang_one_plane_up_with_one_piece_per_tile() {
    let compiled = hip_kit().compile().unwrap_or_else(|e| unreachable!("{e}"));
    let (_, snapshot) = snapshot(20, 14);
    let patch = compiled
        .generate(&snapshot, 0)
        .unwrap_or_else(|e| unreachable!("{e:?}"));
    let mut roof_tiles = std::collections::BTreeSet::new();
    let mut floor_tiles = std::collections::BTreeSet::new();
    for op in patch.ops() {
        match op {
            MapOp::AddObject { placement } if (12..=21).contains(&placement.kind.get()) => {
                assert_eq!(
                    placement.position.plane.index().get(),
                    2,
                    "base plane 1, offset 1"
                );
                assert!([1924, 1790].contains(&placement.object_id.get()));
                assert!(
                    roof_tiles.insert(placement.position.tile),
                    "one roof piece per tile"
                );
            }
            MapOp::SetTerrain { coord, .. } => {
                floor_tiles.insert(coord.tile);
            }
            _ => {}
        }
    }
    assert!(!roof_tiles.is_empty());
    // Real roofs overhang the walls by one tile on every side.
    let mut overhung = std::collections::BTreeSet::new();
    for tile in &floor_tiles {
        for dx in -1..=1 {
            for dy in -1..=1 {
                overhung.insert(MapTile::new(tile.x + dx, tile.y + dy));
            }
        }
    }
    assert!(
        roof_tiles == overhung,
        "the roof covers the floor and one tile more on every side"
    );
    assert!(roof_tiles.len() > floor_tiles.len());
    assert_eq!(
        patch,
        compiled
            .generate(&snapshot, 0)
            .unwrap_or_else(|e| unreachable!("{e:?}"))
    );
}

#[test]
fn bigger_sites_get_more_rooms_and_small_ones_stay_simple() {
    use crate::{BuildingArchetype, building_program};
    let program = building_program(BuildingArchetype::House);
    assert_eq!(program.rooms().len(), 5);
    let small = program.fitted_to_area(48);
    assert_eq!(small.rooms().len(), 2, "never fewer than two rooms");
    assert_eq!(program.fitted_to_area(80).rooms().len(), 3);
    assert_eq!(
        program.fitted_to_area(500).rooms().len(),
        5,
        "never more than the program lists"
    );
    // Whatever is dropped, every kept room is still connected to the entrance.
    for archetype in [
        BuildingArchetype::House,
        BuildingArchetype::Shop,
        BuildingArchetype::Tavern,
        BuildingArchetype::Bank,
        BuildingArchetype::Church,
        BuildingArchetype::Warehouse,
        BuildingArchetype::Castle,
        BuildingArchetype::Dungeon,
    ] {
        for area in [10, 44, 66, 88, 110] {
            let fitted = building_program(archetype).fitted_to_area(area);
            let mut reached = vec![fitted.entrance_room];
            let mut grew = true;
            while grew {
                grew = false;
                for room in fitted.rooms() {
                    if !reached.contains(&room.id)
                        && reached
                            .iter()
                            .any(|r| fitted.neighbors(*r).any(|n| n == room.id))
                    {
                        reached.push(room.id);
                        grew = true;
                    }
                }
            }
            assert_eq!(
                reached.len(),
                fitted.rooms().len(),
                "{archetype:?} at {area}"
            );
        }
    }
}

#[test]
fn two_floor_building_generates_upper_story_stairs_and_lifted_roof() {
    let mut file = hip_kit();
    file.stairs = Some(KitStairs {
        up: asset(90001, 10),
        down: asset(90002, 10),
    });
    let compiled = file.compile().unwrap_or_else(|e| unreachable!("{e}"));
    let plane0 = SourcePlane::new(0).unwrap_or_else(|| unreachable!());
    let request = BuildingRequest {
        seed: 77,
        site: RectSite::new(MapTile::new(3200, 3200), 16, 12).unwrap_or_else(|| unreachable!()),
        base_plane: plane0,
        floors: 2,
        archetype: BuildingArchetype::House,
        style: BuildingStyle::Generic,
        constraints: BuildingConstraints::default(),
    };
    let mut heights = Vec::new();
    for y in 0..=12 {
        for x in 0..=16 {
            heights.push((
                HeightVertexCoord::new(MapTile::new(3200 + x, 3200 + y), plane0),
                -64,
            ));
        }
    }
    let mut document = MapDocument::from_parts([], heights, []);
    let original = document.clone();
    let snapshot = capture_generation_snapshot(&document, request, []);
    let patch = compiled
        .generate(&snapshot, 0)
        .unwrap_or_else(|e| unreachable!("{e:?}"));

    let mut wall_planes = std::collections::BTreeSet::new();
    let mut roof_planes = std::collections::BTreeSet::new();
    let mut has_stair_up = false;
    let mut has_stair_down = false;
    for op in patch.ops() {
        if let MapOp::AddObject { placement } = op {
            let p = placement.position.plane.index().get();
            if (0..=3).contains(&placement.kind.get()) {
                wall_planes.insert(p);
            } else if (12..=21).contains(&placement.kind.get()) {
                roof_planes.insert(p);
            } else if placement.object_id.get() == 90001 {
                has_stair_up = true;
            } else if placement.object_id.get() == 90002 {
                has_stair_down = true;
            }
        }
    }
    assert_eq!(wall_planes, [0, 1].into());
    // The top floor's roof is on the plane above it. A floor above that steps back leaves part of
    // this one exposed, and that part has a roof of its own on plane 1.
    assert!(roof_planes.contains(&2), "{roof_planes:?}");
    assert!(roof_planes.is_subset(&[1, 2].into()), "{roof_planes:?}");
    assert!(has_stair_up && has_stair_down);

    document
        .apply_patch(&patch)
        .unwrap_or_else(|e| unreachable!("{e:?}"));
    document
        .apply_patch(&patch.inverse())
        .unwrap_or_else(|e| unreachable!("{e:?}"));
    assert_eq!(document, original);
}

#[test]
fn legacy_kits_load_but_multiple_floors_require_explicit_stairs() {
    let mut file = hip_kit();
    file.version = 1;
    let json = serde_json::to_string(&file).unwrap_or_else(|e| panic!("{e}"));
    assert!(!json.contains("stairs"));
    let loaded: KitFile = serde_json::from_str(&json).unwrap_or_else(|e| panic!("{e}"));
    let compiled = loaded.compile().unwrap_or_else(|e| panic!("{e}"));
    let (_, snapshot) = snapshot(16, 12);
    assert!(compiled.generate(&snapshot, 0).is_ok());
    let request = BuildingRequest {
        floors: 2,
        ..snapshot.request()
    };
    assert_eq!(
        compiled.generate(&snapshot.with_request(request), 0),
        Err(KitGenerateError::MissingStairs)
    );
    file.stairs = Some(KitStairs {
        up: asset(42, 10),
        down: asset(43, 10),
    });
    assert!(matches!(file.compile(), Err(KitError::Version(1))));
}

#[test]
fn explicit_stairs_validate_and_participate_in_dependency_and_stage_queries() {
    let mut file = hip_kit();
    file.stairs = Some(KitStairs {
        up: asset(42, 10),
        down: asset(43, 11),
    });
    let facts = file
        .footprint_requirements()
        .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(facts.len(), 2);
    assert_eq!(
        facts.iter().map(|f| f.object.get()).collect::<Vec<_>>(),
        [42, 43]
    );
    assert!(facts.iter().all(|f| f.declared_size.is_none()));
    let compiled = file.compile().unwrap_or_else(|e| panic!("{e}"));
    let palette = compiled.stairs.unwrap_or_else(|| panic!("stairs"));
    for stair in [palette.up, palette.down] {
        let placement = stair.placement(
            MapTile::new(0, 0),
            CardinalDirection::East,
            SourcePlane::new(0).unwrap_or_else(|| panic!("plane")),
        );
        assert_eq!(placement.orientation.get(), 2);
        assert_eq!(
            compiled.stage_of(&MapOp::AddObject { placement }),
            BuildingStage::Circulation
        );
    }
    file.stairs
        .as_mut()
        .unwrap_or_else(|| panic!("stairs"))
        .up
        .kind = 0;
    assert!(matches!(
        file.compile(),
        Err(KitError::Value {
            what: "stair scenery kind",
            ..
        })
    ));
    file.stairs
        .as_mut()
        .unwrap_or_else(|| panic!("stairs"))
        .up
        .kind = 10;
    file.stairs
        .as_mut()
        .unwrap_or_else(|| panic!("stairs"))
        .down
        .orientations[0] = 4;
    assert!(matches!(
        file.compile(),
        Err(KitError::Value {
            what: "orientation",
            ..
        })
    ));
}

#[test]
fn a_single_room_request_builds_one_room_with_an_exterior_door_and_undoes_exactly() {
    let kit: KitFile = serde_json::from_str(include_str!(
        "../../../osrs-editor/src/app/prefab_runtime/kits/lumbridge-brick.kit.json"
    ))
    .unwrap_or_else(|error| panic!("{error:?}"));
    let kit = kit.compile().unwrap_or_else(|error| panic!("{error:?}"));
    let request = BuildingRequest {
        seed: 5,
        site: RectSite::new(MapTile::new(3200, 3200), 9, 8)
            .unwrap_or_else(|| panic!("valid fixture site")),
        base_plane: SourcePlane::new(0).unwrap_or_else(|| panic!("valid fixture plane")),
        floors: 1,
        archetype: BuildingArchetype::House,
        style: BuildingStyle::Generic,
        constraints: BuildingConstraints::default(),
    };
    let heights = (3198..=3212).flat_map(|x| {
        (3198..=3212).map(move |y| {
            (
                HeightVertexCoord::new(MapTile::new(x, y), request.base_plane),
                0,
            )
        })
    });
    let mut document = MapDocument::from_parts([], heights, []);
    let before = document.clone();
    let snapshot = crate::capture_generation_snapshot(&document, request, []);
    let patch = kit
        .generate_with_rooms(&snapshot, 0, Some(1))
        .unwrap_or_else(|error| panic!("{error:?}"));
    assert!(patch.ops().iter().any(|op| matches!(op,crate::MapOp::AddObject {placement} if placement.object_id.get()==1535 || placement.object_id.get()==1540)));
    document
        .apply_patch(&patch)
        .unwrap_or_else(|error| panic!("{error:?}"));
    document
        .apply_patch(&patch.inverse())
        .unwrap_or_else(|error| panic!("{error:?}"));
    assert_eq!(document, before);
}
