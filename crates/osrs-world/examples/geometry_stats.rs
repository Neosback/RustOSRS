//! Debug: vertex counts of a scene window by level and kind.
use osrs_world::{
    SceneWindow, TerrainPresentation, WorldDefinitions, build_world_scene, extract_scene_geometry,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let mut defs = WorldDefinitions::open(&args[1])?;
    let window = SceneWindow::new(args[2].parse()?, args[3].parse()?).ok_or("align")?;
    let world = build_world_scene(&mut defs, window, TerrainPresentation::default())?;
    let geometry = extract_scene_geometry(&world);
    let mut by_level = [[0usize; 2]; 4];
    let mut by_min = [0usize; 4];
    for zone in &geometry.zones {
        for group in &zone.groups {
            by_level[usize::from(group.level)][0] += group.geometry.opaque.len();
            by_level[usize::from(group.level)][1] += group.geometry.alpha.len();
            by_min[usize::from(group.min_plane)] += group.geometry.vertex_count();
        }
    }
    for (level, counts) in by_level.iter().enumerate() {
        println!("level {level}: opaque={} alpha={}", counts[0], counts[1]);
    }
    println!("by min_plane: {by_min:?}");
    let models: usize = world.lit.len()
        + world
            .locs
            .iter()
            .filter(|l| !l.renderables.is_empty())
            .count();
    println!(
        "lit models={} locs={} total verts={}",
        world.lit.len(),
        models,
        geometry.vertex_count()
    );
    println!("loc stats: {:?}", world.loc_stats);
    Ok(())
}
