//! Count how many tiles of the whole map use each overlay and underlay (planes 0..3).
//! `RUSTOSRS_TARGET_CACHE_DIR=... cargo run --release -p osrs-world --example floor_usage`

use osrs_core::coords::RegionCoord;
use osrs_world::WorldDefinitions;
use std::collections::BTreeMap;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dir = std::env::var("RUSTOSRS_TARGET_CACHE_DIR")?;
    let definitions = WorldDefinitions::open(dir)?;
    let started = std::time::Instant::now();
    let mut overlays: BTreeMap<u16, u64> = BTreeMap::new();
    let mut underlays: BTreeMap<u16, u64> = BTreeMap::new();
    let mut regions = 0;
    for x in 0..=120 {
        for y in 0..=220 {
            let Ok(Some(map)) = definitions.region(RegionCoord::new(x, y)) else {
                continue;
            };
            regions += 1;
            for plane in 0..4 {
                for lx in 0..64 {
                    for ly in 0..64 {
                        let tile = map.terrain.tile(plane, lx, ly);
                        if let Some(overlay) = tile.overlay {
                            *overlays.entry(overlay.raw_id & 0x7fff).or_default() += 1;
                        }
                        if tile.underlay != 0 {
                            *underlays.entry(tile.underlay).or_default() += 1;
                        }
                    }
                }
            }
        }
    }
    println!("{regions} regions in {:?}", started.elapsed());
    println!("overlays used: {}", overlays.len());
    println!("underlays used: {}", underlays.len());
    let mut by_use: Vec<_> = overlays.iter().collect();
    by_use.sort_by_key(|(_, count)| std::cmp::Reverse(**count));
    println!("overlay usage (id: tiles): {:?}", by_use);
    let mut by_use: Vec<_> = underlays.iter().collect();
    by_use.sort_by_key(|(_, count)| std::cmp::Reverse(**count));
    println!("underlay usage (id: tiles): {:?}", by_use);
    Ok(())
}
