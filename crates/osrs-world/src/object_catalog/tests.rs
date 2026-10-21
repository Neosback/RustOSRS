use super::*;

#[test]
fn decoded_unlisted_definitions_have_dimensions_without_becoming_search_entries() {
    let mut catalog = catalog();
    catalog.unlisted_sizes.insert(123, (3, 2));
    assert_eq!(catalog.definition_size(123), Some((3, 2)));
    assert_eq!(catalog.definition_size(30), Some((2, 1)));
    assert_eq!(catalog.definition_size(124), None);
    assert!(catalog.get(123).is_none());
    assert!(catalog.search("123", 10).is_empty());
    let mut invalid = entry(125, "Oversized", &[10]);
    invalid.size_x = 256;
    assert_eq!(
        ObjectCatalog::from_entries(vec![invalid]).definition_size(125),
        None
    );
}

fn entry(id: u32, name: &str, loc_types: &[u8]) -> ObjectEntry {
    ObjectEntry {
        id,
        name: name.to_owned(),
        symbol: None,
        custom_name: None,
        custom_role: None,
        size_x: 2,
        size_y: 1,
        loc_types: loc_types.to_vec(),
        category: None,
        map_scene: None,
        map_icon: None,
        support_items: None,
        blocking_mask: 0,
    }
}

fn catalog() -> ObjectCatalog {
    let mut unnamed_wall = entry(99, "", &[0]);
    unnamed_wall.map_scene = Some(12);
    unnamed_wall.category = Some(207);
    ObjectCatalog::from_entries(vec![
        entry(30, "Oak tree", &[10]),
        entry(10, "Tree", &[10]),
        entry(20, "Dead tree", &[10]),
        entry(5, "Wooden door", &[0, 9]),
        entry(7, "Bank booth", &[10]),
        unnamed_wall,
    ])
}

#[test]
fn names_people_gave_win_in_lists_and_searches_and_can_be_filtered_by_role() {
    let mut catalog = catalog();
    catalog.apply_custom(|id| {
        if id == 99 {
            (Some("Brick wall".to_owned()), Some("wall".to_owned()))
        } else {
            (None, None)
        }
    });
    let found = catalog.search("brick", 10);
    assert_eq!(found.iter().map(|e| e.id).collect::<Vec<_>>(), vec![99]);
    assert_eq!(found[0].display_name(), "Brick wall");
    let by_role = catalog.search("role:wall", 10);
    assert_eq!(by_role.iter().map(|e| e.id).collect::<Vec<_>>(), vec![99]);
    // Once it is named it is no longer in the unnamed list; forgetting brings it back.
    assert!(catalog.search("unnamed", 10).iter().all(|e| e.id != 99));
    catalog.apply_custom(|_| (None, None));
    assert!(catalog.search("unnamed", 10).iter().any(|e| e.id == 99));
    assert!(catalog.search("brick", 10).is_empty());
}

#[test]
fn search_ranks_exact_id_then_prefix_then_contains() {
    let catalog = catalog();
    let names = |query: &str| -> Vec<String> {
        catalog
            .search(query, 10)
            .into_iter()
            .map(|e| e.name.clone())
            .collect()
    };
    assert_eq!(names("tree"), vec!["Tree", "Dead tree", "Oak tree"]);
    assert_eq!(names("10"), vec!["Tree"]);
    assert_eq!(names("wooden door"), vec!["Wooden door"]);
    assert_eq!(names("DOOR"), vec!["Wooden door"]);
    assert!(names("   ").is_empty());
    assert!(names("zzz").is_empty());
}

#[test]
fn search_respects_the_limit_and_id_lookup_does_not_duplicate() {
    let catalog = catalog();
    assert_eq!(catalog.search("tree", 2).len(), 2);
    assert_eq!(catalog.get(7).map(|e| e.name.as_str()), Some("Bank booth"));
    assert_eq!(catalog.get(8), None);
    // "5" matches id 5 by id; it must not also appear as a name hit.
    let hits = catalog.search("5", 10);
    assert_eq!(hits.iter().filter(|e| e.id == 5).count(), 1);
}

#[test]
fn default_loc_type_prefers_scenery_and_footprint_turns_with_orientation() {
    assert_eq!(entry(1, "A", &[0, 9, 10]).default_loc_type(), 10);
    assert_eq!(entry(1, "A", &[2, 9]).default_loc_type(), 2);
    let e = entry(1, "A", &[10]);
    assert_eq!(e.footprint(0), (2, 1));
    assert_eq!(e.footprint(1), (1, 2));
    assert_eq!(e.footprint(2), (2, 1));
}

#[test]
fn unnamed_architectural_entries_match_by_id_symbol_slot_and_metadata() {
    let mut catalog = catalog();
    assert_eq!(
        catalog.get(99).map(ObjectEntry::display_name),
        Some("Object 99".to_owned())
    );
    assert_eq!(
        catalog
            .search("99", 5)
            .into_iter()
            .map(|e| e.id)
            .collect::<Vec<_>>(),
        vec![99]
    );
    assert_eq!(
        catalog
            .search("slot:wall", 5)
            .into_iter()
            .map(|e| e.id)
            .collect::<Vec<_>>(),
        vec![5, 99]
    );
    assert_eq!(
        catalog
            .search("scene:12 category:207", 5)
            .into_iter()
            .map(|e| e.id)
            .collect::<Vec<_>>(),
        vec![99]
    );
    catalog.apply_symbols(|id| (id == 99).then(|| "stone wall straight".to_owned()));
    assert_eq!(
        catalog.get(99).map(ObjectEntry::display_name),
        Some("[stone wall straight]".to_owned())
    );
    assert_eq!(
        catalog
            .search("stone wall", 5)
            .into_iter()
            .map(|e| e.id)
            .collect::<Vec<_>>(),
        vec![99]
    );
}
