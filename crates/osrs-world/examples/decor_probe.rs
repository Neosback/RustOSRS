//! Debug: perpendicular position of a type-8 wall decoration's two entities relative to the
//! diagonal host wall, for the current offset rule.
//! usage: decor_probe <cache> <decor-id> <decor-orient> <host-id> <host-orient>
use osrs_core::{definitions::LocType, ids::ObjectId};
use osrs_world::WorldDefinitions;

fn range(points: &[(i32, i32)], normal: (f64, f64)) -> (f64, f64) {
    let proj: Vec<f64> = points
        .iter()
        .map(|p| f64::from(p.0) * normal.0 + f64::from(p.1) * normal.1)
        .collect();
    (
        proj.iter().cloned().fold(f64::MAX, f64::min),
        proj.iter().cloned().fold(f64::MIN, f64::max),
    )
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let a: Vec<String> = std::env::args().collect();
    let mut defs = WorldDefinitions::open(&a[1])?;
    let decor = defs.object(ObjectId::new(a[2].parse()?))?.ok_or("decor")?;
    let host = defs.object(ObjectId::new(a[4].parse()?))?.ok_or("host")?;
    let (d_or, h_or): (u8, u8) = (a[3].parse()?, a[5].parse()?);
    let pts = |m: &osrs_core::model_construction::AssembledModel| -> Vec<(i32, i32)> {
        let mut used = vec![false; m.vertices().len()];
        for f in m.faces() {
            for i in [f.a.get(), f.b.get(), f.c.get()] {
                used[i as usize] = true;
            }
        }
        m.vertices()
            .iter()
            .zip(&used)
            .filter(|(_, u)| **u)
            .map(|(v, _)| (v.x, v.z))
            .collect()
    };
    let wall = defs
        .resolve_model(&host, LocType::new(9), h_or)?
        .ok_or("wall")?;
    let wall_pts = pts(&wall);
    let opposite = (d_or + 2) & 3;
    let e0 = defs
        .resolve_model(&decor, LocType::new(4), d_or + 4)?
        .ok_or("e0")?;
    let e1 = defs
        .resolve_model(&decor, LocType::new(4), opposite + 4)?
        .ok_or("e1")?;
    let k = std::f64::consts::FRAC_1_SQRT_2;
    println!(
        "wall x/z bbox: {:?}",
        (
            wall_pts.iter().map(|p| p.0).min(),
            wall_pts.iter().map(|p| p.0).max(),
            wall_pts.iter().map(|p| p.1).min(),
            wall_pts.iter().map(|p| p.1).max()
        )
    );
    for (name, normal) in [("n=(1,1)", (k, k)), ("n=(1,-1)", (k, -k))] {
        println!(
            "{name}: wall {:?}  e0 {:?}  e1 {:?}",
            range(&wall_pts, normal),
            range(&pts(&e0), normal),
            range(&pts(&e1), normal)
        );
    }
    println!(
        "e0 bbox x {:?}..{:?} z {:?}..{:?}",
        pts(&e0).iter().map(|p| p.0).min(),
        pts(&e0).iter().map(|p| p.0).max(),
        pts(&e0).iter().map(|p| p.1).min(),
        pts(&e0).iter().map(|p| p.1).max()
    );
    println!(
        "e1 bbox x {:?}..{:?} z {:?}..{:?}",
        pts(&e1).iter().map(|p| p.0).min(),
        pts(&e1).iter().map(|p| p.0).max(),
        pts(&e1).iter().map(|p| p.1).min(),
        pts(&e1).iter().map(|p| p.1).max()
    );
    Ok(())
}
