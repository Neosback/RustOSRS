//! Sweep: build every map square in a region range and report failures.
use osrs_core::coords::RegionCoord;
use osrs_world::{TerrainPresentation, WorldDefinitions, build_region_geometry};
use std::time::Instant;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let mut defs = WorldDefinitions::open(&args[1])?;
    let (x0, y0, x1, y1): (i32, i32, i32, i32) = (
        args[2].parse()?,
        args[3].parse()?,
        args[4].parse()?,
        args[5].parse()?,
    );
    let started = Instant::now();
    let (mut ok, mut empty, mut failed) = (0, 0, 0);
    let mut animated = 0usize;
    let mut slowest = (0.0f32, (0, 0));
    for x in x0..=x1 {
        for y in y0..=y1 {
            match build_region_geometry(
                &mut defs,
                RegionCoord::new(x, y),
                TerrainPresentation::default(),
            ) {
                Ok(Some(region)) => {
                    ok += 1;
                    animated += region.animations.len();
                    if region.build_ms > slowest.0 {
                        slowest = (region.build_ms, (x, y));
                    }
                }
                Ok(None) => empty += 1,
                Err(error) => {
                    failed += 1;
                    println!("FAIL ({x},{y}): {error}");
                }
            }
        }
    }
    println!(
        "ok={ok} empty={empty} failed={failed} animated={animated} slowest={:.0} ms at {:?}, wall {:?}",
        slowest.0,
        slowest.1,
        started.elapsed()
    );
    Ok(())
}
