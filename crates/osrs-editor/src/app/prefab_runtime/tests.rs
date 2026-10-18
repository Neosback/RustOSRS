use super::*;
use osrs_core::coords::{MapTile, SourcePlane};
use osrs_edit::{MapDocument, PREFAB_FORMAT, PREFAB_VERSION, PrefabObject};

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("rustosrs-prefab-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn prefab(name: &str) -> Prefab {
    Prefab {
        building: None,
        format: PREFAB_FORMAT.into(),
        version: PREFAB_VERSION,
        name: name.into(),
        tags: vec!["house".into()],
        notes: String::new(),
        source: PrefabSource::Authored,
        width: 3,
        height: 3,
        planes: 1,
        tiles: Vec::new(),
        heights: Vec::new(),
        objects: vec![PrefabObject {
            id: 100,
            x: 1,
            y: 1,
            plane: 0,
            kind: 0,
            orientation: 1,
        }],
    }
}

#[test]
fn prefabs_survive_a_restart_and_keep_unique_files() {
    let dir = temp_dir("restart");
    let mut library = PrefabRuntime::in_directory(dir.clone());
    let first = library
        .add(prefab("Fisher's Hut"))
        .unwrap_or_else(|e| unreachable!("{e}"));
    let second = library
        .add(prefab("Fisher's Hut"))
        .unwrap_or_else(|e| unreachable!("{e}"));
    assert_ne!(first, second);
    assert_eq!(first, "fisher-s-hut.prefab.json");

    let reopened = PrefabRuntime::in_directory(dir.clone());
    assert_eq!(reopened.entries().len(), 2);
    assert_eq!(
        reopened.entry(&first).map(|e| e.prefab.clone()),
        Some(prefab("Fisher's Hut"))
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn edits_remove_and_duplicate_are_saved() {
    let dir = temp_dir("edit");
    let mut library = PrefabRuntime::in_directory(dir.clone());
    let file = library
        .add(prefab("Hut"))
        .unwrap_or_else(|e| unreachable!("{e}"));
    library
        .edit(&file, |p| {
            p.tags.push("fishing".into());
            p.notes = "by the pier".into();
        })
        .unwrap_or_else(|e| unreachable!("{e}"));
    let copy = library
        .duplicate(&file)
        .unwrap_or_else(|e| unreachable!("{e}"));
    assert_eq!(
        library.entry(&copy).map(|e| e.prefab.name.as_str()),
        Some("Hut copy")
    );

    let reopened = PrefabRuntime::in_directory(dir.clone());
    assert_eq!(
        reopened.entry(&file).map(|e| e.prefab.tags.clone()),
        Some(vec!["house".to_owned(), "fishing".to_owned()])
    );
    library
        .remove(&file)
        .unwrap_or_else(|e| unreachable!("{e}"));
    assert!(!dir.join(&file).exists());
    assert_eq!(PrefabRuntime::in_directory(dir.clone()).entries().len(), 1);
    // Renaming to nothing is refused and changes nothing.
    assert!(library.edit(&copy, |p| p.name.clear()).is_err());
    assert_eq!(
        library.entry(&copy).map(|e| e.prefab.name.as_str()),
        Some("Hut copy")
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_damaged_file_is_reported_and_the_rest_still_load() {
    let dir = temp_dir("damaged");
    let mut library = PrefabRuntime::in_directory(dir.clone());
    library
        .add(prefab("Good"))
        .unwrap_or_else(|e| unreachable!("{e}"));
    std::fs::write(dir.join("bad.prefab.json"), "{ not json")
        .unwrap_or_else(|e| unreachable!("{e}"));
    let reopened = PrefabRuntime::in_directory(dir.clone());
    assert_eq!(reopened.entries().len(), 1);
    assert!(
        reopened
            .notice()
            .is_some_and(|n| n.contains("bad.prefab.json"))
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn search_matches_name_tags_and_notes_and_sorts_by_name() {
    let mut library = PrefabRuntime::default();
    for name in ["Zeta hut", "Alpha tower"] {
        library
            .add(prefab(name))
            .unwrap_or_else(|e| unreachable!("{e}"));
    }
    library.filter = "house".into();
    let names: Vec<_> = library
        .visible()
        .iter()
        .map(|e| e.prefab.name.clone())
        .collect();
    assert_eq!(names, ["Alpha tower", "Zeta hut"]);
    library.filter = "tower".into();
    assert_eq!(library.visible().len(), 1);
    library.filter = "castle".into();
    assert!(library.visible().is_empty());
}

#[test]
fn import_and_export_round_trip_and_reject_foreign_files() {
    let mut library = PrefabRuntime::default();
    let file = library
        .add(prefab("Hut"))
        .unwrap_or_else(|e| unreachable!("{e}"));
    let (name, text) = library
        .export_text(&file)
        .unwrap_or_else(|| unreachable!("exports"));
    assert_eq!(name, "hut.prefab.json");
    let mut other = PrefabRuntime::default();
    let imported = other
        .import_text(&text)
        .unwrap_or_else(|e| unreachable!("{e}"));
    assert_eq!(
        other.entry(&imported).map(|e| e.prefab.clone()),
        Some(prefab("Hut"))
    );
    assert!(other.import_text("{\"format\": \"x\"}").is_err());
    assert!(
        other
            .import_text(&text.replace(&format!("\"version\": {PREFAB_VERSION}"), "\"version\": 7"))
            .is_err()
    );
    let blank = other
        .create_blank_scene()
        .unwrap_or_else(|e| unreachable!("{e}"));
    let blank_entry = other.entry(&blank).unwrap_or_else(|| unreachable!());
    assert_eq!(
        (blank_entry.prefab.width, blank_entry.prefab.height),
        (12, 12)
    );
    assert!(blank_entry.prefab.building.is_some());
}

#[test]
fn removing_the_armed_prefab_disarms_it() {
    let mut library = PrefabRuntime::default();
    let file = library
        .add(prefab("Hut"))
        .unwrap_or_else(|e| unreachable!("{e}"));
    library.arm(&file);
    if let Some(armed) = library.armed.as_mut() {
        armed.turns = 3;
    }
    library.arm(&file);
    assert_eq!(
        library.armed.as_ref().map(|a| a.turns),
        Some(3),
        "re-arming keeps the turn"
    );
    library
        .remove(&file)
        .unwrap_or_else(|e| unreachable!("{e}"));
    assert!(library.armed.is_none());
    assert!(library.selected.is_none());
}

#[test]
fn capture_names_itself_and_records_where_it_came_from() {
    let mut library = PrefabRuntime::default();
    let plane = SourcePlane::new(0).unwrap_or_else(|| unreachable!());
    let document = MapDocument::from_parts(
        [],
        [],
        [osrs_edit::ObjectPlacement {
            object_id: osrs_core::ids::ObjectId::new(5),
            position: osrs_edit::MapPosition::new(MapTile::new(3201, 3201), plane),
            kind: osrs_edit::LocKind::new(10).unwrap_or_else(|| unreachable!()),
            orientation: osrs_edit::ObjectOrientation::new(0).unwrap_or_else(|| unreachable!()),
        }],
    );
    let spec = CaptureSpec {
        origin: MapTile::new(3200, 3200),
        width: 4,
        height: 4,
        base_plane: plane,
        planes: 1,
        terrain: true,
        heights: true,
        objects: true,
    };
    let file = library
        .capture(&document, spec, "  ")
        .unwrap_or_else(|e| unreachable!("{e}"));
    let entry = library.entry(&file).unwrap_or_else(|| unreachable!());
    assert_eq!(entry.prefab.name, "Capture 4x4");
    assert_eq!(
        entry.prefab.source,
        PrefabSource::Captured {
            tile_x: 3200,
            tile_y: 3200,
            plane: 0
        }
    );
    assert_eq!(entry.prefab.objects.len(), 1);
    let empty = library.capture(&MapDocument::new(), spec, "x");
    assert!(
        empty.is_err(),
        "nothing to capture is an error, not an empty prefab"
    );
}

// ----------------------------------------------------------------------------------- generator

fn flat_ground(plane: SourcePlane, origin: (i32, i32), w: i32, h: i32) -> MapDocument {
    let mut heights = Vec::new();
    for y in -1..=h + 1 {
        for x in -1..=w + 1 {
            heights.push((
                osrs_edit::HeightVertexCoord::new(MapTile::new(origin.0 + x, origin.1 + y), plane),
                -64,
            ));
        }
    }
    MapDocument::from_parts([], heights, [])
}

#[test]
fn the_built_in_kit_plans_a_building_that_applies_undoes_and_saves() {
    let plane = SourcePlane::new(0).unwrap_or_else(|| unreachable!());
    let mut document = flat_ground(plane, (3300, 3300), 14, 10);
    let mut library = PrefabRuntime::default();
    assert_eq!(
        library.build.kits().len(),
        2,
        "the two built-in kits are there from the start"
    );
    assert!(library.build.kit_problems().is_empty());
    library.build.form.width = 14;
    library.build.form.height = 10;
    library.build.centre_site_on((3307, 3305), 0);
    library.build.refresh(&document, 1, &|_, _| (1, 1));
    assert!(library.build.error.is_none(), "{:?}", library.build.error);
    let preview = library
        .build
        .preview
        .clone()
        .unwrap_or_else(|| unreachable!("a preview"));
    assert!(preview.objects > 20, "walls, doors, windows and a roof");

    // The same inputs make the same building; a new seed makes another.
    library.build.refresh(&document, 1, &|_, _| (1, 1));
    let again = library
        .build
        .preview
        .clone()
        .unwrap_or_else(|| unreachable!());
    assert_eq!(again.apply, preview.apply);
    library.build.reroll();
    library.build.refresh(&document, 1, &|_, _| (1, 1));
    let other = library
        .build
        .preview
        .clone()
        .unwrap_or_else(|| unreachable!());
    assert_ne!(other.apply, preview.apply);

    // It applies to the document and undoes exactly.
    let original = document.clone();
    document
        .apply_patch(&preview.apply)
        .unwrap_or_else(|e| unreachable!("{e:?}"));
    assert!(document.object_count() > 20);
    document
        .apply_patch(&preview.apply.inverse())
        .unwrap_or_else(|e| unreachable!("{e:?}"));
    assert_eq!(document, original);

    // It can be kept in the library as a generated prefab and placed again elsewhere.
    library.build.refresh(&document, 1, &|_, _| (1, 1));
    let prefab = library
        .build
        .to_prefab()
        .unwrap_or_else(|e| unreachable!("{e}"));
    assert!(matches!(prefab.source, PrefabSource::Generated { .. }));
    assert!(prefab.tags.contains(&"generated".to_owned()));
    let file = library.add(prefab).unwrap_or_else(|e| unreachable!("{e}"));
    assert!(
        library
            .entry(&file)
            .is_some_and(|e| e.prefab.objects.len() >= 20)
    );
}

#[test]
fn a_site_without_map_data_says_why_and_clearing_the_site_removes_what_stands_there() {
    let plane = SourcePlane::new(0).unwrap_or_else(|| unreachable!());
    let mut library = PrefabRuntime::default();
    library.build.centre_site_on((3400, 3400), 0);
    library
        .build
        .refresh(&MapDocument::new(), 1, &|_, _| (1, 1));
    assert!(library.build.preview.is_none());
    assert!(library.build.error.is_some());

    let tree = osrs_edit::ObjectPlacement {
        object_id: osrs_core::ids::ObjectId::new(1276),
        position: osrs_edit::MapPosition::new(MapTile::new(3402, 3401), plane),
        kind: osrs_edit::LocKind::new(10).unwrap_or_else(|| unreachable!()),
        orientation: osrs_edit::ObjectOrientation::new(0).unwrap_or_else(|| unreachable!()),
    };
    let mut document = flat_ground(plane, (3394, 3395), 14, 10);
    document
        .apply_patch(&osrs_edit::MapPatch::new([osrs_edit::MapOp::AddObject {
            placement: tree,
        }]))
        .unwrap_or_else(|e| unreachable!("{e:?}"));
    library.build.refresh(&document, 2, &|_, _| (1, 1));
    let preview = library
        .build
        .preview
        .clone()
        .unwrap_or_else(|| unreachable!("{:?}", library.build.error));
    assert_eq!(preview.removed, 1);
    document
        .apply_patch(&preview.apply)
        .unwrap_or_else(|e| unreachable!("{e:?}"));
    assert!(!document.contains_object(tree));

    library.build.form.clear_site = false;
    let mut standing = flat_ground(plane, (3394, 3395), 14, 10);
    standing
        .apply_patch(&osrs_edit::MapPatch::new([osrs_edit::MapOp::AddObject {
            placement: tree,
        }]))
        .unwrap_or_else(|e| unreachable!("{e:?}"));
    library.build.refresh(&standing, 3, &|_, _| (1, 1));
    assert_eq!(library.build.preview.as_ref().map(|p| p.removed), Some(0));
}

#[test]
fn a_bad_kit_file_is_reported_and_the_built_in_kit_stays() {
    let mut library = PrefabRuntime::default();
    assert!(!library.build.add_kit_text("broken.kit.json", "{ nope"));
    assert_eq!(library.build.kits().len(), 2);
    assert!(library.build.kit_problems()[0].starts_with("broken.kit.json"));
}

#[test]
fn choosing_a_site_ends_the_gesture_and_centres_the_site_on_the_click() {
    let mut library = PrefabRuntime::default();
    library.build.form.width = 12;
    library.build.form.height = 8;
    library.build.choosing = true;
    library.build.centre_site_on((3300, 3300), 0);
    assert!(!library.build.choosing);
    assert_eq!(library.build.site, Some(MapTile::new(3294, 3296)));
    assert_eq!(
        library.build.site_rect(),
        Some((MapTile::new(3294, 3296), 12, 8))
    );
}

#[test]
fn applying_the_same_building_twice_leaves_what_stands_instead_of_failing() {
    let plane = SourcePlane::new(0).unwrap_or_else(|| unreachable!());
    let mut document = flat_ground(plane, (3500, 3500), 14, 10);
    let mut library = PrefabRuntime::default();
    library.build.centre_site_on((3507, 3505), 0);
    library.build.refresh(&document, 1, &|_, _| (1, 1));
    let first = library
        .build
        .preview
        .clone()
        .unwrap_or_else(|| unreachable!("{:?}", library.build.error));
    document
        .apply_patch(&first.apply)
        .unwrap_or_else(|e| unreachable!("{e:?}"));

    // Without clearing, everything already stands: the plan is valid and adds nothing new.
    library.build.form.clear_site = false;
    library.build.refresh(&document, 2, &|_, _| (1, 1));
    let again = library
        .build
        .preview
        .clone()
        .unwrap_or_else(|| unreachable!("{:?}", library.build.error));
    assert!(again.standing > 20, "the whole building is already there");
    document
        .apply_patch(&again.apply)
        .unwrap_or_else(|e| unreachable!("{e:?}"));

    // With clearing, the same building replaces itself.
    library.build.form.clear_site = true;
    library.build.refresh(&document, 3, &|_, _| (1, 1));
    let replaced = library
        .build
        .preview
        .clone()
        .unwrap_or_else(|| unreachable!("{:?}", library.build.error));
    assert!(replaced.removed > 20);
    document
        .apply_patch(&replaced.apply)
        .unwrap_or_else(|e| unreachable!("{e:?}"));
}

#[test]
fn detached_save_refuses_a_newer_library_edit_and_preserves_both_versions() {
    let dir = temp_dir("detached-conflict");
    let mut world = PrefabRuntime::in_directory(dir.clone());
    let original = prefab("test");
    let file = world
        .add(original.clone())
        .unwrap_or_else(|e| panic!("{e}"));
    let mut other = PrefabRuntime::in_directory(dir.clone());
    other
        .edit(&file, |p| p.notes = "world changed this".into())
        .unwrap_or_else(|e| panic!("{e}"));
    let mut edited = original.clone();
    edited.name = "Studio work".into();
    assert!(
        world
            .replace_if_unchanged(&file, &original, edited.clone())
            .is_err()
    );
    assert_eq!(
        world.entry(&file).map(|e| e.prefab.notes.as_str()),
        Some("world changed this")
    );
    let recovered = world.add(edited).unwrap_or_else(|e| panic!("{e}"));
    assert_ne!(recovered, file);
    assert_eq!(world.entries().len(), 2);
    let _ = std::fs::remove_dir_all(dir);
}
