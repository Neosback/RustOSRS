//! n-range (along the host slab normal) of the black faces of a decoration plate.
//! usage: black_faces <cache> <decor-id> <orient 4..7>
use osrs_core::{definitions::LocType, ids::ObjectId};
use osrs_world::WorldDefinitions;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let a: Vec<String> = std::env::args().collect();
    let mut defs = WorldDefinitions::open(&a[1])?;
    let def = defs.object(ObjectId::new(a[2].parse()?))?.ok_or("def")?;
    let model = defs
        .resolve_model(&def, LocType::new(4), a[3].parse()?)?
        .ok_or("model")?;
    let k = std::f64::consts::FRAC_1_SQRT_2;
    for (name, n) in [("n=(1,1)", (k, k)), ("n=(1,-1)", (k, -k))] {
        let (mut all, mut black): (Vec<f64>, Vec<f64>) = (vec![], vec![]);
        for (i, f) in model.faces().iter().enumerate() {
            for v in [f.a, f.b, f.c] {
                let p = model.vertices()[v.get() as usize];
                let proj = f64::from(p.x) * n.0 + f64::from(p.z) * n.1;
                all.push(proj);
                if model.face_colors()[i] & 0x7f < 4 {
                    black.push(proj);
                }
            }
        }
        let r = |v: &Vec<f64>| {
            (
                v.iter().cloned().fold(f64::MAX, f64::min),
                v.iter().cloned().fold(f64::MIN, f64::max),
            )
        };
        println!(
            "{name}: all {:?}  black {:?} ({} black verts)",
            r(&all),
            if black.is_empty() {
                (0.0, 0.0)
            } else {
                r(&black)
            },
            black.len()
        );
    }
    // facing of each face along +n=(1,1): sign of (b-a)x(c-a) projected on n (x,z components)
    let n = (k, k);
    let (mut frame_pos, mut frame_neg, mut black_pos, mut black_neg) = (0, 0, 0, 0);
    for (i, f) in model.faces().iter().enumerate() {
        let p = |v: osrs_core::model::VertexIndex| model.vertices()[v.get() as usize];
        let (a, b, c) = (p(f.a), p(f.b), p(f.c));
        let (ux, uy, uz) = (
            f64::from(b.x - a.x),
            f64::from(b.y - a.y),
            f64::from(b.z - a.z),
        );
        let (vx, vy, vz) = (
            f64::from(c.x - a.x),
            f64::from(c.y - a.y),
            f64::from(c.z - a.z),
        );
        let normal = (uy * vz - uz * vy, uz * vx - ux * vz, ux * vy - uy * vx);
        let d = normal.0 * n.0 + normal.2 * n.1;
        let black = model.face_colors()[i] & 0x7f < 4;
        match (black, d > 0.0) {
            (true, true) => black_pos += 1,
            (true, false) => black_neg += 1,
            (false, true) => frame_pos += 1,
            (false, false) => frame_neg += 1,
        }
    }
    println!(
        "cross-product normal along +n: frame +{frame_pos}/-{frame_neg}, black +{black_pos}/-{black_neg}"
    );
    Ok(())
}
