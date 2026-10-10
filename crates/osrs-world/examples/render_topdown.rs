//! Debug: render an assembled scene window top-down to a PPM (convert with `sips -s format png`).
//!
//! usage: render_topdown <cache-dir> <base-x> <base-y> <plane> <out.ppm>

use osrs_core::{
    color_palette::build_color_palette,
    coords::{SceneTile, StoragePlane},
};
use osrs_scene::terrain::TerrainSurface;
use osrs_world::{
    LocRenderable, SceneWindow, TerrainPresentation, WorldDefinitions, build_world_scene,
};
use std::{env, fs, io::Write};

const SCALE: usize = 8;
const TILES: usize = 104;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    let mut definitions = WorldDefinitions::open(&args[1])?;
    let window =
        SceneWindow::new(args[2].parse()?, args[3].parse()?).ok_or("window must be 8-aligned")?;
    let plane: u8 = args[4].parse()?;
    let world = build_world_scene(&mut definitions, window, TerrainPresentation::default())?;
    let palette = build_color_palette(0.8);
    let rgb = |hsl: i32| -> [u8; 3] {
        let value = palette.get((hsl as usize) & 0xFFFF).copied().unwrap_or(0);
        [(value >> 16) as u8, (value >> 8) as u8, value as u8]
    };

    let width = TILES * SCALE;
    let mut image = vec![[20u8, 20, 30]; width * width];
    let storage = StoragePlane::new(plane).ok_or("plane")?;

    for x in 0..TILES {
        for y in 0..TILES {
            let Some(tile) = world
                .scene
                .tile(storage, SceneTile::new(x as u32, y as u32))
            else {
                continue;
            };
            let color = match &tile.terrain {
                Some(TerrainSurface::Flat(flat)) => rgb(flat.colors.southwest),
                Some(TerrainSurface::Shaped(model)) => {
                    let face = model.faces.iter().find(|f| f.colors[0] != 12_345_678);
                    face.map_or([60, 60, 60], |f| rgb(f.colors[0]))
                }
                None => continue,
            };
            paint(&mut image, width, x, y, 1, 1, color);
        }
    }
    let mut counts = [0usize; 4];
    for loc in &world.locs {
        if loc.plane != storage || loc.renderables.is_empty() {
            continue;
        }
        let first = match loc.renderables[0] {
            LocRenderable::Lit(index) => world.lit.get(index),
            LocRenderable::ModelData(id) => world.finalizer.lit_model(id),
            LocRenderable::Animated(_) | LocRenderable::Omitted => None,
        };
        let animated = matches!(loc.renderables[0], LocRenderable::Animated(_));
        let textured = first.is_some_and(|m| {
            m.face_textures
                .as_ref()
                .is_some_and(|t| t.iter().any(Option::is_some))
        });
        counts[usize::from(animated) + 2 * usize::from(textured)] += 1;
        if !animated && std::env::var("MAGENTA").is_ok() {
            let hidden = first.map(|m| m.face_colors.iter().filter(|c| c.c == -2).count());
            let total = first.map(|m| m.face_colors.len());
            if first.is_none() || hidden == total {
                let name = definitions
                    .object(loc.object_id)?
                    .and_then(|d| d.name.clone());
                println!(
                    "magenta: id={} type={} tile=({},{}) faces={total:?} hidden={hidden:?} name={name:?}",
                    loc.object_id.get(),
                    loc.loc_type,
                    loc.tile.x,
                    loc.tile.y
                );
            }
        }
        let color = if animated {
            [0, 255, 255]
        } else {
            first
                .and_then(|m| m.face_colors.iter().find(|c| c.c != -2).map(|c| rgb(c.a)))
                .unwrap_or([255, 0, 255])
        };
        let fp = loc.plan.kind.storage_footprint();
        let (w, h) = match loc.loc_type {
            0..=8 => (1, 1),
            _ => (usize::from(fp.width), usize::from(fp.depth)),
        };
        paint(
            &mut image,
            width,
            loc.tile.x as usize,
            loc.tile.y as usize,
            w,
            h,
            color,
        );
    }

    let mut out = fs::File::create(&args[5])?;
    write!(out, "P6\n{width} {width}\n255\n")?;
    // Flip so north (+y) is up.
    for row in (0..width).rev() {
        for column in 0..width {
            out.write_all(&image[row * width + column])?;
        }
    }
    println!("stats: {:?}", world.loc_stats);
    println!(
        "plane {plane} locs: static-untextured={} animated={} textured-static={} textured-animated={}",
        counts[0], counts[1], counts[2], counts[3]
    );
    Ok(())
}

fn paint(
    image: &mut [[u8; 3]],
    width: usize,
    x: usize,
    y: usize,
    w: usize,
    h: usize,
    color: [u8; 3],
) {
    for py in y * SCALE..((y + h) * SCALE).min(width) {
        for px in x * SCALE..((x + w) * SCALE).min(width) {
            image[py * width + px] = color;
        }
    }
}
