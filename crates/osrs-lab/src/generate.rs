//! Build a building from a brief (no example to copy) and judge it.
//!
//! The brief says what is wanted: a kind of building, a size, a number of floors and a seed. The
//! building comes from the same kit planner the editor's Build page uses, so everything it hard
//! requires (a closed shell, a way in, reachable rooms) is still guaranteed by the planner; the
//! judge then checks it from the finished objects alone, the way it would check a real building.

use crate::task::{Snapshot, source_plane};
use osrs_core::{coords::MapTile, ids::ObjectId};
use osrs_edit::{
    BuildingArchetype, BuildingConstraints, BuildingRequest, BuildingStyle, KitFile, PrefabHeight,
    RectSite, capture_generation_snapshot,
};
use osrs_scene::building_census::{
    Building, CensusArea, FEATURE_NAMES, Ground, Judgement, ObjectInfo, ObjectTable, analyze, judge,
};
use osrs_scene::object_semantics::Placed;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// What to build.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Brief {
    /// `house`, `shop`, `tavern`, `bank`, `church`, `warehouse`, `castle` or `dungeon`.
    pub kind: String,
    pub width: u16,
    pub height: u16,
    #[serde(default = "one")]
    pub floors: u8,
    #[serde(default)]
    pub seed: u64,
    /// A different look (windows, doors) for the same layout.
    #[serde(default)]
    pub variation: u32,
    /// At most this many rooms; leave out to let the planner choose from the size.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rooms: Option<u8>,
}

fn one() -> u8 {
    1
}

pub fn archetype_of(kind: &str) -> Result<BuildingArchetype, String> {
    Ok(match kind {
        "house" => BuildingArchetype::House,
        "shop" => BuildingArchetype::Shop,
        "tavern" => BuildingArchetype::Tavern,
        "bank" => BuildingArchetype::Bank,
        "church" => BuildingArchetype::Church,
        "warehouse" => BuildingArchetype::Warehouse,
        "castle" => BuildingArchetype::Castle,
        "dungeon" => BuildingArchetype::Dungeon,
        other => return Err(format!("unknown building kind {other:?}")),
    })
}

/// How a building scored, in a form that serialises for a trainer or a screen.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Verdict {
    pub usable: bool,
    pub problems: Vec<String>,
    pub rooms: usize,
    pub entrances: u32,
    pub rooms_reachable: usize,
    pub blocked_doorways: u32,
    /// The numbers the realism critic reads, by name ([`FEATURE_NAMES`]).
    pub features: BTreeMap<String, f32>,
}

impl Verdict {
    fn none_found() -> Self {
        Self {
            usable: false,
            problems: vec!["no enclosed room".to_owned()],
            rooms: 0,
            entrances: 0,
            rooms_reachable: 0,
            blocked_doorways: 0,
            features: BTreeMap::new(),
        }
    }

    fn from(judgement: &Judgement) -> Self {
        Self {
            usable: judgement.is_usable(),
            problems: judgement
                .problems()
                .into_iter()
                .map(str::to_owned)
                .collect(),
            rooms: judgement.rooms,
            entrances: judgement.entrances,
            rooms_reachable: judgement.rooms_reachable,
            blocked_doorways: judgement.blocked_doorways,
            features: FEATURE_NAMES
                .iter()
                .zip(judgement.features)
                .map(|(name, value)| ((*name).to_owned(), value))
                .collect(),
        }
    }
}

/// A generated building and its verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Generated {
    pub brief: Brief,
    pub width: u16,
    pub height: u16,
    pub planes: u8,
    pub snapshot: Snapshot,
    pub verdict: Verdict,
}

/// What the kit's objects are called, so the judge can tell a door from a wall: the census
/// reads names, and a generated building has no cache to read them from.
pub fn kit_objects(kit: &KitFile) -> ObjectTable {
    let mut table = ObjectTable::new();
    let mut name = |id: u32, name: &str, verbs: &[&str]| {
        table.insert(
            ObjectId::new(id),
            ObjectInfo {
                name: name.to_owned(),
                symbol: String::new(),
                actions: verbs.iter().map(|v| (*v).to_owned()).collect(),
                size: (1, 1),
            },
        );
    };
    name(kit.exterior_wall.id, "wall", &[]);
    name(kit.interior_wall.id, "wall", &[]);
    if let Some(corner) = &kit.corner {
        name(corner.id, "wall", &[]);
    }
    // Kits often make a window out of the wall piece itself plus a decoration on each face; that
    // piece is a wall, and the decoration is what makes it a window.
    if let Some(window) = &kit.window
        && window.id != kit.exterior_wall.id
        && window.id != kit.interior_wall.id
    {
        name(window.id, "window", &[]);
    }
    name(kit.door.id, "door", &["open"]);
    for variant in &kit.door_variants {
        name(variant.asset.id, "door", &["open"]);
    }
    if let Some(hip) = &kit.roof_hip {
        name(hip.slope_object, "roof", &[]);
        name(hip.edge_object, "roof", &[]);
    }
    if let Some(finish) = &kit.finish {
        for rules in finish.furniture.values() {
            for rule in rules {
                name(rule.id, "furniture", &[]);
            }
        }
        for decor in &finish.wall_decor {
            name(decor.id, "wall decoration", &[]);
        }
        if let Some(windows) = &finish.window_decor {
            for piece in [&windows.inside, &windows.outside].into_iter().flatten() {
                name(piece.id, "window decoration", &[]);
            }
        }
    }
    table
}

/// A generated building as a prefab, ready for the library: the building's kind and the model
/// (or kit) that made it are in its name, tags and source.
pub fn generated_prefab(
    made: &Generated,
    name: &str,
    style: &str,
    notes: &str,
) -> osrs_edit::Prefab {
    osrs_edit::Prefab {
        format: osrs_edit::PREFAB_FORMAT.to_owned(),
        version: osrs_edit::PREFAB_VERSION,
        name: name.to_owned(),
        tags: vec!["generated".to_owned(), made.brief.kind.clone()],
        notes: notes.to_owned(),
        source: osrs_edit::PrefabSource::Generated {
            archetype: made.brief.kind.clone(),
            style: style.to_owned(),
            seed: made.brief.seed,
            floors: made.brief.floors,
        },
        width: made.width,
        height: made.height,
        planes: made.planes,
        tiles: made.snapshot.tiles.clone(),
        heights: made.snapshot.heights.clone(),
        objects: made.snapshot.objects.clone(),
        building: None,
    }
}

/// The buildings the census finds in `snapshot` (task-local tiles, heights and objects), with
/// `objects` as the names it reads doors, windows and furniture from.
pub fn census_buildings(
    snapshot: &Snapshot,
    width: u16,
    height: u16,
    objects: &ObjectTable,
) -> Vec<Building> {
    let mut area = CensusArea::new((0, 0), usize::from(width), usize::from(height));
    let corner: BTreeMap<(u8, u16, u16), i32> = snapshot
        .heights
        .iter()
        .map(|h| ((h.plane, h.x, h.y), h.value))
        .collect();
    for tile in &snapshot.tiles {
        let height = corner.get(&(tile.plane, tile.x, tile.y)).copied();
        area.set(
            tile.plane,
            (i32::from(tile.x), i32::from(tile.y)),
            Ground {
                underlay: tile.underlay.map_or(0, |u| u as u16),
                overlay: tile.overlay.as_ref().map_or(0, |o| o.id as u16),
                shape: tile.overlay.as_ref().map_or(0, |o| o.shape),
                settings: tile.flags,
                height: height.unwrap_or(0),
                height_known: height.is_some(),
            },
        );
    }
    let placements: Vec<Placed> = snapshot
        .objects
        .iter()
        .map(|o| Placed {
            id: ObjectId::new(o.id),
            kind: o.kind,
            orientation: o.orientation,
            tile: (i32::from(o.x), i32::from(o.y)),
            plane: o.plane,
        })
        .collect();
    analyze(&area, &placements, objects)
}

/// Judge what is in `snapshot` (the biggest building in it) with `objects` as the names.
pub fn judge_snapshot(
    snapshot: &Snapshot,
    width: u16,
    height: u16,
    objects: &ObjectTable,
) -> Verdict {
    let buildings = census_buildings(snapshot, width, height, objects);
    let biggest: Option<&Building> = buildings
        .iter()
        .max_by_key(|b| b.rooms.iter().map(|r| r.tiles.len()).sum::<usize>());
    biggest.map_or_else(Verdict::none_found, |b| Verdict::from(&judge(b)))
}

/// Build the building a brief asks for with `kit` and judge it, reading names from the kit.
pub fn generate(kit: &KitFile, brief: &Brief) -> Result<Generated, String> {
    generate_named(kit, brief, &ObjectTable::new())
}

/// Like [`generate`], with real object names: where `names` has one it is used (so a bed is a
/// bed and its room a bedroom, the way the census would read a real building), and the kit's
/// own names fill the rest.
pub fn generate_named(
    kit: &KitFile,
    brief: &Brief,
    names: &ObjectTable,
) -> Result<Generated, String> {
    let archetype = archetype_of(&brief.kind)?;
    if !(6..=48).contains(&brief.width) || !(6..=48).contains(&brief.height) {
        return Err("a building is between 6 and 48 tiles on a side".into());
    }
    if !(1..=3).contains(&brief.floors) {
        return Err("a building has 1 to 3 floors".into());
    }
    let margin = 2_u16;
    let (width, height) = (brief.width + 2 * margin, brief.height + 2 * margin);
    // The roof is stored on the plane above the top floor.
    let planes = (brief.floors + 1).min(4);
    let heights = (0..planes)
        .flat_map(|p| {
            (0..=width).flat_map(move |x| {
                (0..=height).map(move |y| PrefabHeight {
                    x,
                    y,
                    plane: p,
                    value: -i32::from(p) * 240,
                })
            })
        })
        .collect();
    let initial = Snapshot {
        heights,
        ..Snapshot::default()
    };
    let mut document = initial.document(width, height, planes)?;
    let request = BuildingRequest {
        site: RectSite::new(
            MapTile::new(i32::from(margin), i32::from(margin)),
            brief.width,
            brief.height,
        )
        .ok_or("site")?,
        archetype,
        style: BuildingStyle::Generic,
        seed: brief.seed,
        base_plane: source_plane(0, planes)?,
        floors: brief.floors,
        constraints: BuildingConstraints::default(),
    };
    let snapshot = capture_generation_snapshot(&document, request, []);
    let patch = kit
        .compile()
        .map_err(|e| e.to_string())?
        .generate_with_rooms(&snapshot, brief.variation, brief.rooms.map(usize::from))
        .map_err(|e| format!("{e:?}"))?;
    document.apply_patch(&patch).map_err(|e| format!("{e:?}"))?;
    let result = Snapshot::capture(&document);
    let mut table = kit_objects(kit);
    for (id, info) in names {
        // The kit knows which pieces are walls, doors and windows; real names add the rest.
        let entry = table.entry(*id).or_default();
        if entry.name.is_empty() || entry.name == "furniture" || entry.name == "wall decoration" {
            *entry = info.clone();
        }
    }
    let verdict = judge_snapshot(&result, width, height, &table);
    Ok(Generated {
        brief: brief.clone(),
        width,
        height,
        planes,
        snapshot: result,
        verdict,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const KIT: &str =
        include_str!("../../osrs-editor/src/app/prefab_runtime/kits/lumbridge-brick.kit.json");

    fn kit() -> KitFile {
        serde_json::from_str(KIT).unwrap_or_else(|e| panic!("kit: {e}"))
    }

    fn brief(kind: &str, width: u16, height: u16, seed: u64) -> Brief {
        Brief {
            kind: kind.to_owned(),
            width,
            height,
            floors: 1,
            seed,
            variation: 0,
            rooms: None,
        }
    }

    #[test]
    fn fewer_rooms_can_be_asked_for_and_stay_usable() {
        let kit = kit();
        let (mut many, mut few) = (0, 0);
        for seed in 0..8 {
            let mut one = brief("house", 14, 10, seed);
            let free = generate(&kit, &one);
            one.rooms = Some(2);
            let two = generate(&kit, &one);
            if let (Ok(free), Ok(two)) = (free, two) {
                assert!(two.verdict.usable, "{:?}", two.verdict.problems);
                many += free.verdict.rooms;
                few += two.verdict.rooms;
            }
        }
        assert!(
            few < many,
            "asking for two rooms must give fewer ({few} vs {many})"
        );
    }

    #[test]
    fn every_generated_building_is_usable_and_most_briefs_succeed() {
        let (mut total, mut built) = (0, 0);
        for kind in ["house", "shop", "tavern", "bank", "church", "warehouse"] {
            for (w, h) in [(10, 8), (14, 12), (20, 16)] {
                for seed in 0..6 {
                    total += 1;
                    // A layout the planner cannot split is an explicit error, never a bad building.
                    if let Ok(g) = generate(&kit(), &brief(kind, w, h, seed)) {
                        built += 1;
                        assert!(
                            g.verdict.usable,
                            "{kind} {w}x{h} #{seed}: {:?}",
                            g.verdict.problems
                        );
                        assert!(g.verdict.rooms >= 1);
                    }
                }
            }
        }
        assert!(
            built * 10 >= total * 8,
            "only {built} of {total} briefs built"
        );
    }

    #[test]
    fn the_judge_notices_a_door_taken_away() {
        let kit = kit();
        let made = generate(&kit, &brief("house", 12, 10, 1)).unwrap_or_else(|e| panic!("{e}"));
        let objects = kit_objects(&kit);
        let mut broken = made.snapshot.clone();
        // Every door becomes plain wall (taking it out would leave a doorway, which is a way in).
        let wall = kit.exterior_wall.id;
        for object in &mut broken.objects {
            if objects
                .get(&ObjectId::new(object.id))
                .is_some_and(|i| i.name == "door")
            {
                object.id = wall;
            }
        }
        let verdict = judge_snapshot(&broken, made.width, made.height, &objects);
        assert!(!verdict.usable);
        assert!(
            verdict
                .problems
                .iter()
                .any(|p| p == "no way in" || p == "no enclosed room"),
            "{:?}",
            verdict.problems
        );
    }

    #[test]
    fn the_same_brief_builds_the_same_building() {
        let a = generate(&kit(), &brief("tavern", 16, 12, 9)).unwrap_or_else(|e| panic!("{e}"));
        let b = generate(&kit(), &brief("tavern", 16, 12, 9)).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(a, b);
        let c = generate(&kit(), &brief("tavern", 16, 12, 10)).unwrap_or_else(|e| panic!("{e}"));
        assert_ne!(a.snapshot, c.snapshot);
    }

    #[test]
    fn a_generated_building_is_a_valid_prefab_that_the_judge_still_likes() {
        let kit = kit();
        let made = generate(&kit, &brief("house", 12, 10, 2)).unwrap_or_else(|e| panic!("{e}"));
        let prefab = generated_prefab(&made, "House 2", "test kit", "made in a test");
        prefab.validate().unwrap_or_else(|e| panic!("{e}"));
        let again = judge_snapshot(
            &Snapshot {
                tiles: prefab.tiles.clone(),
                heights: prefab.heights.clone(),
                objects: prefab.objects.clone(),
            },
            prefab.width,
            prefab.height,
            &kit_objects(&kit),
        );
        assert_eq!(again, made.verdict);
    }

    #[test]
    fn a_bad_brief_is_an_error() {
        assert!(generate(&kit(), &brief("castle", 3, 3, 0)).is_err());
        assert!(generate(&kit(), &brief("igloo", 12, 10, 0)).is_err());
    }
}
