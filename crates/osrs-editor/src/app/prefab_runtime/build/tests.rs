//! Preview admission uses real semantic plans; controlled completion channels fix event ordering.
use super::*;

fn fixture() -> (BuildState, MapDocument) {
    let plane = SourcePlane::new(0).unwrap_or_else(|| panic!("plane"));
    let heights = (-1..=11)
        .flat_map(|y| {
            (-1..=15).map(move |x| {
                (
                    osrs_edit::HeightVertexCoord::new(MapTile::new(3300 + x, 3300 + y), plane),
                    -64,
                )
            })
        })
        .collect::<Vec<_>>();
    let document = MapDocument::from_parts([], heights, []);
    let mut state = BuildState::default();
    state.form.width = 14;
    state.form.height = 10;
    state.centre_site_on((3307, 3305), 0);
    state.refresh_for_script(&document, 7, 9, &|_, _| Some((1, 1)));
    assert!(state.preview.is_some(), "{:?}", state.error);
    (state, document)
}

#[test]
fn catalog_arrival_and_size_changes_revoke_every_preview_admission_without_a_new_revision() {
    let (mut state, document) = fixture();
    state.refresh_for_script(&document, 7, 9, &|_, _| None);
    assert!(
        state.preview.is_some(),
        "portable kit hints: {:?}",
        state.error
    );
    for source in [Some((1, 1)), Some((2, 3)), None] {
        let provider = |_, _| source;
        if source.is_none() {
            state.refresh_for_script(&document, 7, 9, &|_, _| Some((1, 1)));
        }
        assert!(state.current_preview(7, 9, &provider).is_none());
        assert!(state.candidates(7, 9, &provider).is_empty());
        assert!(state.to_prefab(7, 9, &provider).is_err());
        state.select_candidate(0, 7, 9, &provider);
        assert!(state.current_preview(7, 9, &provider).is_none());
    }
}

#[test]
fn source_dimensions_override_kit_label_placeholders_during_candidate_validation() {
    let (mut state, document) = fixture();
    let preview = state.preview.as_ref().unwrap_or_else(|| panic!("preview"));
    let labels = osrs_lab::kit_objects(&state.kits[state.form.kit].file);
    let id = preview
        .thumbnail
        .objects
        .iter()
        .find(|p| p.kind == 10 && labels.contains_key(&ObjectId::new(p.id)))
        .unwrap_or_else(|| panic!("fixture must contain kit furniture"))
        .id;
    state.refresh_for_script(&document, 7, 9, &|object, _| {
        Some(if object.get() == id {
            (255, 255)
        } else {
            (1, 1)
        })
    });
    assert!(
        state.preview.is_none(),
        "source-sized furniture cannot fit the site"
    );
    assert!(state.error.is_some());
}

#[test]
fn unresolved_required_scenery_blocks_planning_but_far_away_unknowns_do_not() {
    let (mut state, document) = fixture();
    let object = |x, y| osrs_edit::ObjectPlacement {
        object_id: ObjectId::new(999_999),
        position: osrs_edit::MapPosition::new(
            MapTile::new(x, y),
            SourcePlane::new(0).unwrap_or_else(|| panic!("plane")),
        ),
        kind: LocKind::new(11).unwrap_or_else(|| panic!("kind")),
        orientation: osrs_edit::ObjectOrientation::new(1).unwrap_or_else(|| panic!("orientation")),
    };
    let mut near = document.clone();
    near.apply_patch(&MapPatch::new([MapOp::AddObject {
        placement: object(3299, 3300),
    }]))
    .unwrap_or_else(|e| panic!("{e:?}"));
    state.refresh_for_script(&near, 7, 9, &|_, _| None);
    assert!(state.preview.is_none());
    assert!(state.error.as_ref().is_some_and(|e| e.contains("999999")));
    state.refresh_for_script(&near, 7, 9, &|id, _| {
        (id.get() == 999_999).then_some((1, 2))
    });
    let preview = state
        .preview
        .as_ref()
        .unwrap_or_else(|| panic!("known footprint: {:?}", state.error));
    assert!(
        preview.apply.ops().contains(&MapOp::RemoveObject {
            placement: object(3299, 3300)
        }),
        "rotated outside anchor reaches the site"
    );
    let before = near.clone();
    near.apply_patch(&preview.apply)
        .unwrap_or_else(|e| panic!("{e:?}"));
    near.apply_patch(&preview.apply.inverse())
        .unwrap_or_else(|e| panic!("{e:?}"));
    assert_eq!(
        near, before,
        "clearing the overlapping occurrence undoes exactly"
    );
    let mut far = document;
    far.apply_patch(&MapPatch::new([MapOp::AddObject {
        placement: object(3000, 3000),
    }]))
    .unwrap_or_else(|e| panic!("{e:?}"));
    state.refresh_for_script(&far, 7, 9, &|_, _| None);
    assert!(
        state.preview.is_some(),
        "irrelevant unknown: {:?}",
        state.error
    );
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn catalog_arrival_supersedes_an_inflight_batch_even_at_the_same_map_revision() {
    let (mut state, document) = fixture();
    state.refresh_for_script(&document, 7, 9, &|_, _| None);
    let old = state.preview.take().unwrap_or_else(|| panic!("preview"));
    let old_input = Arc::downgrade(&old.input);
    state.candidates.clear();
    let (job, complete) = candidates::Job::controlled();
    state.pending = Some(job);
    complete
        .send(candidates::Batch {
            input: Arc::clone(&old.input),
            previews: vec![old],
            failures: vec![],
        })
        .unwrap_or_else(|e| panic!("{e}"));
    state.refresh_source(&document, 7, 9, &|_, _| Some((1, 1)));
    assert!(state.preview.is_none());
    assert!(state.pending.is_some());
    assert!(
        old_input.upgrade().is_none(),
        "obsolete batch and metadata must be released"
    );
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while state.pending.is_some() {
        assert!(std::time::Instant::now() < deadline, "worker stalled");
        std::thread::sleep(std::time::Duration::from_millis(2));
        state.refresh_source(&document, 7, 9, &|_, _| Some((1, 1)));
    }
    assert!(
        state.current_preview(7, 9, &|_, _| Some((1, 1))).is_some(),
        "{:?}",
        state.error
    );
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn invalid_current_inputs_cannot_publish_an_older_building_completion() {
    for change in ["kit", "plane"] {
        let (mut state, document) = fixture();
        let old = state.preview.take().unwrap_or_else(|| panic!("preview"));
        state.candidates.clear();
        let (job, complete) = candidates::Job::controlled();
        state.pending = Some(job);
        match change {
            "kit" => state.form.kit = usize::MAX,
            "plane" => state.site_plane = 4,
            _ => unreachable!(),
        }
        state.refresh_source(&document, 7, 9, &|_, _| Some((1, 1)));
        let _ = complete.send(candidates::Batch {
            input: Arc::clone(&old.input),
            previews: vec![old],
            failures: vec![],
        });
        state.refresh_source(&document, 7, 9, &|_, _| Some((1, 1)));
        assert!(
            state.preview.is_none(),
            "stale {change} candidate must not become applicable"
        );
        assert!(state.candidates.is_empty());
        assert!(state.pending.is_none());
        assert!(state.error.is_some());
    }
}

#[test]
fn synchronous_invalid_inputs_clear_previous_building_candidates() {
    for change in ["kit", "plane", "site"] {
        let (mut state, document) = fixture();
        match change {
            "kit" => state.form.kit = usize::MAX,
            "plane" => state.site_plane = 4,
            "site" => state.site = None,
            _ => unreachable!(),
        }
        state.refresh_for_script(&document, 8, 10, &|_, _| Some((1, 1)));
        assert!(
            state.preview.is_none(),
            "stale synchronous {change} candidate"
        );
        assert!(state.candidates.is_empty());
        assert!(state.to_prefab(8, 10, &|_, _| Some((1, 1))).is_err());
    }
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn a_stopped_building_worker_reports_failure_and_retires_its_pending_slot() {
    let (mut state, document) = fixture();
    state.preview = None;
    state.candidates.clear();
    let (job, complete) = candidates::Job::controlled();
    state.pending = Some(job);
    drop(complete);
    state.refresh_source(&document, 7, 9, &|_, _| Some((1, 1)));
    assert!(state.pending.is_none(), "disconnection is terminal");
    assert!(state.error.is_some());
    assert!(state.preview.is_none());
    state.refresh_source(&document, 7, 9, &|_, _| Some((1, 1)));
    assert!(
        state.pending.is_none(),
        "a failure must not cause a per-frame retry loop"
    );
}

#[test]
fn apply_save_selection_and_drawing_reads_refuse_changed_preview_inputs() {
    for change in ["source", "revision", "form", "site", "plane", "kit"] {
        let (mut state, _document) = fixture();
        let (mut source, mut revision) = (7, 9);
        match change {
            "source" => source += 1,
            "revision" => revision += 1,
            "form" => state.form.width += 1,
            "site" => state.site = Some(MapTile::new(3400, 3400)),
            "plane" => state.site_plane = 1,
            "kit" => state.form.kit = 1,
            _ => unreachable!(),
        }
        assert!(
            state
                .current_preview(source, revision, &|_, _| Some((1, 1)))
                .is_none(),
            "{change}"
        );
        assert!(
            state
                .candidates(source, revision, &|_, _| Some((1, 1)))
                .is_empty(),
            "{change}"
        );
        assert!(
            state
                .to_prefab(source, revision, &|_, _| Some((1, 1)))
                .is_err(),
            "{change}"
        );
        state.select_candidate(0, source, revision, &|_, _| Some((1, 1)));
        assert!(
            state
                .current_preview(source, revision, &|_, _| Some((1, 1)))
                .is_none()
        );
        assert!(
            state.last_preview().is_some(),
            "read-only verification may inspect an old committed plan"
        );
    }
}

#[test]
fn hydration_invalidates_the_preview_without_an_authored_map_revision_or_history_entry() {
    use crate::app::edit_runtime::EditRuntime;
    use osrs_core::coords::RegionCoord;
    use osrs_edit::MapRegionSnapshot;
    let (mut state, document) = fixture();
    let region = RegionCoord::new(51, 51);
    let heights: Vec<_> = document.height_entries().collect();
    let snapshot = MapRegionSnapshot::from_parts(region, [], heights.clone(), [])
        .unwrap_or_else(|error| panic!("{error:?}"));
    let mut session = EditRuntime::default();
    session.ingest_region(&snapshot);
    let map_revision = session.map_revision();
    let original_revision = session.revision();
    state.refresh_for_script(session.document(), 7, session.revision(), &|_, _| {
        Some((1, 1))
    });
    assert!(
        state
            .current_preview(7, original_revision, &|_, _| Some((1, 1)))
            .is_some()
    );
    let replacement = MapRegionSnapshot::from_parts(
        region,
        [],
        heights.into_iter().map(|(coord, _)| (coord, -80)),
        [],
    )
    .unwrap_or_else(|error| panic!("{error:?}"));
    session.ingest_region(&replacement);
    assert_eq!(session.map_revision(), map_revision);
    assert!(session.revision() > original_revision);
    assert!(!session.can_undo());
    assert!(
        state
            .current_preview(7, session.revision(), &|_, _| Some((1, 1)))
            .is_none()
    );
    state.refresh_for_script(session.document(), 7, session.revision(), &|_, _| {
        Some((1, 1))
    });
    assert!(
        state
            .current_preview(7, session.revision(), &|_, _| Some((1, 1)))
            .is_some(),
        "{:?}",
        state.error
    );
    assert!(!session.can_undo());
}

#[test]
fn a_candidate_retains_the_original_source_revision_site_and_form() {
    let (state, _) = fixture();
    let preview = state
        .current_preview(7, 9, &|_, _| Some((1, 1)))
        .unwrap_or_else(|| panic!("preview"));
    assert_eq!(preview.input.source, 7);
    assert_eq!(preview.input.revision, 9);
    assert_eq!(preview.input.origin, preview.origin);
    assert_eq!(preview.input.plane, preview.base_plane);
    assert_eq!(preview.input.form, state.form);
    assert!(Arc::ptr_eq(
        &preview.input,
        state
            .made_from
            .as_ref()
            .unwrap_or_else(|| panic!("issued input"))
    ));
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn mismatched_batch_or_candidate_inputs_cannot_become_current() {
    for change in ["batch", "candidate"] {
        let (mut state, document) = fixture();
        let mut preview = state.preview.take().unwrap_or_else(|| panic!("preview"));
        state.candidates.clear();
        let current = Arc::clone(&preview.input);
        let valid = preview.clone();
        let mut foreign = (*current).clone();
        foreign.source += 1;
        let foreign = Arc::new(foreign);
        preview.input = Arc::clone(&foreign);
        let (job, complete) = candidates::Job::controlled();
        state.pending = Some(job);
        complete
            .send(candidates::Batch {
                input: if change == "batch" {
                    foreign
                } else {
                    Arc::clone(&current)
                },
                previews: vec![preview],
                failures: vec![],
            })
            .unwrap_or_else(|error| panic!("{error:?}"));
        state.refresh_source(&document, 7, 9, &|_, _| Some((1, 1)));
        assert!(
            state.current_preview(7, 9, &|_, _| Some((1, 1))).is_none(),
            "{change}"
        );
        assert!(state.candidates(7, 9, &|_, _| Some((1, 1))).is_empty());
        if change == "batch" {
            assert!(
                state.pending.is_some(),
                "stale batch cannot retire a current job"
            );
            complete
                .send(candidates::Batch {
                    input: current,
                    previews: vec![valid],
                    failures: vec![],
                })
                .unwrap_or_else(|error| panic!("{error:?}"));
            state.refresh_source(&document, 7, 9, &|_, _| Some((1, 1)));
            assert!(state.current_preview(7, 9, &|_, _| Some((1, 1))).is_some());
            assert!(state.pending.is_none());
        } else {
            assert!(state.error.is_some());
            assert!(state.pending.is_none());
        }
    }
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn the_real_candidate_worker_shares_one_current_input_and_releases_superseded_metadata() {
    let (mut state, document) = fixture();
    let old = Arc::downgrade(
        &state
            .current_preview(7, 9, &|_, _| Some((1, 1)))
            .unwrap_or_else(|| panic!("preview"))
            .input,
    );
    state.refresh_source(&document, 8, 10, &|_, _| Some((1, 1)));
    assert!(state.current_preview(8, 10, &|_, _| Some((1, 1))).is_none());
    assert!(old.upgrade().is_none());
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while state.pending.is_some() {
        assert!(
            std::time::Instant::now() < deadline,
            "candidate worker stalled"
        );
        std::thread::sleep(std::time::Duration::from_millis(2));
        state.refresh_source(&document, 8, 10, &|_, _| Some((1, 1)));
    }
    let input = state.made_from.as_ref().unwrap_or_else(|| panic!("input"));
    assert!(
        state.current_preview(8, 10, &|_, _| Some((1, 1))).is_some(),
        "{:?}",
        state.error
    );
    assert!(!state.candidates(8, 10, &|_, _| Some((1, 1))).is_empty());
    for candidate in state.candidates(8, 10, &|_, _| Some((1, 1))) {
        assert!(Arc::ptr_eq(&candidate.input, input));
        assert_eq!(candidate.input.source, 8);
        assert_eq!(candidate.input.revision, 10);
        assert!((0..8).contains(&candidate.form.seed.wrapping_sub(input.form.seed)));
    }
    state.select_candidate(0, 8, 10, &|_, _| Some((1, 1)));
    assert!(state.to_prefab(8, 10, &|_, _| Some((1, 1))).is_ok());
}
