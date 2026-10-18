//! Version-2 exact building corpus. usage: building_dataset <OpenRune root> <out> [min_rx max_rx min_ry max_ry]
//! Neighbor context and canonical terrain/collision planners replace raw height-byte statistics.
#[path = "building_dataset/areas.rs"]
mod areas;
#[path = "building_dataset/context.rs"]
mod context;
#[path = "building_dataset/record.rs"]
mod record;
use osrs_cache::{openrune::installation::Installation, source::CacheSource};
use osrs_core::{
    coords::{MapTile, RegionCoord},
    definitions::ObjectParameter,
};
use osrs_scene::building_census::{ObjectTable, analyze, description::describe_with_census};
use osrs_world::WorldDefinitions;
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::Write,
};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let root = args.next().ok_or("OpenRune installation root required")?;
    let out = std::path::PathBuf::from(args.next().ok_or("output directory required")?);
    let mut range = |default| args.next().map_or(Ok(default), |s| s.parse::<i32>());
    let (min_x, max_x, min_y, max_y) = (range(28)?, range(80)?, range(30)?, range(170)?);
    std::fs::create_dir_all(&out)?;
    let installation = Installation::read(&root)?;
    let symbols = installation
        .symbols
        .table_symbols("loc")
        .into_iter()
        .map(|(id, name)| (id, name.strip_prefix("loc.").unwrap_or(name).to_lowercase()))
        .collect();
    let mut definitions =
        WorldDefinitions::open(CacheSource::Dir(installation.root.join(".data/cache/LIVE")))?;
    let source = json!({"cache_fingerprint":definitions.cache().fingerprint(),"profile":definitions.profile_id(),"build":definitions.build(),"exporter":"building_dataset-v2","deob_revision":"1ad572d7dcdbc0fb67a4a00f0c2f959d5ab25abc"});
    let areas = std::env::var("AREAS")
        .ok()
        .map(|p| areas::read_areas(&p))
        .transpose()?
        .unwrap_or_default();
    let mut excluded = BTreeSet::new();
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
    let mut seen = BTreeSet::new();
    let mut lines = std::io::BufWriter::new(std::fs::File::create(out.join("buildings.jsonl"))?);
    let mut written = 0;
    for rx in min_x..=max_x {
        for ry in min_y..=max_y {
            if excluded.contains(&(rx, ry)) {
                continue;
            }
            let key = RegionCoord::new(rx, ry);
            if definitions.region(key)?.is_none() {
                continue;
            }
            let context = context::load(&mut definitions, key, &mut objects, &symbols)?;
            let buildings = analyze(&context.area, &context.placements, &objects);
            for building in &buildings {
                for room in &building.rooms {
                    let Some(&(x, y)) = room.tiles.first() else {
                        continue;
                    };
                    let Some(description) = describe_with_census(
                        &context.area,
                        &context.placements,
                        &objects,
                        &buildings,
                        MapTile::new(x, y),
                        room.plane,
                    ) else {
                        continue;
                    };
                    let Some((&base, floors)) = description.floors.first_key_value() else {
                        continue;
                    };
                    let Some(&first) = floors.first() else {
                        continue;
                    };
                    if first.split_region().0 != key || !seen.insert((base, first)) {
                        continue;
                    }
                    writeln!(
                        lines,
                        "{}",
                        record::record(&context, &description, building, &objects, &areas, &source)
                    )?;
                    written += 1;
                }
            }
        }
    }
    lines.flush()?;
    let mut table = BTreeMap::new();
    for (id, info) in &objects {
        let definition = definitions.object(*id)?;
        let parameters = definition.as_ref().map(|d| {
            d.parameters
                .iter()
                .map(|(k, v)| {
                    (
                        k.to_string(),
                        match v {
                            ObjectParameter::Integer(n) => json!({"type":"integer","value":n}),
                            ObjectParameter::String(s) => json!({"type":"string","value":s}),
                        },
                    )
                })
                .collect::<BTreeMap<_, _>>()
        });
        table.insert(id.get().to_string(),json!({"name":info.name,"symbol":info.symbol,"actions":info.actions,"size":[info.size.0,info.size.1],"category":definition.as_ref().and_then(|d|d.category.map(|c|c.get())),"map_scene":definition.as_ref().and_then(|d|d.map_scene.map(|s|s.get())),"map_icon":definition.as_ref().and_then(|d|d.map_icon.map(|i|i.get())),"blocking_mask":definition.as_ref().map(|d|d.blocking_mask),"support_items":definition.as_ref().and_then(|d|d.support_items),"parameters":parameters,"translation":definition.as_ref().map(|d|[d.translation.x,d.translation.y,d.translation.z]),"scale":definition.as_ref().map(|d|[d.scale.x,d.scale.y,d.scale.z]),"mirrored":definition.as_ref().map(|d|d.is_rotated),"morphs":definition.as_ref().and_then(|d|d.morphs.as_ref().map(|m|json!({"varbit":m.transform_varbit.map(|v|v.get()),"varp":m.transform_varp.map(|v|v.get()),"transforms":m.transforms.iter().map(|v|v.map(|id|id.get())).collect::<Vec<_>>(),"fallback":m.fallback.map(|v|v.get())}))),"decoration_displacement":definition.as_ref().map(|d|d.decoration_displacement),"contour_clip":definition.as_ref().and_then(|d|d.contour_clip)}));
    }
    std::fs::write(out.join("objects.json"), serde_json::to_vec(&table)?)?;
    std::fs::write(
        out.join("manifest.json"),
        serde_json::to_vec_pretty(
            &json!({"format":"rustosrs-building-dataset","version":2,"source":source,"buildings":written,"objects":objects.len(),"splits":"geographic 8x8-region groups; rotations retain the source group","area_labels":"weak; not confirmed","human_labels":"separate reviewed prefab metadata"}),
        )?,
    )?;
    println!(
        "{written} buildings, {} object definitions -> {}",
        objects.len(),
        out.display()
    );
    Ok(())
}
