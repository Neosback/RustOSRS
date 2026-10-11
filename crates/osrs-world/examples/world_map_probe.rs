//! Render a block of world-map regions to a PPM. usage: world_map_probe <cache> <rx0> <ry0> <rx1> <ry1> <out.ppm>
use osrs_core::coords::RegionCoord;
use osrs_world::{WORLD_MAP_IMAGE, WorldDefinitions, render_region_image};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let a: Vec<String> = std::env::args().collect();
    let defs = WorldDefinitions::open(&a[1])?;
    let (x0, y0, x1, y1): (i32, i32, i32, i32) =
        (a[2].parse()?, a[3].parse()?, a[4].parse()?, a[5].parse()?);
    let (w, h) = (
        (x1 - x0 + 1) as usize * WORLD_MAP_IMAGE,
        (y1 - y0 + 1) as usize * WORLD_MAP_IMAGE,
    );
    let mut canvas = vec![0_u8; w * h * 3];
    let started = std::time::Instant::now();
    let mut count = 0;
    for ry in y0..=y1 {
        for rx in x0..=x1 {
            if let Some(image) = render_region_image(&defs, RegionCoord::new(rx, ry))? {
                count += 1;
                let ox = (rx - x0) as usize * WORLD_MAP_IMAGE;
                let oy = (y1 - ry) as usize * WORLD_MAP_IMAGE;
                for row in 0..WORLD_MAP_IMAGE {
                    for col in 0..WORLD_MAP_IMAGE {
                        let src = (row * WORLD_MAP_IMAGE + col) * 4;
                        let dst = ((oy + row) * w + ox + col) * 3;
                        canvas[dst..dst + 3].copy_from_slice(&image[src..src + 3]);
                    }
                }
            }
        }
    }
    println!("{count} regions in {:?}", started.elapsed());
    let mut out = format!("P6\n{w} {h}\n255\n").into_bytes();
    out.extend_from_slice(&canvas);
    std::fs::write(&a[6], out)?;
    Ok(())
}
