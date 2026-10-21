use super::*;
use osrs_core::{
    coords::MapTile,
    provenance::{CacheFingerprint, ProfileDigest, TargetProvenance},
};
pub(super) fn fixture() -> (BuildState, MapDocument, Source) {
    let mut state = BuildState {
        site: Some(MapTile::new(3300, 3300)),
        form: BuildForm {
            clear_site: false,
            ..Default::default()
        },
        ..Default::default()
    };
    let plane = SourcePlane::new(0).unwrap_or_else(|| unreachable!());
    let document = MapDocument::from_parts(
        [],
        (-3..=17).flat_map(|x| {
            (-3..=15).map(move |y| {
                (
                    osrs_edit::HeightVertexCoord::new(MapTile::new(3300 + x, 3300 + y), plane),
                    -64,
                )
            })
        }),
        [],
    );
    state.refresh_for_script(&document, 7, 9, &|_, _| None);
    assert!(state.preview.is_some(), "{:?}", state.error);
    let target = TargetProvenance::new(
        "fixture",
        ProfileDigest::from_digest(Digest256::from_bytes([1; 32])),
        CacheFingerprint::from_digest(Digest256::from_bytes([2; 32])),
        1,
    )
    .unwrap_or_else(|e| panic!("{e:?}"));
    (state, document, Source::new(&target, 241))
}
#[test]
fn frozen_candidate_replans_commits_and_round_trips_through_existing_gateway() {
    let (state, document, source) = fixture();
    let text = state
        .freeze_reproduction(&document, 7, 9, &|_, _| None, source)
        .unwrap_or_else(|e| {
            panic!(
                "{e}; mode={:?}; stage={:?}; planning={:?}",
                state.form.mode, state.form.stage, state.error
            )
        })
        .encode()
        .unwrap_or_else(|e| panic!("{e}"));
    if let Some(path) = std::env::var_os("RUSTOSRS_REPRO_FIXTURE_OUTPUT") {
        std::fs::write(path, &text).unwrap_or_else(|e| panic!("{e}"));
    }
    let report = verify(&text).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(report.status, "MATCH");
    assert!(report.same_recorded_build_context);
    assert!(report.operation_count > 0);
}
fn encode(state: &BuildState, document: &MapDocument, source: Source) -> String {
    state
        .freeze_reproduction(document, 7, 9, &|_, _| None, source)
        .unwrap_or_else(|e| {
            panic!(
                "{e}; mode={:?}; stage={:?}; planning={:?}",
                state.form.mode, state.form.stage, state.error
            )
        })
        .encode()
        .unwrap_or_else(|e| panic!("{e}"))
}
fn bundle(text: &str) -> Bundle {
    serde_json::from_str(text).unwrap_or_else(|e| panic!("{e}"))
}
fn forged(mut bundle: Bundle, change: impl FnOnce(&mut Bundle)) -> String {
    change(&mut bundle);
    bundle.inputs_digest = input_digest(&bundle.inputs).unwrap_or_else(|e| panic!("{e}"));
    serde_json::to_string(&bundle).unwrap_or_else(|e| panic!("{e}"))
}
#[test]
fn capture_rejects_stale_sources_revisions_forms_and_arriving_definitions_but_retains_frozen_evidence()
 {
    let (mut state, document, source) = fixture();
    for (generation, revision) in [(8, 9), (7, 10)] {
        assert!(
            state
                .freeze_reproduction(
                    &document,
                    generation,
                    revision,
                    &|_, _| None,
                    source.clone()
                )
                .is_err()
        );
    }
    assert!(
        state
            .freeze_reproduction(&document, 7, 9, &|_, _| Some((1, 1)), source.clone())
            .is_err()
    );
    let capture = state
        .freeze_reproduction(&document, 7, 9, &|_, _| None, source.clone())
        .unwrap_or_else(|e| panic!("{e}"));
    state.form.variation += 1;
    assert!(
        state
            .freeze_reproduction(&document, 7, 9, &|_, _| None, source)
            .is_err()
    );
    state.site = None;
    state.refresh_source(&document, 7, 10, &|_, _| None);
    assert!(verify(&capture.encode().unwrap_or_else(|e| panic!("{e}"))).is_ok());
}
#[test]
fn selected_wrapping_batch_seed_is_not_replaced_by_the_request_base_seed() {
    let (mut state, document, source) = fixture();
    state.form.seed = u64::MAX;
    state.refresh_source(&document, 7, 9, &|_, _| None);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while state.pending.is_some() {
        assert!(
            std::time::Instant::now() < deadline,
            "candidate worker did not settle"
        );
        state.refresh_source(&document, 7, 9, &|_, _| None);
        std::thread::yield_now();
    }
    let index = state
        .candidates
        .iter()
        .position(|p| p.form.seed != u64::MAX)
        .unwrap_or_else(|| panic!("{:?}", state.failures));
    state.select_candidate(index, 7, 9, &|_, _| None);
    let text = encode(&state, &document, source);
    let b = bundle(&text);
    assert_eq!(b.inputs.request.seed, u64::MAX);
    assert!(b.inputs.candidate.seed.wrapping_sub(u64::MAX) < 8);
    assert_ne!(b.inputs.candidate.seed, b.inputs.request.seed);
    assert!(verify(&text).is_ok());
}
#[test]
fn negative_world_coordinates_reproduce_without_task_local_narrowing() {
    let (mut state, document, source) = fixture();
    let plane = SourcePlane::new(0).unwrap_or_else(|| unreachable!());
    let shifted = MapDocument::from_parts(
        [],
        document.height_entries().map(|(c, h)| {
            (
                osrs_edit::HeightVertexCoord::new(
                    MapTile::new(c.corner.x - 3400, c.corner.y - 3400),
                    plane,
                ),
                h,
            )
        }),
        [],
    );
    state.site = Some(MapTile::new(-100, -100));
    state.refresh_for_script(&shifted, 7, 9, &|_, _| None);
    let text = encode(&state, &shifted, source);
    assert_eq!(bundle(&text).inputs.origin, [-100, -100]);
    let report = verify(&text).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(report.status, "MATCH");
}
fn damaged(
    document: &MapDocument,
    state: &BuildState,
    stage: osrs_edit::BuildingStage,
) -> MapDocument {
    use osrs_edit::{BuildingStage as S, HeightVertexCoord, TerrainTile};
    let origin = state.site.unwrap_or_else(|| unreachable!());
    let p = SourcePlane::new(0).unwrap_or_else(|| unreachable!());
    let op = match stage {
        S::Foundation => {
            let coord = HeightVertexCoord::new(MapTile::new(origin.x + 3, origin.y + 3), p);
            let before = document.height_at(coord).unwrap_or_else(|| unreachable!());
            MapOp::SetHeight {
                coord,
                before,
                after: before - 16,
            }
        }
        S::Floors => {
            let (coord, before) = document
                .terrain_entries()
                .next()
                .unwrap_or_else(|| panic!("floor fixture is empty"));
            MapOp::SetTerrain {
                coord,
                before,
                after: TerrainTile::default(),
            }
        }
        _ => {
            let placement = document
                .object_entries()
                .into_iter()
                .find(|placement| {
                    !state.form.locks.contains(placement)
                        && state.kits[0].kit.stage_of(&MapOp::AddObject {
                            placement: *placement,
                        }) == stage
                })
                .unwrap_or_else(|| panic!("No {stage:?} object in authored fixture"));
            MapOp::RemoveObject { placement }
        }
    };
    let mut result = document.clone();
    result
        .apply_patch(&MapPatch::new([op]))
        .unwrap_or_else(|e| panic!("{e:?}"));
    result
}
#[test]
fn all_product_modes_and_stage_lock_repair_inputs_round_trip_through_real_planning() {
    use osrs_edit::BuildingStage as S;
    let (mut state, mut document, source) = fixture();
    let origin = state.site.unwrap_or_else(|| unreachable!());
    // Explicit fixture material makes a missing floor a meaningful completion, rather than a
    // no-op default terrain channel. Generation/validation continue through the canonical kit.
    let mut file = state.kits[0].file.clone();
    file.floor.underlay = Some(3);
    if let Some(finish) = &mut file.finish {
        for floor in finish.floors.values_mut() {
            floor.underlay = Some(3);
        }
    }
    let kit = file.compile().unwrap_or_else(|e| panic!("{e}"));
    state.kits[0] = Kit {
        name: kit.name().into(),
        kit,
        file,
    };
    state.form.sketch = Some(osrs_edit::BuildingSketch {
        origin,
        floors: (1..state.form.width - 1)
            .flat_map(|x| {
                (1..state.form.height - 1).map(move |y| osrs_edit::PrefabCell { x, y, plane: 0 })
            })
            .collect(),
        semantics: osrs_edit::PrefabSemantics::default(),
    });
    state.refresh_for_script(&document, 7, 9, &|_, _| None);
    let patch = state
        .preview
        .as_ref()
        .unwrap_or_else(|| panic!("{:?}", state.error))
        .apply
        .clone();
    document
        .apply_patch(&patch)
        .unwrap_or_else(|e| panic!("{e:?}"));
    state.form.locks = document.object_entries().into_iter().take(1).collect();
    state.form.region = RectSite::new(origin, state.form.width, state.form.height);
    for (mode, stage) in [
        (GenerationMode::Generate, S::Shell),
        (GenerationMode::Complete, S::Shell),
        (GenerationMode::Furnish, S::Furnishings),
        (GenerationMode::ReplaceStyle, S::Shell),
        (GenerationMode::Repair, S::Shell),
    ] {
        let altered = damaged(&document, &state, stage);
        let mut run = BuildState {
            kits: state.kits.clone(),
            site: state.site,
            form: BuildForm {
                mode,
                ..state.form.clone()
            },
            ..Default::default()
        };
        run.refresh_for_script(&altered, 7, 9, &|_, _| None);
        let text = encode(&run, &altered, source.clone());
        let b = bundle(&text);
        assert_eq!(b.inputs.candidate, run.form);
        assert_eq!(b.inputs.request, run.form);
        verify(&text).unwrap_or_else(|e| panic!("{mode:?}: {e}"));
    }
    for stage in [
        S::Foundation,
        S::Floors,
        S::Circulation,
        S::Shell,
        S::Roof,
        S::Furnishings,
        S::Decoration,
    ] {
        let altered = damaged(&document, &state, stage);
        let mut run = BuildState {
            kits: state.kits.clone(),
            site: state.site,
            form: BuildForm {
                stage: Some(stage),
                ..state.form.clone()
            },
            ..Default::default()
        };
        run.refresh_for_script(&altered, 7, 9, &|_, _| None);
        let text = encode(&run, &altered, source.clone());
        assert_eq!(bundle(&text).inputs.candidate.stage, Some(stage));
        verify(&text).unwrap_or_else(|e| panic!("{stage:?}: {e}"));
    }
}

#[test]
fn changed_inputs_and_each_expected_digest_are_detected_without_replacing_expectations() {
    let (state, document, source) = fixture();
    let text = encode(&state, &document, source);
    let b = bundle(&text);
    let mut raw: serde_json::Value = serde_json::from_str(&text).unwrap_or_else(|e| panic!("{e}"));
    raw["inputs"]["candidate"]["variation"] = serde_json::json!(42);
    assert!(matches!(
        verify(&raw.to_string()),
        Err(Error::Mismatch {
            stage: "inputs",
            ..
        })
    ));
    for stage in ["building patch", "apply patch", "result map"] {
        let changed = forged(b.clone(), |b| match stage {
            "building patch" => b.expected_building_digest = "0".repeat(64),
            "apply patch" => b.expected_patch_digest = "0".repeat(64),
            _ => b.expected_result_digest = "0".repeat(64),
        });
        assert!(
            matches!(verify(&changed),Err(Error::Mismatch{stage:s,..}) if s==stage),
            "{stage}"
        );
    }
    let changed = forged(b, |b| {
        let initial = MapDocument::from_snapshot_json_v1(&b.inputs.initial_map_json)
            .unwrap_or_else(|e| panic!("{e}"));
        let shifted = MapDocument::from_parts(
            initial.terrain_entries(),
            initial.height_entries().map(|(c, h)| (c, h - 8)),
            initial.object_entries(),
        );
        b.inputs.initial_map_json = shifted
            .to_snapshot_json_v1()
            .unwrap_or_else(|e| panic!("{e}"));
    });
    assert!(
        matches!(verify(&changed), Err(Error::Mismatch { .. })),
        "Changed terrain must not match the recorded result"
    );
}
#[test]
fn unsupported_missing_malformed_and_mismatched_records_fail_closed() {
    let (state, document, source) = fixture();
    let text = encode(&state, &document, source);
    let b = bundle(&text);
    for field in ["format", "feature", "planner_contract"] {
        let mut raw: serde_json::Value =
            serde_json::from_str(&text).unwrap_or_else(|e| panic!("{e}"));
        raw[field] = serde_json::json!("unsupported");
        assert!(matches!(verify(&raw.to_string()), Err(Error::Input(_))));
    }
    for path in [
        "",
        "/inputs",
        "/inputs/request",
        "/inputs/candidate",
        "/inputs/source",
        "/inputs/editor_build",
    ] {
        let mut raw: serde_json::Value =
            serde_json::from_str(&text).unwrap_or_else(|e| panic!("{e}"));
        raw.pointer_mut(path)
            .unwrap_or_else(|| unreachable!())
            .as_object_mut()
            .unwrap_or_else(|| unreachable!())
            .insert("unsupported".into(), serde_json::json!(true));
        assert!(
            matches!(verify(&raw.to_string()), Err(Error::Schema(_))),
            "{path}"
        );
    }
    for field in ["stage", "sketch", "region", "locks"] {
        let mut raw: serde_json::Value =
            serde_json::from_str(&text).unwrap_or_else(|e| panic!("{e}"));
        raw["inputs"]["candidate"]
            .as_object_mut()
            .unwrap_or_else(|| unreachable!())
            .remove(field);
        assert!(
            matches!(verify(&raw.to_string()), Err(Error::Schema(_))),
            "{field}"
        );
    }
    for changed in [
        forged(b.clone(), |b| b.inputs.source.build = 0),
        forged(b.clone(), |b| b.inputs.source.decoder_schema_version = 0),
        forged(b.clone(), |b| b.inputs.source.profile_id = " ".into()),
        forged(b.clone(), |b| {
            b.inputs.source.cache_fingerprint = "X".repeat(64)
        }),
        forged(b.clone(), |b| b.inputs.plane = 4),
        forged(b.clone(), |b| b.inputs.origin = [i32::MAX, i32::MIN]),
        forged(b.clone(), |b| {
            b.inputs.request.seed = b.inputs.candidate.seed.wrapping_add(8)
        }),
        forged(b.clone(), |b| b.inputs.candidate.width = 0),
        forged(b.clone(), |b| b.inputs.kit_json = "{}".into()),
        forged(b.clone(), |b| b.inputs.initial_map_json = "{}".into()),
    ] {
        assert!(verify(&changed).is_err());
    }
    assert!(verify(&text[..text.len() - 1]).is_err());
    let duplicate = text.replacen("\"format\":", "\"format\":\"ignored\",\"format\":", 1);
    assert!(matches!(verify(&duplicate), Err(Error::Schema(_))));
    let changed = forged(b, |b| b.expected_result_digest = "bad".into());
    assert!(matches!(verify(&changed), Err(Error::Input(_))));
}
#[test]
fn footprints_reuse_canonical_requirements_and_reject_missing_duplicate_or_false_declarations() {
    let (state, document, source) = fixture();
    let text = encode(&state, &document, source);
    let b = bundle(&text);
    assert!(!b.inputs.footprints.is_empty());
    for changed in [
        forged(b.clone(), |b| {
            b.inputs.footprints.pop();
        }),
        forged(b.clone(), |b| {
            b.inputs.footprints.push(b.inputs.footprints[0].clone());
        }),
    ] {
        assert!(matches!(verify(&changed), Err(Error::Input(_))));
    }
    let mut raw: serde_json::Value = serde_json::from_str(&text).unwrap_or_else(|e| panic!("{e}"));
    raw["inputs"]["footprints"][0]["declared"] = serde_json::json!([37, 39]);
    let mut changed: Bundle = serde_json::from_value(raw).unwrap_or_else(|e| panic!("{e}"));
    changed.inputs_digest = input_digest(&changed.inputs).unwrap_or_else(|e| panic!("{e}"));
    assert!(matches!(
        verify(&serde_json::to_string(&changed).unwrap_or_else(|e| panic!("{e}"))),
        Err(Error::Input(_))
    ));
}
#[test]
fn recorded_build_context_is_reported_honestly_across_verifier_builds() {
    let (state, document, source) = fixture();
    let b = bundle(&encode(&state, &document, source));
    let changed = forged(b, |b| {
        b.inputs.editor_build.workspace_source_digest = "1".repeat(64);
        b.inputs.editor_build.rustc = "recorded different compiler".into();
    });
    let report = verify(&changed).unwrap_or_else(|e| panic!("{e}"));
    assert!(!report.same_recorded_build_context);
    assert_eq!(report.status, "MATCH");
    assert_ne!(report.captured_build, report.verifier_build);
}
#[test]
fn background_export_is_bounded_published_complete_and_never_overwrites_existing_reports() {
    let (mut state, document, source) = fixture();
    let dir = crate::export_flow::test_support::temp("building-reproduction");
    let out = dir.join("bundle.json");
    let first = state
        .freeze_reproduction(&document, 7, 9, &|_, _| None, source.clone())
        .unwrap_or_else(|e| panic!("{e}"));
    let second = state
        .freeze_reproduction(&document, 7, 9, &|_, _| None, source.clone())
        .unwrap_or_else(|e| panic!("{e}"));
    state
        .start_reproduction(first, out.clone())
        .unwrap_or_else(|e| panic!("{e}"));
    assert!(
        state
            .start_reproduction(second, dir.join("second.json"))
            .is_err()
    );
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    let result = loop {
        if let Some(r) = state.poll_reproduction() {
            break r;
        }
        assert!(std::time::Instant::now() < deadline);
        std::thread::yield_now();
    };
    assert!(result.is_ok());
    let text = std::fs::read_to_string(&out).unwrap_or_else(|e| panic!("{e}"));
    assert!(verify(&text).is_ok());
    let next = state
        .freeze_reproduction(&document, 7, 9, &|_, _| None, source)
        .unwrap_or_else(|e| panic!("{e}"));
    state
        .start_reproduction(next, out.clone())
        .unwrap_or_else(|e| panic!("{e}"));
    let result = loop {
        if let Some(r) = state.poll_reproduction() {
            break r;
        }
        assert!(std::time::Instant::now() < deadline);
        std::thread::yield_now();
    };
    assert!(result.is_err());
    assert_eq!(
        std::fs::read_to_string(&out).unwrap_or_else(|e| panic!("{e}")),
        text
    );
    assert_eq!(
        std::fs::read_dir(&dir)
            .unwrap_or_else(|e| panic!("{e}"))
            .count(),
        1,
        "staging retires on success and failure"
    );
    assert!(native::run_cli(&[out.display().to_string()]).is_ok());
    assert!(native::run_cli(&[]).is_err());
    assert!(native::run_cli(&[dir.join("missing").display().to_string()]).is_err());
    std::fs::remove_dir_all(dir).unwrap_or_else(|e| panic!("{e}"));
}

#[test]
fn frozen_source_dimensions_are_used_instead_of_portable_furniture_declarations() {
    let (mut state, document, source) = fixture();
    state.refresh_for_script(&document, 7, 9, &|_, _| Some((1, 1)));
    let text = state
        .freeze_reproduction(&document, 7, 9, &|_, _| Some((1, 1)), source)
        .unwrap_or_else(|e| panic!("{e}"))
        .encode()
        .unwrap_or_else(|e| panic!("{e}"));
    let b = bundle(&text);
    let file: KitFile = serde_json::from_str(&b.inputs.kit_json).unwrap_or_else(|e| panic!("{e}"));
    let records = serde_json::to_value(&b.inputs.footprints).unwrap_or_else(|e| panic!("{e}"));
    let declared = records
        .as_array()
        .unwrap_or_else(|| unreachable!())
        .iter()
        .find(|r| r["declared"].is_array() && r["declared"] != serde_json::json!([1, 1]))
        .unwrap_or_else(|| panic!("fixture needs a non-unit furniture declaration"));
    let id = ObjectId::new(
        u32::try_from(declared["id"].as_u64().unwrap_or_else(|| unreachable!()))
            .unwrap_or_else(|e| panic!("{e}")),
    );
    let kind = LocKind::new(
        u8::try_from(declared["kind"].as_u64().unwrap_or_else(|| unreachable!()))
            .unwrap_or_else(|e| panic!("{e}")),
    )
    .unwrap_or_else(|| unreachable!());
    let restored = FootprintInputs::restore(b.inputs.footprints, &document, &file)
        .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(
        restored.resolve(id, kind).unwrap_or_else(|e| panic!("{e}")),
        (1, 1)
    );
    verify(&text).unwrap_or_else(|e| panic!("{e}"));
}

#[test]
fn headless_size_and_file_kind_guards_precede_untrusted_allocation_or_execution() {
    let dir = crate::export_flow::test_support::temp("reproduction-limits");
    let large = dir.join("large.json");
    let file = std::fs::File::create(&large).unwrap_or_else(|e| panic!("{e}"));
    file.set_len(MAX_BYTES as u64 + 1)
        .unwrap_or_else(|e| panic!("{e}"));
    drop(file);
    assert!(native::run_cli(&[large.display().to_string()]).is_err());
    assert!(native::run_cli(&[dir.display().to_string()]).is_err());
    std::fs::remove_dir_all(dir).unwrap_or_else(|e| panic!("{e}"));
    let (state, document, source) = fixture();
    let b = bundle(&encode(&state, &document, source));
    let invalid = forged(b, |b| {
        b.inputs.request.floors = 255;
        b.inputs.candidate.floors = 255;
    });
    assert!(matches!(verify(&invalid), Err(Error::Input(_))));
}

#[test]
fn clearing_dense_existing_objects_reproduces_exact_patch_order_after_snapshot_reconstruction() {
    let (mut state, base, source) = fixture();
    state.form.clear_site = true;
    let p = SourcePlane::new(0).unwrap_or_else(|| unreachable!());
    let objects: Vec<_> = (0..48)
        .map(|i| osrs_edit::ObjectPlacement {
            object_id: ObjectId::new(900000 + i),
            position: osrs_edit::MapPosition::new(
                MapTile::new(3300 + (i % 12) as i32, 3300 + (i / 12) as i32),
                p,
            ),
            kind: LocKind::new(22).unwrap_or_else(|| unreachable!()),
            orientation: osrs_edit::ObjectOrientation::new((i % 4) as u8)
                .unwrap_or_else(|| unreachable!()),
        })
        .collect();
    let mut expected = None;
    for shift in 0..16 {
        let mut input = objects.clone();
        input.rotate_left(shift);
        let document =
            MapDocument::from_parts(base.terrain_entries(), base.height_entries(), input);
        state.refresh_for_script(&document, 7, 9, &|_, _| None);
        let preview = state
            .preview
            .as_ref()
            .unwrap_or_else(|| panic!("{:?}", state.error));
        assert_eq!(preview.removed, objects.len());
        let text = encode(&state, &document, source.clone());
        let report = verify(&text).unwrap_or_else(|e| panic!("{e}"));
        if let Some((patch, result)) = &expected {
            assert_eq!(&report.patch_digest, patch);
            assert_eq!(&report.result_digest, result);
        } else {
            expected = Some((report.patch_digest, report.result_digest));
        }
    }
}
