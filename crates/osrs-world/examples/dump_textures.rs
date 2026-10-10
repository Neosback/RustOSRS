//! Debug: dump every decoded texture into one atlas PPM (16 columns of 128x128).
use osrs_world::{TEXTURE_SIZE, WorldDefinitions};
use std::{fs, io::Write};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let defs = WorldDefinitions::open(&args[1])?;
    let table = defs.textures();
    let columns = 16;
    let rows = table.layer_count().div_ceil(columns);
    let (width, height) = (columns * TEXTURE_SIZE, rows * TEXTURE_SIZE);
    let mut image = vec![[255u8, 0, 255]; width * height];
    let mut present = 0;
    for id in 0..table.layer_count() {
        let Some(texture) = table.image(id as u32) else {
            continue;
        };
        present += 1;
        let (cx, cy) = ((id % columns) * TEXTURE_SIZE, (id / columns) * TEXTURE_SIZE);
        for (index, pixel) in texture.rgba.iter().enumerate() {
            let (x, y) = (index % TEXTURE_SIZE, index / TEXTURE_SIZE);
            // Transparent texels show as dark blue so cutouts are visible.
            image[(cy + y) * width + cx + x] = if pixel[3] == 0 {
                [0, 0, 60]
            } else {
                [pixel[0], pixel[1], pixel[2]]
            };
        }
    }
    println!(
        "textures present: {present} of {} layers",
        table.layer_count()
    );
    let mut out = fs::File::create(&args[2])?;
    write!(out, "P6\n{width} {height}\n255\n")?;
    for pixel in &image {
        out.write_all(pixel)?;
    }
    Ok(())
}
