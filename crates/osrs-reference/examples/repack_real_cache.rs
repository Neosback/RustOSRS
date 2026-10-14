//! Copy the real cache, repack one edited map square into the copy, and verify the result.
//!
//! usage: `repack_real_cache <cache-dir>`. Checks that the repacked group decodes to the edit, that
//! its other files and every untouched group are unchanged, and that the reference table row (CRC,
//! sizes, version) matches the written bytes.
use osrs_cache::decode::{
    ArchiveFileProvenance, DecoderContext, EncodedTerrainTile, decode_terrain,
};
use osrs_cache::encode::{
    CacheStore, GroupUpdate, REFERENCE_TABLE_INDEX, ReferenceTable, crc32, decode_container,
    encode_terrain, pack_group, repack_groups, split_group,
};
use osrs_cache::profile::TargetProfile;
use osrs_cache::transport::CacheRepository;
use osrs_core::coords::RegionCoord;
use std::path::PathBuf;

const PROFILE_YAML: &str =
    include_str!("../../../profiles/osrs-live-241-2026-09-30-openrs2-2727.yaml");

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let src = PathBuf::from(
        std::env::args()
            .nth(1)
            .ok_or("usage: repack_real_cache <cache-dir>")?,
    );
    let dst = std::env::temp_dir().join(format!("rustosrs-repack-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dst);
    std::fs::create_dir_all(&dst)?;
    for entry in std::fs::read_dir(&src)? {
        let entry = entry?;
        std::fs::copy(entry.path(), dst.join(entry.file_name()))?;
    }

    let profile = TargetProfile::from_yaml_str(PROFILE_YAML)?;
    let cache = CacheRepository::open(&src, &profile)?;
    let context = DecoderContext::from_profile(&profile)?;
    let region = RegionCoord::new(50, 50);
    let group_id = 50 * 256 + 50;
    let files = cache.read_group_files(5, group_id)?;
    let terrain = decode_terrain(&files[0].1.bytes, &context, &files[0].1.provenance, region)?;

    // Edit: set tile (10, 10) on plane 0 to underlay 9.
    let mut tiles: Vec<EncodedTerrainTile> = terrain.tiles().to_vec();
    tiles[10 * 64 + 10].underlay_id = 9;
    let edited = osrs_cache::decode::DecodedTerrain::from_parts(region, tiles, 0);
    let new_terrain = encode_terrain(&edited)?;
    let mut parts: Vec<&[u8]> = files.iter().map(|(_, f)| f.bytes.as_slice()).collect();
    parts[0] = &new_terrain;
    let payload = pack_group(&parts)?;
    let report = repack_groups(
        &dst,
        &[GroupUpdate {
            index_id: 5,
            group_id,
            payload,
        }],
    )?;
    println!("repacked: {report:?}");

    // Verify in the copy.
    let store = CacheStore::open(&dst);
    let original = CacheStore::open(&src);
    let raw = store.read_raw_group(5, group_id)?.ok_or("group missing")?;
    let table_raw = store
        .read_raw_group(REFERENCE_TABLE_INDEX, 5)?
        .ok_or("table missing")?;
    let table = ReferenceTable::parse(&decode_container(table_raw)?)?;
    let old_table = ReferenceTable::parse(&decode_container(
        original
            .read_raw_group(REFERENCE_TABLE_INDEX, 5)?
            .ok_or("old table missing")?,
    )?)?;
    let row = table.group(group_id).ok_or("row missing")?;
    let old_row = old_table.group(group_id).ok_or("old row missing")?;
    let body = raw.len() - 2;
    assert_eq!(
        row.crc,
        crc32(&raw[..body]),
        "table crc matches the written container"
    );
    assert_eq!(row.version, old_row.version + 1, "version bumped");
    assert_eq!(row.file_ids, old_row.file_ids);
    assert_eq!(row.sizes.map(|s| s.0 as usize), Some(body));

    let decoded = decode_container(raw)?;
    assert_eq!(row.sizes.map(|s| s.1 as usize), Some(decoded.len()));
    let split = split_group(&decoded, 5)?;
    let reread = decode_terrain(
        &split[0].1,
        &context,
        &ArchiveFileProvenance::new(5, group_id, Some(0)),
        region,
    )?;
    assert_eq!(
        reread.tiles()[10 * 64 + 10].underlay_id,
        9,
        "the edit is in the repacked square"
    );
    assert_eq!(
        reread.tiles()[0],
        terrain.tiles()[0],
        "other tiles are unchanged"
    );
    for k in 1..5 {
        assert_eq!(split[k].1, files[k].1.bytes, "file {k} unchanged");
    }

    // Every other group in index 5 and the other tables are byte-identical to the source.
    let mut compared = 0;
    for other in &old_table.groups {
        if other.id == group_id {
            continue;
        }
        assert_eq!(
            store.read_raw_group(5, other.id)?,
            original.read_raw_group(5, other.id)?,
            "group {} changed",
            other.id
        );
        compared += 1;
    }
    for index in [0u32, 2, 7, 12] {
        assert_eq!(
            store.read_raw_group(REFERENCE_TABLE_INDEX, index)?,
            original.read_raw_group(REFERENCE_TABLE_INDEX, index)?
        );
    }
    println!(
        "ok: group {group_id} v{} -> v{}, crc {:08x}, {} other index-5 groups identical",
        old_row.version, row.version, row.crc, compared
    );
    let _ = std::fs::remove_dir_all(&dst);
    Ok(())
}
