use sha2::{Digest, Sha256};
use std::{path::Path, time::Instant};
macro_rules! reader {
    ($name:ident, $store:ident) => {
        fn $name(path: &Path, round: usize) -> Result<(), Box<dyn std::error::Error>> {
            let start = Instant::now();
            let indices = $store::Indices::new(path)?;
            let data = $store::Dat2::new(path.join($store::MAIN_DATA))?;
            let setup = start.elapsed().as_secs_f64();
            let mut groups: Vec<_> = (&indices)
                .into_iter()
                .filter(|(id, _)| **id != 255)
                .flat_map(|(_, i)| i.metadata.iter().filter_map(|m| i.archive_refs.get(&m.id)))
                .collect();
            groups.sort_by_key(|r| (r.index_id, r.id));
            let mut hash = Sha256::new();
            let mut bytes = 0usize;
            let start = Instant::now();
            for group in &groups {
                let read = data.read(group)?;
                bytes += read.len();
                hash.update(read.as_slice());
            }
            println!(
                "{} round={} setup_s={:.6} read_hash_s={:.6} groups={} bytes={} sha256={:x}",
                stringify!($name),
                round,
                setup,
                start.elapsed().as_secs_f64(),
                groups.len(),
                bytes,
                hash.finalize()
            );
            Ok(())
        }
    };
}
reader!(mapped, runefs);
reader!(positional, osrs_filestore);
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args().nth(1).ok_or("cache path")?;
    let path = Path::new(&path);
    for round in 0..4 {
        if round % 2 == 0 {
            mapped(path, round)?;
            positional(path, round)?;
        } else {
            positional(path, round)?;
            mapped(path, round)?;
        }
    }
    Ok(())
}
