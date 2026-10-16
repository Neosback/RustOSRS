use super::{tests::*, *};

fn snapshot(dir: &Path) -> Vec<(String, Vec<u8>)> {
    let mut files: Vec<_> = std::fs::read_dir(dir).unwrap_or_else(|e| panic!("{e}"))
        .map(|entry| {
            let entry = entry.unwrap_or_else(|e| panic!("{e}"));
            (entry.file_name().to_string_lossy().into_owned(),
             std::fs::read(entry.path()).unwrap_or_else(|e| panic!("{e}")))
        }).collect();
    files.sort_by(|a, b| a.0.cmp(&b.0));
    files
}

#[test]
fn identical_repack_keeps_every_byte_and_version() {
    let dir = temp_dir("unchanged");
    build_cache(&dir);
    let before = snapshot(&dir);
    let report = repack_changed_groups(&dir, &[GroupUpdate { index_id: 5, group_id: 1, payload: pack(1) }])
        .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(report, RepackReport::default());
    assert_eq!(snapshot(&dir), before);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn mixed_repack_only_rewrites_changed_groups_and_is_idempotent() {
    let dir = temp_dir("mixed");
    let store = build_cache(&dir);
    let old_raw = store.read_raw_group(5, 0).unwrap_or_else(|e| panic!("{e}"));
    let updates = [
        GroupUpdate { index_id: 5, group_id: 0, payload: pack(0) },
        GroupUpdate { index_id: 5, group_id: 1, payload: vec![7; 1234] },
    ];
    let report = repack_changed_groups(&dir, &updates).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(report, RepackReport { groups: 1, tables: 1 });
    assert_eq!(store.read_raw_group(5, 0).unwrap_or_else(|e| panic!("{e}")), old_raw);
    assert_eq!(table_of(&store).group(1).map(|entry| entry.version), Some(2));
    let after = snapshot(&dir);
    assert_eq!(repack_changed_groups(&dir, &updates).unwrap_or_else(|e| panic!("{e}")), RepackReport::default());
    assert_eq!(snapshot(&dir), after);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn later_invalid_target_cannot_partially_repack_an_earlier_group() {
    let dir = temp_dir("preflight");
    build_cache(&dir);
    let before = snapshot(&dir);
    for index_id in [5, 6] {
        let result = repack_groups(&dir, &[
            GroupUpdate { index_id: 5, group_id: 0, payload: vec![8; 900] },
            GroupUpdate { index_id, group_id: 999, payload: vec![1] },
        ]);
        assert!(result.is_err());
        assert_eq!(snapshot(&dir), before);
    }
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn duplicate_targets_are_rejected_before_writing() {
    let dir = temp_dir("duplicate");
    build_cache(&dir);
    let before = snapshot(&dir);
    assert!(repack_groups(&dir, &[
        GroupUpdate { index_id: 5, group_id: 0, payload: vec![8] },
        GroupUpdate { index_id: 5, group_id: 0, payload: vec![9] },
    ]).is_err());
    assert_eq!(snapshot(&dir), before);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn identical_payload_with_corrupt_metadata_is_not_silently_skipped() {
    let dir = temp_dir("corrupt-metadata");
    let store = build_cache(&dir);
    let mut table = table_of(&store);
    table.group_mut(1).unwrap_or_else(|| unreachable!()).crc ^= 1;
    let encoded = encode_container(table.encode().unwrap_or_else(|e| panic!("{e}")), ContainerCompression::Gzip, None)
        .unwrap_or_else(|e| panic!("{e}"));
    store.write_raw_group(255, 5, &encoded).unwrap_or_else(|e| panic!("{e}"));
    let before = snapshot(&dir);
    assert!(repack_changed_groups(&dir, &[GroupUpdate { index_id: 5, group_id: 1, payload: pack(1) }]).is_err());
    assert_eq!(snapshot(&dir), before);
    let _ = std::fs::remove_dir_all(dir);
}
