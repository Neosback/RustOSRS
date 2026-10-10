//! Debug: per-face colour/priority/bias/alpha/vertex positions of a resolved object model.
//! usage: model_faces <cache> <object-id> <loc-type> <orientation>
use osrs_core::{definitions::LocType, ids::ObjectId};
use osrs_world::WorldDefinitions;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let a: Vec<String> = std::env::args().collect();
    let mut defs = WorldDefinitions::open(&a[1])?;
    let def = defs.object(ObjectId::new(a[2].parse()?))?.ok_or("def")?;
    let model = defs
        .resolve_model(&def, LocType::new(a[3].parse()?), a[4].parse()?)?
        .ok_or("model")?;
    println!(
        "ambient={} contrast={} vertices={} faces={} default_priority={:?}",
        def.ambient,
        def.contrast,
        model.vertices().len(),
        model.faces().len(),
        model.default_priority()
    );
    for (i, f) in model.faces().iter().enumerate() {
        let v = |n: u32| model.vertices()[n as usize];
        let (a, b, c) = (v(f.a.get()), v(f.b.get()), v(f.c.get()));
        println!(
            "f{i}: color={:#06x} pri={:?} bias={:?} alpha={:?} tex={:?} render_type={:?} z=[{},{},{}] y=[{},{},{}]",
            model.face_colors()[i],
            model.face_priorities().map(|p| p[i]),
            model.face_biases().map(|p| p[i]),
            model.face_alphas().map(|p| p[i]),
            model.face_textures().map(|p| p[i]),
            model.face_render_types().map(|p| p[i]),
            a.z,
            b.z,
            c.z,
            a.y,
            b.y,
            c.y
        );
    }
    Ok(())
}
