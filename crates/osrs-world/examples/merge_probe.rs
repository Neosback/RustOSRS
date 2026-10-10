//! Debug: how many vertices of two adjacent placed models coincide in x/z, and their y deltas.
//! usage: merge_probe <cache> <id-a> <type-a> <orient-a> <tile-ax> <tile-az> <id-b> <type-b> <orient-b> <tile-bx> <tile-bz>
use osrs_core::{definitions::LocType, ids::ObjectId};
use osrs_world::WorldDefinitions;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let a: Vec<String> = std::env::args().collect();
    let mut defs = WorldDefinitions::open(&a[1])?;
    let mut models = Vec::new();
    for base in [2usize, 7] {
        let id: u32 = a[base].parse()?;
        let def = defs.object(ObjectId::new(id))?.ok_or("def")?;
        let model = defs
            .resolve_model(
                &def,
                LocType::new(a[base + 1].parse()?),
                a[base + 2].parse()?,
            )?
            .ok_or("model")?;
        let (tx, tz): (i32, i32) = (a[base + 3].parse()?, a[base + 4].parse()?);
        let points: Vec<(i32, i32, i32)> = model
            .vertices()
            .iter()
            .map(|v| (v.x + tx * 128 + 64, v.y, v.z + tz * 128 + 64))
            .collect();
        models.push(points);
    }
    let (mut exact, mut near, mut xz_only) = (0, 0, 0);
    for p in &models[0] {
        for q in &models[1] {
            if p.0 == q.0 && p.2 == q.2 {
                xz_only += 1;
                if p.1 == q.1 {
                    exact += 1
                }
                if (p.1 - q.1).abs() <= 2 {
                    near += 1
                }
                println!("xz match: y {} vs {} (delta {})", p.1, q.1, p.1 - q.1);
            }
        }
    }
    println!("xz-coincident={xz_only} exact(strict y)={exact} within-2={near}");
    Ok(())
}
