//! Write the numbers that describe every real building ([`FEATURE_NAMES`]) as one compact JSON
//! file: the corpus a realism critic is trained against. Read-only.
//!
//! usage: building_features <OpenRune root> <out.json> [min_rx max_rx min_ry max_ry]
//!
//! Each row is `[width, height, split, ...numbers]`; the split (0 train, 1 validation, 2 test) is
//! the dataset's geographic split, so no region is in two splits. Set `CENSUS_EXCLUDE` to the
//! wiki's underground/instanced list to skip those map squares, as `building_census` does.
use osrs_cache::{openrune::installation::Installation, source::CacheSource};
use osrs_core::{coords::RegionCoord, ids::ObjectId};
use osrs_scene::{
    building_census::{CensusArea, FEATURE_NAMES, Ground, ObjectInfo, ObjectTable, analyze, judge},
    object_semantics::Placed,
    terrain_load::RegionTileHeight,
};
use osrs_world::WorldDefinitions;
use serde_json::json;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let usage = "usage: building_features <OpenRune root> <out.json> [min_rx max_rx min_ry max_ry]";
    let root = args.next().ok_or(usage)?;
    let out = args.next().ok_or(usage)?;
    let mut range = |default: i32| args.next().map_or(Ok(default), |v| v.parse::<i32>());
    let (min_x, max_x, min_y, max_y) = (range(28)?, range(80)?, range(30)?, range(170)?);
    let installation = Installation::read(&root)?;
    let loc_symbols: std::collections::HashMap<u32, String> = installation
        .symbols
        .table_symbols("loc")
        .into_iter()
        .map(|(id, name)| (id, name.strip_prefix("loc.").unwrap_or(name).to_lowercase()))
        .collect();
    let mut definitions =
        WorldDefinitions::open(CacheSource::Dir(installation.root.join(".data/cache/LIVE")))?;
    let mut excluded = std::collections::HashSet::new();
    if let Ok(path) = std::env::var("CENSUS_EXCLUDE") {
        let list: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(path)?)?;
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
    let mut rows = Vec::new();
    for rx in min_x..=max_x {
        for ry in min_y..=max_y {
            if excluded.contains(&(rx, ry)) {
                continue;
            }
            let region = RegionCoord::new(rx, ry);
            let Some(map) = definitions.region(region)? else {
                continue;
            };
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
                    let info = match definitions.object(loc.object_id)? {
                        Some(def) => ObjectInfo {
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
                        None => ObjectInfo::default(),
                    };
                    slot.insert(info);
                }
            }
            let split = match (rx.div_euclid(8) * 31 + ry.div_euclid(8)).rem_euclid(5) {
                0 => 2,
                1 => 1,
                _ => 0,
            };
            for building in analyze(&area, &placements, &objects) {
                let judgement = judge(&building);
                if judgement.rooms == 0 {
                    continue;
                }
                let (width, height) = building.size();
                let mut row = vec![json!(width), json!(height), json!(split)];
                row.extend(
                    judgement
                        .features
                        .iter()
                        .map(|v| json!((f64::from(*v) * 1000.0).round() / 1000.0)),
                );
                rows.push(serde_json::Value::Array(row));
            }
        }
    }
    let _ = ObjectId::new(0);
    let file = json!({ "names": FEATURE_NAMES, "rows": rows });
    std::fs::write(&out, serde_json::to_string(&file)?)?;
    println!(
        "{} buildings -> {out}",
        file["rows"].as_array().map_or(0, Vec::len)
    );
    Ok(())
}
