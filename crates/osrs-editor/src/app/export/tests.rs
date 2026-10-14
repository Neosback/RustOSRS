use super::*;
use osrs_cache::encode::{decode_container, pack_group};

#[test]
fn packaging_replaces_only_the_terrain_and_location_files() {
    let original = vec![
        (0, vec![1, 1, 1]),
        (1, vec![2, 2]),
        (2, vec![9, 9, 9, 9]),
        (3, vec![]),
    ];
    let container = package_group(&original, &[7; 5], &[8]).unwrap_or_else(|e| unreachable!("{e}"));
    let payload = decode_container(container).unwrap_or_else(|e| unreachable!("{e}"));
    let expected =
        pack_group(&[&[7; 5], &[8], &[9, 9, 9, 9], &[]]).unwrap_or_else(|e| unreachable!("{e}"));
    assert_eq!(payload, expected);
}

/// Needs the pinned cache (`RUSTOSRS_TARGET_CACHE_DIR`); skipped when it is not configured.
#[test]
fn exporting_a_real_edit_writes_a_container_that_decodes_back_to_the_edit() {
    use osrs_cache::decode::{DecoderContext, decode_terrain};
    use osrs_core::coords::{MapTile, SourcePlane};
    use osrs_core::ids::FloorUnderlayId;
    use osrs_edit::{MapOp, MapPatch, MapPosition, TerrainTile};

    let Some(dir) = std::env::var_os("RUSTOSRS_TARGET_CACHE_DIR") else {
        return;
    };
    let source = || osrs_cache::source::CacheSource::Dir(PathBuf::from(&dir));
    let squares = SourceSquares::open(source()).unwrap_or_else(|e| unreachable!("{e}"));
    let region = RegionCoord::new(50, 50);
    let square = squares
        .read(region)
        .unwrap_or_else(|e| unreachable!("{e}"))
        .unwrap_or_else(|| unreachable!("square 50,50 exists"));
    assert_eq!(
        square.files.len(),
        5,
        "build-241 map groups hold five files"
    );

    // The loaded document of a freshly streamed region, then one painted tile.
    let mut document = crate::session::baseline_document(&square.terrain, &square.locations)
        .unwrap_or_else(|e| unreachable!("{e:?}"));
    let plane = SourcePlane::new(0).unwrap_or_else(|| unreachable!("plane zero"));
    let position = MapPosition::new(MapTile::new(3210, 3210), plane);
    let before = document.terrain_at(position);
    let after = TerrainTile {
        underlay: Some(FloorUnderlayId::new(5)),
        ..before
    };
    document
        .apply_patch(&MapPatch::new(vec![MapOp::SetTerrain {
            coord: position,
            before,
            after,
        }]))
        .unwrap_or_else(|e| unreachable!("{e:?}"));

    let out = std::env::temp_dir().join(format!("rustosrs-export-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&out);
    let report = run_export(source(), &out, &document, &[region], ExportMode::Loose)
        .unwrap_or_else(|e| unreachable!("{e}"));
    assert_eq!((report.written, report.unchanged), (1, 0));

    let file = out.join(format!("5/{}.dat", square.group_id));
    let container = std::fs::read(&file).unwrap_or_else(|e| unreachable!("{e}"));
    let payload = decode_container(container).unwrap_or_else(|e| unreachable!("{e}"));
    let files =
        osrs_cache::encode::split_group(&payload, 5).unwrap_or_else(|e| unreachable!("{e}"));
    assert_eq!(files[2..], square.files[2..], "files 2..=4 are preserved");

    let profiles =
        osrs_cache::registry::bundled_profiles().unwrap_or_else(|e| unreachable!("{e:?}"));
    let profile = profiles
        .iter()
        .find(|p| p.id == squares.profile_id())
        .unwrap_or_else(|| unreachable!("profile"));
    let context = DecoderContext::from_profile(profile).unwrap_or_else(|e| unreachable!("{e:?}"));
    let decoded = decode_terrain(
        &files[0].1,
        &context,
        &osrs_cache::decode::ArchiveFileProvenance::new(5, square.group_id, Some(0)),
        region,
    )
    .unwrap_or_else(|e| unreachable!("{e:?}"));
    let index = (10 * 64) + 10;
    assert_eq!(decoded.tiles()[index].underlay_id, 5);
    let manifest = std::fs::read_to_string(out.join("manifest.json")).unwrap_or_default();
    assert!(manifest.contains("rustosrs-map-export-v1") && manifest.contains("tiles_changed"));
    let _ = std::fs::remove_dir_all(&out);
}

/// Needs the pinned cache (`RUSTOSRS_TARGET_CACHE_DIR`); skipped when it is not configured.
#[test]
fn a_cache_copy_export_repacks_the_edit_and_keeps_the_table_consistent() {
    use osrs_cache::decode::{ArchiveFileProvenance, DecoderContext, decode_terrain};
    use osrs_cache::encode::{
        CacheStore, REFERENCE_TABLE_INDEX, ReferenceTable, crc32, decode_container, split_group,
    };
    use osrs_core::coords::{MapTile, SourcePlane};
    use osrs_core::ids::FloorUnderlayId;
    use osrs_edit::{MapOp, MapPatch, MapPosition, TerrainTile};

    let Some(dir) = std::env::var_os("RUSTOSRS_TARGET_CACHE_DIR") else {
        return;
    };
    let source = || osrs_cache::source::CacheSource::Dir(PathBuf::from(&dir));
    let squares = SourceSquares::open(source()).unwrap_or_else(|e| unreachable!("{e}"));
    let region = RegionCoord::new(50, 50);
    let square = squares
        .read(region)
        .unwrap_or_else(|e| unreachable!("{e}"))
        .unwrap_or_else(|| unreachable!("square 50,50 exists"));
    let mut document = crate::session::baseline_document(&square.terrain, &square.locations)
        .unwrap_or_else(|e| unreachable!("{e:?}"));
    let plane = SourcePlane::new(0).unwrap_or_else(|| unreachable!("plane zero"));
    let position = MapPosition::new(MapTile::new(3210, 3210), plane);
    let before = document.terrain_at(position);
    document
        .apply_patch(&MapPatch::new(vec![MapOp::SetTerrain {
            coord: position,
            before,
            after: TerrainTile {
                underlay: Some(FloorUnderlayId::new(5)),
                ..before
            },
        }]))
        .unwrap_or_else(|e| unreachable!("{e:?}"));

    let out = std::env::temp_dir().join(format!("rustosrs-cache-copy-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&out);
    std::fs::create_dir_all(&out).unwrap_or_else(|e| unreachable!("{e}"));
    let report = run_export(source(), &out, &document, &[region], ExportMode::CacheCopy)
        .unwrap_or_else(|e| unreachable!("{e}"));
    assert_eq!((report.written, report.unchanged), (1, 0));

    let copy = CacheStore::open(&out);
    let original = CacheStore::open(&dir);
    let raw = copy
        .read_raw_group(5, square.group_id)
        .unwrap_or_else(|e| unreachable!("{e}"))
        .unwrap_or_else(|| unreachable!("group in the copy"));
    let table_of = |store: &CacheStore| {
        ReferenceTable::parse(
            &decode_container(
                store
                    .read_raw_group(REFERENCE_TABLE_INDEX, 5)
                    .unwrap_or_else(|e| unreachable!("{e}"))
                    .unwrap_or_else(|| unreachable!("table")),
            )
            .unwrap_or_else(|e| unreachable!("{e}")),
        )
        .unwrap_or_else(|e| unreachable!("{e}"))
    };
    let (new_table, old_table) = (table_of(&copy), table_of(&original));
    let row = new_table
        .group(square.group_id)
        .unwrap_or_else(|| unreachable!("row"));
    let old_row = old_table
        .group(square.group_id)
        .unwrap_or_else(|| unreachable!("row"));
    assert_eq!(row.crc, crc32(&raw[..raw.len() - 2]));
    assert_eq!(row.version, old_row.version + 1);

    let files = split_group(
        &decode_container(raw).unwrap_or_else(|e| unreachable!("{e}")),
        5,
    )
    .unwrap_or_else(|e| unreachable!("{e}"));
    assert_eq!(files[2..], square.files[2..]);
    let profiles =
        osrs_cache::registry::bundled_profiles().unwrap_or_else(|e| unreachable!("{e:?}"));
    let profile = profiles
        .iter()
        .find(|p| p.id == squares.profile_id())
        .unwrap_or_else(|| unreachable!("profile"));
    let context = DecoderContext::from_profile(profile).unwrap_or_else(|e| unreachable!("{e:?}"));
    let decoded = decode_terrain(
        &files[0].1,
        &context,
        &ArchiveFileProvenance::new(5, square.group_id, Some(0)),
        region,
    )
    .unwrap_or_else(|e| unreachable!("{e:?}"));
    assert_eq!(decoded.tiles()[10 * 64 + 10].underlay_id, 5);
    assert!(out.join("rustosrs-export.json").is_file());

    // A non-empty destination is refused before anything is written.
    let refused = run_export(source(), &out, &document, &[region], ExportMode::CacheCopy);
    assert!(refused.is_err());
    let _ = std::fs::remove_dir_all(&out);
}
