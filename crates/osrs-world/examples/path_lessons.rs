//! Write one path lesson per map square: the ground (floor overlays, blocked tiles, heights) and
//! the objects on the ground plane, as `osrs-lab` reads them (`PathLesson`). Read-only.
//!
//! usage: path_lessons <OpenRune root> <out dir> [min_rx max_rx min_ry max_ry]
//!
//! Lessons of one 8 by 8 block of map squares share a group, so a learner is never checked on
//! ground next to ground it learned from. Squares with no floor overlay at all are skipped.
use osrs_cache::{openrune::installation::Installation, source::CacheSource};
use osrs_core::coords::RegionCoord;
use osrs_scene::terrain_load::TerrainLoadGrid;
use osrs_world::WorldDefinitions;
use serde_json::json;
use std::collections::BTreeMap;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let usage = "usage: path_lessons <OpenRune root> <out dir> [min_rx max_rx min_ry max_ry]";
    let root = args.next().ok_or(usage)?;
    let out = std::path::PathBuf::from(args.next().ok_or(usage)?);
    let mut range = |default: i32| args.next().map_or(Ok(default), |v| v.parse::<i32>());
    let (min_x, max_x, min_y, max_y) = (range(28)?, range(80)?, range(30)?, range(170)?);
    std::fs::create_dir_all(&out)?;
    let installation = Installation::read(&root)?;
    let mut definitions =
        WorldDefinitions::open(CacheSource::Dir(installation.root.join(".data/cache/LIVE")))?;
    // Named places from the world map (`map_labels.json`), when MAP_LABELS points at the file.
    let labels: Vec<(String, i32, i32, u8)> = match std::env::var("MAP_LABELS") {
        Ok(path) => serde_json::from_str::<serde_json::Value>(&std::fs::read_to_string(path)?)?
            .as_array()
            .into_iter()
            .flatten()
            .filter(|l| l["plane"].as_i64().unwrap_or(0) == 0)
            .filter_map(|l| {
                Some((
                    l["name"].as_str()?.replace("<br>", " "),
                    i32::try_from(l["worldX"].as_i64()?).ok()?,
                    i32::try_from(l["worldY"].as_i64()?).ok()?,
                    u8::try_from(l["textScale"].as_i64().unwrap_or(0))
                        .unwrap_or(0)
                        .min(2),
                ))
            })
            .collect(),
        Err(_) => Vec::new(),
    };
    let mut names: BTreeMap<u32, serde_json::Value> = BTreeMap::new();
    let mut written = 0;
    for rx in min_x..=max_x {
        for ry in min_y..=max_y {
            let Some(map) = definitions.region(RegionCoord::new(rx, ry))? else {
                continue;
            };
            let origin = (rx * 64, ry * 64);
            let mut grid = TerrainLoadGrid::new(64, 64)?;
            grid.load_region(&map.terrain, (0, 0), origin, 0);
            let mut overlay = Vec::with_capacity(4096);
            let mut shape = Vec::with_capacity(4096);
            let mut rotation = Vec::with_capacity(4096);
            let mut underlay = Vec::with_capacity(4096);
            let mut flags = Vec::with_capacity(4096);
            let mut heights = Vec::with_capacity(4096);
            for y in 0..64 {
                for x in 0..64 {
                    overlay.push(grid.overlay_raw(0, x, y) & 0x7fff);
                    shape.push(grid.shape(0, x, y));
                    rotation.push(grid.rotation(0, x, y));
                    underlay.push(grid.underlay(0, x, y));
                    flags.push(grid.settings(0, x, y));
                    heights.push(grid.height(0, x, y));
                }
            }
            if overlay.iter().all(|o| *o == 0) {
                continue;
            }
            let mut objects = Vec::new();
            for loc in map.locations.locations() {
                if loc.source_plane.index().get() != 0 {
                    continue;
                }
                objects.push(json!([
                    loc.object_id.get(),
                    loc.loc_type,
                    loc.orientation,
                    loc.tile.x(),
                    loc.tile.y()
                ]));
                if let std::collections::btree_map::Entry::Vacant(slot) =
                    names.entry(loc.object_id.get())
                    && let Some(def) = definitions.object(loc.object_id)?
                {
                    slot.insert(json!({
                        "name": def.name.clone().unwrap_or_default().to_lowercase(),
                        "actions": def.actions.iter().flatten().map(|a| a.to_lowercase()).collect::<Vec<_>>(),
                        "size": [def.size_x.min(255), def.size_y.min(255)],
                        "blocks": def.placement.interact_type != 0,
                    }));
                }
            }
            let used: std::collections::BTreeSet<u32> = objects
                .iter()
                .filter_map(|o| o[0].as_u64().map(|v| v as u32))
                .collect();
            let region_names: BTreeMap<String, &serde_json::Value> = used
                .iter()
                .filter_map(|id| names.get(id).map(|n| (id.to_string(), n)))
                .collect();
            let lesson = json!({
                "format": "rustosrs-path-lesson",
                "version": 1,
                "name": format!("Map square {rx},{ry}"),
                "origin": [origin.0, origin.1],
                "group": format!("g{}_{}", rx.div_euclid(8), ry.div_euclid(8)),
                "width": 64,
                "height": 64,
                "overlay": overlay,
                "shape": shape,
                "rotation": rotation,
                "underlay": underlay,
                "flags": flags,
                "heights": heights,
                "objects": objects,
                "names": region_names,
                "places": labels
                    .iter()
                    .filter(|(_, x, y, _)| *x > origin.0 - 200 && *x < origin.0 + 264 && *y > origin.1 - 200 && *y < origin.1 + 264)
                    .map(|(name, x, y, size)| json!({"name": name, "x": x, "y": y, "size": size}))
                    .collect::<Vec<_>>(),
            });
            std::fs::write(
                out.join(format!("r{rx}_{ry}.lesson.json")),
                serde_json::to_vec(&lesson)?,
            )?;
            written += 1;
        }
    }
    println!("wrote {written} lessons to {}", out.display());
    Ok(())
}
