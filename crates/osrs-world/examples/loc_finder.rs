//! Debug: find placements of object ids across all map squares and list adjacent pairs.
//! usage: loc_finder <cache> <id-a> <id-b>
use osrs_core::{coords::RegionCoord, ids::ObjectId};
use osrs_world::WorldDefinitions;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let defs = WorldDefinitions::open(&args[1])?;
    let (a, b): (u32, u32) = (args[2].parse()?, args[3].parse()?);
    let mut hits: Vec<(u32, i32, i32, u8, u8, u8)> = Vec::new();
    for rx in 0..100 {
        for ry in 0..200 {
            let Ok(Some(region)) = defs.region(RegionCoord::new(rx, ry)) else {
                continue;
            };
            for loc in region.locations.locations() {
                let id = loc.object_id.get();
                if id == a || id == b {
                    hits.push((
                        id,
                        rx * 64 + i32::from(loc.tile.x()),
                        ry * 64 + i32::from(loc.tile.y()),
                        loc.source_plane.index().get(),
                        loc.loc_type,
                        loc.orientation,
                    ));
                }
            }
        }
    }
    println!("{} hits", hits.len());
    for (i, h) in hits.iter().enumerate() {
        for g in &hits[i + 1..] {
            if h.0 != g.0 && h.3 == g.3 && (h.1 - g.1).abs() <= 1 && (h.2 - g.2).abs() <= 1 {
                println!("pair: {h:?} ~ {g:?}");
            }
        }
    }
    let _ = ObjectId::new(0);
    Ok(())
}
