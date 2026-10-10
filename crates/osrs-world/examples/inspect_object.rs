//! Debug: print an object definition and its resolved model sizes.
use osrs_core::{definitions::LocType, ids::ObjectId};
use osrs_world::WorldDefinitions;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let mut defs = WorldDefinitions::open(&args[1])?;
    for id in &args[2..] {
        let id: u32 = id.parse()?;
        let Some(def) = defs.object(ObjectId::new(id))? else {
            println!("object {id}: no definition");
            continue;
        };
        println!(
            "object {id}: name={:?} size={}x{} models={:?} anim={:?} morphs={} non_flat={} clipped={} interact={}",
            def.name,
            def.size_x,
            def.size_y,
            def.models,
            def.animation,
            def.morphs.is_some(),
            def.non_flat_shading,
            def.placement.clipped,
            def.placement.interact_type
        );
        println!("  scale={:?} translation={:?} recolors={} ambient={} contrast={} contour={:?}", def.scale, def.translation, def.recolors.len(), def.ambient, def.contrast, def.contour_clip);
        for t in [0u8, 9, 10, 22] {
            match defs.resolve_model(&def, LocType::new(t), 0)? {
                Some(model) => println!(
                    "  type {t}: {} vertices, {} faces",
                    model.vertices().len(),
                    model.faces().len()
                ),
                None => println!("  type {t}: no model"),
            }
        }
    }
    Ok(())
}
