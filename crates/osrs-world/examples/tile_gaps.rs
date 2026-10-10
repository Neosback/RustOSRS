//! Debug: list tiles of a window with no terrain surface or with skipped faces.
use osrs_core::coords::{SceneTile, StoragePlane};
use osrs_scene::terrain::TerrainSurface;
use osrs_world::{SceneWindow, TerrainPresentation, WorldDefinitions, build_world_scene};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let mut defs = WorldDefinitions::open(&args[1])?;
    let window = SceneWindow::new(args[2].parse()?, args[3].parse()?).ok_or("align")?;
    let world = build_world_scene(&mut defs, window, TerrainPresentation::default())?;
    let plane = StoragePlane::new(0).unwrap();
    let (mut none, mut skipped_faces, mut skipped_flat, mut total) = (0, 0, 0, 0);
    let mut shown = 0;
    for x in 1..103u32 {
        for y in 1..103u32 {
            total += 1;
            let overlay = world.load.overlay_raw(0, x as usize, y as usize);
            let underlay = world.load.underlay(0, x as usize, y as usize);
            let shape = world.load.shape(0, x as usize, y as usize);
            let tile = world.scene.tile(plane, SceneTile::new(x, y));
            let Some(surface) = tile.and_then(|t| t.terrain.as_ref()) else {
                none += 1;
                if shown < 25 {
                    shown += 1;
                    println!("NONE ({},{}) overlay={overlay} underlay={underlay} shape={shape}", window.base_x + x as i32, window.base_y + y as i32);
                }
                continue;
            };
            match surface {
                TerrainSurface::Flat(f) => {
                    if f.colors.northeast == 12345678 {
                        skipped_flat += 1;
                        if shown < 25 { shown += 1; println!("SKIPFLAT ({},{}) overlay={overlay} underlay={underlay}", window.base_x + x as i32, window.base_y + y as i32); }
                    }
                }
                TerrainSurface::Shaped(s) => {
                    let skipped = s.faces.iter().filter(|f| f.colors[0] == 12345678).count();
                    if skipped > 0 {
                        skipped_faces += 1;
                        if shown < 25 { shown += 1; println!("SKIPFACES ({},{}) overlay={overlay} underlay={underlay} shape={shape} skipped={skipped}/{}", window.base_x + x as i32, window.base_y + y as i32, s.faces.len()); }
                    }
                }
            }
        }
    }
    println!("total={total} none={none} skipped_flat={skipped_flat} tiles_with_skipped_faces={skipped_faces}");
    Ok(())
}
