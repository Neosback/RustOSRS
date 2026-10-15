use super::*;
use crate::export_flow::test_support::*;
use osrs_cache::transport::CacheRepository;

#[test]
fn both_modes_reopen_identical_semantics_and_preserved_files_without_changing_source() {
    let f = fixture();
    let prepared = prepared(&f);
    assert!(prepared.can_publish(), "{:?}", prepared.issues);
    let before = cache_digest(&f.dir).unwrap_or_else(|e| unreachable!("{e}"));
    let root = temp("publish");
    for mode in [ExportMode::Loose, ExportMode::CacheCopy] {
        let out = root.join(mode.name());
        let report = publish_with_profiles(
            &prepared,
            mode,
            &out,
            std::slice::from_ref(&f.profile),
            |_| Ok(()),
        )
        .unwrap_or_else(|e| unreachable!("{e}"));
        assert_eq!(report.manifest.regions.len(), 2);
        assert_eq!(report.stamp, prepared.stamp);
        assert_eq!(report.unchanged, 2);
        if mode == ExportMode::CacheCopy {
            let verified = VerifiedExport::validate(
                &f.dir,
                &out,
                &report.manifest,
                std::slice::from_ref(&f.profile),
            )
            .unwrap_or_else(|e| unreachable!("{e}"));
            let (cache, _) =
                CacheRepository::open_matching(verified.source(), std::slice::from_ref(&f.profile))
                    .unwrap_or_else(|e| unreachable!("{e}"));
            assert_eq!(
                cache
                    .group_ids(5)
                    .unwrap_or_else(|e| unreachable!("{e}"))
                    .len(),
                4
            );
        }
    }
    assert_eq!(
        cache_digest(&f.dir).unwrap_or_else(|e| unreachable!("{e}")),
        before
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn write_and_verification_failures_never_publish_and_remove_staging() {
    let f = fixture();
    let prepared = prepared(&f);
    let root = temp("failure");
    let out = root.join("output");
    std::fs::create_dir(&out).unwrap_or_else(|e| unreachable!("{e}"));
    let failed = publish_with_profiles(
        &prepared,
        ExportMode::Loose,
        &out,
        std::slice::from_ref(&f.profile),
        |_| Err("injected write failure".into()),
    );
    assert!(failed.is_err());
    assert_eq!(
        std::fs::read_dir(&out)
            .unwrap_or_else(|e| unreachable!("{e}"))
            .count(),
        0
    );
    let corrupt = publish_with_profiles(
        &prepared,
        ExportMode::Loose,
        &out,
        std::slice::from_ref(&f.profile),
        |stage| {
            std::fs::write(
                stage.join(format!("5/{}.dat", prepared.regions[0].group)),
                [255],
            )
            .map_err(|e| e.to_string())
        },
    );
    assert!(corrupt.is_err());
    assert_eq!(
        std::fs::read_dir(&out)
            .unwrap_or_else(|e| unreachable!("{e}"))
            .count(),
        0
    );
    assert_eq!(
        std::fs::read_dir(&root)
            .unwrap_or_else(|e| unreachable!("{e}"))
            .count(),
        1,
        "staging directories must be cleaned"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn occupied_overlapping_and_concurrently_populated_destinations_are_preserved() {
    let f = fixture();
    let prepared = prepared(&f);
    let root = temp("destinations");
    let out = root.join("output");
    std::fs::create_dir(&out).unwrap_or_else(|e| unreachable!("{e}"));
    std::fs::write(out.join("keep"), b"original").unwrap_or_else(|e| unreachable!("{e}"));
    assert!(
        publish_with_profiles(
            &prepared,
            ExportMode::CacheCopy,
            &out,
            std::slice::from_ref(&f.profile),
            |_| Ok(())
        )
        .is_err()
    );
    assert_eq!(
        std::fs::read(out.join("keep")).unwrap_or_else(|e| unreachable!("{e}")),
        b"original"
    );
    let overlap = f.dir.join("export");
    assert!(
        publish_with_profiles(
            &prepared,
            ExportMode::Loose,
            &overlap,
            std::slice::from_ref(&f.profile),
            |_| Ok(())
        )
        .is_err()
    );
    let out2 = root.join("race");
    std::fs::create_dir(&out2).unwrap_or_else(|e| unreachable!("{e}"));
    assert!(
        publish_with_profiles(
            &prepared,
            ExportMode::Loose,
            &out2,
            std::slice::from_ref(&f.profile),
            |_| { std::fs::write(out2.join("keep"), b"concurrent").map_err(|e| e.to_string()) }
        )
        .is_err()
    );
    assert_eq!(
        std::fs::read(out2.join("keep")).unwrap_or_else(|e| unreachable!("{e}")),
        b"concurrent"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn continuing_to_edit_after_capture_does_not_change_the_exported_revision() {
    let f = fixture();
    let mut snapshot = snapshot(&f);
    let p = position(3263, 3202, 0);
    paint(&mut snapshot.document, p, 5);
    let prepared = super::super::prepare::prepare_with(
        snapshot.clone(),
        f.profile
            .cache_fingerprint
            .value
            .clone()
            .unwrap_or_default(),
        |r| Ok(Some(square(r))),
    );
    paint(&mut snapshot.document, p, 8);
    snapshot.stamp.edit_revision += 1;
    let root = temp("captured");
    let out = root.join("output");
    let report = publish_with_profiles(
        &prepared,
        ExportMode::CacheCopy,
        &out,
        std::slice::from_ref(&f.profile),
        |_| Ok(()),
    )
    .unwrap_or_else(|e| unreachable!("{e}"));
    assert_eq!(report.manifest.edit_revision, 1);
    assert_eq!(
        prepared.regions[0]
            .expected
            .terrain_at(p)
            .underlay
            .map(|id| id.get()),
        Some(5)
    );
    assert_eq!(
        snapshot.document.terrain_at(p).underlay.map(|id| id.get()),
        Some(8)
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
#[ignore = "requires RUSTOSRS_TARGET_CACHE_DIR pointing to the pinned real build-241 cache"]
fn real_cache_copy_can_be_verified_and_read_through_all_source_consumers() {
    let dir = std::env::var_os("RUSTOSRS_TARGET_CACHE_DIR")
        .unwrap_or_else(|| panic!("Set RUSTOSRS_TARGET_CACHE_DIR for this extended test."));
    let source = PathBuf::from(dir);
    let squares = osrs_world::SourceSquares::open(&source).unwrap_or_else(|e| unreachable!("{e}"));
    let r = osrs_core::coords::RegionCoord::new(50, 50);
    let square = squares
        .read(r)
        .unwrap_or_else(|e| unreachable!("{e}"))
        .unwrap_or_else(|| unreachable!());
    let mut document = crate::session::baseline_document(&square.terrain, &square.locations)
        .unwrap_or_else(|e| unreachable!("{e:?}"));
    paint(&mut document, position(3210, 3210, 0), 5);
    let snapshot = ExportSnapshot {
        stamp: ExportStamp {
            epoch: 0,
            job: 0,
            edit_revision: 1,
        },
        source: CacheSource::Dir(source.clone()),
        profile: squares.profile_id().into(),
        document,
        authored: [r].into_iter().collect(),
        pending: Vec::new(),
    };
    let prepared = crate::export_flow::prepare(snapshot);
    assert!(prepared.can_publish(), "{:?}", prepared.issues);
    let root = temp("real");
    let out = root.join("output");
    let report =
        publish(&prepared, ExportMode::CacheCopy, &out).unwrap_or_else(|e| unreachable!("{e}"));
    let profiles = osrs_cache::registry::bundled_profiles().unwrap_or_else(|e| unreachable!("{e}"));
    let verified = VerifiedExport::validate(&source, &out, &report.manifest, &profiles)
        .unwrap_or_else(|e| unreachable!("{e}"));
    let verified = verified.source();
    assert!(osrs_world::SourceSquares::open(verified.clone()).is_ok());
    assert!(osrs_world::WorldDefinitions::open(verified.clone()).is_ok());
    assert!(osrs_world::load_floor_palette(verified.clone()).is_ok());
    assert!(osrs_world::load_object_catalog(verified).is_ok());
    let _ = std::fs::remove_dir_all(root);
}
