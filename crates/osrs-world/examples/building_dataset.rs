//! Export every real building as one labelled record, for training and for measuring (read-only).
//!
//! usage: building_dataset <OpenRune root> <out dir> [min_rx max_rx min_ry max_ry]
//!
//! Writes `<out>/buildings.jsonl` (one building per line: where it is, its rooms with role tags,
//! floors, furniture and wall decoration, every wall edge with its piece and which side is
//! outside, the roof pieces, the ground around it) and `<out>/objects.json` (name, gameval symbol,
//! menu verbs, size and cache category of every object the buildings use, so a model can read what
//! an id is). Set `AREAS=<117HD scene/areas.json>` to add the names of the hand-made areas
//! (banks, churches, inns, houses...) that contain each building: the only human labels of what a
//! building is. Set `CENSUS_EXCLUDE=<user_world_defs.json>` to skip underground and instanced
//! map squares, as `building_census` does.
use osrs_cache::{openrune::installation::Installation, source::CacheSource};
use osrs_core::{coords::RegionCoord, ids::ObjectId};
use osrs_scene::{
    building_census::{
        Building, CensusArea, Dir, EdgeFeature, Ground, Material, ObjectInfo, ObjectTable, analyze,
    },
    object_semantics::Placed,
    terrain_load::RegionTileHeight,
};
use osrs_world::WorldDefinitions;
use serde_json::{Value, json};
use std::{collections::BTreeMap, io::Write};

fn dir_word(dir: Dir) -> &'static str {
    match dir {
        Dir::West => "west",
        Dir::North => "north",
        Dir::East => "east",
        Dir::South => "south",
    }
}

fn feature_word(feature: EdgeFeature) -> &'static str {
    match feature {
        EdgeFeature::Wall => "wall",
        EdgeFeature::Door => "door",
        EdgeFeature::Window => "window",
        EdgeFeature::Opening => "opening",
    }
}

fn material(m: &Material, count: u32) -> Value {
    json!({ "underlay": m.underlay, "overlay": m.overlay, "shape": m.shape, "tiles": count })
}

/// A named rectangle from 117HD's `areas.json`.
struct Area {
    name: String,
    boxes: Vec<[i32; 4]>,
}

fn read_areas(path: &str) -> Result<Vec<Area>, Box<dyn std::error::Error>> {
    // The file has `//` comment lines; drop them before parsing.
    let text: String = std::fs::read_to_string(path)?
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    let value: Value = serde_json::from_str(&text)?;
    let mut areas = Vec::new();
    for entry in value.as_array().into_iter().flatten() {
        let Some(name) = entry["name"].as_str() else {
            continue;
        };
        let boxes = entry["aabbs"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|b| {
                let n: Vec<i32> = b
                    .as_array()?
                    .iter()
                    .filter_map(|v| v.as_i64())
                    .map(|v| v as i32)
                    .collect();
                (n.len() >= 4).then(|| [n[0], n[1], n[2], n[3]])
            })
            .collect();
        areas.push(Area {
            name: name.to_owned(),
            boxes,
        });
    }
    Ok(areas)
}

fn record(
    region: RegionCoord,
    index: usize,
    building: &Building,
    objects: &ObjectTable,
    areas: &[Area],
) -> Value {
    let (min, max) = building.bounds();
    let centre = ((min.0 + max.0) / 2, (min.1 + max.1) / 2);
    let area_names: Vec<&str> = areas
        .iter()
        .filter(|a| {
            a.boxes.iter().any(|b| {
                centre.0 >= b[0] && centre.0 <= b[2] && centre.1 >= b[1] && centre.1 <= b[3]
            })
        })
        .map(|a| a.name.as_str())
        .collect();
    let rooms: Vec<Value> = building
        .rooms
        .iter()
        .map(|room| {
            json!({
                "tag": room.tag.word(),
                "plane": room.plane,
                "tiles": room.tiles.iter().map(|t| [t.0 - min.0, t.1 - min.1]).collect::<Vec<_>>(),
                "roofed": room.roofed,
                "floor": room.floor.iter().map(|(m, c)| material(m, *c)).collect::<Vec<_>>(),
                "furniture": room.furniture.iter().map(|f| json!({
                    "object": f.object.get(), "kind": f.kind,
                    "tile": [f.tile.0 - min.0, f.tile.1 - min.1],
                    "orientation": f.orientation, "wall_distance": f.wall_distance,
                    "against": f.against.map(dir_word),
                })).collect::<Vec<_>>(),
                "decorations": room.decorations.iter().map(|d| json!({
                    "object": d.object.get(), "kind": d.kind,
                    "tile": [d.tile.0 - min.0, d.tile.1 - min.1],
                    "orientation": d.orientation, "on": d.on.map(ObjectId::get),
                    "edge_side": d.edge_side.map(dir_word), "inside": d.inside,
                })).collect::<Vec<_>>(),
                "ground_decor": room.ground_decor.iter()
                    .map(|(o, t)| json!({ "object": o.get(), "tile": [t.0 - min.0, t.1 - min.1] }))
                    .collect::<Vec<_>>(),
            })
        })
        .collect();
    let walls: Vec<Value> = building
        .walls
        .iter()
        .map(|w| {
            json!({
                "tile": [w.tile.0 - min.0, w.tile.1 - min.1],
                "dir": dir_word(w.dir),
                "exterior": w.exterior,
                "feature": feature_word(w.piece.feature),
                "object": (w.piece.object != osrs_scene::building_census::OPENING_OBJECT)
                    .then(|| w.piece.object.get()),
                "kind": w.piece.kind,
            })
        })
        .collect();
    let _ = objects;
    json!({
        "id": format!("r{}_{}_{}", region.x, region.y, index),
        "region": [region.x, region.y],
        "plane": building.plane,
        "origin": [min.0, min.1],
        "size": [max.0 - min.0 + 1, max.1 - min.1 + 1],
        "area_names": area_names,
        "rooms": rooms,
        "walls": walls,
        "roof": building.roof.iter()
            .map(|((o, k), c)| json!({ "object": o.get(), "kind": k, "tiles": c }))
            .collect::<Vec<_>>(),
        "ring": building.ring.iter().map(|(m, c)| material(m, *c)).collect::<Vec<_>>(),
        "door_aprons": building.door_aprons.iter()
            .map(|(a, b)| json!([material(a, 0), material(b, 0)]))
            .collect::<Vec<_>>(),
        "height_spread": { "inside": building.spread_inside, "ring": building.spread_ring },
    })
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let usage = "usage: building_dataset <OpenRune root> <out dir> [min_rx max_rx min_ry max_ry]";
    let root = args.next().ok_or(usage)?;
    let out = std::path::PathBuf::from(args.next().ok_or(usage)?);
    let mut range = |default: i32| args.next().map_or(Ok(default), |v| v.parse::<i32>());
    let (min_x, max_x, min_y, max_y) = (range(28)?, range(80)?, range(30)?, range(170)?);
    std::fs::create_dir_all(&out)?;
    let installation = Installation::read(&root)?;
    let loc_symbols: std::collections::HashMap<u32, String> = installation
        .symbols
        .table_symbols("loc")
        .into_iter()
        .map(|(id, name)| (id, name.strip_prefix("loc.").unwrap_or(name).to_lowercase()))
        .collect();
    let mut definitions =
        WorldDefinitions::open(CacheSource::Dir(installation.root.join(".data/cache/LIVE")))?;
    let areas = match std::env::var("AREAS") {
        Ok(path) => read_areas(&path)?,
        Err(_) => Vec::new(),
    };
    let mut excluded = std::collections::HashSet::new();
    if let Ok(path) = std::env::var("CENSUS_EXCLUDE") {
        let list: Value = serde_json::from_str(&std::fs::read_to_string(path)?)?;
        for area in list.as_array().into_iter().flatten() {
            for region in area["regionList"].as_array().into_iter().flatten() {
                for (x, y) in [("oldX", "oldY"), ("newX", "newY")] {
                    if let (Some(x), Some(y)) = (region[x].as_i64(), region[y].as_i64()) {
                        excluded.insert((x as i32, y as i32));
                    }
                }
            }
        }
    }

    let mut objects = ObjectTable::new();
    let mut categories: BTreeMap<u32, Option<u32>> = BTreeMap::new();
    let mut lines = std::io::BufWriter::new(std::fs::File::create(out.join("buildings.jsonl"))?);
    let (mut regions, mut written, mut named) = (0, 0, 0);
    for rx in min_x..=max_x {
        for ry in min_y..=max_y {
            if excluded.contains(&(rx, ry)) {
                continue;
            }
            let region = RegionCoord::new(rx, ry);
            let Some(map) = definitions.region(region)? else {
                continue;
            };
            regions += 1;
            let origin = (rx * 64, ry * 64);
            let mut area = CensusArea::new(origin, 64, 64);
            for plane in 0..4u8 {
                for x in 0..64usize {
                    for y in 0..64usize {
                        let tile = map.terrain.tile(usize::from(plane), x, y);
                        let (height, height_known) = match tile.height {
                            RegionTileHeight::Explicit(v) => (i32::from(v) * 8, true),
                            RegionTileHeight::Default => (0, false),
                        };
                        area.set(
                            plane,
                            (origin.0 + x as i32, origin.1 + y as i32),
                            Ground {
                                underlay: tile.underlay,
                                overlay: tile.overlay.map_or(0, |o| o.raw_id & 0x7fff),
                                shape: tile.overlay.map_or(0, |o| o.shape),
                                settings: tile.settings.unwrap_or(0),
                                height,
                                height_known,
                            },
                        );
                    }
                }
            }
            let mut placements = Vec::new();
            for loc in map.locations.locations() {
                placements.push(Placed {
                    id: loc.object_id,
                    kind: loc.loc_type,
                    orientation: loc.orientation,
                    tile: (
                        origin.0 + i32::from(loc.tile.x()),
                        origin.1 + i32::from(loc.tile.y()),
                    ),
                    plane: loc.source_plane.index().get(),
                });
                if let std::collections::btree_map::Entry::Vacant(slot) =
                    objects.entry(loc.object_id)
                {
                    let (info, category) = match definitions.object(loc.object_id)? {
                        Some(def) => (
                            ObjectInfo {
                                name: def.name.clone().unwrap_or_default().to_lowercase(),
                                symbol: loc_symbols
                                    .get(&loc.object_id.get())
                                    .cloned()
                                    .unwrap_or_default(),
                                actions: def
                                    .actions
                                    .iter()
                                    .flatten()
                                    .map(|a| a.to_lowercase())
                                    .collect(),
                                size: (def.size_x.min(255) as u8, def.size_y.min(255) as u8),
                            },
                            def.category.map(|c| c.get()),
                        ),
                        None => (ObjectInfo::default(), None),
                    };
                    categories.insert(loc.object_id.get(), category);
                    slot.insert(info);
                }
            }
            for (index, building) in analyze(&area, &placements, &objects).iter().enumerate() {
                let value = record(region, index, building, &objects, &areas);
                if value["area_names"]
                    .as_array()
                    .is_some_and(|n| !n.is_empty())
                {
                    named += 1;
                }
                writeln!(lines, "{value}")?;
                written += 1;
            }
        }
    }
    lines.flush()?;
    let table: BTreeMap<String, Value> = objects
        .iter()
        .map(|(id, info)| {
            (
                id.get().to_string(),
                json!({
                    "name": info.name, "symbol": info.symbol, "actions": info.actions,
                    "size": [info.size.0, info.size.1],
                    "category": categories.get(&id.get()).copied().flatten(),
                }),
            )
        })
        .collect();
    std::fs::write(out.join("objects.json"), serde_json::to_string(&table)?)?;
    println!(
        "{regions} map squares, {written} buildings ({named} inside a named 117HD area), {} objects -> {}",
        objects.len(),
        out.display()
    );
    Ok(())
}
