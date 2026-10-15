use super::*;
use crate::encode::{
    CacheStore, ContainerCompression, GroupEntry, GroupUpdate, ReferenceTable, crc32,
    encode_container, pack_group, repack_groups,
};
use std::sync::atomic::{AtomicU64, Ordering};

struct Fixture {
    root: PathBuf,
    original: PathBuf,
    output: PathBuf,
    profile: TargetProfile,
    manifest: ExportManifest,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn group(store: &CacheStore, index: u8, id: u32, payload: Vec<u8>, files: Vec<u32>) -> GroupEntry {
    let mut raw = encode_container(payload.clone(), ContainerCompression::Gzip, None)
        .unwrap_or_else(|e| unreachable!("{e}"));
    let row = GroupEntry {
        id,
        name_hash: None,
        crc: crc32(&raw),
        hash: None,
        whirlpool: None,
        sizes: Some((raw.len() as u32, payload.len() as u32)),
        version: 1,
        file_ids: files,
        file_name_hashes: None,
    };
    raw.extend_from_slice(&1u16.to_be_bytes());
    store
        .write_raw_group(index, id, &raw)
        .unwrap_or_else(|e| unreachable!("{e}"));
    row
}
fn table(store: &CacheStore, index: u8, groups: Vec<GroupEntry>) {
    let table = ReferenceTable {
        protocol: 7,
        revision: Some(3),
        flags: 4,
        groups,
    };
    let bytes = encode_container(
        table.encode().unwrap_or_else(|e| unreachable!("{e}")),
        ContainerCompression::Gzip,
        None,
    )
    .unwrap_or_else(|e| unreachable!("{e}"));
    store
        .write_raw_group(255, u32::from(index), &bytes)
        .unwrap_or_else(|e| unreachable!("{e}"));
}
fn fixture() -> Fixture {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let root = std::env::temp_dir().join(format!(
        "rustosrs-verified-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let original = root.join("source");
    let output = root.join("output");
    let store = CacheStore::create(&original).unwrap_or_else(|e| unreachable!("{e}"));
    let payload = pack_group(&[&[1, 2], &[3, 4], &[5, 6]]).unwrap_or_else(|e| unreachable!("{e}"));
    let row = group(&store, 5, 0, payload, vec![0, 1, 2]);
    table(&store, 5, vec![row]);
    let row = group(&store, 2, 0, vec![9, 8, 7], vec![0]);
    table(&store, 2, vec![row]);
    let mut profile = crate::registry::bundled_profiles()
        .unwrap_or_else(|e| unreachable!("{e}"))
        .remove(0);
    let fingerprint = CacheProbe::of(&original)
        .unwrap_or_else(|e| unreachable!("{e}"))
        .fingerprint;
    profile.cache_fingerprint.value = Some(fingerprint.clone());
    std::fs::create_dir(&output).unwrap_or_else(|e| unreachable!("{e}"));
    for e in std::fs::read_dir(&original).unwrap_or_else(|e| unreachable!("{e}")) {
        let e = e.unwrap_or_else(|e| unreachable!("{e}"));
        std::fs::copy(e.path(), output.join(e.file_name())).unwrap_or_else(|e| unreachable!("{e}"));
    }
    let payload =
        pack_group(&[&[11, 22], &[33, 44], &[5, 6]]).unwrap_or_else(|e| unreachable!("{e}"));
    let digest = payload_digest(&payload);
    repack_groups(
        &output,
        &[GroupUpdate {
            index_id: 5,
            group_id: 0,
            payload,
        }],
    )
    .unwrap_or_else(|e| unreachable!("{e}"));
    let manifest = ExportManifest {
        format: EXPORT_FORMAT.into(),
        mode: "cache-copy".into(),
        profile: profile.id.clone(),
        source_fingerprint: fingerprint,
        edit_revision: 7,
        output_sha256: cache_digest(&output).unwrap_or_else(|e| unreachable!("{e}")),
        regions: vec![ExportMapGroup {
            region: [0, 0],
            group: 0,
            payload_sha256: digest,
            tiles_changed: 1,
            heights_changed: 0,
            objects_added: 0,
            objects_removed: 0,
        }],
    };
    Fixture {
        root,
        original,
        output,
        profile,
        manifest,
    }
}
fn validate(f: &Fixture) -> Result<VerifiedExport, ExportReadError> {
    VerifiedExport::validate(
        &f.original,
        &f.output,
        &f.manifest,
        std::slice::from_ref(&f.profile),
    )
}

#[test]
fn verified_source_opens_through_the_normal_consumer_api_without_weakening_matching() {
    let f = fixture();
    assert!(CacheRepository::open_matching(&f.output, std::slice::from_ref(&f.profile)).is_err());
    let source = validate(&f)
        .unwrap_or_else(|e| unreachable!("{e}"))
        .source();
    let (cache, profile) = CacheRepository::open_matching(source, std::slice::from_ref(&f.profile))
        .unwrap_or_else(|e| unreachable!("{e}"));
    assert_eq!(profile.id, f.profile.id);
    assert_ne!(cache.fingerprint(), f.manifest.source_fingerprint);
    let files = cache
        .read_group_files(5, 0)
        .unwrap_or_else(|e| unreachable!("{e}"));
    assert_eq!(files[0].1.bytes, vec![11, 22]);
}

#[test]
fn unsupported_manifest_and_wrong_original_are_rejected() {
    let mut f = fixture();
    f.manifest.format = "rustosrs-map-export-v1".into();
    assert!(validate(&f).is_err_and(|e| e.0.contains("Unsupported")));
    f.manifest.format = EXPORT_FORMAT.into();
    f.manifest.source_fingerprint = "wrong".into();
    assert!(validate(&f).is_err_and(|e| e.0.contains("different original")));
}

#[test]
fn corrupted_output_is_rejected_and_verified_tokens_detect_later_writes() {
    let f = fixture();
    let verified = validate(&f).unwrap_or_else(|e| unreachable!("{e}"));
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .append(true)
        .open(f.output.join("main_file_cache.dat2"))
        .unwrap_or_else(|e| unreachable!("{e}"));
    file.write_all(&[123])
        .unwrap_or_else(|e| unreachable!("{e}"));
    assert!(validate(&f).is_err());
    assert!(
        CacheRepository::open_matching(verified.source(), std::slice::from_ref(&f.profile))
            .is_err()
    );
}

#[test]
fn recomputing_the_digest_cannot_hide_an_unlisted_non_map_change() {
    let mut f = fixture();
    let store = CacheStore::open(&f.output);
    let raw = encode_container(vec![99, 88], ContainerCompression::Gzip, Some(1))
        .unwrap_or_else(|e| unreachable!("{e}"));
    store
        .write_raw_group(2, 0, &raw)
        .unwrap_or_else(|e| unreachable!("{e}"));
    f.manifest.output_sha256 = cache_digest(&f.output).unwrap_or_else(|e| unreachable!("{e}"));
    assert!(validate(&f).is_err_and(|e| e.0.contains("Unlisted change")));
}

#[test]
fn preserved_group_files_and_table_versions_are_independently_checked() {
    let mut f = fixture();
    let payload =
        pack_group(&[&[11, 22], &[33, 44], &[99, 99]]).unwrap_or_else(|e| unreachable!("{e}"));
    f.manifest.regions[0].payload_sha256 = payload_digest(&payload);
    repack_groups(
        &f.output,
        &[GroupUpdate {
            index_id: 5,
            group_id: 0,
            payload,
        }],
    )
    .unwrap_or_else(|e| unreachable!("{e}"));
    f.manifest.output_sha256 = cache_digest(&f.output).unwrap_or_else(|e| unreachable!("{e}"));
    assert!(validate(&f).is_err_and(|e| e.0.contains("preserved file")));
}

#[test]
fn duplicate_and_nonexistent_manifest_groups_are_rejected() {
    let mut f = fixture();
    f.manifest.regions.push(f.manifest.regions[0].clone());
    assert!(validate(&f).is_err_and(|e| e.0.contains("duplicate")));
    f.manifest.regions.pop();
    f.manifest.regions[0].region = [1, 1];
    f.manifest.regions[0].group = 257;
    assert!(validate(&f).is_err());
}

#[test]
fn a_valid_manifest_digest_cannot_hide_incorrect_reference_table_metadata() {
    let mut f = fixture();
    let store = CacheStore::open(&f.output);
    let raw = store
        .read_raw_group(255, 5)
        .unwrap_or_else(|e| unreachable!("{e}"))
        .unwrap_or_else(|| unreachable!());
    let mut changed = ReferenceTable::parse(
        &crate::encode::decode_container(raw).unwrap_or_else(|e| unreachable!("{e}")),
    )
    .unwrap_or_else(|e| unreachable!("{e}"));
    changed.groups[0].version += 1;
    table(&store, 5, changed.groups);
    f.manifest.output_sha256 = cache_digest(&f.output).unwrap_or_else(|e| unreachable!("{e}"));
    assert!(validate(&f).is_err_and(|e| e.0.contains("Reference table")));
}
