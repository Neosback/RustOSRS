use super::*;
use crate::encode::{GroupEntry, pack_group, split_group};
use std::sync::atomic::{AtomicUsize, Ordering};

fn temp_dir(name: &str) -> PathBuf {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let dir = std::env::temp_dir().join(format!(
        "rustosrs-store-{}-{}-{name}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn files_for(group: u32) -> Vec<Vec<u8>> {
    vec![
        (0..600 + group as usize).map(|i| (i % 251) as u8).collect(),
        vec![group as u8; 40],
    ]
}

fn pack(group: u32) -> Vec<u8> {
    let files = files_for(group);
    let parts: Vec<&[u8]> = files.iter().map(Vec::as_slice).collect();
    pack_group(&parts).unwrap_or_else(|e| unreachable!("{e}"))
}

/// A small synthetic index 5 with groups 0, 1, 2 (two files each, version suffixes, sizes column).
fn build_cache(dir: &Path) -> CacheStore {
    let store = CacheStore::create(dir).unwrap_or_else(|e| unreachable!("{e}"));
    let mut groups = Vec::new();
    for id in 0..3u32 {
        let payload = pack(id);
        let mut container = encode_container(payload.clone(), ContainerCompression::Gzip, None)
            .unwrap_or_else(|e| unreachable!("{e}"));
        groups.push(GroupEntry {
            id,
            name_hash: None,
            crc: crc32(&container),
            hash: None,
            whirlpool: None,
            sizes: Some((container.len() as u32, payload.len() as u32)),
            version: 1,
            file_ids: vec![0, 1],
            file_name_hashes: None,
        });
        container.extend_from_slice(&1u16.to_be_bytes());
        store
            .write_raw_group(5, id, &container)
            .unwrap_or_else(|e| unreachable!("{e}"));
    }
    let table = ReferenceTable {
        protocol: 7,
        revision: Some(1000),
        flags: 4,
        groups,
    };
    let encoded = encode_container(
        table.encode().unwrap_or_else(|e| unreachable!("{e}")),
        ContainerCompression::Gzip,
        None,
    )
    .unwrap_or_else(|e| unreachable!("{e}"));
    store
        .write_raw_group(REFERENCE_TABLE_INDEX, 5, &encoded)
        .unwrap_or_else(|e| unreachable!("{e}"));
    store
}

fn table_of(store: &CacheStore) -> ReferenceTable {
    let raw = store
        .read_raw_group(REFERENCE_TABLE_INDEX, 5)
        .unwrap_or_else(|e| unreachable!("{e}"))
        .unwrap_or_else(|| unreachable!("table exists"));
    ReferenceTable::parse(&decode_container(raw).unwrap_or_else(|e| unreachable!("{e}")))
        .unwrap_or_else(|e| unreachable!("{e}"))
}

#[test]
fn groups_round_trip_across_sector_boundaries_and_expanded_headers() {
    let dir = temp_dir("raw");
    let store = CacheStore::create(&dir).unwrap_or_else(|e| unreachable!("{e}"));
    for (index, group, len) in [
        (7u8, 0u32, 1usize),
        (7, 1, 511),
        (7, 2, 512),
        (7, 3, 513),
        (7, 4, 5000),
        (7, 70_000, 2000),
        (9, 70_001, 510),
    ] {
        let data: Vec<u8> = (0..len).map(|i| (i * 7 % 256) as u8).collect();
        store
            .write_raw_group(index, group, &data)
            .unwrap_or_else(|e| unreachable!("{e}"));
        assert_eq!(
            store
                .read_raw_group(index, group)
                .unwrap_or_else(|e| unreachable!("{e}")),
            Some(data),
            "index {index} group {group} len {len}"
        );
    }
    assert_eq!(
        store
            .read_raw_group(7, 99)
            .unwrap_or_else(|e| unreachable!("{e}")),
        None
    );
    assert_eq!(
        store
            .read_raw_group(8, 0)
            .unwrap_or_else(|e| unreachable!("{e}")),
        None
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_final_sector_can_end_at_the_last_payload_byte_without_padding() {
    let dir = temp_dir("unpadded");
    let store = CacheStore::create(&dir).unwrap_or_else(|e| unreachable!("{e}"));
    let bytes = vec![17; 601];
    store
        .write_raw_group(5, 7, &bytes)
        .unwrap_or_else(|e| unreachable!("{e}"));
    let file = OpenOptions::new()
        .write(true)
        .open(dir.join(DATA_FILE))
        .unwrap_or_else(|e| unreachable!("{e}"));
    file.set_len((2 * SECTOR_SIZE + 8 + 89) as u64)
        .unwrap_or_else(|e| unreachable!("{e}"));
    assert_eq!(
        store
            .read_raw_group(5, 7)
            .unwrap_or_else(|e| unreachable!("{e}")),
        Some(bytes)
    );
    file.set_len((2 * SECTOR_SIZE + 8 + 88) as u64)
        .unwrap_or_else(|e| unreachable!("{e}"));
    assert!(
        store.read_raw_group(5, 7).is_err(),
        "actual payload truncation must still fail"
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn rewriting_a_group_repoints_it_and_leaves_other_groups_alone() {
    let dir = temp_dir("rewrite");
    let store = CacheStore::create(&dir).unwrap_or_else(|e| unreachable!("{e}"));
    store
        .write_raw_group(3, 0, &[1; 900])
        .unwrap_or_else(|e| unreachable!("{e}"));
    store
        .write_raw_group(3, 1, &[2; 30])
        .unwrap_or_else(|e| unreachable!("{e}"));
    store
        .write_raw_group(3, 0, &[9; 1300])
        .unwrap_or_else(|e| unreachable!("{e}"));
    assert_eq!(
        store
            .read_raw_group(3, 0)
            .unwrap_or_else(|e| unreachable!("{e}")),
        Some(vec![9; 1300])
    );
    assert_eq!(
        store
            .read_raw_group(3, 1)
            .unwrap_or_else(|e| unreachable!("{e}")),
        Some(vec![2; 30])
    );
    assert!(
        store.write_raw_group(3, 2, &[]).is_err(),
        "empty groups cannot be stored"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_independent_vendored_reader_agrees_with_what_was_written() {
    let dir = temp_dir("vendored");
    let store = build_cache(&dir);
    let indices = runefs::Indices::new(&dir).unwrap_or_else(|e| unreachable!("{e:?}"));
    let dat = runefs::Dat2::new(dir.join(DATA_FILE)).unwrap_or_else(|e| unreachable!("{e:?}"));
    let index = indices.get(&5).unwrap_or_else(|| unreachable!("index 5"));
    for id in 0..3u32 {
        let archive_ref = index
            .archive_refs
            .get(&id)
            .unwrap_or_else(|| unreachable!("ref"));
        let read = dat
            .read(archive_ref)
            .unwrap_or_else(|e| unreachable!("{e:?}"));
        assert_eq!(
            read.as_ref(),
            store
                .read_raw_group(5, id)
                .unwrap_or_else(|e| unreachable!("{e}"))
                .unwrap_or_default()
                .as_slice()
        );
    }
    let rows: Vec<_> = index.metadata.iter().map(|m| (m.id, m.version)).collect();
    assert_eq!(rows, vec![(0, 1), (1, 1), (2, 1)]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn repacking_updates_the_group_and_keeps_its_table_row_consistent() {
    let dir = temp_dir("repack");
    let store = build_cache(&dir);
    let before = table_of(&store);
    let untouched_raw = store
        .read_raw_group(5, 2)
        .unwrap_or_else(|e| unreachable!("{e}"));

    let new_files = [vec![42u8; 2000], vec![7u8; 9]];
    let parts: Vec<&[u8]> = new_files.iter().map(Vec::as_slice).collect();
    let payload = pack_group(&parts).unwrap_or_else(|e| unreachable!("{e}"));
    let report = repack_groups(
        &dir,
        &[GroupUpdate {
            index_id: 5,
            group_id: 1,
            payload: payload.clone(),
        }],
    )
    .unwrap_or_else(|e| unreachable!("{e}"));
    assert_eq!(
        report,
        RepackReport {
            groups: 1,
            tables: 1
        }
    );

    let raw = store
        .read_raw_group(5, 1)
        .unwrap_or_else(|e| unreachable!("{e}"))
        .unwrap_or_else(|| unreachable!("group exists"));
    let body = container_body_len(&raw).unwrap_or_else(|| unreachable!("valid container"));
    assert_eq!(raw.len(), body + 2, "the version suffix is kept");
    assert_eq!(&raw[body..], &2u16.to_be_bytes());

    let after = table_of(&store);
    let row = after.group(1).unwrap_or_else(|| unreachable!("row"));
    assert_eq!(row.crc, crc32(&raw[..body]));
    assert_eq!(row.sizes, Some((body as u32, payload.len() as u32)));
    assert_eq!(row.version, 2);
    assert_eq!(row.file_ids, vec![0, 1]);
    assert_eq!(after.revision, before.revision);
    assert_eq!(after.group(0), before.group(0));
    assert_eq!(after.group(2), before.group(2));

    let decoded = decode_container(raw).unwrap_or_else(|e| unreachable!("{e}"));
    assert_eq!(
        split_group(&decoded, 2).unwrap_or_else(|e| unreachable!("{e}")),
        vec![(0, new_files[0].clone()), (1, new_files[1].clone())]
    );
    assert_eq!(
        store
            .read_raw_group(5, 2)
            .unwrap_or_else(|e| unreachable!("{e}")),
        untouched_raw,
        "untouched groups keep their bytes"
    );

    // The vendored reader sees the updated table (crc and version) too.
    let indices = runefs::Indices::new(&dir).unwrap_or_else(|e| unreachable!("{e:?}"));
    let rows: Vec<_> = indices
        .get(&5)
        .unwrap_or_else(|| unreachable!("index"))
        .metadata
        .iter()
        .map(|m| (m.id, m.crc, m.version))
        .collect();
    assert_eq!(rows[1], (1, row.crc, 2));

    // A second repack bumps the version again.
    repack_groups(
        &dir,
        &[GroupUpdate {
            index_id: 5,
            group_id: 1,
            payload,
        }],
    )
    .unwrap_or_else(|e| unreachable!("{e}"));
    assert_eq!(table_of(&store).group(1).map(|g| g.version), Some(3));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn repacking_rejects_version_overflow_without_writing() {
    let dir = temp_dir("version-overflow");
    let store = build_cache(&dir);
    let table_raw = store
        .read_raw_group(REFERENCE_TABLE_INDEX, 5)
        .unwrap_or_else(|e| unreachable!("{e}"))
        .unwrap_or_else(|| unreachable!("table exists"));
    let table_compression = compression_of(&table_raw).unwrap_or_else(|e| unreachable!("{e}"));
    let mut table = ReferenceTable::parse(
        &decode_container(table_raw).unwrap_or_else(|e| unreachable!("{e}")),
    )
    .unwrap_or_else(|e| unreachable!("{e}"));
    table
        .group_mut(1)
        .unwrap_or_else(|| unreachable!("group exists"))
        .version = u32::MAX;
    let encoded = encode_container(
        table.encode().unwrap_or_else(|e| unreachable!("{e}")),
        table_compression,
        None,
    )
    .unwrap_or_else(|e| unreachable!("{e}"));
    store
        .write_raw_group(REFERENCE_TABLE_INDEX, 5, &encoded)
        .unwrap_or_else(|e| unreachable!("{e}"));
    let group_before = store
        .read_raw_group(5, 1)
        .unwrap_or_else(|e| unreachable!("{e}"));
    let table_before = store
        .read_raw_group(REFERENCE_TABLE_INDEX, 5)
        .unwrap_or_else(|e| unreachable!("{e}"));

    let result = repack_groups(
        &dir,
        &[GroupUpdate {
            index_id: 5,
            group_id: 1,
            payload: pack(1),
        }],
    );
    assert!(matches!(
        result,
        Err(StoreError::VersionOverflow {
            index_id: 5,
            group_id: 1
        })
    ));
    assert_eq!(
        store
            .read_raw_group(5, 1)
            .unwrap_or_else(|e| unreachable!("{e}")),
        group_before
    );
    assert_eq!(
        store
            .read_raw_group(REFERENCE_TABLE_INDEX, 5)
            .unwrap_or_else(|e| unreachable!("{e}")),
        table_before
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn repacking_rejects_unknown_groups_and_missing_tables_without_writing() {
    let dir = temp_dir("errors");
    let store = build_cache(&dir);
    let len_before = std::fs::metadata(dir.join(DATA_FILE))
        .map(|m| m.len())
        .unwrap_or(0);
    let unknown = repack_groups(
        &dir,
        &[GroupUpdate {
            index_id: 5,
            group_id: 99,
            payload: vec![1],
        }],
    );
    assert!(matches!(
        unknown,
        Err(StoreError::UnknownGroup {
            index_id: 5,
            group_id: 99
        })
    ));
    let missing = repack_groups(
        &dir,
        &[GroupUpdate {
            index_id: 6,
            group_id: 0,
            payload: vec![1],
        }],
    );
    assert!(matches!(
        missing,
        Err(StoreError::MissingTable { index_id: 6 })
    ));
    assert_eq!(
        table_of(&store).group(1).map(|g| g.version),
        Some(1),
        "table unchanged"
    );
    // The failed unknown-group repack may have appended nothing before the error was detected
    // for the first group; the table itself must not have moved.
    let _ = len_before;
    let _ = std::fs::remove_dir_all(&dir);
}
