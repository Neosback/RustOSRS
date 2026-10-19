use super::*;
use osrs_core::{
    coords::{MapTile, SourcePlane},
    ids::ObjectId,
};
use osrs_edit::{
    HeightVertexCoord, LocKind, MapDocument, MapOp, MapPosition, ObjectOrientation, ObjectPlacement,
};
use std::collections::BTreeSet;

fn plane() -> SourcePlane {
    SourcePlane::new(0).unwrap_or_else(|| unreachable!("plane zero is valid"))
}

fn vertex(x: i32, y: i32) -> HeightVertexCoord {
    HeightVertexCoord::new(MapTile::new(x, y), plane())
}

fn object(
    object_id: u32,
    x: i32,
    y: i32,
    plane_value: u8,
    kind: u8,
    orientation: u8,
) -> ObjectPlacement {
    let source_plane =
        SourcePlane::new(plane_value).unwrap_or_else(|| unreachable!("test source plane is valid"));
    ObjectPlacement {
        object_id: ObjectId::new(object_id),
        position: MapPosition::new(MapTile::new(x, y), source_plane),
        kind: LocKind::new(kind).unwrap_or_else(|| unreachable!("test loc kind is valid")),
        orientation: ObjectOrientation::new(orientation)
            .unwrap_or_else(|| unreachable!("test orientation is valid")),
    }
}

fn height_patch(coord: HeightVertexCoord, before: i32, after: i32) -> MapPatch {
    MapPatch::new([MapOp::SetHeight {
        coord,
        before,
        after,
    }])
}

fn runtime_with_redo() -> (EditRuntime, HeightVertexCoord) {
    let prior = vertex(3200, 3200);
    let edited = vertex(3264, 3200);
    let mut runtime = EditRuntime::new(EditorSession::new(MapDocument::from_parts(
        [],
        [(prior, 0), (edited, 0)],
        [],
    )));
    runtime
        .session
        .commit(height_patch(prior, 0, 32))
        .unwrap_or_else(|error| unreachable!("valid setup commit: {error:?}"));
    runtime
        .session
        .undo()
        .unwrap_or_else(|error| unreachable!("valid setup undo: {error:?}"));
    assert!(runtime.can_redo());
    (runtime, edited)
}

#[test]
fn successful_execution_finalizes_once_and_returns_receipt() {
    let (mut runtime, edited) = runtime_with_redo();
    let revision_before = runtime.revision();
    let result = runtime
        .execute(height_patch(edited, 0, 64), |document| {
            assert_eq!(document.height_at(edited), Some(64));
            Ok::<_, &'static str>("projected")
        })
        .unwrap_or_else(|error| unreachable!("gateway execution succeeds: {error:?}"));

    let EditExecution::Committed {
        receipt,
        projection,
    } = result
    else {
        unreachable!("non-empty patch commits")
    };
    assert_eq!(projection, "projected");
    assert_eq!(receipt.revision, revision_before + 1);
    assert!(receipt.impact.contains(ChangeImpact::TERRAIN_GEOMETRY));
    assert_eq!(receipt.touched_regions, vec![RegionCoord::new(51, 50)]);
    assert_eq!(
        receipt.touched_regions.footprint.terrain_tiles,
        Vec::<MapTile>::new()
    );
    assert_eq!(
        receipt.touched_regions.footprint.height_vertices,
        vec![edited]
    );
    assert_eq!(
        receipt.touched_regions.footprint.object_tiles,
        Vec::<MapTile>::new()
    );
    assert!(receipt.touched_regions.footprint.object_removed.is_empty());
    assert!(receipt.touched_regions.footprint.object_added.is_empty());
    assert_eq!(runtime.document().height_at(edited), Some(64));
    assert!(runtime.can_undo());
    assert!(!runtime.can_redo());
}

#[test]
fn object_patch_footprint_preserves_exact_polarity_and_canonical_order() {
    let removed_a = object(300, 64, -1, 2, 10, 3);
    let removed_b = object(100, -64, 128, 0, 22, 1);
    let added_a = object(300, 65, -1, 2, 10, 3);
    let added_b = object(100, -63, 128, 0, 22, 1);
    let patch = MapPatch::new([
        MapOp::RemoveObject {
            placement: removed_a,
        },
        MapOp::AddObject { placement: added_a },
        MapOp::RemoveObject {
            placement: removed_b,
        },
        MapOp::AddObject { placement: added_b },
    ]);

    let footprint = EditFootprint::from_patch(&patch);

    assert_eq!(footprint.object_removed, vec![removed_b, removed_a]);
    assert_eq!(footprint.object_added, vec![added_b, added_a]);
    assert_eq!(
        footprint.object_tiles,
        vec![
            MapTile::new(-64, 128),
            MapTile::new(-63, 128),
            MapTile::new(64, -1),
            MapTile::new(65, -1),
        ]
    );
    assert_eq!(
        footprint.clone().into_touches().regions,
        vec![
            MapTile::new(-64, 128).split_region().0,
            MapTile::new(-63, 128).split_region().0,
            MapTile::new(64, -1).split_region().0,
            MapTile::new(65, -1).split_region().0,
        ]
        .into_iter()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>()
    );
}

#[test]
fn object_mutation_constructor_deduplicates_exact_placements_without_losing_polarity() {
    let removed = object(100, 3200, 3200, 0, 10, 1);
    let added = object(100, 3201, 3200, 0, 10, 1);
    let footprint =
        EditFootprint::with_object_mutations([], [], [removed, removed], [added, added]);

    assert_eq!(footprint.object_removed, vec![removed]);
    assert_eq!(footprint.object_added, vec![added]);
    assert_eq!(
        footprint.object_tiles,
        vec![MapTile::new(3200, 3200), MapTile::new(3201, 3200)]
    );
}

#[test]
fn no_op_execution_preserves_existing_redo_and_skips_projection() {
    let (mut runtime, _) = runtime_with_redo();
    let before = runtime.clone();
    let mut projected = false;
    let result = runtime
        .execute(MapPatch::default(), |_| {
            projected = true;
            Ok::<_, &'static str>(())
        })
        .unwrap_or_else(|error| unreachable!("empty patch is valid: {error:?}"));

    assert_eq!(result, EditExecution::NoChange);
    assert!(!projected);
    assert_eq!(runtime, before);
}

#[test]
fn semantic_rejection_skips_projection_and_preserves_session() {
    let coord = vertex(3200, 3200);
    let mut runtime = EditRuntime::new(EditorSession::new(MapDocument::from_parts(
        [],
        [(coord, 0)],
        [],
    )));
    let before = runtime.clone();
    let mut projected = false;
    let result = runtime.execute(height_patch(coord, 99, 64), |_| {
        projected = true;
        Ok::<_, &'static str>(())
    });

    assert!(matches!(result, Err(EditExecutionError::Semantic(_))));
    assert!(!projected);
    assert_eq!(runtime, before);
}

#[test]
fn projection_failure_restores_exact_pre_transaction_state() {
    let (mut runtime, edited) = runtime_with_redo();
    let before = runtime.clone();
    let result = runtime.execute(height_patch(edited, 0, 64), |document| {
        assert_eq!(document.height_at(edited), Some(64));
        Err::<(), _>("injected projection failure")
    });

    assert_eq!(
        result,
        Err(EditExecutionError::Projection(
            "injected projection failure"
        ))
    );
    assert_eq!(runtime, before);
}

#[test]
fn successful_undo_replay_finalizes_once_and_returns_receipt() {
    let coord = vertex(3200, 3200);
    let mut runtime = EditRuntime::new(EditorSession::new(MapDocument::from_parts(
        [],
        [(coord, 0)],
        [],
    )));
    runtime
        .session
        .commit(height_patch(coord, 0, 64))
        .unwrap_or_else(|error| unreachable!("valid setup commit: {error:?}"));
    let revision_before = runtime.revision();
    let footprint = EditFootprint::new([], [coord], []);

    let result = runtime
        .replay(EditReplayDirection::Undo, |document| {
            assert_eq!(document.height_at(coord), Some(0));
            Ok::<_, &'static str>("undo projected")
        })
        .unwrap_or_else(|error| unreachable!("gateway Undo succeeds: {error:?}"));

    let EditExecution::Committed {
        receipt,
        projection,
    } = result
    else {
        unreachable!("Undo history exists")
    };
    assert_eq!(projection, "undo projected");
    assert_eq!(receipt.revision, revision_before + 1);
    assert_eq!(receipt.touched_regions, vec![RegionCoord::new(50, 50)]);
    assert_eq!(receipt.touched_regions.footprint, footprint);
    assert_eq!(runtime.document().height_at(coord), Some(0));
    assert!(runtime.can_redo());
}

#[test]
fn failed_redo_replay_projection_restores_exact_pre_transaction_state() {
    let coord = vertex(3200, 3200);
    let mut runtime = EditRuntime::new(EditorSession::new(MapDocument::from_parts(
        [],
        [(coord, 0)],
        [],
    )));
    runtime
        .session
        .commit(height_patch(coord, 0, 64))
        .unwrap_or_else(|error| unreachable!("valid setup commit: {error:?}"));
    runtime
        .session
        .undo()
        .unwrap_or_else(|error| unreachable!("valid setup Undo: {error:?}"));
    let before = runtime.clone();

    let result = runtime.replay(EditReplayDirection::Redo, |document| {
        assert_eq!(document.height_at(coord), Some(64));
        Err::<(), _>("injected Redo projection failure")
    });

    assert_eq!(
        result,
        Err(EditExecutionError::Projection(
            "injected Redo projection failure"
        ))
    );
    assert_eq!(runtime, before);
}

#[test]
fn rollback_invariant_failure_is_explicit() {
    let first = vertex(3200, 3200);
    let second = vertex(3264, 3200);
    let mut runtime = EditRuntime::new(EditorSession::new(MapDocument::from_parts(
        [],
        [(first, 0), (second, 0)],
        [],
    )));
    let (commit, checkpoint) = runtime
        .session
        .commit_checkpointed(height_patch(first, 0, 32))
        .unwrap_or_else(|error| unreachable!("valid candidate commit: {error:?}"))
        .unwrap_or_else(|| unreachable!("candidate changes state"));
    runtime
        .session
        .commit(height_patch(second, 0, 64))
        .unwrap_or_else(|error| unreachable!("valid intervening commit: {error:?}"));

    let result = runtime.finish_execution(
        commit.revision,
        commit.impact,
        EditFootprint::new([], [first], []).into_touches(),
        checkpoint,
        Err::<(), _>("projection failed after intervening mutation"),
    );

    assert_eq!(
        result,
        Err(EditExecutionError::Rollback {
            projection: "projection failed after intervening mutation",
            rollback: EditCheckpointError::StaleSession,
        })
    );
}

#[test]
fn finalize_invariant_failure_is_explicit() {
    let first = vertex(3200, 3200);
    let second = vertex(3264, 3200);
    let mut runtime = EditRuntime::new(EditorSession::new(MapDocument::from_parts(
        [],
        [(first, 0), (second, 0)],
        [],
    )));
    let (commit, checkpoint) = runtime
        .session
        .commit_checkpointed(height_patch(first, 0, 32))
        .unwrap_or_else(|error| unreachable!("valid candidate commit: {error:?}"))
        .unwrap_or_else(|| unreachable!("candidate changes state"));
    runtime
        .session
        .commit(height_patch(second, 0, 64))
        .unwrap_or_else(|error| unreachable!("valid intervening commit: {error:?}"));

    let result = runtime.finish_execution(
        commit.revision,
        commit.impact,
        EditFootprint::new([], [first], []).into_touches(),
        checkpoint,
        Ok::<_, &'static str>(()),
    );

    assert_eq!(
        result,
        Err(EditExecutionError::Finalize(
            EditCheckpointError::StaleSession
        ))
    );
}

mod scenario {
    use super::*;
    use osrs_core::{
        coords::PlaneIndex,
        ids::NpcId,
        scenario::{ActorLocation, ScenarioDefinition},
    };
    use osrs_edit::{
        ScenarioPatch, SpawnDraft, plan_add_spawn, plan_replace_scenario, plan_update_spawn,
    };

    fn location(x: i32, y: i32) -> ActorLocation {
        ActorLocation {
            tile: MapTile::new(x, y),
            plane: PlaneIndex::new(0).unwrap_or_else(|| unreachable!("plane zero is valid")),
        }
    }

    fn definition() -> ScenarioDefinition {
        ScenarioDefinition {
            version: 1,
            build: 240,
            client_fingerprint: "c".repeat(64),
            seed: 1,
            spawns: Vec::new(),
        }
    }

    fn commit_scenario(runtime: &mut EditRuntime, patch: ScenarioPatch) -> bool {
        matches!(
            runtime.execute_scenario(patch, |_| Ok::<_, &'static str>(())),
            Ok(EditExecution::Committed { .. })
        )
    }

    fn runtime_with_scenario() -> EditRuntime {
        let mut runtime = EditRuntime::new(EditorSession::new(MapDocument::from_parts(
            [],
            [(vertex(3200, 3200), 0)],
            [],
        )));
        let patch = plan_replace_scenario(runtime.scenario(), Some(definition()))
            .unwrap_or_else(|e| unreachable!("valid scenario: {e}"));
        assert!(commit_scenario(&mut runtime, patch));
        runtime
    }

    fn add(runtime: &mut EditRuntime, x: i32) -> osrs_core::scenario::NpcSpawnId {
        let (patch, id) = plan_add_spawn(
            runtime.scenario(),
            SpawnDraft::new(NpcId::new(3), "npc.hans", location(x, 3200)),
        )
        .unwrap_or_else(|e| unreachable!("valid spawn: {e}"));
        assert!(commit_scenario(runtime, patch));
        id
    }

    #[test]
    fn map_and_scenario_edits_undo_and_redo_in_chronological_order() {
        let mut runtime = runtime_with_scenario();
        let coord = vertex(3200, 3200);
        runtime
            .execute(height_patch(coord, 0, 64), |_| Ok::<_, &'static str>(()))
            .unwrap_or_else(|e| unreachable!("map edit: {e:?}"));
        let id = add(&mut runtime, 3201);
        let map_revision = runtime.map_revision();
        assert_eq!(map_revision, 1, "only the height edit is a map change");

        let undo = |runtime: &mut EditRuntime| {
            runtime
                .replay(EditReplayDirection::Undo, |_| Ok::<_, &'static str>(()))
                .unwrap_or_else(|e| unreachable!("undo: {e:?}"))
        };
        let redo = |runtime: &mut EditRuntime| {
            runtime
                .replay(EditReplayDirection::Redo, |_| Ok::<_, &'static str>(()))
                .unwrap_or_else(|e| unreachable!("redo: {e:?}"))
        };

        // Newest first: the spawn, then the height, then the scenario itself.
        undo(&mut runtime);
        assert!(runtime.scenario().spawn(id).is_none());
        assert_eq!(runtime.document().height_at(coord), Some(64));
        assert_eq!(
            runtime.map_revision(),
            map_revision,
            "scenario undo is not a map change"
        );
        undo(&mut runtime);
        assert_eq!(runtime.document().height_at(coord), Some(0));
        assert_eq!(
            runtime.map_revision(),
            map_revision + 1,
            "height undo is a map change"
        );
        undo(&mut runtime);
        assert!(runtime.scenario().definition().is_none());

        redo(&mut runtime);
        redo(&mut runtime);
        redo(&mut runtime);
        assert_eq!(runtime.document().height_at(coord), Some(64));
        assert!(runtime.scenario().spawn(id).is_some());
    }

    #[test]
    fn empty_scenario_gesture_keeps_redo_and_skips_projection() {
        let mut runtime = runtime_with_scenario();
        let id = add(&mut runtime, 3201);
        runtime
            .replay(EditReplayDirection::Undo, |_| Ok::<_, &'static str>(()))
            .unwrap_or_else(|e| unreachable!("undo: {e:?}"));
        assert!(runtime.can_redo());
        let before = runtime.clone();
        // The same definition replaced by itself changes nothing.
        let unchanged = ScenarioPatch::Replace {
            before: runtime.scenario().definition().cloned(),
            after: runtime.scenario().definition().cloned(),
        };
        let mut projected = false;
        let result = runtime.execute_scenario(unchanged, |_| {
            projected = true;
            Ok::<_, &'static str>(())
        });
        assert_eq!(result, Ok(EditExecution::NoChange));
        assert!(!projected);
        assert!(runtime.can_redo());
        assert_eq!(runtime, before);
        let _ = id;
    }

    #[test]
    fn scenario_projection_failure_restores_exact_state_and_redo() {
        let mut runtime = runtime_with_scenario();
        let id = add(&mut runtime, 3201);
        runtime
            .replay(EditReplayDirection::Undo, |_| Ok::<_, &'static str>(()))
            .unwrap_or_else(|e| unreachable!("undo: {e:?}"));
        let id = {
            // Re-add under a fresh redo-free history: failure must restore *this* state.
            let _ = id;
            add(&mut runtime, 3202)
        };
        let before = runtime.clone();
        let patch = plan_update_spawn(runtime.scenario(), id, |s| s.facing = 512)
            .unwrap_or_else(|e| unreachable!("valid update: {e}"));
        let result = runtime.execute_scenario(patch, |_| Err::<(), _>("projection failed"));
        assert_eq!(
            result,
            Err(EditExecutionError::Projection("projection failed"))
        );
        assert_eq!(runtime, before);
        assert_eq!(runtime.scenario().spawn(id).map(|s| s.facing), Some(0));
    }

    #[test]
    fn stale_scenario_patch_is_rejected_before_projection() {
        let mut runtime = runtime_with_scenario();
        let id = add(&mut runtime, 3201);
        let stale = plan_update_spawn(runtime.scenario(), id, |s| s.facing = 256)
            .unwrap_or_else(|e| unreachable!("valid update: {e}"));
        // A later committed change makes the planned patch stale.
        let newer = plan_update_spawn(runtime.scenario(), id, |s| s.facing = 512)
            .unwrap_or_else(|e| unreachable!("valid update: {e}"));
        assert!(commit_scenario(&mut runtime, newer));
        let before = runtime.clone();
        let mut projected = false;
        let result = runtime.execute_scenario(stale, |_| {
            projected = true;
            Ok::<_, &'static str>(())
        });
        assert!(matches!(result, Err(EditExecutionError::Semantic(_))));
        assert!(!projected);
        assert_eq!(runtime, before);
    }
}

#[test]
fn scene_baselines_are_untracked_and_failed_projection_restores_every_state() {
    let c = vertex(3200, 3200);
    let mut runtime = EditRuntime::new(EditorSession::new(MapDocument::from_parts(
        [],
        [(c, 0)],
        [],
    )));
    let before = runtime.clone();
    let result = runtime.execute_baseline(height_patch(c, 0, 32), |_| Err::<(), _>("unavailable"));
    assert!(matches!(
        result,
        Err(EditExecutionError::Projection("unavailable"))
    ));
    assert_eq!(runtime, before);
    runtime
        .execute_baseline(height_patch(c, 0, 32), |_| Ok::<_, ()>(()))
        .unwrap_or_else(|e| panic!("{e:?}"));
    assert_eq!(runtime.document().height_at(c), Some(32));
    assert!(!runtime.can_undo());
    runtime
        .commit(height_patch(c, 32, 64))
        .unwrap_or_else(|e| panic!("{e:?}"));
    runtime.undo().unwrap_or_else(|e| panic!("{e:?}"));
    assert_eq!(runtime.document().height_at(c), Some(32));
}
