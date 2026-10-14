//! Parse and re-encode every real reference table, and test which container bytes a group's stored
//! CRC covers.
//!
//! usage: `reference_table_roundtrip <cache-dir>`. Exits non-zero when a table does not round-trip.
use osrs_cache::encode::{ReferenceTable, crc32, decode_container};
use osrs_cache::profile::TargetProfile;
use osrs_cache::transport::CacheRepository;

const PROFILE_YAML: &str =
    include_str!("../../../profiles/osrs-live-241-2026-09-30-openrs2-2727.yaml");

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dir = std::env::args()
        .nth(1)
        .ok_or("usage: reference_table_roundtrip <cache-dir>")?;
    let profile = TargetProfile::from_yaml_str(PROFILE_YAML)?;
    let cache = CacheRepository::open(&dir, &profile)?;
    let (mut exact, mut failed) = (0, 0);
    let (mut crc_full, mut crc_no_version, mut crc_neither, mut groups) =
        (0usize, 0usize, 0usize, 0usize);
    for index in cache.content_index_ids() {
        let raw = cache.read_encoded_reference_table(index)?;
        let decoded = decode_container(raw.bytes.clone())?;
        let table = match ReferenceTable::parse(&decoded) {
            Ok(table) => table,
            Err(error) => {
                println!("index {index}: parse failed: {error}");
                failed += 1;
                continue;
            }
        };
        if table.encode().as_deref() == Ok(decoded.as_slice()) {
            exact += 1;
        } else {
            println!("index {index}: re-encode differs");
            failed += 1;
        }
        println!(
            "index {index}: protocol {} revision {:?} flags {} groups {}",
            table.protocol,
            table.revision,
            table.flags,
            table.groups.len()
        );
        // Which bytes does the stored CRC cover? Sample up to 200 groups per index.
        for group in table.groups.iter().take(200) {
            let Ok(encoded) = cache.read_encoded_group(index, group.id) else {
                continue;
            };
            groups += 1;
            let bytes = &encoded.bytes;
            if crc32(bytes) == group.crc {
                crc_full += 1;
            } else if bytes.len() > 2 && crc32(&bytes[..bytes.len() - 2]) == group.crc {
                crc_no_version += 1;
            } else {
                crc_neither += 1;
            }
        }
    }
    println!("tables: exact={exact} failed={failed}");
    println!(
        "group crc over: whole container={crc_full}, container minus trailing version={crc_no_version}, neither={crc_neither} (of {groups})"
    );
    if failed != 0 {
        return Err("a reference table did not round-trip".into());
    }
    Ok(())
}
