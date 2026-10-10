//! Debug: gap between the free-air plate of a type-8 wall decoration and the diagonal host
//! wedge face, for every decoration/host orientation and several offset rules.
//! usage: decor_matrix <cache> <decor-id> <host-id>
use osrs_core::{definitions::LocType, ids::ObjectId};
use osrs_world::WorldDefinitions;

const OFF_X: [i32; 4] = [1, -1, -1, 1];
const OFF_Z: [i32; 4] = [-1, -1, 1, 1];

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let a: Vec<String> = std::env::args().collect();
    let mut defs = WorldDefinitions::open(&a[1])?;
    let decor = defs.object(ObjectId::new(a[2].parse()?))?.ok_or("decor")?;
    let host = defs.object(ObjectId::new(a[3].parse()?))?.ok_or("host")?;
    let k = std::f64::consts::FRAC_1_SQRT_2;
    println!("rule A = client (e0 at +8*off, e1 at 0); rule B = both 0; rule C = e0 0, e1 -8*off");
    for h in 0..4u8 {
        let wall = defs
            .resolve_model(&host, LocType::new(9), h)?
            .ok_or("wall")?;
        let pts: Vec<(i32, i32)> = wall.vertices().iter().map(|v| (v.x, v.z)).collect();
        // wedge normal: the axis whose projection range is one-sided
        let mut best = None;
        for normal in [(k, k), (k, -k)] {
            let proj: Vec<f64> = pts
                .iter()
                .map(|p| f64::from(p.0) * normal.0 + f64::from(p.1) * normal.1)
                .collect();
            let (lo, hi) = (
                proj.iter().cloned().fold(f64::MAX, f64::min),
                proj.iter().cloned().fold(f64::MIN, f64::max),
            );
            if lo.abs() < 1.0 || hi.abs() < 1.0 {
                // one-sided: face at 0, wedge toward the far side
                let sign = if hi.abs() < 1.0 { 1.0 } else { -1.0 }; // wedge side is -sign*normal... outside is +sign
                best = Some((normal, sign));
            }
        }
        let Some((normal, outside_sign)) = best else {
            println!("host r{h}: no one-sided axis");
            continue;
        };
        for d in 0..4u8 {
            let opposite = (d + 2) & 3;
            let e0 = defs
                .resolve_model(&decor, LocType::new(4), d + 4)?
                .ok_or("e0")?;
            let e1 = defs
                .resolve_model(&decor, LocType::new(4), opposite + 4)?
                .ok_or("e1")?;
            let offset = (8 * OFF_X[d as usize], 8 * OFF_Z[d as usize]);
            // distance of a plate from the face along the outward direction (0 = flush)
            let gap = |m: &osrs_core::model_construction::AssembledModel, shift: (i32, i32)| {
                let proj: Vec<f64> = m
                    .vertices()
                    .iter()
                    .map(|v| {
                        f64::from(v.x + shift.0) * normal.0 + f64::from(v.z + shift.1) * normal.1
                    })
                    .map(|p| p * outside_sign)
                    .collect();
                let (lo, hi) = (
                    proj.iter().cloned().fold(f64::MAX, f64::min),
                    proj.iter().cloned().fold(f64::MIN, f64::max),
                );
                (lo, hi)
            };
            let fmt = |(lo, hi): (f64, f64)| format!("[{lo:6.1},{hi:6.1}]");
            println!(
                "host r{h} decor r{d}: A e0 {} e1 {} | B e0 {} e1 {} | C e1 {}",
                fmt(gap(&e0, offset)),
                fmt(gap(&e1, (0, 0))),
                fmt(gap(&e0, (0, 0))),
                fmt(gap(&e1, (0, 0))),
                fmt(gap(&e1, (-offset.0, -offset.1))),
            );
        }
    }
    println!("(positive = outside the wedge face; a flush plate spans about [0, 7.8])");
    Ok(())
}
