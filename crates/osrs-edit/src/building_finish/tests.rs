use super::*;
use crate::{
    BuildingArchetype, BuildingConstraints, BuildingRequest, BuildingStyle, HeightVertexCoord,
    KitAsset, KitFile, KitFloor, KitHipRoof, MapDocument, MapOp, RectSite,
    capture_generation_snapshot,
};
use osrs_core::coords::SourcePlane;

fn asset(id: u32) -> KitAsset {
    KitAsset {
        id,
        kind: 0,
        orientations: [1, 2, 3, 0],
    }
}

fn finish() -> KitFinish {
    let floor = |overlay: u32| KitFloorFile {
        underlay: None,
        overlay: Some((overlay, 0, 0)),
    };
    let rule = |id: u32, place, relative, min, max, size| KitFurniture {
        id,
        kind: 10,
        place,
        relative,
        min,
        max,
        size,
    };
    KitFinish {
        learned_roles: None,
        floors: BTreeMap::from([
            ("default".to_owned(), floor(5)),
            ("hallway".to_owned(), floor(81)),
            ("lobby".to_owned(), floor(171)),
        ]),
        window_decor: Some(KitWindowDecor {
            outside: Some(KitDecor {
                id: 1821,
                kind: 4,
                weight: 1,
            }),
            inside: Some(KitDecor {
                id: 1822,
                kind: 4,
                weight: 1,
            }),
        }),
        wall_decor: vec![KitDecor {
            id: 196,
            kind: 4,
            weight: 1,
        }],
        wall_decor_percent: 30,
        furniture: BTreeMap::from([
            (
                "lobby".to_owned(),
                vec![
                    rule(10355, KitPlace::AgainstWall, 0, 3, 5, [1, 1]),
                    rule(612, KitPlace::AgainstWall, 1, 1, 2, [2, 1]),
                    rule(604, KitPlace::Center, 0, 1, 2, [1, 1]),
                ],
            ),
            (
                "living".to_owned(),
                vec![rule(422, KitPlace::AgainstWall, 3, 1, 2, [1, 2])],
            ),
        ]),
        grade: true,
    }
}

fn kit() -> KitFile {
    KitFile {
        format: crate::KIT_FORMAT.into(),
        version: crate::KIT_VERSION,
        name: "finish test".into(),
        notes: String::new(),
        floor: KitFloor {
            underlay: None,
            overlay: None,
        },
        exterior_wall: asset(1902),
        interior_wall: asset(1902),
        door: asset(1535),
        window: Some(asset(1902)),
        corner: None,
        door_variants: Vec::new(),
        roof_plane_offset: 1,
        roof_hip: Some(KitHipRoof {
            slope_object: 1924,
            edge_object: 1790,
        }),
        finish: Some(finish()),
        roof: None,
    }
}

fn site(
    archetype: BuildingArchetype,
    seed: u64,
    w: u16,
    h: u16,
) -> (MapDocument, crate::GenerationSnapshot) {
    let plane = SourcePlane::new(0).unwrap_or_else(|| unreachable!());
    let request = BuildingRequest {
        seed,
        site: RectSite::new(MapTile::new(3000, 3000), w, h).unwrap_or_else(|| unreachable!()),
        base_plane: plane,
        floors: 1,
        archetype,
        style: BuildingStyle::Generic,
        constraints: BuildingConstraints::default(),
    };
    // A slope, so grading has something to do.
    let mut heights = Vec::new();
    for y in 0..=i32::from(h) {
        for x in 0..=i32::from(w) {
            heights.push((
                HeightVertexCoord::new(MapTile::new(3000 + x, 3000 + y), plane),
                -240 + x * 8 + y * 16,
            ));
        }
    }
    let document = MapDocument::from_parts([], heights, []);
    let snapshot = capture_generation_snapshot(&document, request, []);
    (document, snapshot)
}

fn objects(patch: &MapPatch) -> Vec<ObjectPlacement> {
    patch
        .ops()
        .iter()
        .filter_map(|op| match op {
            MapOp::AddObject { placement } => Some(*placement),
            _ => None,
        })
        .collect()
}

#[test]
fn a_finished_bank_has_its_floors_windows_decor_and_furniture_and_applies_exactly() {
    let compiled = kit().compile().unwrap_or_else(|e| unreachable!("{e}"));
    let (mut document, snapshot) = site(BuildingArchetype::Bank, 7, 22, 16);
    let patch = compiled
        .generate(&snapshot, 1)
        .unwrap_or_else(|e| unreachable!("{e:?}"));
    let placed = objects(&patch);
    let ids = |id: u32| placed.iter().filter(|p| p.object_id.get() == id).count();

    // Floors by role: stone in the lobby, dark wood in the halls, wood elsewhere.
    let overlays: std::collections::BTreeSet<u32> = patch
        .ops()
        .iter()
        .filter_map(|op| match op {
            MapOp::SetTerrain { after, .. } => after.overlay.map(|o| o.id.get()),
            _ => None,
        })
        .collect();
    assert!(
        overlays.contains(&171),
        "the bank lobby is stone: {overlays:?}"
    );
    assert!(overlays.is_subset(&[5, 81, 171].into()));

    // Windows hang on both faces: as many outside as inside.
    assert!(ids(1821) > 0);
    assert_eq!(ids(1821), ids(1822));
    // Furniture and wall decoration are there.
    assert!(ids(10355) >= 3, "bank booths line the lobby");
    assert!(ids(196) > 0, "torches on the walls");

    // The roof overhangs and sits on plane 1.
    assert!(
        placed
            .iter()
            .any(|p| (12..=21).contains(&p.kind.get()) && p.position.plane.index().get() == 1)
    );

    // It applies and undoes exactly, heights included.
    let original = document.clone();
    document
        .apply_patch(&patch)
        .unwrap_or_else(|e| unreachable!("{e:?}"));
    document
        .apply_patch(&patch.inverse())
        .unwrap_or_else(|e| unreachable!("{e:?}"));
    assert_eq!(document, original);
}

#[test]
fn window_decoration_stands_on_the_right_tile_facing_the_wall() {
    let compiled = kit().compile().unwrap_or_else(|e| unreachable!("{e}"));
    let (_, snapshot) = site(BuildingArchetype::House, 3, 18, 14);
    let patch = compiled
        .generate(&snapshot, 0)
        .unwrap_or_else(|e| unreachable!("{e:?}"));
    let placed = objects(&patch);
    let step = |o: u8| match o & 3 {
        0 => (-1, 0),
        1 => (0, 1),
        2 => (1, 0),
        _ => (0, -1),
    };
    // A wall is an edge: either tile beside it may hold the piece.
    let mut walls: HashSet<(MapTile, u8)> = HashSet::new();
    for p in placed
        .iter()
        .filter(|p| p.object_id.get() == 1902 && p.kind.get() == 0)
    {
        let o = p.orientation.get();
        walls.insert((p.position.tile, o));
        let (dx, dy) = step(o);
        walls.insert((
            MapTile::new(p.position.tile.x + dx, p.position.tile.y + dy),
            (o + 2) & 3,
        ));
    }
    let mut checked = 0;
    for inside in placed.iter().filter(|p| p.object_id.get() == 1822) {
        let o = inside.orientation.get();
        // A wall stands on the very edge the inside decoration is attached to...
        assert!(
            walls.contains(&(inside.position.tile, o)),
            "inside window decoration has a wall behind it"
        );
        // ...and the outside decoration is on the other side of that wall, facing back at it.
        let (dx, dy) = step(o);
        let out_tile = MapTile::new(inside.position.tile.x + dx, inside.position.tile.y + dy);
        let outside = placed.iter().find(|p| {
            p.object_id.get() == 1821
                && p.position.tile == out_tile
                && p.orientation.get() == (o + 2) & 3
        });
        assert!(
            outside.is_some(),
            "outside decoration faces the same wall from the other tile"
        );
        checked += 1;
    }
    assert!(checked > 0);
}

#[test]
fn furniture_stays_inside_rooms_clear_of_doors_and_never_overlaps() {
    let compiled = kit().compile().unwrap_or_else(|e| unreachable!("{e}"));
    for seed in 0..12u64 {
        let (_, snapshot) = site(BuildingArchetype::Bank, seed, 24, 18);
        let patch = compiled
            .generate(&snapshot, 0)
            .unwrap_or_else(|e| unreachable!("{e:?}"));
        let placed = objects(&patch);
        let doors: Vec<_> = placed
            .iter()
            .filter(|p| p.object_id.get() == 1535)
            .collect();
        let mut covered: HashSet<MapTile> = HashSet::new();
        for piece in placed.iter().filter(|p| p.kind.get() == 10) {
            let size = match piece.object_id.get() {
                612 => (2, 1),
                422 => (1, 2),
                _ => (1, 1),
            };
            let (w, h) = if piece.orientation.get() & 1 == 1 {
                (size.1, size.0)
            } else {
                size
            };
            for dx in 0..w {
                for dy in 0..h {
                    let tile = MapTile::new(piece.position.tile.x + dx, piece.position.tile.y + dy);
                    assert!(covered.insert(tile), "seed {seed}: two pieces on {tile:?}");
                    for door in &doors {
                        assert_ne!(
                            door.position.tile, tile,
                            "seed {seed}: furniture in a doorway"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn grading_levels_the_building_and_leaves_the_site_edge_alone() {
    let compiled = kit().compile().unwrap_or_else(|e| unreachable!("{e}"));
    let (_, snapshot) = site(BuildingArchetype::House, 5, 20, 16);
    let patch = compiled
        .generate(&snapshot, 0)
        .unwrap_or_else(|e| unreachable!("{e:?}"));
    let mut after: HashMap<MapTile, i32> = HashMap::new();
    for op in patch.ops() {
        if let MapOp::SetHeight {
            coord,
            after: a,
            before,
        } = op
        {
            assert_eq!(a % 8, 0, "heights stay in steps of 8");
            assert_ne!(*a, *before);
            after.insert(coord.corner, *a);
        }
    }
    assert!(!after.is_empty(), "a slope gets graded");
    // The vertices on the site's own boundary are never touched.
    for tile in after.keys() {
        let on_edge = tile.x == 3000 || tile.x == 3020 || tile.y == 3000 || tile.y == 3016;
        assert!(!on_edge, "{tile:?} is on the site edge");
    }
    // Under the building: one level.
    let under: std::collections::BTreeSet<i32> = (3003..=3008)
        .flat_map(|x| (3003..=3008).map(move |y| (x, y)))
        .filter_map(|(x, y)| after.get(&MapTile::new(x, y)).copied())
        .collect();
    assert!(under.len() <= 1, "flat under the footprint: {under:?}");
}

#[test]
fn a_kit_with_unknown_roles_or_bad_kinds_is_refused() {
    let mut file = kit();
    if let Some(f) = &mut file.finish {
        f.floors.insert(
            "garden_shed".into(),
            KitFloorFile {
                underlay: Some(1),
                overlay: None,
            },
        );
    }
    assert!(file.compile().is_err());
    let mut file = kit();
    if let Some(f) = &mut file.finish {
        f.wall_decor[0].kind = 40;
    }
    assert!(file.compile().is_err());
    let mut file = kit();
    if let Some(f) = &mut file.finish
        && let Some(rules) = f.furniture.get_mut("lobby")
    {
        rules[0].min = 9;
    }
    assert!(file.compile().is_err());
}

#[test]
fn a_row_of_booths_stands_side_by_side_against_one_wall_all_facing_the_same_way() {
    let mut file = kit();
    if let Some(f) = &mut file.finish
        && let Some(rules) = f.furniture.get_mut("lobby")
    {
        rules[0].place = KitPlace::Row;
        rules[0].min = 4;
        rules[0].max = 4;
    }
    let compiled = file.compile().unwrap_or_else(|e| unreachable!("{e}"));
    let mut rows = 0;
    for seed in 0..10u64 {
        let (_, snapshot) = site(BuildingArchetype::Bank, seed, 24, 18);
        let patch = compiled
            .generate(&snapshot, 0)
            .unwrap_or_else(|e| unreachable!("{e:?}"));
        let booths: Vec<_> = objects(&patch)
            .into_iter()
            .filter(|p| p.object_id.get() == 10355)
            .collect();
        if booths.is_empty() {
            continue;
        }
        rows += 1;
        assert_eq!(booths.len(), 4, "seed {seed}: the whole row or none of it");
        let orientation = booths[0].orientation.get();
        assert!(
            booths.iter().all(|b| b.orientation.get() == orientation),
            "one facing"
        );
        // Consecutive along one axis: same row or column, neighbouring tiles.
        let same_x = booths
            .iter()
            .all(|b| b.position.tile.x == booths[0].position.tile.x);
        let same_y = booths
            .iter()
            .all(|b| b.position.tile.y == booths[0].position.tile.y);
        assert!(same_x || same_y, "seed {seed}: in a line");
        let mut along: Vec<i32> = booths
            .iter()
            .map(|b| {
                if same_x {
                    b.position.tile.y
                } else {
                    b.position.tile.x
                }
            })
            .collect();
        along.sort_unstable();
        assert!(
            along.windows(2).all(|w| w[1] - w[0] == 1),
            "seed {seed}: touching: {along:?}"
        );
    }
    assert!(rows >= 5, "most seeds manage a row of four");
}

/// Every side of a floor tile that faces the outside must have a wall piece, door or window on it,
/// and no wall piece may stand where two floor tiles meet unless it is a real divider.
fn assert_closed(patch: &MapPatch, label: &str) {
    let floor: HashSet<MapTile> = patch
        .ops()
        .iter()
        .filter_map(|op| match op {
            MapOp::SetTerrain { coord, .. } => Some(coord.tile),
            _ => None,
        })
        .collect();
    assert!(!floor.is_empty(), "{label}: no floor");
    let step = |direction: u8| match direction & 3 {
        0 => (-1, 0),
        1 => (0, 1),
        2 => (1, 0),
        _ => (0, -1),
    };
    let mut edges: HashSet<(MapTile, u8)> = HashSet::new();
    for piece in objects(patch).iter().filter(|p| p.kind.get() == 0) {
        let o = piece.orientation.get();
        edges.insert((piece.position.tile, o));
        let (dx, dy) = step(o);
        edges.insert((
            MapTile::new(piece.position.tile.x + dx, piece.position.tile.y + dy),
            (o + 2) & 3,
        ));
    }
    let mut leaks = Vec::new();
    for tile in &floor {
        for side in 0..4u8 {
            let (dx, dy) = step(side);
            let next = MapTile::new(tile.x + dx, tile.y + dy);
            if !floor.contains(&next) && !edges.contains(&(*tile, side)) {
                leaks.push((tile.x, tile.y, side));
            }
        }
    }
    leaks.sort_unstable();
    assert!(
        leaks.is_empty(),
        "{label}: open outside edges (x, y, side): {leaks:?}"
    );
}

#[test]
fn every_generated_building_is_closed_on_the_outside() {
    let compiled = kit().compile().unwrap_or_else(|e| unreachable!("{e}"));
    let mut built = 0;
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
        for (w, h) in [(14u16, 12u16), (18, 14), (22, 16), (30, 20)] {
            for seed in 0..12u64 {
                let (_, snapshot) = site(archetype, seed, w, h);
                let Ok(patch) = compiled.generate(&snapshot, (seed % 3) as u32) else {
                    continue;
                };
                built += 1;
                assert_closed(&patch, &format!("{archetype:?} {w}x{h} seed {seed}"));
            }
        }
    }
    assert!(built > 200, "most requests produce a building ({built})");
}
