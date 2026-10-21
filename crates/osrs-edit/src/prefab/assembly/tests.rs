use super::*;
use crate::EditHistory;

fn prefab() -> Prefab {
    let mut p = Prefab {
        format: PREFAB_FORMAT.into(),
        version: PREFAB_VERSION,
        name: "Furniture".into(),
        tags: vec![],
        notes: String::new(),
        source: PrefabSource::Captured {
            tile_x: -100,
            tile_y: 200,
            plane: 0,
        },
        width: 6,
        height: 6,
        planes: 2,
        tiles: vec![PrefabTile {
            x: 0,
            y: 0,
            plane: 0,
            underlay: Some(3),
            overlay: None,
            flags: 0,
        }],
        heights: [(0, 0), (0, 1), (1, 0), (1, 1)]
            .into_iter()
            .map(|(x, y)| PrefabHeight {
                x,
                y,
                plane: 0,
                value: 0,
            })
            .collect(),
        objects: vec![
            PrefabObject {
                id: 10,
                x: 1,
                y: 1,
                plane: 0,
                kind: 10,
                orientation: 1,
            },
            PrefabObject {
                id: 11,
                x: 2,
                y: 2,
                plane: 1,
                kind: 10,
                orientation: 0,
            },
            PrefabObject {
                id: 12,
                x: 5,
                y: 5,
                plane: 0,
                kind: 4,
                orientation: 0,
            },
        ],
        building: Some(PrefabBuilding {
            semantics: Default::default(),
            floors: vec![PrefabCell {
                x: 0,
                y: 0,
                plane: 0,
            }],
            support: [(0, 0), (0, 1), (1, 0), (1, 1)]
                .into_iter()
                .map(|(x, y)| PrefabCell { x, y, plane: 0 })
                .collect(),
            source_datum: 0,
            object_sizes: BTreeMap::from([(10, (2, 1)), (11, (2, 1)), (12, (1, 1))]),
            provenance: "pinned cache".into(),
            relations: vec![],
            reviewed: true,
            labels: BTreeMap::new(),
        }),
    };
    p.link_assembly(&[1, 0]).unwrap_or_else(|e| panic!("{e}"));
    p
}

#[test]
fn only_explicit_human_assembly_links_form_sorted_groups_and_invalid_linking_is_atomic() {
    let mut p = prefab();
    let before = p.clone();
    p.link_assembly(&[0, 1, 1])
        .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(p, before);
    assert!(p.link_assembly(&[0, 9]).is_err());
    assert_eq!(p, before);
    let b = p.building.as_mut().unwrap_or_else(|| panic!("building"));
    b.relations.push(PrefabRelation {
        from: 1,
        to: 2,
        kind: "attached".into(),
        human: true,
        evidence: "human".into(),
    });
    b.relations.push(PrefabRelation {
        from: 0,
        to: 2,
        kind: "assembly".into(),
        human: false,
        evidence: "guess".into(),
    });
    assert_eq!(
        p.reviewed_assemblies().unwrap_or_else(|e| panic!("{e}")),
        [vec![0, 1]]
    );
    p.link_assembly(&[1, 2]).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(
        p.reviewed_assemblies().unwrap_or_else(|e| panic!("{e}")),
        [vec![0, 1, 2]]
    );
}

#[test]
fn extraction_keeps_complete_occurrences_and_rotated_footprints_but_no_ground_or_unrelated_objects()
{
    let p = prefab();
    let before = p.clone();
    let extracted = p
        .extract_assembly(1, "Counter")
        .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(p, before);
    assert_eq!(
        (extracted.width, extracted.height, extracted.planes),
        (3, 2, 2)
    );
    assert!(
        extracted.tiles.is_empty() && extracted.heights.is_empty() && extracted.building.is_none()
    );
    assert_eq!(
        extracted.objects.iter().map(|o| o.id).collect::<Vec<_>>(),
        [10, 11]
    );
    assert_eq!(
        (
            extracted.objects[0].x,
            extracted.objects[0].y,
            extracted.objects[1].x,
            extracted.objects[1].y
        ),
        (0, 0, 1, 1)
    );
    assert_eq!(
        extracted.source,
        PrefabSource::Captured {
            tile_x: -99,
            tile_y: 201,
            plane: 0
        }
    );
    let text = serde_json::to_string(&extracted).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(
        serde_json::from_str::<Prefab>(&text).unwrap_or_else(|e| panic!("{e}")),
        extracted
    );
    assert_eq!(
        p.extract_assembly(2, "Other"),
        Err(PrefabError::NoReviewedAssembly { member: 2 })
    );
}

#[test]
fn extracted_assemblies_rotate_through_canonical_placement_as_one_undoable_patch() {
    let p = prefab()
        .extract_assembly(0, "Counter")
        .unwrap_or_else(|e| panic!("{e}"));
    for turns in 0..4 {
        let mut document = MapDocument::default();
        let before = document.clone();
        let placed = try_plan_place_prefab(
            &document,
            &p,
            PlaceSpec {
                origin: MapTile::new(-50, -80),
                base_plane: SourcePlane::new(0).unwrap_or_else(|| panic!("plane")),
                quarter_turns: turns,
                terrain: true,
                heights: true,
                objects: true,
                replace_objects: false,
            },
            &|_, _| Ok((2, 1)),
        )
        .unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(placed.patch.ops().len(), 2);
        let mut history = EditHistory::new();
        assert_eq!(history.undo_len(), 0); // planning/canceling has no history effect
        history
            .commit(&mut document, placed.patch)
            .unwrap_or_else(|e| panic!("{e:?}"));
        assert_eq!(history.undo_len(), 1);
        let committed = document.clone();
        for o in document.object_entries() {
            assert!((-50..-50 + i32::from(placed.width)).contains(&o.position.tile.x));
            assert!((-80..-80 + i32::from(placed.height)).contains(&o.position.tile.y));
        }
        history
            .undo(&mut document)
            .unwrap_or_else(|e| panic!("{e:?}"));
        assert_eq!(document, before);
        history
            .redo(&mut document)
            .unwrap_or_else(|e| panic!("{e:?}"));
        assert_eq!(document, committed);
    }
}

#[test]
fn unreviewed_membership_and_unrepresentable_extraction_are_rejected_without_changes() {
    let mut p = prefab();
    p.building
        .as_mut()
        .unwrap_or_else(|| panic!("building"))
        .reviewed = false;
    assert!(matches!(
        p.extract_assembly(0, "Counter"),
        Err(PrefabError::Building(BuildingPrefabError::Unreviewed))
    ));
    p.building
        .as_mut()
        .unwrap_or_else(|| panic!("building"))
        .reviewed = true;
    p.building
        .as_mut()
        .unwrap_or_else(|| panic!("building"))
        .object_sizes
        .insert(11, (255, 255));
    let before = p.clone();
    assert!(matches!(
        p.extract_assembly(0, "Huge"),
        Err(PrefabError::BadSize { .. })
    ));
    assert_eq!(p, before);
}
