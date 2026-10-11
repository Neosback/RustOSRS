//! How real maps lay overlay shapes: for every overlay tile, the pattern of which of its eight
//! neighbours carry the same overlay id, against the (shape, rotation) the mapper chose.
//!
//! `RUSTOSRS_TARGET_CACHE_DIR=... cargo run --release -p osrs-world --example shape_census > out.txt`
//!
//! Output lines: `pattern shape rotation count`, pattern = bits N,NE,E,SE,S,SW,W,NW (bit 0 = N).

use osrs_core::coords::RegionCoord;
use osrs_world::WorldDefinitions;
use std::collections::BTreeMap;

const NEIGHBOURS: [(i32, i32); 8] = [
    (0, 1),
    (1, 1),
    (1, 0),
    (1, -1),
    (0, -1),
    (-1, -1),
    (-1, 0),
    (-1, 1),
];

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dir = std::env::var("RUSTOSRS_TARGET_CACHE_DIR")?;
    let definitions = WorldDefinitions::open(dir)?;
    let mut counts: BTreeMap<(u8, u8, u8), u64> = BTreeMap::new();
    let mut regions = 0;
    for x in 0..=120 {
        for y in 0..=220 {
            let Ok(Some(map)) = definitions.region(RegionCoord::new(x, y)) else {
                continue;
            };
            regions += 1;
            for plane in 0..4 {
                // Overlay id per tile of this region plane (0 = none); neighbours across the region
                // edge are unknown and count as absent, so edge tiles are skipped.
                let id_at = |lx: i32, ly: i32| -> u16 {
                    if !(0..64).contains(&lx) || !(0..64).contains(&ly) {
                        return u16::MAX;
                    }
                    map.terrain
                        .tile(plane, lx as usize, ly as usize)
                        .overlay
                        .map_or(0, |o| o.raw_id & 0x7fff)
                };
                for lx in 1..63 {
                    for ly in 1..63 {
                        let tile = map.terrain.tile(plane, lx as usize, ly as usize);
                        let Some(overlay) = tile.overlay else {
                            continue;
                        };
                        let id = overlay.raw_id & 0x7fff;
                        let mut pattern = 0u8;
                        for (bit, (dx, dy)) in NEIGHBOURS.iter().enumerate() {
                            if id_at(lx + dx, ly + dy) == id {
                                pattern |= 1 << bit;
                            }
                        }
                        *counts
                            .entry((pattern, overlay.shape, overlay.rotation))
                            .or_default() += 1;
                    }
                }
            }
        }
    }
    eprintln!("{regions} regions");
    for ((pattern, shape, rotation), count) in counts {
        println!("{pattern} {shape} {rotation} {count}");
    }
    Ok(())
}
