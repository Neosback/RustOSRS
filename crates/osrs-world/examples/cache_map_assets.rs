//! Survey of the cache groups behind the minimap, compass and world map.
//! usage: cache_map_assets <cache-dir>
use osrs_world::WorldDefinitions;

fn name_hash(name: &str) -> i32 {
    name.bytes().fold(0_i32, |hash, byte| {
        hash.wrapping_mul(31).wrapping_add(i32::from(byte))
    })
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let defs = WorldDefinitions::open(&args[1])?;
    let cache = defs.cache();
    println!("content indexes: {:?}", cache.content_index_ids());
    for index in cache.content_index_ids() {
        let groups = cache.group_ids(index)?;
        println!("  index {index}: {} groups", groups.len());
    }
    // World map data: look for the named groups in every index.
    let wanted = [
        "details",
        "compositemap",
        "compositetexture",
        "area",
        "labels",
        "main",
        "main_icons",
    ];
    for index in cache.content_index_ids() {
        for group in cache.group_ids(index)? {
            let meta = cache.group_metadata(index, group)?;
            if let Some(name) = wanted.iter().find(|n| name_hash(n) == meta.name_hash) {
                println!(
                    "index {index} group {group} = {name:?}: files {} bytes {}",
                    meta.file_ids.len(),
                    meta.encoded_length
                );
            }
        }
    }
    for index in [17_u8, 18, 19, 20] {
        let ids = cache.group_ids(index)?;
        let named = ids
            .iter()
            .filter(|g| {
                cache
                    .group_metadata(index, **g)
                    .is_ok_and(|m| m.name_hash != 0)
            })
            .count();
        println!(
            "index {index}: {} groups, {named} with name hashes; first ids {:?}",
            ids.len(),
            &ids[..ids.len().min(8)]
        );
    }
    // Sprites (index 8): the compass, map back and wiki icon.
    for (name, id) in [
        ("MAPBACK", 12_u32),
        ("COMPASS", 169),
        ("WIKI_DESELECTED", 2420),
        ("WIKI_SELECTED", 2421),
        ("COMPASS_OUTLINE", 5813),
        ("ICON_COMPASS", 3039),
    ] {
        match cache.group_metadata(8, id) {
            Ok(meta) => println!(
                "sprite {name} ({id}): {} files, {} bytes",
                meta.file_ids.len(),
                meta.encoded_length
            ),
            Err(error) => println!("sprite {name} ({id}): missing ({error})"),
        }
    }
    Ok(())
}
