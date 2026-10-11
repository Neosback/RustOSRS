//! Render minimap regions like the client. usage: minimap_render <cache> <out.ppm> <plane> <x0> <y0> <w> <h>
use osrs_world::{WorldDefinitions, minimap, worldmap::Catalog};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let a: Vec<String> = std::env::args().collect();
    let mut defs = WorldDefinitions::open(&a[1])?;
    let plane: u8 = a[3].parse()?;
    let (x0, y0, w, h): (i32, i32, i32, i32) =
        (a[4].parse()?, a[5].parse()?, a[6].parse()?, a[7].parse()?);
    let catalog = Catalog::load(&defs)?;
    let side = minimap::REGION_PX;
    let (width, height) = (w as usize * side, h as usize * side);
    let mut out = vec![0_u8; width * height * 3];
    for rx in 0..w {
        for ry in 0..h {
            if let Some(region) =
                minimap::render_region(&mut defs, &catalog, (x0 + rx, y0 + ry), plane)?
            {
                println!(
                    "region {},{}: {} icons",
                    x0 + rx,
                    y0 + ry,
                    region.icons.len()
                );
                let px = rx as usize * side;
                let py = (h - 1 - ry) as usize * side;
                for row in 0..side {
                    for col in 0..side {
                        let s = (row * side + col) * 4;
                        let d = ((py + row) * width + px + col) * 3;
                        out[d..d + 3].copy_from_slice(&region.rgba[s..s + 3]);
                    }
                }
            }
        }
    }
    let mut file = format!("P6\n{width} {height}\n255\n").into_bytes();
    file.extend(out);
    std::fs::write(&a[2], file)?;
    Ok(())
}
