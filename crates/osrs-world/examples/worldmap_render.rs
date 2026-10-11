//! Render world-map regions like the client. usage: worldmap_render <cache> <out.ppm> <ppt> <x0> <y0> <w> <h> [area]
//! Regions are map regions; the image is `w x h` regions, north up.
use osrs_world::{
    WorldDefinitions,
    worldmap::{Catalog, RegionRenderer},
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let a: Vec<String> = std::env::args().collect();
    let mut defs = WorldDefinitions::open(&a[1])?;
    let ppt: i32 = a[3].parse()?;
    let (x0, y0, w, h): (i32, i32, i32, i32) =
        (a[4].parse()?, a[5].parse()?, a[6].parse()?, a[7].parse()?);
    let catalog = Catalog::load(&defs)?;
    for (index, area) in catalog.areas.iter().enumerate() {
        println!(
            "{index}: {} / {} id {} main {} zoom {} regions x {}..{} y {}..{} origin {:?}",
            area.internal_name,
            area.external_name,
            area.id,
            area.is_main,
            area.zoom_percent,
            area.low_x,
            area.high_x,
            area.low_y,
            area.high_y,
            area.origin
        );
    }
    println!("{} elements", catalog.elements.len());
    let area = a.get(8).map_or(Ok(catalog.main_area()), |s| s.parse())?;
    let mut renderer = RegionRenderer::new();
    let side = 64 * ppt;
    let (width, height) = ((w * side) as usize, (h * side) as usize);
    let mut out = vec![0_u8; width * height * 3];
    for rx in 0..w {
        for ry in 0..h {
            if let Some(image) = renderer.render(&mut defs, &catalog, area, (x0 + rx, y0 + ry), ppt)
            {
                let px = (rx * side) as usize;
                let py = ((h - 1 - ry) * side) as usize;
                for row in 0..side as usize {
                    for col in 0..side as usize {
                        let s = (row * side as usize + col) * 4;
                        let d = ((py + row) * width + px + col) * 3;
                        out[d..d + 3].copy_from_slice(&image.rgba[s..s + 3]);
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
