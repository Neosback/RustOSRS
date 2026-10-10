//! Debug: how much model geometry is identical across placements (instancing potential).
use osrs_world::{
    LocRenderable, SceneWindow, TerrainPresentation, WorldDefinitions, build_world_scene,
};
use std::collections::{HashMap, hash_map::DefaultHasher};
use std::hash::{Hash, Hasher};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let mut defs = WorldDefinitions::open(&args[1])?;
    let window = SceneWindow::new(args[2].parse()?, args[3].parse()?).ok_or("align")?;
    let world = build_world_scene(&mut defs, window, TerrainPresentation::default())?;

    let mut total_faces = 0usize;
    let mut unique: HashMap<u64, usize> = HashMap::new();
    let (mut lit_inst, mut data_inst) = (0usize, 0usize);
    let (mut lit_faces, mut data_faces) = (0usize, 0usize);
    for loc in &world.locs {
        for renderable in &loc.renderables {
            let model = match *renderable {
                LocRenderable::Lit(i) => {
                    lit_inst += 1;
                    world.lit.get(i)
                }
                LocRenderable::ModelData(id) => {
                    data_inst += 1;
                    world.finalizer.lit_model(id)
                }
                LocRenderable::Animated(_) | LocRenderable::Omitted => None,
            };
            let Some(model) = model else { continue };
            total_faces += model.faces.len();
            match renderable {
                LocRenderable::Lit(_) => lit_faces += model.faces.len(),
                _ => data_faces += model.faces.len(),
            }
            let mut hasher = DefaultHasher::new();
            format!(
                "{:?}{:?}{:?}",
                model.vertices, model.faces, model.face_colors
            )
            .hash(&mut hasher);
            *unique.entry(hasher.finish()).or_default() += model.faces.len();
        }
    }
    let unique_faces: usize = unique.len();
    let _ = unique_faces;
    // Faces if every distinct geometry were stored once.
    let mut once = 0usize;
    let mut seen: HashMap<u64, ()> = HashMap::new();
    for loc in &world.locs {
        for renderable in &loc.renderables {
            let model = match *renderable {
                LocRenderable::Lit(i) => world.lit.get(i),
                LocRenderable::ModelData(id) => world.finalizer.lit_model(id),
                LocRenderable::Animated(_) | LocRenderable::Omitted => None,
            };
            let Some(model) = model else { continue };
            let mut hasher = DefaultHasher::new();
            format!(
                "{:?}{:?}{:?}",
                model.vertices, model.faces, model.face_colors
            )
            .hash(&mut hasher);
            if seen.insert(hasher.finish(), ()).is_none() {
                once += model.faces.len();
            }
        }
    }
    println!("placed model instances: lit={lit_inst} modeldata={data_inst}");
    println!("faces: total={total_faces} (lit={lit_faces}, modeldata={data_faces})");
    println!(
        "identical-geometry groups: {} ; faces if each stored once = {once} ({:.0}% of total)",
        unique.len(),
        100.0 * once as f64 / total_faces.max(1) as f64
    );
    Ok(())
}
