//! Debug: dump an object's resolved model (vertices and faces) for a type/orientation.
//! usage: dump_model <cache> <object-id> <loc-type> <orientation>
use osrs_core::{definitions::LocType, ids::ObjectId};
use osrs_world::WorldDefinitions;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let a: Vec<String> = std::env::args().collect();
    let mut defs = WorldDefinitions::open(&a[1])?;
    let def = defs.object(ObjectId::new(a[2].parse()?))?.ok_or("def")?;
    let model = defs
        .resolve_model(&def, LocType::new(a[3].parse()?), a[4].parse()?)?
        .ok_or("model")?;
    for (i, v) in model.vertices().iter().enumerate() {
        println!("v{i}: ({}, {}, {})", v.x, v.y, v.z);
    }
    for (i, f) in model.faces().iter().enumerate() {
        println!("f{i}: {} {} {}", f.a.get(), f.b.get(), f.c.get());
    }
    Ok(())
}
