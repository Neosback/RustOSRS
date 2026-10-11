//! Dump the structure of the world map archives. usage: worldmap_probe <cache>
use osrs_world::WorldDefinitions;

fn hash(name: &str) -> i32 {
    name.bytes().fold(0_i32, |h, b| h.wrapping_mul(31).wrapping_add(i32::from(b)))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let a: Vec<String> = std::env::args().collect();
    let defs = WorldDefinitions::open(&a[1])?;
    let cache = defs.cache();
    for name in ["details", "compositemap", "compositetexture", "area", "labels", "main"] {
        println!("hash {name} = {}", hash(name));
    }
    for group in cache.group_ids(19)? {
        let m = cache.group_metadata(19, group)?;
        println!("19/{group}: name_hash {} files {}", m.name_hash, m.file_ids.len());
        if m.file_ids.len() < 80 {
            println!("   ids {:?}", m.file_ids);
        }
    }
    // fonts index 13
    for g in cache.group_ids(13)?.into_iter().take(40) {
        let m = cache.group_metadata(13, g)?;
        println!("13/{g}: name_hash {} files {}", m.name_hash, m.file_ids.len());
    }
    for name in ["verdana_11pt_regular", "verdana_13pt_regular", "verdana_15pt_regular", "p11_full", "p12_full", "b12_full", "mapscene"] {
        println!("hash {name} = {}", hash(name));
    }
    Ok(())
}
