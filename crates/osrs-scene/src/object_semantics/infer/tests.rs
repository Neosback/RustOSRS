#![allow(clippy::expect_used)]
use super::*;
use crate::object_semantics::{Placed, Structure};

fn id(n: u32) -> ObjectId {
    ObjectId::new(n)
}

fn facts(n: u32, symbol: &str, slots: &[Slot], verbs: &[&str]) -> (ObjectId, ObjectFacts) {
    (
        id(n),
        ObjectFacts {
            id: id(n),
            name: None,
            symbol: Some(symbol.to_owned()),
            structure: Structure {
                slots: slots.iter().copied().collect(),
                size: (1, 1),
                verbs: verbs.iter().map(|v| (*v).to_owned()).collect(),
                animated: false,
                morphs: false,
                interact_type: 1,
            },
        },
    )
}

fn placed(n: u32, kind: u8, x: i32, y: i32, orientation: u8) -> Placed {
    Placed {
        id: id(n),
        kind,
        orientation,
        tile: (x, y),
        plane: 0,
    }
}

/// `count` separate buildings: a north run 100, 100, 102 (door), 100, 100 ending in corner 101.
fn buildings(count: i32) -> Vec<Placed> {
    let mut out = Vec::new();
    for b in 0..count {
        let y = 10 + b * 10;
        for (x, n) in [(0, 100), (1, 100), (2, 102), (3, 100), (4, 100)] {
            out.push(placed(n, 0, x, y, 1));
        }
        out.push(placed(101, 2, 5, y, 1));
    }
    out
}

fn world() -> BTreeMap<ObjectId, ObjectFacts> {
    [
        facts(100, "loc.castle_wall", &[Slot::Wall], &["examine"]),
        facts(101, "loc.00", &[Slot::WallCorner], &[]),
        facts(102, "loc.1to2", &[Slot::Wall], &["open"]),
        facts(103, "loc.moat_water", &[Slot::Prop], &[]),
    ]
    .into_iter()
    .collect()
}

fn suggestion(all: &[Suggestion], n: u32) -> &Suggestion {
    all.iter().find(|s| s.id == id(n)).expect("suggestion")
}

#[test]
fn opaque_corner_and_door_inherit_the_family_of_a_named_wall() {
    let mut census = Census::default();
    census.add(&buildings(3));
    let all = infer(&census, &world());

    let wall = suggestion(&all, 100);
    assert_eq!(wall.label_source, LabelSource::Given);
    assert_eq!(wall.label.as_deref(), Some("loc.castle_wall"));
    assert_eq!(wall.role, Some(Role::Straight));

    let corner = suggestion(&all, 101);
    assert_eq!(corner.role, Some(Role::Corner));
    assert_eq!(corner.label.as_deref(), Some("castle corner"));
    assert_eq!(corner.label_source, LabelSource::Inferred);
    assert!(corner.confidence > 0.4 && corner.confidence < 1.0);
    assert!(
        corner
            .evidence
            .iter()
            .any(|e| e.contains("corner between runs of 100"))
    );

    let door = suggestion(&all, 102);
    assert_eq!(
        door.role,
        Some(Role::Door),
        "an Open verb makes an inset a door"
    );
    assert_eq!(door.label.as_deref(), Some("castle door"));

    assert_eq!(wall.family, corner.family);
    assert_eq!(wall.family, door.family);
    assert_eq!(wall.family, Some(FamilyId(id(100))));
}

#[test]
fn an_object_with_no_relations_and_no_name_gets_no_label() {
    let mut census = Census::default();
    census.add(&buildings(3));
    let all = infer(&census, &world());
    let water = suggestion(&all, 103);
    assert_eq!(
        water.label.as_deref(),
        Some("loc.moat_water"),
        "given names are kept"
    );
    let mut unnamed = world();
    unnamed.insert(id(104), facts(104, "loc.77", &[Slot::Prop], &[]).1);
    let all = infer(&census, &unnamed);
    let lonely = suggestion(&all, 104);
    assert_eq!(
        (lonely.role, lonely.family, lonely.label.clone()),
        (None, None, None)
    );
    assert_eq!(lonely.confidence, 0.0);
}

#[test]
fn one_sighting_is_not_enough_to_join_a_family() {
    let mut census = Census::default();
    census.add(&buildings(1));
    let all = infer(&census, &world());
    // A single building gives each pair count 1 or 2; the corner/door are only seen once.
    let corner = suggestion(&all, 101);
    assert_ne!(
        corner.family,
        suggestion(&all, 100).family,
        "a single sighting is noise"
    );
    assert_eq!(corner.label, None);
}

#[test]
fn an_opening_without_a_door_verb_or_name_is_an_opening_and_a_window_name_makes_a_window() {
    let mut census = Census::default();
    census.add(&buildings(3));
    let mut w = world();
    w.insert(id(102), facts(102, "loc.00", &[Slot::Wall], &[]).1);
    assert_eq!(
        suggestion(&infer(&census, &w), 102).role,
        Some(Role::Opening)
    );
    w.insert(
        id(102),
        facts(102, "loc.round_window", &[Slot::Wall], &[]).1,
    );
    assert_eq!(
        suggestion(&infer(&census, &w), 102).role,
        Some(Role::Window)
    );
}

#[test]
fn a_piece_that_only_ever_closes_runs_is_an_end_cap() {
    let mut placements = Vec::new();
    for b in 0..3 {
        let y = 10 + b * 10;
        placements.push(placed(100, 0, 0, y, 1));
        placements.push(placed(100, 0, 1, y, 1));
        placements.push(placed(110, 0, 2, y, 3));
    }
    let mut census = Census::default();
    census.add(&placements);
    let mut w = world();
    w.insert(id(110), facts(110, "loc.99", &[Slot::Wall], &[]).1);
    let all = infer(&census, &w);
    assert_eq!(suggestion(&all, 110).role, Some(Role::EndCap));
}
