//! Bench: build a grid of regions sequentially; report per-region time and geometry size.
use osrs_core::coords::RegionCoord;
use osrs_world::{TerrainPresentation, WorldDefinitions, build_region_geometry};
use std::time::Instant;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let started = Instant::now();
    let mut defs = WorldDefinitions::open(&args[1])?;
    println!("open: {:?}", started.elapsed());
    let (x0, y0, n): (i32, i32, i32) = (args[2].parse()?, args[3].parse()?, args[4].parse()?);
    let mut total_vertices = 0usize;
    let mut total_ms = 0.0f32;
    let mut built = 0;
    for dx in 0..n {
        for dy in 0..n {
            let Some(region) = build_region_geometry(
                &mut defs,
                RegionCoord::new(x0 + dx, y0 + dy),
                TerrainPresentation::default(),
            )?
            else {
                continue;
            };
            total_vertices += region.geometry.vertex_count();
            total_ms += region.build_ms;
            built += 1;
        }
    }
    println!(
        "{built} regions, {:.1}M vertices ({:.0} MiB), {:.0} ms/region avg, wall {:?}",
        total_vertices as f64 / 1e6,
        total_vertices as f64 * 20.0 / 1048576.0,
        total_ms / built as f32,
        started.elapsed()
    );
    Ok(())
}
