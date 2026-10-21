use super::super::tests::fixture;
use super::*;

fn rejected_request() -> (BuildState, MapDocument, Source) {
    let (mut state, document, source) = fixture();
    state.form.mode = GenerationMode::Complete;
    state.refresh_for_script(&document, 7, 9, &|_, _| None);
    assert!(state.preview.is_none());
    assert_eq!(state.failures.len(), 1);
    (state, document, source)
}

fn capture(state: &BuildState, document: &MapDocument, source: Source) -> String {
    state
        .freeze_reproduction(document, 7, 9, &|_, _| None, source)
        .unwrap_or_else(|e| panic!("{e}"))
        .encode()
        .unwrap_or_else(|e| panic!("{e}"))
}

fn decode(text: &str) -> FailureBundle {
    serde_json::from_str(text).unwrap_or_else(|e| panic!("{e}"))
}

fn changed(mut bundle: FailureBundle, edit: impl FnOnce(&mut FailureBundle)) -> String {
    edit(&mut bundle);
    bundle.inputs_digest = input_digest(&bundle.inputs).unwrap_or_else(|e| panic!("{e}"));
    serde_json::to_string(&bundle).unwrap_or_else(|e| panic!("{e}"))
}

#[test]
fn synchronous_rejection_matches_its_typed_boundary_without_an_edit_or_fake_patch() {
    let (state, document, source) = rejected_request();
    let text = capture(&state, &document, source);
    let report = verify_failure(&text).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(report.failure.stage, plan::Stage::Request);
    assert_eq!(
        report.failure.message,
        "Use a drawn room layout before completing, furnishing, restyling or repairing"
    );
    assert_eq!(report.operation_count, 0);
    assert_eq!(
        report.initial_digest,
        document.content_digest_v1().to_string()
    );
    assert_eq!(report.initial_digest, report.result_digest);
    assert!(report.same_recorded_build_context);
    assert!(matches!(verify_any(&text), Ok(Report::Rejected(_))));
    let json = serde_json::to_value(report).unwrap_or_else(|e| panic!("{e}"));
    assert!(json.get("patch_digest").is_none());
    assert!(
        super::super::verify(&text).is_err(),
        "success format must stay closed"
    );
}

#[test]
fn real_native_all_failed_batch_retains_wrapping_seeds_and_reproduces_the_first_rejection() {
    let (mut state, _, source) = fixture();
    let document = MapDocument::default();
    state.form.seed = u64::MAX - 3;
    state.refresh_source(&document, 7, 9, &|_, _| None);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while state.pending.is_some() {
        assert!(
            std::time::Instant::now() < deadline,
            "candidate worker did not finish"
        );
        state.refresh_source(&document, 7, 9, &|_, _| None);
        std::thread::yield_now();
    }
    assert!(state.candidates.is_empty());
    assert_eq!(state.failures.len(), 8);
    for (offset, attempt) in state.failures.iter().enumerate() {
        assert_eq!(
            attempt.form.seed,
            (u64::MAX - 3).wrapping_add(offset as u64)
        );
        assert_eq!(attempt.failure.stage, plan::Stage::Generation);
        assert!(
            attempt.failure.cause.contains("MissingHeight"),
            "{:?}",
            attempt.failure
        );
    }
    assert_eq!(
        state.reproduction_seed(7, 9, &|_, _| None),
        Some(u64::MAX - 3)
    );
    let text = capture(&state, &document, source);
    if let Some(path) = std::env::var_os("RUSTOSRS_FAILURE_FIXTURE_OUTPUT") {
        std::fs::write(path, &text).unwrap_or_else(|e| panic!("{e}"));
    }
    let report = verify_failure(&text).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(report.seed, u64::MAX - 3);
    assert_eq!(report.failure, state.failures[0].failure);
    assert_eq!(report.initial_digest, report.result_digest);
    for attempt in &state.failures {
        let selected = state
            .freeze_rejected_reproduction(
                &document,
                7,
                9,
                &|_, _| None,
                report.source.clone(),
                attempt.form.seed,
            )
            .unwrap_or_else(|e| panic!("{e}"));
        let report = verify_failure(&selected.encode().unwrap_or_else(|e| panic!("{e}")))
            .unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(report.seed, attempt.form.seed);
        assert_eq!(report.failure, attempt.failure);
    }
    assert!(
        state
            .freeze_rejected_reproduction(&document, 7, 9, &|_, _| None, report.source, 100,)
            .is_err()
    );
}

#[test]
fn unsatisfiable_site_and_candidate_validation_failures_keep_their_actual_planner_stages() {
    for expected in [plan::Stage::Generation, plan::Stage::CandidateValidation] {
        let (mut state, document, source) = fixture();
        if expected == plan::Stage::Generation {
            state.form.width = 1;
            state.form.height = 1;
        } else {
            state.form.locks.push(osrs_edit::ObjectPlacement {
                object_id: ObjectId::new(1),
                position: osrs_edit::MapPosition::new(
                    MapTile::new(3300, 3300),
                    SourcePlane::new(0).unwrap_or_else(|| unreachable!()),
                ),
                kind: LocKind::new(0).unwrap_or_else(|| unreachable!()),
                orientation: osrs_edit::ObjectOrientation::new(0).unwrap_or_else(|| unreachable!()),
            });
        }
        state.refresh_for_script(&document, 7, 9, &|_, _| None);
        assert!(state.preview.is_none(), "fixture must fail at {expected:?}");
        let report =
            verify_failure(&capture(&state, &document, source)).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(report.failure.stage, expected);
        assert!(!report.failure.cause.is_empty());
    }
}

#[test]
fn stale_failure_cannot_export_and_frozen_failure_survives_retirement_without_retaining_live_state()
{
    let (mut state, document, source) = rejected_request();
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
        assert_eq!(
            state.reproduction_seed(generation, revision, &|_, _| None),
            None
        );
    }
    assert!(
        state
            .freeze_reproduction(&document, 7, 9, &|_, _| Some((1, 1)), source.clone())
            .is_err()
    );
    let old = Arc::downgrade(state.made_from.as_ref().unwrap_or_else(|| panic!("input")));
    let frozen = state
        .freeze_reproduction(&document, 7, 9, &|_, _| None, source.clone())
        .unwrap_or_else(|e| panic!("{e}"));
    state.form.width += 1;
    assert!(
        state
            .freeze_reproduction(&document, 7, 9, &|_, _| None, source)
            .is_err()
    );
    state.site = None;
    state.refresh_source(&document, 7, 10, &|_, _| None);
    assert!(state.failures.is_empty());
    assert!(
        old.upgrade().is_some(),
        "only the explicit capture may retain retired input"
    );
    assert!(verify_failure(&frozen.encode().unwrap_or_else(|e| panic!("{e}"))).is_ok());
    assert!(old.upgrade().is_none());
}

#[test]
fn failure_identity_changes_and_unexpected_success_are_mismatches_not_successful_reproductions() {
    let (state, document, source) = rejected_request();
    let original = decode(&capture(&state, &document, source));
    for channel in 0..3 {
        let text = changed(original.clone(), |b| match channel {
            0 => b.expected_failure.stage = plan::Stage::Generation,
            1 => b.expected_failure.message.push_str(" changed"),
            _ => b
                .expected_failure
                .cause
                .push_str("changed underlying evidence"),
        });
        assert!(matches!(
            verify_failure(&text),
            Err(Error::Mismatch {
                stage: "planning failure",
                ..
            })
        ));
    }
    let text = changed(original.clone(), |b| {
        b.inputs.request.mode = GenerationMode::Generate;
        b.inputs.candidate.mode = GenerationMode::Generate;
    });
    assert!(matches!(
        verify_failure(&text),
        Err(Error::Mismatch {
            stage: "planning outcome",
            ..
        })
    ));
    let text = changed(original, |b| b.expected_initial_digest = "0".repeat(64));
    assert!(matches!(
        verify_failure(&text),
        Err(Error::Mismatch {
            stage: "initial map",
            ..
        })
    ));
}

#[test]
fn failure_schema_dispatch_preserves_duplicate_keys_and_rejects_missing_or_unknown_evidence() {
    let (state, document, source) = rejected_request();
    let text = capture(&state, &document, source);
    for replacement in [
        "\"stage\": \"request\", \"stage\": \"request\"",
        "\"stage\": \"unknown_stage\"",
        "\"stage\": \"request\", \"unknown\": true",
    ] {
        let malformed = text.replace("\"stage\": \"request\"", replacement);
        assert!(matches!(verify_any(&malformed), Err(Error::Schema(_))));
    }
    for field in ["cause", "message", "stage"] {
        let mut json: serde_json::Value =
            serde_json::from_str(&text).unwrap_or_else(|e| panic!("{e}"));
        json["expected_failure"]
            .as_object_mut()
            .unwrap_or_else(|| panic!("failure"))
            .remove(field);
        assert!(matches!(
            verify_any(&json.to_string()),
            Err(Error::Schema(_))
        ));
    }
    let duplicate = text.replacen(
        "\"format\":",
        "\"format\": \"rustosrs-building-failure-v1\", \"format\":",
        1,
    );
    assert!(matches!(verify_any(&duplicate), Err(Error::Schema(_))));
    let mut b = decode(&text);
    b.inputs.candidate.seed = b.inputs.request.seed.wrapping_add(8);
    let invalid = changed(b, |_| {});
    assert!(matches!(verify_failure(&invalid), Err(Error::Input(_))));
}

#[test]
fn failure_publication_uses_the_same_bounded_native_export_and_dispatch_path() {
    let (mut state, document, source) = rejected_request();
    let frozen = state
        .freeze_reproduction(&document, 7, 9, &|_, _| None, source)
        .unwrap_or_else(|e| panic!("{e}"));
    let dir = crate::export_flow::test_support::temp("building-failure");
    let out = dir.join("failure.json");
    state
        .start_reproduction(frozen, out.clone())
        .unwrap_or_else(|e| panic!("{e}"));
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        if let Some(result) = state.poll_reproduction() {
            assert!(result.is_ok(), "{result:?}");
            break;
        }
        assert!(std::time::Instant::now() < deadline);
        std::thread::yield_now();
    }
    assert!(native::run_cli(&[out.display().to_string()]).is_ok());
    assert!(state.preview.is_none());
    assert_eq!(
        std::fs::read_dir(&dir)
            .unwrap_or_else(|e| panic!("{e}"))
            .count(),
        1
    );
    std::fs::remove_dir_all(dir).unwrap_or_else(|e| panic!("{e}"));
}

#[test]
fn successful_envelopes_keep_the_original_format_and_success_dispatch() {
    let (state, document, source) = fixture();
    let text = capture(&state, &document, source);
    assert!(matches!(verify_any(&text), Ok(Report::Success(_))));
    assert!(verify_failure(&text).is_err());
    assert_eq!(
        state.reproduction_seed(7, 9, &|_, _| None),
        Some(state.form.seed)
    );
}

#[test]
fn mismatched_failure_payload_cannot_publish_or_become_exportable() {
    let (mut state, document, _) = rejected_request();
    let input = Arc::clone(state.made_from.as_ref().unwrap_or_else(|| panic!("input")));
    let mut attempt = state.failures.remove(0);
    let mut obsolete = (*attempt.input).clone();
    obsolete.revision += 1;
    attempt.input = Arc::new(obsolete);
    let (job, send) = candidates::Job::controlled();
    state.pending = Some(job);
    send.send(candidates::Batch {
        input,
        previews: vec![],
        failures: vec![attempt],
    })
    .unwrap_or_else(|_| panic!("send"));
    state.refresh_source(&document, 7, 9, &|_, _| None);
    assert!(state.pending.is_none());
    assert!(state.failures.is_empty());
    assert_eq!(state.reproduction_seed(7, 9, &|_, _| None), None);
    assert!(state.error.is_some());
}

#[test]
fn mixed_batch_can_export_a_rejected_seed_without_replacing_the_selected_successful_preview() {
    let (mut state, mut document, source) = fixture();
    state.form.seed = state.form.seed.wrapping_add(1);
    state.refresh_source(&document, 7, 9, &|_, _| None);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while state.pending.is_some() {
        assert!(std::time::Instant::now() < deadline);
        state.refresh_source(&document, 7, 9, &|_, _| None);
        std::thread::yield_now();
    }
    // Deliberate source occupancy makes a mixed batch without relying on a generator defect.
    // Choose a decoration anchor used by some, but not all, of these deterministic candidates.
    let mut counts = std::collections::BTreeMap::new();
    for preview in &state.candidates {
        let positions: std::collections::BTreeSet<_> = preview
            .building
            .ops()
            .iter()
            .filter_map(|op| match op {
                MapOp::AddObject { placement } if (4..=8).contains(&placement.kind.get()) => {
                    Some(placement.position)
                }
                _ => None,
            })
            .collect();
        for position in positions {
            *counts.entry(position).or_insert(0) += 1;
        }
    }
    let position = counts
        .into_iter()
        .find_map(|(position, count)| {
            (count > 0 && count < state.candidates.len()).then_some(position)
        })
        .unwrap_or_else(|| {
            panic!(
                "fixture must vary optional decoration anchors across {} candidates",
                state.candidates.len()
            )
        });
    document
        .apply_patch(&MapPatch::new([MapOp::AddObject {
            placement: osrs_edit::ObjectPlacement {
                object_id: ObjectId::new(u32::MAX),
                position,
                kind: LocKind::new(4).unwrap_or_else(|| panic!("decor")),
                orientation: osrs_edit::ObjectOrientation::new(0)
                    .unwrap_or_else(|| panic!("orientation")),
            },
        }]))
        .unwrap_or_else(|e| panic!("{e:?}"));
    state.refresh_source(&document, 7, 10, &|_, _| None);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while state.pending.is_some() {
        assert!(std::time::Instant::now() < deadline);
        state.refresh_source(&document, 7, 10, &|_, _| None);
        std::thread::yield_now();
    }
    let selected = state
        .preview
        .as_ref()
        .unwrap_or_else(|| panic!("successful candidate"))
        .form
        .seed;
    let rejected = state
        .failures
        .first()
        .unwrap_or_else(|| panic!("occupied candidate"))
        .form
        .seed;
    let frozen = state
        .freeze_rejected_reproduction(&document, 7, 10, &|_, _| None, source.clone(), rejected)
        .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(
        verify_failure(&frozen.encode().unwrap_or_else(|e| panic!("{e}")))
            .unwrap_or_else(|e| panic!("{e}"))
            .seed,
        rejected
    );
    let success = state
        .freeze_reproduction(&document, 7, 10, &|_, _| None, source)
        .unwrap_or_else(|e| panic!("{e}"));
    assert!(matches!(
        verify_any(&success.encode().unwrap_or_else(|e| panic!("{e}"))),
        Ok(Report::Success(_))
    ));
    assert_eq!(state.reproduction_seed(7, 10, &|_, _| None), Some(selected));
}
