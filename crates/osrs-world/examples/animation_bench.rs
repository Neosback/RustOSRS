//! Bench: animated-loc counts and per-tick pose/geometry cost for a grid of regions.
use osrs_core::coords::RegionCoord;
use osrs_world::{AnimationSystem, TerrainPresentation, WorldDefinitions, build_region_geometry};
use std::time::Instant;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let mut defs = WorldDefinitions::open(&args[1])?;
    let (x0, y0, n): (i32, i32, i32) = (args[2].parse()?, args[3].parse()?, args[4].parse()?);
    let mut system = AnimationSystem::new();
    for dx in 0..n {
        for dy in 0..n {
            let region = RegionCoord::new(x0 + dx, y0 + dy);
            let Some(built) = build_region_geometry(&mut defs, region, TerrainPresentation::default())?
            else {
                continue;
            };
            println!(
                "region ({},{}): {} animated, {} static vertices, {:.0} ms",
                region.x,
                region.y,
                built.animations.len(),
                built.geometry.vertex_count(),
                built.build_ms
            );
            system.insert_region((region.x, region.y), built.animations);
        }
    }
    println!("total animated instances: {}", system.instance_count());
    let center = ((x0 * 64) + 32 * n, (y0 * 64) + 32 * n);
    let mut changed_ticks = 0;
    let started = Instant::now();
    let mut build_total = 0.0f64;
    let mut vertices = 0;
    for _ in 0..200 {
        if system.advance(1) {
            changed_ticks += 1;
            let t = Instant::now();
            let geometry = system.build_geometry(center, 64);
            build_total += t.elapsed().as_secs_f64() * 1000.0;
            vertices = geometry.vertex_count();
        }
    }
    println!(
        "200 cycles: {changed_ticks} with changes, last geometry {vertices} vertices, avg build {:.2} ms, wall {:?}",
        build_total / f64::from(changed_ticks.max(1)),
        started.elapsed()
    );
    Ok(())
}
