//! Print a real area of the map as text, to see how a real building is built.
//!
//! usage: building_ascii_real <OpenRune root> <x0> <y0> <x1> <y1> [plane]
//! Walls #, doors D, windows W (as wall pieces), wall decorations ' (above the tile), roof pieces
//! on the plane above r, furniture by the first letter of its name, tile floor by overlay id.
use osrs_cache::{openrune::installation::Installation, source::CacheSource};
use osrs_core::coords::RegionCoord;
use osrs_scene::building_census::{Dir, EdgeFeature, Edges, ObjectInfo, ObjectTable};
use osrs_scene::object_semantics::Placed;
use osrs_world::WorldDefinitions;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [root, x0, y0, x1, y1, rest @ ..] = args.as_slice() else {
        return Err("usage: building_ascii_real <root> x0 y0 x1 y1 [plane]".into());
    };
    let (x0, y0, x1, y1): (i32, i32, i32, i32) =
        (x0.parse()?, y0.parse()?, x1.parse()?, y1.parse()?);
    let plane: u8 = rest.first().map_or(Ok(0), |p| p.parse())?;
    let installation = Installation::read(root)?;
    let symbols: std::collections::HashMap<u32, String> = installation
        .symbols
        .table_symbols("loc")
        .into_iter()
        .map(|(id, n)| (id, n.strip_prefix("loc.").unwrap_or(n).to_lowercase()))
        .collect();
    let mut definitions =
        WorldDefinitions::open(CacheSource::Dir(installation.root.join(".data/cache/LIVE")))?;
    let mut placements = Vec::new();
    let mut objects = ObjectTable::new();
    let mut overlays = std::collections::HashMap::new();
    for rx in x0.div_euclid(64)..=x1.div_euclid(64) {
        for ry in y0.div_euclid(64)..=y1.div_euclid(64) {
            let Some(map) = definitions.region(RegionCoord::new(rx, ry))? else {
                continue;
            };
            for x in 0..64 {
                for y in 0..64 {
                    let t = map.terrain.tile(usize::from(plane), x, y);
                    overlays.insert(
                        (rx * 64 + x as i32, ry * 64 + y as i32),
                        t.overlay.map_or(0, |o| o.raw_id & 0x7fff),
                    );
                }
            }
            for loc in map.locations.locations() {
                let tile = (
                    rx * 64 + i32::from(loc.tile.x()),
                    ry * 64 + i32::from(loc.tile.y()),
                );
                if tile.0 < x0 - 1 || tile.0 > x1 + 1 || tile.1 < y0 - 1 || tile.1 > y1 + 1 {
                    continue;
                }
                placements.push(Placed {
                    id: loc.object_id,
                    kind: loc.loc_type,
                    orientation: loc.orientation,
                    tile,
                    plane: loc.source_plane.index().get(),
                });
                if let std::collections::btree_map::Entry::Vacant(slot) =
                    objects.entry(loc.object_id)
                {
                    let info = definitions.object(loc.object_id)?.map_or_else(
                        ObjectInfo::default,
                        |def| ObjectInfo {
                            name: def.name.clone().unwrap_or_default().to_lowercase(),
                            symbol: symbols
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
                    );
                    slot.insert(info);
                }
            }
        }
    }
    let edges = Edges::of_plane(&placements, plane, &objects);
    let (w, h) = ((x1 - x0 + 1) as usize, (y1 - y0 + 1) as usize);
    let mut canvas = vec![vec![' '; 3 * w + 1]; 2 * h + 1];
    let mut legend: std::collections::BTreeMap<char, String> = Default::default();
    for y in y0..=y1 {
        for x in x0..=x1 {
            let (cx, cy) = (3 * (x - x0) as usize + 1, 2 * (y1 - y) as usize + 1);
            let overlay = overlays.get(&(x, y)).copied().unwrap_or(0);
            canvas[cy][cx] = if overlay == 0 {
                '.'
            } else {
                char::from_digit(u32::from(overlay % 36), 36).unwrap_or('?')
            };
            for dir in Dir::ALL {
                if let Some(piece) = edges.between((x, y), dir) {
                    let symbol = match piece.feature {
                        EdgeFeature::Door => 'D',
                        EdgeFeature::Window => 'W',
                        EdgeFeature::Opening => 'o',
                        EdgeFeature::Wall => '#',
                    };
                    let (ex, ey) = match dir {
                        Dir::West => (cx - 1, cy),
                        Dir::East => (cx + 1, cy),
                        Dir::North => (cx, cy - 1),
                        Dir::South => (cx, cy + 1),
                    };
                    canvas[ey][ex] = symbol;
                }
            }
        }
    }
    for p in placements.iter().filter(|p| p.plane == plane) {
        if p.tile.0 < x0 || p.tile.0 > x1 || p.tile.1 < y0 || p.tile.1 > y1 {
            continue;
        }
        let (cx, cy) = (
            3 * (p.tile.0 - x0) as usize + 1,
            2 * (y1 - p.tile.1) as usize + 1,
        );
        let info = objects.get(&p.id);
        let name = info.map_or("", |i| {
            if i.name.is_empty() {
                i.symbol.as_str()
            } else {
                i.name.as_str()
            }
        });
        match p.kind {
            10 | 11 => {
                let c = name.chars().next().unwrap_or('?').to_ascii_uppercase();
                canvas[cy][cx + 1] = c;
                legend
                    .entry(c)
                    .or_default()
                    .push_str(&format!("{} ", p.id.get()));
            }
            4..=8 => canvas[cy][cx + 1] = '\'',
            _ => {}
        }
    }
    for p in placements
        .iter()
        .filter(|p| p.plane == plane + 1 && (12..=21).contains(&p.kind))
    {
        if p.tile.0 < x0 || p.tile.0 > x1 || p.tile.1 < y0 || p.tile.1 > y1 {
            continue;
        }
        let (cx, cy) = (
            3 * (p.tile.0 - x0) as usize + 1,
            2 * (y1 - p.tile.1) as usize + 1,
        );
        canvas[cy][cx - 1] = 'r';
    }
    for (row, line) in canvas.iter().enumerate() {
        let y = y1 - ((row as i32 - 1).max(0) / 2);
        println!(
            "{:5} {}",
            if row % 2 == 1 {
                y.to_string()
            } else {
                String::new()
            },
            line.iter().collect::<String>()
        );
    }
    println!("furniture letters:");
    for (c, ids) in legend {
        let first: u32 = ids
            .split_whitespace()
            .next()
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);
        let label = objects
            .get(&osrs_core::ids::ObjectId::new(first))
            .map_or("", |i| i.symbol.as_str());
        println!("  {c}: {label} ({ids})");
    }
    Ok(())
}
