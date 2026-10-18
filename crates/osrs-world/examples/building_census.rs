//! Measure how real buildings are made: rooms, inside and outside walls, doors, windows,
//! decorations, furniture, floors, roofs and the ground around them. Read-only.
//!
//! usage: building_census <OpenRune root> [min_rx max_rx min_ry max_ry]
use osrs_cache::{openrune::installation::Installation, source::CacheSource};
use osrs_core::{coords::RegionCoord, ids::ObjectId};
use osrs_scene::{
    building_census::{
        Building, CensusArea, Dir, EdgeFeature, Ground, Material, ObjectInfo, ObjectTable, RoomTag,
        analyze,
    },
    object_semantics::Placed,
    terrain_load::RegionTileHeight,
};
use osrs_world::WorldDefinitions;
use std::collections::BTreeMap;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let root = args
        .next()
        .ok_or("usage: building_census <OpenRune root> [min_rx max_rx min_ry max_ry]")?;
    let mut range = |default: i32| args.next().map_or(Ok(default), |v| v.parse::<i32>());
    let (min_x, max_x, min_y, max_y) = (range(28)?, range(80)?, range(30)?, range(170)?);
    let installation = Installation::read(&root)?;
    let symbol = |table: &str| -> std::collections::HashMap<u32, String> {
        installation
            .symbols
            .table_symbols(table)
            .into_iter()
            .map(|(id, name)| {
                (
                    id,
                    name.strip_prefix(&format!("{table}."))
                        .unwrap_or(name)
                        .to_lowercase(),
                )
            })
            .collect()
    };
    let loc_symbols = symbol("loc");
    let mut definitions =
        WorldDefinitions::open(CacheSource::Dir(installation.root.join(".data/cache/LIVE")))?;

    // CENSUS_EXCLUDE=user_world_defs.json (the wiki's underground and instanced map areas): skip
    // the map squares those areas use, so dungeons and cutscene copies do not count as buildings.
    let mut excluded = std::collections::HashSet::new();
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
        println!("excluding {} map squares", excluded.len());
    }
    let mut objects = ObjectTable::new();
    let mut buildings: Vec<(RegionCoord, Building)> = Vec::new();
    let mut regions = 0;
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
            for building in analyze(&area, &placements, &objects) {
                buildings.push((region, building));
            }
        }
    }
    println!("regions={regions} buildings={}", buildings.len());
    // CENSUS_REGION="49,53": list what was found in that region.
    if let Ok(want) = std::env::var("CENSUS_REGION") {
        for (region, b) in &buildings {
            if format!("{},{}", region.x, region.y) != want {
                continue;
            }
            let (min, max) = b.bounds();
            let tiles: usize = b.rooms.iter().map(|r| r.tiles.len()).sum();
            let roofed: f32 = b
                .rooms
                .iter()
                .map(|r| r.roofed * r.tiles.len() as f32)
                .sum::<f32>()
                / tiles as f32;
            println!(
                "plane {} {}..{} x {}..{}: {} rooms {} tiles, roofed {:.0}%, tags {:?}",
                b.plane,
                min.0,
                max.0,
                min.1,
                max.1,
                b.rooms.len(),
                tiles,
                roofed * 100.0,
                b.rooms.iter().map(|r| r.tag.word()).collect::<Vec<_>>()
            );
        }
    }
    let palette = osrs_world::load_floor_palette(CacheSource::Dir(
        installation.root.join(".data/cache/LIVE"),
    ))?;
    report(&buildings, &objects, &palette);
    Ok(())
}

fn label(objects: &ObjectTable, id: ObjectId) -> String {
    let info = objects.get(&id);
    let symbol = info.map_or("", |i| i.symbol.as_str());
    let name = info.map_or("", |i| i.name.as_str());
    format!(
        "{} {}{}",
        id.get(),
        symbol,
        if name.is_empty() || name == "null" {
            String::new()
        } else {
            format!(" \"{name}\"")
        }
    )
}

fn top<K: Clone + Ord>(counts: &BTreeMap<K, u32>, n: usize) -> Vec<(K, u32)> {
    let mut all: Vec<_> = counts.iter().map(|(k, v)| (k.clone(), *v)).collect();
    all.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    all.truncate(n);
    all
}

fn material(m: &Material, palette: &osrs_world::FloorPalette) -> String {
    if m.overlay != 0 {
        match palette.overlays.iter().find(|o| o.id == m.overlay) {
            Some(o) => format!(
                "overlay {} rgb({},{},{}){} shape {}",
                m.overlay,
                o.rgb[0],
                o.rgb[1],
                o.rgb[2],
                o.texture.map_or(String::new(), |t| format!(" texture {t}")),
                m.shape
            ),
            None => format!("overlay {} shape {}", m.overlay, m.shape),
        }
    } else {
        match palette.underlays.iter().find(|u| u.id == m.underlay) {
            Some(u) => format!(
                "underlay {} rgb({},{},{})",
                m.underlay, u.rgb[0], u.rgb[1], u.rgb[2]
            ),
            None => format!("underlay {}", m.underlay),
        }
    }
}

fn report(
    all: &[(RegionCoord, Building)],
    objects: &ObjectTable,
    palette: &osrs_world::FloorPalette,
) {
    // Real houses and shops have a roof over most of them; the rest are fenced yards, ruins and
    // dungeons.
    let roofed: Vec<&Building> = all
        .iter()
        .map(|(_, b)| b)
        .filter(|b| {
            let tiles: usize = b.rooms.iter().map(|r| r.tiles.len()).sum();
            let covered: f32 = b
                .rooms
                .iter()
                .map(|r| r.roofed * r.tiles.len() as f32)
                .sum();
            tiles >= 12 && covered / tiles as f32 >= 0.3
        })
        .collect();
    println!(
        "\n== buildings: {} found, {} roofed (>= 30% roof, >= 12 tiles) ==",
        all.len(),
        roofed.len()
    );
    let mut sizes = BTreeMap::new();
    for b in &roofed {
        let (w, h) = b.size();
        *sizes
            .entry(((w.min(w.max(h)) / 2) * 2, (w.max(h) / 2) * 2))
            .or_insert(0u32) += 1;
    }
    println!("size classes (short, long), by 2: {:?}", top(&sizes, 12));
    let rooms_per: BTreeMap<usize, u32> = roofed.iter().fold(BTreeMap::new(), |mut m, b| {
        *m.entry(b.rooms.len().min(8)).or_default() += 1;
        m
    });
    println!("rooms per building: {rooms_per:?}");

    // Walls.
    let mut exterior: BTreeMap<ObjectId, u32> = BTreeMap::new();
    let mut interior: BTreeMap<ObjectId, u32> = BTreeMap::new();
    let mut doors_out: BTreeMap<ObjectId, u32> = BTreeMap::new();
    let mut doors_in: BTreeMap<ObjectId, u32> = BTreeMap::new();
    let mut windows: BTreeMap<ObjectId, u32> = BTreeMap::new();
    let mut openings = 0u32;
    let (mut ext_edges, mut ext_windows, mut ext_doors, mut int_edges, mut int_doors) =
        (0u32, 0u32, 0u32, 0u32, 0u32);
    for b in &roofed {
        for w in &b.walls {
            let target = match (w.exterior, w.piece.feature) {
                (true, EdgeFeature::Wall) => &mut exterior,
                (false, EdgeFeature::Wall) => &mut interior,
                (true, EdgeFeature::Door) => &mut doors_out,
                (false, EdgeFeature::Door) => &mut doors_in,
                (_, EdgeFeature::Window) => &mut windows,
                (_, EdgeFeature::Opening) => {
                    openings += 1;
                    continue;
                }
            };
            *target.entry(w.piece.object).or_default() += 1;
            if w.exterior {
                ext_edges += 1;
                ext_windows += u32::from(w.piece.feature == EdgeFeature::Window);
                ext_doors += u32::from(w.piece.feature == EdgeFeature::Door);
            } else {
                int_edges += 1;
                int_doors += u32::from(w.piece.feature == EdgeFeature::Door);
            }
        }
    }
    println!(
        "\n== walls ==\nopen doorways (gap in a wall run, no door) {openings}; exterior edges {ext_edges}: {:.1}% windows, {:.1}% doors; interior edges {int_edges}: {:.1}% doors",
        100.0 * f64::from(ext_windows) / f64::from(ext_edges.max(1)),
        100.0 * f64::from(ext_doors) / f64::from(ext_edges.max(1)),
        100.0 * f64::from(int_doors) / f64::from(int_edges.max(1))
    );
    for (title, counts) in [
        ("exterior walls", &exterior),
        ("interior walls", &interior),
        ("exterior doors", &doors_out),
        ("interior doors", &doors_in),
        ("windows", &windows),
    ] {
        println!("-- {title}");
        for (id, n) in top(counts, 14) {
            println!("   {n:6}  {}", label(objects, id));
        }
    }

    // Decorations by side.
    let mut deco_in: BTreeMap<ObjectId, u32> = BTreeMap::new();
    let mut deco_out: BTreeMap<ObjectId, u32> = BTreeMap::new();
    for b in &roofed {
        for r in &b.rooms {
            for d in &r.decorations {
                *(if d.inside {
                    &mut deco_in
                } else {
                    &mut deco_out
                })
                .entry(d.object)
                .or_default() += 1;
            }
        }
    }
    let mut deco_kind_of: BTreeMap<(u32, u8), u32> = BTreeMap::new();
    for b in &roofed {
        for r in &b.rooms {
            for d in &r.decorations {
                *deco_kind_of.entry((d.object.get(), d.kind)).or_default() += 1;
            }
        }
    }
    let kinds_of = |id: ObjectId| -> String {
        deco_kind_of
            .iter()
            .filter(|((o, _), _)| *o == id.get())
            .map(|((_, k), n)| format!("kind {k} x{n}"))
            .collect::<Vec<_>>()
            .join(", ")
    };
    println!("\n== wall decorations ==");
    for (title, counts) in [("inside", &deco_in), ("outside", &deco_out)] {
        println!("-- {title}");
        for (id, n) in top(counts, 14) {
            println!("   {n:6}  {}  [{}]", label(objects, id), kinds_of(id));
        }
    }

    // Wall decorations: which way they face compared with the wall they hang on.
    let mut facing: BTreeMap<(u8, bool), [u32; 5]> = BTreeMap::new();
    for b in &roofed {
        for r in &b.rooms {
            for d in &r.decorations {
                if let Some(side) = d.edge_side {
                    let relative = (i32::from(d.orientation) - side as i32).rem_euclid(4) as usize;
                    facing.entry((d.kind, d.inside)).or_default()[relative] += 1;
                } else {
                    facing.entry((d.kind, d.inside)).or_default()[4] += 1;
                }
            }
        }
    }
    println!(
        "\n== wall decoration facing: (kind, inside) -> orientation minus wall side, 0..3, then no wall found =="
    );
    for ((kind, inside), counts) in &facing {
        println!(
            "   kind {kind} {}: {counts:?}",
            if *inside { "inside" } else { "outside" }
        );
    }

    // The ground in front of exterior doors, and just outside the walls.
    let mut apron_first: BTreeMap<Material, u32> = BTreeMap::new();
    let mut apron_second: BTreeMap<Material, u32> = BTreeMap::new();
    for b in &roofed {
        for (first, second) in &b.door_aprons {
            *apron_first.entry(*first).or_default() += 1;
            *apron_second.entry(*second).or_default() += 1;
        }
    }
    println!("\n== ground in front of exterior doors ==");
    for (title, counts) in [
        ("first tile outside", &apron_first),
        ("second tile outside", &apron_second),
    ] {
        println!("-- {title}");
        for (m, n) in top(counts, 6) {
            println!("   {n:6}  {}", material(&m, palette));
        }
    }

    // Styles: buildings grouped by their most used exterior wall.
    println!("\n== styles: roofed buildings grouped by exterior wall ==");
    let mut styles: BTreeMap<ObjectId, Vec<&Building>> = BTreeMap::new();
    for b in &roofed {
        let mut walls: BTreeMap<ObjectId, u32> = BTreeMap::new();
        for w in b
            .walls
            .iter()
            .filter(|w| w.exterior && w.piece.feature == EdgeFeature::Wall)
        {
            *walls.entry(w.piece.object).or_default() += 1;
        }
        if let Some((id, _)) = top(&walls, 1).into_iter().next() {
            styles.entry(id).or_default().push(b);
        }
    }
    let mut ranked: Vec<_> = styles.iter().collect();
    ranked.sort_by_key(|entry| std::cmp::Reverse(entry.1.len()));
    for (wall, group) in ranked.into_iter().take(10) {
        println!("\n[{}] {} buildings", label(objects, *wall), group.len());
        let mut roofs: BTreeMap<ObjectId, u32> = BTreeMap::new();
        let mut doors: BTreeMap<ObjectId, u32> = BTreeMap::new();
        let mut windows: BTreeMap<ObjectId, u32> = BTreeMap::new();
        let mut inner: BTreeMap<ObjectId, u32> = BTreeMap::new();
        let mut deco_in: BTreeMap<ObjectId, u32> = BTreeMap::new();
        let mut deco_out: BTreeMap<ObjectId, u32> = BTreeMap::new();
        let mut floors: BTreeMap<Material, u32> = BTreeMap::new();
        let mut edge_objects: BTreeMap<ObjectId, u32> = BTreeMap::new();
        for b in group {
            for ((id, kind), n) in &b.roof {
                if (12..=17).contains(kind) {
                    *roofs.entry(*id).or_default() += n;
                } else {
                    *edge_objects.entry(*id).or_default() += n;
                }
            }
            for w in &b.walls {
                match (w.exterior, w.piece.feature) {
                    (_, EdgeFeature::Door) => *doors.entry(w.piece.object).or_default() += 1,
                    (_, EdgeFeature::Window) => *windows.entry(w.piece.object).or_default() += 1,
                    (false, EdgeFeature::Wall) => *inner.entry(w.piece.object).or_default() += 1,
                    _ => {}
                }
            }
            for r in &b.rooms {
                for (m, n) in &r.floor {
                    *floors.entry(*m).or_default() += n;
                }
                for d in &r.decorations {
                    *(if d.inside {
                        &mut deco_in
                    } else {
                        &mut deco_out
                    })
                    .entry(d.object)
                    .or_default() += 1;
                }
            }
        }
        let show = |title: &str, counts: &BTreeMap<ObjectId, u32>| {
            let line: Vec<String> = top(counts, 4)
                .into_iter()
                .map(|(id, n)| format!("{n}x {}", label(objects, id)))
                .collect();
            println!("   {title}: {}", line.join(" | "));
        };
        show("roof", &roofs);
        show("roof edge", &edge_objects);
        show("doors", &doors);
        show("window walls", &windows);
        show("interior walls", &inner);
        show("decor inside", &deco_in);
        show("decor outside", &deco_out);
        let line: Vec<String> = top(&floors, 4)
            .into_iter()
            .map(|(m, n)| format!("{n}: {}", material(&m, palette)))
            .collect();
        println!("   floors: {}", line.join(" | "));
    }

    // Rooms by tag.
    let mut tags: BTreeMap<RoomTag, Vec<&osrs_scene::building_census::Room>> = BTreeMap::new();
    for b in &roofed {
        for r in &b.rooms {
            tags.entry(r.tag).or_default().push(r);
        }
    }
    println!("\n== rooms by what is in them ==");
    for (tag, rooms) in &tags {
        let tiles: usize = rooms.iter().map(|r| r.tiles.len()).sum();
        println!(
            "\n[{}] {} rooms, average {:.1} tiles",
            tag.word(),
            rooms.len(),
            tiles as f64 / rooms.len() as f64
        );
        let mut floor: BTreeMap<Material, u32> = BTreeMap::new();
        let mut things: BTreeMap<ObjectId, (u32, u32, [u32; 4])> = BTreeMap::new();
        for r in rooms {
            for (m, n) in &r.floor {
                *floor.entry(*m).or_default() += n;
            }
            for f in &r.furniture {
                let e = things.entry(f.object).or_default();
                e.0 += 1;
                if let Some(side) = f.against {
                    e.1 += 1;
                    let relative = (i32::from(f.orientation) - side as i32).rem_euclid(4) as usize;
                    e.2[relative] += 1;
                }
            }
        }
        println!("  floors:");
        for (m, n) in top(&floor, 5) {
            println!("   {n:6}  {}", material(&m, palette));
        }
        println!(
            "  furniture (count, % against a wall, orientation relative to that wall side 0..3):"
        );
        let mut ranked: Vec<_> = things.iter().collect();
        ranked.sort_by_key(|entry| std::cmp::Reverse(entry.1.0));
        for (id, (n, against, rel)) in ranked.into_iter().take(10) {
            println!(
                "   {n:5}  {:3.0}%  {rel:?}  {}",
                100.0 * f64::from(*against) / f64::from(*n),
                label(objects, *id)
            );
        }
    }

    // Roofs and ground.
    let mut roof: BTreeMap<(ObjectId, u8), u32> = BTreeMap::new();
    let mut ring: BTreeMap<Material, u32> = BTreeMap::new();
    let (mut flatter, mut compared) = (0, 0);
    let mut inside_spreads = Vec::new();
    let mut ring_spreads = Vec::new();
    for b in &roofed {
        for (k, n) in &b.roof {
            *roof.entry(*k).or_default() += n;
        }
        for (m, n) in &b.ring {
            *ring.entry(*m).or_default() += n;
        }
        if b.spread_inside > 0 || b.spread_ring > 0 {
            compared += 1;
            flatter += u32::from(b.spread_inside <= b.spread_ring);
        }
        inside_spreads.push(b.spread_inside);
        ring_spreads.push(b.spread_ring);
    }
    inside_spreads.sort_unstable();
    ring_spreads.sort_unstable();
    let median = |v: &[i32]| v.get(v.len() / 2).copied().unwrap_or(0);
    println!("\n== roofs (object, loc kind) ==");
    for ((id, kind), n) in top(&roof, 14) {
        println!("   {n:6}  kind {kind}  {}", label(objects, id));
    }
    println!(
        "\n== ground ==\nauthored height spread (client units): median under buildings {}, median in the ring around them {}; under is flatter than ring in {flatter}/{compared}",
        median(&inside_spreads),
        median(&ring_spreads)
    );
    println!("ring materials (the ground just outside the walls):");
    for (m, n) in top(&ring, 10) {
        println!("   {n:6}  {}", material(&m, palette));
    }
    let _ = Dir::ALL;
}
