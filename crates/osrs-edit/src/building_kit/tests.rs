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
        orientations: [0, 1, 2, 3],
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
        roof: KitRoof {
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
        },
    }
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
    for pattern in &mut real.roof.patterns {
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
    file.version = 2;
    assert!(matches!(file.compile(), Err(KitError::Version(2))));

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
    file.roof.ridge = vec![0, 99];
    assert!(matches!(file.compile(), Err(KitError::Roof(_))));

    let mut file = kit();
    file.roof.junctions[0].requires = vec![("sideways".into(), "north".into())];
    assert!(matches!(file.compile(), Err(KitError::Roof(_))));

    let mut file = kit();
    file.roof.patterns.clear();
    assert!(matches!(file.compile(), Err(KitError::Roof(_))));

    let text = serde_json::to_string(&kit())
        .unwrap_or_else(|e| unreachable!("{e}"))
        .replace("\"notes\"", "\"surprise\":1,\"notes\"");
    assert!(
        serde_json::from_str::<KitFile>(&text).is_err(),
        "unknown fields are rejected"
    );
}
