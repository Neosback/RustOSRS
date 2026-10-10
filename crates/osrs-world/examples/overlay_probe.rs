//! Debug: overlay ids/shapes around a world tile and the overlay definition fields.
use osrs_scene::terrain_build::FloorLookup;
use osrs_world::{SceneWindow, TerrainPresentation, WorldDefinitions, build_world_scene};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let mut defs = WorldDefinitions::open(&args[1])?;
    let (wx, wy): (i32, i32) = (args[2].parse()?, args[3].parse()?);
    let window = SceneWindow::new((wx - 52).div_euclid(8) * 8, (wy - 52).div_euclid(8) * 8).ok_or("align")?;
    let world = build_world_scene(&mut defs, window, TerrainPresentation::default())?;
    let mut seen = std::collections::BTreeSet::new();
    for dy in (-6..=6).rev() {
        let mut line = String::new();
        for dx in -8..=8 {
            let (x, y) = ((wx + dx - window.base_x) as usize, (wy + dy - window.base_y) as usize);
            let overlay = world.load.overlay_raw(0, x, y);
            let underlay = world.load.underlay(0, x, y);
            seen.insert(overlay);
            line += &format!("{:>3}/{:<3}s{}r{} ", overlay, underlay, world.load.shape(0, x, y), world.load.rotation(0, x, y));
        }
        println!("{line}");
    }
    for id in seen {
        if id == 0 { continue; }
        // raw overlay id is stored +1 in the map file
        let o = defs.floors().overlay(u32::from(id) - 1);
        println!("overlay raw {id}: {o:?}");
    }
    Ok(())
}
