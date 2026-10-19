//! Learning a style from saved buildings: which wall, door, window and roof pieces they use, what
//! the floors of each kind of room are made of, and what stands in those rooms and where.
//!
//! The result is an ordinary building kit (the same file the Build page uses), so everything the
//! generator guarantees (a closed shell, a way in, reachable rooms) still holds for a building
//! built in the learned style. Nothing here copies a building: a style is counts and rules.

use crate::task::Snapshot;
use osrs_edit::{
    KitAsset, KitDecor, KitFile, KitFloorFile, KitFurniture, KitHipRoof, KitPlace,
    KitWeightedAsset, KitWindowDecor, Prefab,
};
use osrs_scene::building_census::{
    Building, Dir, EdgeFeature, Material, OPENING_OBJECT, ObjectTable, RoomTag,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// What was learned, in numbers a person can read.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StyleReport {
    pub buildings: usize,
    pub rooms: usize,
    pub furniture_rules: usize,
    pub floor_rules: usize,
    pub wall_decorations: usize,
    /// Pieces of the kit that came from the saved buildings.
    pub learned: Vec<String>,
    /// Pieces the saved buildings did not show, so the starting kit's are kept.
    pub kept: Vec<String>,
    /// Objects in the buildings that have no name, with how many of them there are.
    pub unnamed: BTreeMap<u32, u32>,
}

/// The kit roles a kind of room stands for.
fn roles_of(tag: RoomTag) -> &'static [&'static str] {
    match tag {
        RoomTag::Bank => &["lobby", "vault", "security"],
        RoomTag::Kitchen => &["kitchen"],
        RoomTag::Bedroom => &["bedroom", "guest_room"],
        RoomTag::Church => &["nave", "altar", "vestry"],
        RoomTag::Shop => &["sales_floor"],
        RoomTag::Tavern => &["common_room", "bar"],
        RoomTag::Study => &["office"],
        RoomTag::Smithy => &["workshop"],
        RoomTag::Storage => &["storage", "loading"],
        RoomTag::Armory => &["armory", "guard_room"],
        RoomTag::Cell => &["cell"],
        RoomTag::Living => &["living"],
        RoomTag::General => &["living", "utility", "bathroom"],
    }
}

fn top<K: Clone + Ord>(counts: &BTreeMap<K, u32>) -> Option<K> {
    counts
        .iter()
        .max_by(|a, b| a.1.cmp(b.1).then(b.0.cmp(a.0)))
        .map(|(k, _)| k.clone())
}

fn bump<K: Ord>(map: &mut BTreeMap<K, u32>, key: K) {
    *map.entry(key).or_default() += 1;
}

/// The quarter turn between the wall a piece stands against and its own orientation.
fn relative_to(against: Dir, orientation: u8) -> u8 {
    let side = match against {
        Dir::West => 0,
        Dir::North => 1,
        Dir::East => 2,
        Dir::South => 3,
    };
    (orientation + 4 - side) & 3
}

fn floor_of(material: &Material) -> KitFloorFile {
    if material.overlay != 0 {
        KitFloorFile {
            underlay: None,
            overlay: Some((u32::from(material.overlay), material.shape, 0)),
        }
    } else {
        KitFloorFile {
            underlay: Some(u32::from(material.underlay)),
            overlay: None,
        }
    }
}

#[derive(Default)]
struct Rule {
    seen: u32,
    rooms: BTreeMap<usize, u32>,
    against: u32,
    relative: BTreeMap<u8, u32>,
    orientation: BTreeMap<u8, u32>,
}

/// Learn a style from `buildings` (found by the census in the saved prefabs) on top of `base`.
pub fn learn_style(
    base: &KitFile,
    name: &str,
    buildings: &[Building],
    objects: &ObjectTable,
) -> Result<(KitFile, StyleReport), String> {
    if buildings.is_empty() {
        return Err("no building with a closed room was found in the saved prefabs".into());
    }
    let mut kit = base.clone();
    kit.name = name.to_owned();
    let mut report = StyleReport {
        buildings: buildings.len(),
        ..StyleReport::default()
    };
    let mut finish = kit.finish.take().unwrap_or_default();

    // Walls, doors and windows.
    let (mut exterior, mut interior) = (BTreeMap::new(), BTreeMap::new());
    let (mut doors, mut windows) = (BTreeMap::new(), BTreeMap::new());
    for wall in buildings.iter().flat_map(|b| &b.walls) {
        let piece = wall.piece;
        if piece.object == OPENING_OBJECT {
            continue;
        }
        let id = piece.object.get();
        match piece.feature {
            EdgeFeature::Wall if wall.exterior => bump(&mut exterior, id),
            EdgeFeature::Wall => bump(&mut interior, id),
            EdgeFeature::Door => bump(&mut doors, id),
            EdgeFeature::Window if wall.exterior => bump(&mut windows, id),
            EdgeFeature::Window | EdgeFeature::Opening => {}
        }
    }
    let with_id = |asset: &KitAsset, id: u32| KitAsset {
        id,
        ..asset.clone()
    };
    if let Some(id) = top(&exterior) {
        kit.exterior_wall = with_id(&kit.exterior_wall, id);
        report.learned.push("outside walls".into());
        // Inside walls are usually the same piece as the outside ones.
        kit.interior_wall = with_id(&kit.interior_wall, top(&interior).unwrap_or(id));
    } else {
        report.kept.push("outside walls".into());
    }
    if let Some(id) = top(&doors) {
        kit.door = with_id(&kit.door, id);
        kit.door_variants = doors
            .iter()
            .filter(|(other, _)| **other != id)
            .map(|(other, count)| KitWeightedAsset {
                asset: with_id(&kit.door, *other),
                weight: *count,
            })
            .collect();
        report.learned.push("doors".into());
    } else {
        report.kept.push("doors".into());
    }
    if let Some(id) = top(&windows) {
        let template = kit
            .window
            .clone()
            .unwrap_or_else(|| kit.exterior_wall.clone());
        kit.window = Some(with_id(&template, id));
        report.learned.push("windows".into());
    }

    // Roofs.
    let (mut slopes, mut edges) = (BTreeMap::new(), BTreeMap::new());
    for ((object, kind), count) in buildings.iter().flat_map(|b| &b.roof) {
        match kind {
            12 | 15 | 16 | 17 => *slopes.entry(object.get()).or_default() += count,
            18..=20 => *edges.entry(object.get()).or_default() += count,
            _ => {}
        }
    }
    if let (Some(slope), Some(edge)) = (top(&slopes), top(&edges)) {
        kit.roof_hip = Some(KitHipRoof {
            slope_object: slope,
            edge_object: edge,
        });
        report.learned.push("roof".into());
    } else {
        report.kept.push("roof".into());
    }

    // Floors by room kind, and what hangs on the walls.
    let mut floors: BTreeMap<RoomTag, BTreeMap<Material, u32>> = BTreeMap::new();
    let mut everywhere: BTreeMap<Material, u32> = BTreeMap::new();
    let mut window_inside: BTreeMap<u32, u32> = BTreeMap::new();
    let mut window_outside: BTreeMap<u32, u32> = BTreeMap::new();
    let mut hung: BTreeMap<(u32, u8), u32> = BTreeMap::new();
    let (mut rooms, mut decorations, mut interior_edges) = (0, 0_u32, 0_u32);
    let window_ids: Vec<u32> = windows.keys().copied().collect();
    for building in buildings {
        interior_edges += building.walls.iter().filter(|w| !w.exterior).count() as u32;
        for room in &building.rooms {
            rooms += 1;
            for (material, count) in &room.floor {
                *floors
                    .entry(room.tag)
                    .or_default()
                    .entry(*material)
                    .or_default() += count;
                *everywhere.entry(*material).or_default() += count;
            }
            for decoration in &room.decorations {
                let id = decoration.object.get();
                let on_window = decoration
                    .on
                    .is_some_and(|piece| window_ids.contains(&piece.get()));
                if on_window {
                    let side = if decoration.inside {
                        &mut window_inside
                    } else {
                        &mut window_outside
                    };
                    bump(side, id);
                } else if decoration.inside {
                    bump(&mut hung, (id, decoration.kind));
                    decorations += 1;
                }
            }
        }
    }
    report.rooms = rooms;
    for (tag, counts) in &floors {
        if let Some(material) = top(counts) {
            for role in roles_of(*tag) {
                finish
                    .floors
                    .insert((*role).to_owned(), floor_of(&material));
                report.floor_rules += 1;
            }
        }
    }
    if let Some(material) = top(&everywhere) {
        finish
            .floors
            .insert("default".to_owned(), floor_of(&material));
    }
    let decor = |counts: &BTreeMap<u32, u32>, kind: u8| {
        top(counts).map(|id| KitDecor {
            id,
            kind,
            weight: counts[&id],
        })
    };
    if !window_inside.is_empty() || !window_outside.is_empty() {
        finish.window_decor = Some(KitWindowDecor {
            inside: decor(&window_inside, 4),
            outside: decor(&window_outside, 4),
        });
        report.learned.push("window decoration".into());
    }
    if !hung.is_empty() {
        let mut ranked: Vec<_> = hung.iter().collect();
        ranked.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
        finish.wall_decor = ranked
            .into_iter()
            .take(6)
            .map(|((id, kind), count)| KitDecor {
                id: *id,
                kind: *kind,
                weight: *count,
            })
            .collect();
        report.wall_decorations = finish.wall_decor.len();
        let per_edge = decorations * 100 / interior_edges.max(1);
        finish.wall_decor_percent = per_edge.clamp(5, 60) as u8;
    }

    // Furniture by room kind.
    let mut rules: BTreeMap<(RoomTag, u32, u8), Rule> = BTreeMap::new();
    let mut tag_rooms: BTreeMap<RoomTag, usize> = BTreeMap::new();
    let mut room_index = 0;
    for building in buildings {
        for room in &building.rooms {
            room_index += 1;
            *tag_rooms.entry(room.tag).or_default() += 1;
            for piece in &room.furniture {
                let rule = rules
                    .entry((room.tag, piece.object.get(), piece.kind))
                    .or_default();
                rule.seen += 1;
                *rule.rooms.entry(room_index).or_default() += 1;
                *rule.orientation.entry(piece.orientation & 3).or_default() += 1;
                if piece.wall_distance == 0 {
                    rule.against += 1;
                }
                if let Some(side) = piece.against {
                    *rule
                        .relative
                        .entry(relative_to(side, piece.orientation))
                        .or_default() += 1;
                }
            }
        }
    }
    let mut by_tag: BTreeMap<RoomTag, Vec<KitFurniture>> = BTreeMap::new();
    for ((tag, id, kind), rule) in &rules {
        let size = objects
            .get(&osrs_core::ids::ObjectId::new(*id))
            .map_or((1, 1), |i| i.size);
        let against = rule.against * 10 >= rule.seen * 7;
        let rooms_with = rule.rooms.len();
        let max = rule.rooms.values().copied().max().unwrap_or(1).min(4);
        by_tag.entry(*tag).or_default().push(KitFurniture {
            id: *id,
            kind: *kind,
            place: if against {
                KitPlace::AgainstWall
            } else {
                KitPlace::Center
            },
            relative: if against {
                top(&rule.relative).unwrap_or(0)
            } else {
                top(&rule.orientation).unwrap_or(0)
            },
            min: u8::from(rooms_with * 10 >= tag_rooms[tag] * 6),
            max: max as u8,
            size: [size.0.max(1), size.1.max(1)],
        });
    }
    for (tag, mut list) in by_tag {
        list.sort_by_key(|f| std::cmp::Reverse(rules[&(tag, f.id, f.kind)].seen));
        list.truncate(12);
        for role in roles_of(tag) {
            finish.furniture.insert((*role).to_owned(), list.clone());
            report.furniture_rules += list.len();
        }
    }
    if report.furniture_rules > 0 {
        report.learned.push("furniture".into());
    } else {
        report.kept.push("furniture".into());
    }

    // Objects that have no name.
    for building in buildings {
        for object in building
            .rooms
            .iter()
            .flat_map(|r| r.furniture.iter().map(|f| f.object))
            .chain(building.walls.iter().map(|w| w.piece.object))
            .filter(|o| *o != OPENING_OBJECT)
        {
            let named = objects
                .get(&object)
                .is_some_and(|i| !i.name.is_empty() && i.name != "null" || !i.symbol.is_empty());
            if !named {
                *report.unnamed.entry(object.get()).or_default() += 1;
            }
        }
    }
    kit.finish = Some(finish);
    kit.notes = format!(
        "Learned from {} building(s), {} room(s). Learned: {}. Kept from the starting kit: {}.",
        report.buildings,
        report.rooms,
        report.learned.join(", "),
        report.kept.join(", ")
    );
    Ok((kit, report))
}

/// Which buildings the census finds in each prefab (each prefab on its own, so two prefabs never
/// merge into one building).
pub fn prefab_buildings(prefabs: &[Prefab], objects: &ObjectTable) -> Vec<Building> {
    prefabs
        .iter()
        .flat_map(|prefab| {
            let snapshot = Snapshot {
                tiles: prefab.tiles.clone(),
                heights: prefab.heights.clone(),
                objects: prefab.objects.clone(),
            };
            crate::census_buildings(&snapshot, prefab.width, prefab.height, objects)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Brief, census_buildings, generate, kit_objects};

    const BRICK: &str =
        include_str!("../../osrs-editor/src/app/prefab_runtime/kits/lumbridge-brick.kit.json");
    const TIMBER: &str =
        include_str!("../../osrs-editor/src/app/prefab_runtime/kits/timber-town.kit.json");

    fn kit(text: &str) -> KitFile {
        serde_json::from_str(text).unwrap_or_else(|e| panic!("kit: {e}"))
    }

    fn brief(kind: &str, seed: u64) -> Brief {
        Brief {
            sketch: None,
            kind: kind.to_owned(),
            width: 16,
            height: 12,
            floors: 1,
            seed,
            variation: 0,
            rooms: None,
        }
    }

    /// Buildings in the brick style, found the way the editor finds them in a saved prefab.
    fn brick_buildings() -> (Vec<Building>, ObjectTable) {
        let brick = kit(BRICK);
        let objects = kit_objects(&brick);
        let mut buildings = Vec::new();
        for (kind, seed) in [("tavern", 1), ("house", 2), ("shop", 4)] {
            let made = generate(&brick, &brief(kind, seed)).unwrap_or_else(|e| panic!("{e}"));
            buildings.extend(census_buildings(
                &made.snapshot,
                made.width,
                made.height,
                &objects,
            ));
        }
        (buildings, objects)
    }

    #[test]
    fn a_style_learned_from_brick_buildings_uses_brick_pieces_in_a_timber_kit() {
        let (buildings, objects) = brick_buildings();
        let (learned, report) = learn_style(&kit(TIMBER), "brick again", &buildings, &objects)
            .unwrap_or_else(|e| panic!("{e}"));
        let brick = kit(BRICK);
        assert_eq!(learned.name, "brick again");
        eprintln!(
            "{report:?} exterior={} brick={} timber={}",
            learned.exterior_wall.id,
            brick.exterior_wall.id,
            kit(TIMBER).exterior_wall.id
        );
        assert_eq!(learned.exterior_wall.id, brick.exterior_wall.id);
        assert_eq!(learned.door.id, brick.door.id);
        assert_ne!(learned.exterior_wall.id, kit(TIMBER).exterior_wall.id);
        assert!(report.learned.contains(&"outside walls".to_owned()));
        assert!(report.furniture_rules > 0, "{report:?}");
        assert!(report.buildings >= 3);
    }

    #[test]
    fn a_building_built_in_a_learned_style_is_still_usable() {
        let (buildings, objects) = brick_buildings();
        let (learned, _) = learn_style(&kit(TIMBER), "brick again", &buildings, &objects)
            .unwrap_or_else(|e| panic!("{e}"));
        let mut built = 0;
        for seed in 0..6 {
            if let Ok(made) = generate(&learned, &brief("house", seed)) {
                built += 1;
                assert!(
                    made.verdict.usable,
                    "seed {seed}: {:?}",
                    made.verdict.problems
                );
            }
        }
        assert!(built >= 4, "only {built} of 6 built");
    }

    #[test]
    fn learning_from_nothing_is_an_explicit_error_and_unnamed_objects_are_counted() {
        assert!(learn_style(&kit(BRICK), "x", &[], &ObjectTable::new()).is_err());
        let (buildings, _) = brick_buildings();
        // With no names at all, every piece is unnamed.
        let (_, report) = learn_style(&kit(BRICK), "x", &buildings, &ObjectTable::new())
            .unwrap_or_else(|e| panic!("{e}"));
        assert!(!report.unnamed.is_empty());
    }
}
