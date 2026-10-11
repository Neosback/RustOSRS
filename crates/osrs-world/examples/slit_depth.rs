//! Where a decoration's black / frame faces lie along a wall normal (world units).
//! usage: slit_depth <cache> <decor-id> <orient> <origin-offset> <axis x|z>
use osrs_core::{definitions::LocType, ids::ObjectId};
use osrs_world::WorldDefinitions;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let a: Vec<String> = std::env::args().collect();
    let mut defs = WorldDefinitions::open(&a[1])?;
    let def = defs.object(ObjectId::new(a[2].parse()?))?.ok_or("def")?;
    let model = defs
        .resolve_model(&def, LocType::new(4), a[3].parse()?)?
        .ok_or("model")?;
    let shift: i32 = a[4].parse()?;
    let axis = &a[5];
    let coord = |v: osrs_core::model::VertexIndex| {
        let p = model.vertices()[v.get() as usize];
        (if axis == "x" { p.x } else { p.z }) + shift
    };
    let (mut black, mut frame): (Vec<i32>, Vec<i32>) = (vec![], vec![]);
    for (i, f) in model.faces().iter().enumerate() {
        let target = if model.face_colors()[i] & 0x7f < 4 {
            &mut black
        } else {
            &mut frame
        };
        for v in [f.a, f.b, f.c] {
            target.push(coord(v));
        }
    }
    let r = |v: &Vec<i32>| (v.iter().min().copied(), v.iter().max().copied());
    println!("black faces {:?}   frame faces {:?}", r(&black), r(&frame));
    Ok(())
}
