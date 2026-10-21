//! Dump the roof pieces of real roofed buildings, to measure how roofs are built. Read-only.
//!
//! usage: roof_census <OpenRune root> <out.json> [min_rx max_rx min_ry max_ry]
//! For each building with at least 12 floor tiles and 30% of them roofed (the corpus rule), writes
//! its floor tiles and every roof piece (kinds 12 to 21) on the plane above within two tiles of
//! it: object, kind, orientation, tile and the object's size, all relative to the building's
//! south-west corner.
//! CENSUS_EXCLUDE=user_world_defs.json skips underground and instanced map squares, as the
//! building census does.
use osrs_cache::{openrune::installation::Installation, source::CacheSource};
use osrs_core::coords::RegionCoord;
use osrs_scene::{
    building_census::{CensusArea, Ground, ObjectInfo, ObjectTable, analyze},
    object_semantics::Placed,
};
use osrs_world::WorldDefinitions;
use serde_json::json;
use std::collections::{BTreeMap, HashSet};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let usage = "usage: roof_census <OpenRune root> <out.json> [min_rx max_rx min_ry max_ry]";
    let root = args.next().ok_or(usage)?;
    let out = args.next().ok_or(usage)?;
    let mut range = |default: i32| args.next().map_or(Ok(default), |v| v.parse::<i32>());
    let (min_x, max_x, min_y, max_y) = (range(28)?, range(80)?, range(30)?, range(170)?);
    let installation = Installation::read(&root)?;
    let mut definitions =
        WorldDefinitions::open(CacheSource::Dir(installation.root.join(".data/cache/LIVE")))?;

    let mut excluded = HashSet::new();
    if let Ok(path) = std::env::var("CENSUS_EXCLUDE") {
        let areas: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(path)?)?;
        for area in areas.as_array().into_iter().flatten() {
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
    let mut names: BTreeMap<u32, String> = BTreeMap::new();
    let mut buildings = Vec::new();
    for rx in min_x..=max_x {
        for ry in min_y..=max_y {
            if excluded.contains(&(rx, ry)) {
                continue;
            }
            let Some(map) = definitions.region(RegionCoord::new(rx, ry))? else {
                continue;
            };
            let origin = (rx * 64, ry * 64);
            let mut area = CensusArea::new(origin, 64, 64);
            for plane in 0..4u8 {
                for x in 0..64usize {
                    for y in 0..64usize {
                        let tile = map.terrain.tile(usize::from(plane), x, y);
                        area.set(
                            plane,
                            (origin.0 + x as i32, origin.1 + y as i32),
                            Ground {
                                underlay: tile.underlay,
                                overlay: tile.overlay.map_or(0, |o| o.raw_id & 0x7fff),
                                shape: tile.overlay.map_or(0, |o| o.shape),
                                settings: tile.settings.unwrap_or(0),
                                height: 0,
                                height_known: false,
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
                    let info = match definitions.object(loc.object_id)? {
                        Some(def) => ObjectInfo {
                            name: def.name.clone().unwrap_or_default().to_lowercase(),
                            symbol: String::new(),
                            actions: def
                                .actions
                                .iter()
                                .flatten()
                                .map(|a| a.to_lowercase())
                                .collect(),
                            size: (def.size_x.min(255) as u8, def.size_y.min(255) as u8),
                        },
                        None => ObjectInfo::default(),
                    };
                    names.insert(loc.object_id.get(), info.name.clone());
                    slot.insert(info);
                }
            }
            for building in analyze(&area, &placements, &objects) {
                let tiles: Vec<(i32, i32)> = building.tiles().collect();
                let roofed: f32 = building
                    .rooms
                    .iter()
                    .map(|r| r.roofed * r.tiles.len() as f32)
                    .sum::<f32>()
                    / tiles.len().max(1) as f32;
                if tiles.len() < 12 || roofed < 0.3 {
                    continue;
                }
                let (min, max) = building.bounds();
                let roof_plane = building.plane + 1;
                let pieces: Vec<_> = placements
                    .iter()
                    .filter(|p| {
                        p.plane == roof_plane
                            && (12..=21).contains(&p.kind)
                            && p.tile.0 >= min.0 - 2
                            && p.tile.0 <= max.0 + 2
                            && p.tile.1 >= min.1 - 2
                            && p.tile.1 <= max.1 + 2
                    })
                    .map(|p| {
                        let size = objects.get(&p.id).map_or((1, 1), |i| i.size);
                        json!([
                            p.id.get(),
                            p.kind,
                            p.orientation,
                            p.tile.0 - min.0,
                            p.tile.1 - min.1,
                            size.0,
                            size.1
                        ])
                    })
                    .collect();
                buildings.push(json!({
                    "region": [rx, ry],
                    "origin": [min.0, min.1],
                    "plane": building.plane,
                    "rooms": building.rooms.len(),
                    "tiles": tiles.iter().map(|t| json!([t.0 - min.0, t.1 - min.1])).collect::<Vec<_>>(),
                    "roof": pieces,
                }));
            }
        }
    }
    let used: HashSet<u32> = buildings
        .iter()
        .flat_map(|b| b["roof"].as_array().into_iter().flatten())
        .filter_map(|p| p[0].as_u64().map(|v| v as u32))
        .collect();
    let names: BTreeMap<String, String> = names
        .into_iter()
        .filter(|(id, _)| used.contains(id))
        .map(|(id, name)| (id.to_string(), name))
        .collect();
    println!("{} roofed buildings", buildings.len());
    std::fs::write(
        out,
        serde_json::to_string(&json!({ "buildings": buildings, "names": names }))?,
    )?;
    Ok(())
}
