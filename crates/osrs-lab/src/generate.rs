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
    Building, CensusArea, FEATURE_NAMES, Ground, Judgement, ObjectInfo, ObjectTable, analyze,
    judge, judge_from,
};
use osrs_scene::object_semantics::Placed;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

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
    #[serde(default)]
    pub sketch: Option<osrs_edit::BuildingSketch>,
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

/// One hard failure and where it is, in a form that serialises. `tile` is absent for a failure of
/// the building as a whole.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerdictFinding {
    pub kind: String,
    pub plane: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tile: Option<[i32; 2]>,
}

/// How a building scored, in a form that serialises for a trainer or a screen.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Verdict {
    pub usable: bool,
    pub problems: Vec<String>,
    /// Where each problem is; `problems` has one line per kind, this has one entry per place.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub findings: Vec<VerdictFinding>,
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
            findings: Vec::new(),
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
            findings: judgement
                .findings
                .iter()
                .map(|f| VerdictFinding {
                    kind: f.kind.describe().to_owned(),
                    plane: f.plane,
                    tile: f.tile.map(|(x, y)| [x, y]),
                })
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
    if let Some(stairs) = &kit.stairs {
        name(stairs.up.id, "stairs", &["climb-up"]);
        name(stairs.down.id, "stairs", &["climb-down"]);
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

/// Judge what is in `snapshot` (the biggest building on its lowest floor) with `objects` as the
/// names. When the object names mark stairs (`climb-up` / `climb-down`), the floors above are
/// judged too: they are reached by the stairs, and the stairs must join: an up stair with a down
/// stair on the same tile one floor higher, both standing in a room.
pub fn judge_snapshot(
    snapshot: &Snapshot,
    width: u16,
    height: u16,
    objects: &ObjectTable,
) -> Verdict {
    let buildings = census_buildings(snapshot, width, height, objects);
    // The way in is on the lowest floor, so that is the floor judged first.
    let judged: Option<&Building> = buildings.iter().min_by_key(|b| {
        (
            b.plane,
            std::cmp::Reverse(b.rooms.iter().map(|r| r.tiles.len()).sum::<usize>()),
        )
    });
    let Some(ground) = judged else {
        return Verdict::none_found();
    };
    let mut verdict = Verdict::from(&judge(ground));
    let found = judge_upper_floors(snapshot, ground.plane, &buildings, objects);
    for finding in found {
        if !verdict.problems.contains(&finding.kind) {
            verdict.problems.push(finding.kind.clone());
        }
        verdict.findings.push(finding);
        verdict.usable = false;
    }
    verdict
}

const STAIRS_DO_NOT_JOIN: &str = "stairs do not join the floors";

/// What is wrong above the ground floor: stairs that do not meet, and upper rooms that cannot be
/// reached from where the stairs arrive.
fn judge_upper_floors(
    snapshot: &Snapshot,
    ground: u8,
    buildings: &[Building],
    objects: &ObjectTable,
) -> Vec<VerdictFinding> {
    let stair = |action: &str| -> BTreeSet<(u8, i32, i32)> {
        snapshot
            .objects
            .iter()
            .filter(|o| {
                objects
                    .get(&ObjectId::new(o.id))
                    .is_some_and(|info| info.actions.iter().any(|a| a == action))
            })
            .map(|o| (o.plane, i32::from(o.x), i32::from(o.y)))
            .collect()
    };
    let (up, down) = (stair("climb-up"), stair("climb-down"));
    let mut found = Vec::new();
    let mut add = |plane: u8, tile: (i32, i32)| {
        found.push(VerdictFinding {
            kind: STAIRS_DO_NOT_JOIN.to_owned(),
            plane,
            tile: Some([tile.0, tile.1]),
        });
    };
    let in_a_room = |plane: u8, tile: (i32, i32)| {
        buildings
            .iter()
            .filter(|b| b.plane == plane)
            .any(|b| b.rooms.iter().any(|r| r.tiles.contains(&tile)))
    };
    for &(plane, x, y) in &up {
        if !down.contains(&(plane + 1, x, y)) || !in_a_room(plane, (x, y)) {
            add(plane, (x, y));
        }
    }
    for &(plane, x, y) in &down {
        if plane == 0 || !up.contains(&(plane - 1, x, y)) || !in_a_room(plane, (x, y)) {
            add(plane, (x, y));
        }
    }
    // Each upper floor is reached from the tiles its down stairs stand on.
    let mut planes: BTreeSet<u8> = buildings
        .iter()
        .map(|b| b.plane)
        .filter(|p| *p > ground)
        .collect();
    planes.extend(down.iter().map(|d| d.0).filter(|p| *p > ground));
    for plane in planes {
        let arrivals: Vec<(i32, i32)> = down
            .iter()
            .filter(|d| d.0 == plane)
            .map(|d| (d.1, d.2))
            .collect();
        for building in buildings.iter().filter(|b| b.plane == plane) {
            for finding in judge_from(building, &arrivals).findings {
                found.push(VerdictFinding {
                    kind: finding.kind.describe().to_owned(),
                    plane: finding.plane,
                    tile: finding.tile.map(|(x, y)| [x, y]),
                });
            }
        }
    }
    found
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
    let compiled = kit.compile().map_err(|e| e.to_string())?;
    let patch = match &brief.sketch {
        Some(sketch) => compiled.generate_in_rooms(&snapshot, brief.variation, sketch),
        None => {
            compiled.generate_with_rooms(&snapshot, brief.variation, brief.rooms.map(usize::from))
        }
    }
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

    #[test]
    fn kit_stairs_supply_vertical_action_evidence_without_cache_definitions() {
        let kit = kit();
        let stairs = kit
            .stairs
            .as_ref()
            .unwrap_or_else(|| panic!("starter stairs"));
        let objects = kit_objects(&kit);
        assert_eq!(objects[&ObjectId::new(stairs.up.id)].actions, ["climb-up"]);
        assert_eq!(
            objects[&ObjectId::new(stairs.down.id)].actions,
            ["climb-down"]
        );
    }

    fn brief(kind: &str, width: u16, height: u16, seed: u64) -> Brief {
        Brief {
            sketch: None,
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

    /// Yield of the planner over a wide sweep, with every rejection counted by its reason. Run with
    /// `cargo test -p osrs-lab generation_yield_report -- --ignored --nocapture` to see where the
    /// planner gives up; the numbers are what the yield test below ratchets.
    #[test]
    #[ignore = "prints a report; the ratchet below is the check"]
    fn generation_yield_report() {
        let kit = kit();
        let mut reasons: BTreeMap<String, u32> = BTreeMap::new();
        let (mut total, mut built, mut unusable) = (0_u32, 0_u32, 0_u32);
        for kind in [
            "house",
            "shop",
            "tavern",
            "bank",
            "church",
            "warehouse",
            "castle",
            "dungeon",
        ] {
            for floors in [1_u8, 2] {
                for (w, h) in [(8, 6), (10, 8), (14, 12), (20, 16), (30, 24)] {
                    for seed in 0..12 {
                        total += 1;
                        let mut b = brief(kind, w, h, seed);
                        b.floors = floors;
                        match generate(&kit, &b) {
                            Ok(g) if g.verdict.usable => built += 1,
                            Ok(g) => {
                                unusable += 1;
                                if unusable <= 3 {
                                    println!(
                                        "{kind} {w}x{h} floors={floors} seed={seed}: {:?}",
                                        g.verdict.findings
                                    );
                                }
                                *reasons
                                    .entry(format!(
                                        "UNUSABLE {kind} floors={floors} {}",
                                        g.verdict.problems.join("; ")
                                    ))
                                    .or_default() += 1;
                            }
                            Err(e) => {
                                let key: String =
                                    format!("{kind} {w}x{h}: {e}").chars().take(90).collect();
                                *reasons.entry(key).or_default() += 1;
                            }
                        }
                    }
                }
            }
        }
        println!("built {built} / {total}; unusable {unusable}");
        for (reason, count) in reasons {
            println!("{count:5}  {reason}");
        }
    }

    /// The baseline before the planner fitted rooms to the site and kept stairs off doorways was
    /// 456 of 960 built and usable (80 layouts refused, 424 flagged). Every brief in this range
    /// must now build, and the judge must find nothing wrong with the ground floor.
    #[test]
    fn every_brief_in_the_sweep_builds_and_is_usable() {
        let kit = kit();
        let mut failures = Vec::new();
        for kind in [
            "house",
            "shop",
            "tavern",
            "bank",
            "church",
            "warehouse",
            "castle",
            "dungeon",
        ] {
            for floors in 1_u8..=3 {
                for (w, h) in [(8, 6), (10, 8), (14, 12), (20, 16), (30, 24)] {
                    for seed in 0..8 {
                        let mut b = brief(kind, w, h, seed);
                        b.floors = floors;
                        match generate(&kit, &b) {
                            Ok(g) if g.verdict.usable => {}
                            Ok(g) => failures.push(format!(
                                "{kind} {w}x{h} floors={floors} #{seed}: {:?}",
                                g.verdict.findings
                            )),
                            Err(e) => failures
                                .push(format!("{kind} {w}x{h} floors={floors} #{seed}: {e}")),
                        }
                    }
                }
            }
        }
        assert!(
            failures.is_empty(),
            "{} briefs failed, first: {:#?}",
            failures.len(),
            &failures[..failures.len().min(5)]
        );
    }

    /// The rectangles of the rooms the planner kept for a brief, in site coordinates (the census
    /// reads the same coordinates, because the building stands in a document of the same origin).
    fn planned_room_rects(b: &Brief) -> Vec<osrs_edit::RoomRect> {
        let site = RectSite::new(MapTile::new(2, 2), b.width, b.height)
            .unwrap_or_else(|| unreachable!("the sweep sites are valid"));
        let archetype = archetype_of(&b.kind).unwrap_or_else(|e| panic!("{e}"));
        let (_, layout) = osrs_edit::plan_compact_layout(archetype, site, b.seed, None)
            .unwrap_or_else(|e| panic!("layout: {e:?}"));
        layout.rooms().iter().map(|room| room.rect).collect()
    }

    /// Room detection and the planner must agree: each room the planner laid out is found by the
    /// census as exactly one enclosed room with exactly the planned tiles. (Corridor bands are not
    /// compared: where one crosses another's wall line the census reads the crossing as a doorway
    /// gap and cuts the corridor, which is how it reads real buildings too.)
    #[test]
    fn every_planned_room_is_detected_as_exactly_its_rectangle() {
        let kit = kit();
        let objects = kit_objects(&kit);
        let mut checked = 0;
        let mut failures = Vec::new();
        for kind in ["house", "shop", "tavern", "bank", "church", "warehouse"] {
            for (w, h) in [(8, 6), (10, 8), (14, 12), (20, 16), (30, 24)] {
                for seed in 0..8 {
                    let b = brief(kind, w, h, seed);
                    let Ok(g) = generate(&kit, &b) else { continue };
                    let detected: Vec<std::collections::BTreeSet<(i32, i32)>> =
                        census_buildings(&g.snapshot, g.width, g.height, &objects)
                            .iter()
                            .filter(|building| building.plane == 0)
                            .flat_map(|building| &building.rooms)
                            .map(|room| room.tiles.iter().copied().collect())
                            .collect();
                    for rect in planned_room_rects(&b) {
                        checked += 1;
                        let wanted: std::collections::BTreeSet<(i32, i32)> =
                            (0..i32::from(rect.height))
                                .flat_map(|dy| {
                                    (0..i32::from(rect.width))
                                        .map(move |dx| (rect.origin.x + dx, rect.origin.y + dy))
                                })
                                .collect();
                        if !detected.contains(&wanted) {
                            failures.push(format!("{kind} {w}x{h} #{seed}: {rect:?}"));
                        }
                    }
                }
            }
        }
        assert!(checked > 100, "only {checked} rooms checked");
        assert!(
            failures.is_empty(),
            "{} of {checked} planned rooms were not detected as their rectangle, first: {:#?}",
            failures.len(),
            &failures[..failures.len().min(5)]
        );
    }

    /// The room-count table the planner samples from is the shipped corpus, counted again.
    #[test]
    fn the_room_count_table_is_the_shipped_corpus_counted_again() {
        let corpus = crate::Corpus::shipped();
        for row in osrs_edit::ROOM_COUNT_TABLE {
            let mut counted = [0_u32; osrs_edit::ROOM_COUNT_BUCKETS];
            for r in &corpus.rows {
                // Real roofed buildings, as the trainer reads them: width, height, split, then
                // the numbers of FEATURE_NAMES (floor tiles first, rooms second, roofed eighth).
                if r.len() != 19 || r[3] < 12.0 || r[10] < 0.3 {
                    continue;
                }
                let tiles = r[3] as u32;
                if tiles >= row.min_tiles && tiles < row.max_tiles {
                    let rooms = (r[4] as usize).clamp(1, osrs_edit::ROOM_COUNT_BUCKETS);
                    counted[rooms - 1] += 1;
                }
            }
            assert_eq!(
                counted, row.buildings,
                "tiles {}..{}",
                row.min_tiles, row.max_tiles
            );
        }
    }

    /// Generated layouts against the real buildings (470 roofed ones in the shipped corpus):
    /// multi-room buildings have about 0.64 interior doors per room, and the share that is not a
    /// rectangle is 27% at two rooms and 59% at three or more. The bands are wide because the
    /// sites here are small; the point is to catch a layout that goes back to hallway networks or
    /// to rectangles only.
    #[test]
    fn generated_layouts_follow_the_real_door_and_outline_statistics() {
        let kit = kit();
        let (mut multi, mut links) = (0_u32, 0.0_f32);
        // Buildings and non-rectangular ones, with two rooms and with three or more.
        let (mut two, mut two_odd, mut many, mut many_odd) = (0_u32, 0_u32, 0_u32, 0_u32);
        for kind in ["house", "shop", "tavern"] {
            for w in (8..=22_u16).step_by(2) {
                for h in (8..=16_u16).step_by(2) {
                    for seed in 0..20 {
                        let Ok(g) = generate(&kit, &brief(kind, w, h, seed)) else {
                            continue;
                        };
                        let rooms = g.verdict.rooms;
                        if rooms < 2 {
                            continue;
                        }
                        multi += 1;
                        links += g.verdict.features["links_per_room"];
                        let bounding = (f32::from(w) - 2.0) * (f32::from(h) - 2.0);
                        let odd = g.verdict.features["floor_tiles"] < bounding * 0.9;
                        if rooms == 2 {
                            two += 1;
                            two_odd += u32::from(odd);
                        } else {
                            many += 1;
                            many_odd += u32::from(odd);
                        }
                    }
                }
            }
        }
        let share = |part: u32, whole: u32| part as f32 / whole.max(1) as f32;
        let mean_links = links / multi.max(1) as f32;
        assert!(
            (0.5..=0.75).contains(&mean_links),
            "links per room {mean_links}"
        );
        let (odd_two, odd_many) = (share(two_odd, two), share(many_odd, many));
        assert!(
            (0.10..=0.40).contains(&odd_two),
            "two rooms non-rect {odd_two}"
        );
        assert!(
            (0.40..=0.75).contains(&odd_many),
            "three+ rooms non-rect {odd_many}"
        );
    }

    fn storeys(kind: &str, floors: u8, seed: u64) -> Generated {
        let mut b = brief(kind, 16, 12, seed);
        b.floors = floors;
        generate(&kit(), &b).unwrap_or_else(|e| panic!("{kind} {floors} floors #{seed}: {e}"))
    }

    fn tiles_on(made: &Generated, plane: u8) -> BTreeSet<(i32, i32)> {
        let objects = kit_objects(&kit());
        census_buildings(&made.snapshot, made.width, made.height, &objects)
            .iter()
            .filter(|b| b.plane == plane)
            .flat_map(|b| b.tiles())
            .collect()
    }

    /// An upper floor is laid out for itself: it stands wholly on the floor below, often covers
    /// less of it, and is rarely the ground floor's rooms again.
    #[test]
    fn upper_floors_have_their_own_rooms_standing_on_the_floor_below() {
        let objects = kit_objects(&kit());
        let (mut floors_seen, mut smaller, mut different_rooms) = (0, 0, 0);
        for kind in ["house", "tavern", "shop", "castle"] {
            for floors in [2_u8, 3] {
                for seed in 0..12 {
                    let made = storeys(kind, floors, seed);
                    let buildings =
                        census_buildings(&made.snapshot, made.width, made.height, &objects);
                    for plane in 1..floors {
                        let (below, here) = (tiles_on(&made, plane - 1), tiles_on(&made, plane));
                        assert!(
                            !here.is_empty(),
                            "{kind} {floors}f #{seed}: no floor {plane}"
                        );
                        assert!(
                            here.is_subset(&below),
                            "{kind} {floors}f #{seed}: floor {plane}"
                        );
                        floors_seen += 1;
                        smaller += usize::from(here.len() < below.len());
                        let rooms = |p: u8| -> BTreeSet<BTreeSet<(i32, i32)>> {
                            buildings
                                .iter()
                                .filter(|b| b.plane == p)
                                .flat_map(|b| &b.rooms)
                                .map(|r| r.tiles.iter().copied().collect())
                                .collect()
                        };
                        different_rooms += usize::from(rooms(plane) != rooms(plane - 1));
                    }
                }
            }
        }
        let share = |n: usize| n as f32 / floors_seen as f32;
        assert!(
            (0.15..=0.65).contains(&share(smaller)),
            "setbacks {}",
            share(smaller)
        );
        assert!(
            share(different_rooms) > 0.9,
            "different rooms {}",
            share(different_rooms)
        );
    }

    /// Each level's exposed part is under a roof on the plane above it, and no roof piece stands on
    /// a tile of the floor that rises there.
    #[test]
    fn every_exposed_level_has_a_roof_clear_of_the_floor_above() {
        let mut stepped = 0;
        for kind in ["house", "tavern", "shop"] {
            for floors in [2_u8, 3] {
                for seed in 0..16 {
                    let made = storeys(kind, floors, seed);
                    let roof_tiles = |plane: u8| -> BTreeSet<(i32, i32)> {
                        made.snapshot
                            .objects
                            .iter()
                            .filter(|o| o.plane == plane && (12..=21).contains(&o.kind))
                            .map(|o| (i32::from(o.x), i32::from(o.y)))
                            .collect()
                    };
                    for floor in 0..floors {
                        let here = tiles_on(&made, floor);
                        let above = if floor + 1 < floors {
                            tiles_on(&made, floor + 1)
                        } else {
                            BTreeSet::new()
                        };
                        let roof = roof_tiles(floor + 1);
                        let exposed: BTreeSet<_> = here.difference(&above).copied().collect();
                        assert!(
                            exposed.is_subset(&roof),
                            "{kind} {floors}f #{seed}: floor {floor} has an unroofed tile"
                        );
                        assert!(
                            roof.is_disjoint(&above),
                            "{kind} {floors}f #{seed}: floor {floor}'s roof is on the floor above"
                        );
                        stepped += usize::from(floor + 1 < floors && !exposed.is_empty());
                    }
                }
            }
        }
        assert!(stepped > 0, "no building in the sweep stepped back");
    }

    /// An upper floor is a lived-in floor, not an empty shell: it has windows and furniture.
    #[test]
    fn upper_floors_are_furnished_and_windowed() {
        let objects = kit_objects(&kit());
        let count = |made: &Generated, name: &str| {
            made.snapshot
                .objects
                .iter()
                .filter(|o| o.plane == 1)
                .filter(|o| {
                    objects
                        .get(&ObjectId::new(o.id))
                        .is_some_and(|i| i.name == name)
                })
                .count()
        };
        let (mut furniture, mut windows) = (0, 0);
        for seed in 0..30 {
            let made = storeys("house", 2, seed);
            furniture += count(&made, "furniture");
            windows += count(&made, "window decoration");
        }
        assert!(
            furniture >= 60,
            "{furniture} pieces of furniture on 30 upper floors"
        );
        assert!(
            windows >= 150,
            "{windows} window decorations on 30 upper floors"
        );
    }

    #[test]
    fn the_judge_notices_stairs_that_do_not_join_the_floors() {
        let kit = kit();
        let objects = kit_objects(&kit);
        let stairs = kit
            .stairs
            .clone()
            .unwrap_or_else(|| panic!("the kit has stairs"));
        for seed in 0..6 {
            let made = storeys("house", 2, seed);
            assert!(made.verdict.usable, "{:?}", made.verdict.problems);
            for (removed, name) in [(stairs.up.id, "up"), (stairs.down.id, "down")] {
                let mut broken = made.snapshot.clone();
                broken.objects.retain(|o| o.id != removed);
                let verdict = judge_snapshot(&broken, made.width, made.height, &objects);
                assert!(!verdict.usable, "seed {seed}: {name} stair gone");
                assert!(
                    verdict.problems.iter().any(|p| p == STAIRS_DO_NOT_JOIN),
                    "seed {seed}: {name} stair gone: {:?}",
                    verdict.problems
                );
            }
            // A down stair that has moved a tile no longer meets the up stair.
            let mut moved = made.snapshot.clone();
            for o in moved.objects.iter_mut().filter(|o| o.id == stairs.down.id) {
                o.x += 1;
            }
            let verdict = judge_snapshot(&moved, made.width, made.height, &objects);
            assert!(!verdict.usable, "seed {seed}: down stair moved");
        }
    }

    #[test]
    fn the_judge_notices_an_upper_room_nobody_can_reach() {
        let kit = kit();
        let objects = kit_objects(&kit);
        let stairs = kit
            .stairs
            .clone()
            .unwrap_or_else(|| panic!("the kit has stairs"));
        let mut checked = 0;
        for seed in 0..20 {
            let made = storeys("tavern", 2, seed);
            // Every door on the upper floor becomes wall: only the stair room is left reachable.
            let upper_rooms = census_buildings(&made.snapshot, made.width, made.height, &objects)
                .iter()
                .filter(|b| b.plane == 1)
                .map(|b| b.rooms.len())
                .sum::<usize>();
            if upper_rooms < 2 {
                continue;
            }
            let wall = kit.exterior_wall.id;
            let mut sealed = made.snapshot.clone();
            for o in sealed.objects.iter_mut().filter(|o| o.plane == 1) {
                if objects
                    .get(&ObjectId::new(o.id))
                    .is_some_and(|i| i.name == "door")
                {
                    o.id = wall;
                }
            }
            let verdict = judge_snapshot(&sealed, made.width, made.height, &objects);
            assert!(!verdict.usable, "seed {seed}");
            assert!(
                verdict
                    .problems
                    .iter()
                    .any(|p| p == "a room cannot be reached"),
                "seed {seed}: {:?}",
                verdict.problems
            );
            assert!(verdict.findings.iter().any(|f| f.plane == 1));
            checked += 1;
        }
        assert!(
            checked >= 3,
            "only {checked} upper floors had several rooms"
        );
        let _ = stairs;
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
