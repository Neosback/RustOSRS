use super::*;
use osrs_edit::{
    PrefabCell, PrefabHeight, PrefabObject, PrefabOverlay, PrefabTile,
    height::{Falloff, HeightOp},
};

fn task() -> LabTask {
    let initial = Snapshot {
        heights: (0..=4)
            .flat_map(|x| {
                (0..=4).map(move |y| PrefabHeight {
                    x,
                    y,
                    plane: 0,
                    value: 0,
                })
            })
            .collect(),
        ..Snapshot::default()
    };
    let mut target = initial.clone();
    target.objects.push(object());
    LabTask {
        locks: vec![],
        object_names: Default::default(),
        format: "rustosrs-lab-task".into(),
        version: 1,
        name: "fixture".into(),
        source: "unit".into(),
        group: "fixture-family".into(),
        width: 4,
        height: 4,
        planes: 1,
        budget: 8,
        initial,
        target,
        palette: vec![PaletteEntry {
            id: 10,
            kind: 0,
            size: [1, 1],
        }],
        overlays: vec![],
        underlays: vec![],
        dont_care_tiles: vec![],
        dont_care_heights: vec![],
    }
}

fn object() -> PrefabObject {
    PrefabObject {
        id: 10,
        x: 1,
        y: 1,
        plane: 0,
        kind: 0,
        orientation: 0,
    }
}

#[test]
fn tool_actions_undo_redo_and_replay_restore_exact_state() {
    let task = task();
    let mut env = Env::reset(task.clone(), 42).unwrap_or_else(|e| panic!("{e}"));
    let initial = env.document().clone();
    let result = env
        .step(Action::Place { object: object() })
        .unwrap_or_else(|e| panic!("{e}"));
    assert!(result.exact);
    assert!(result.reward > 0.0);
    env.step(Action::Undo).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(*env.document(), initial);
    env.step(Action::Redo).unwrap_or_else(|e| panic!("{e}"));
    assert!(env.score().exact);
    let replay = Env::replay(task, 42, env.trace()).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(replay.document(), env.document());
}

#[test]
fn score_only_observations_never_serialize_target() {
    let env = Env::reset(task(), 1).unwrap_or_else(|e| panic!("{e}"));
    let hidden = serde_json::to_value(env.observation(ObservationMode::ScoreOnly))
        .unwrap_or_else(|e| panic!("{e}"));
    assert!(hidden.get("target").is_none());
    assert!(hidden.get("current").is_some());
    assert!(env.observation(ObservationMode::Visible).target.is_some());
}

#[test]
fn invalid_actions_preserve_document_and_raw_target_writes_are_unrepresentable() {
    let mut env = Env::reset(task(), 1).unwrap_or_else(|e| panic!("{e}"));
    let initial = env.document().clone();
    let mut p = object();
    p.id = 999;
    assert!(env.step(Action::Place { object: p }).is_err());
    assert_eq!(*env.document(), initial);
    assert!(serde_json::from_str::<Action>(r#"{"tool":"replace_region","target":{}}"#).is_err());
}

#[test]
fn failures_do_not_destroy_redo_and_budget_is_bounded() {
    let mut env = Env::reset(task(), 1).unwrap_or_else(|e| panic!("{e}"));
    env.step(Action::Place { object: object() })
        .unwrap_or_else(|e| panic!("{e}"));
    env.step(Action::Undo).unwrap_or_else(|e| panic!("{e}"));
    let mut bad = object();
    bad.x = 99;
    assert!(
        env.step(Action::Place {
            object: bad.clone()
        })
        .is_err()
    );
    assert!(env.legal_mask().can_redo);
    for _ in 0..5 {
        let _ = env.step(Action::Place {
            object: bad.clone(),
        });
    }
    assert_eq!(env.step(Action::Redo), Err(LabError::BudgetExhausted));
}

#[test]
fn failed_attempts_replay_with_identical_budget_and_state() {
    let t = task();
    let mut env = Env::reset(t.clone(), 7).unwrap_or_else(|e| panic!("{e}"));
    let mut invalid = object();
    invalid.x = 99;
    assert!(env.step(Action::Place { object: invalid }).is_err());
    env.step(Action::Place { object: object() })
        .unwrap_or_else(|e| panic!("{e}"));
    let replay = Env::replay(t, 7, env.trace()).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(replay.document(), env.document());
    assert_eq!(replay.legal_mask().remaining, env.legal_mask().remaining);
}

#[test]
fn task_height_coverage_must_match_and_nearer_occurrences_earn_partial_credit() {
    let mut bad = task();
    bad.target.heights.pop();
    assert!(Env::reset(bad, 1).is_err());
    let mut env = Env::reset(task(), 1).unwrap_or_else(|e| panic!("{e}"));
    let mut far = object();
    far.x = 3;
    env.step(Action::Place {
        object: far.clone(),
    })
    .unwrap_or_else(|e| panic!("{e}"));
    let initial = env.score();
    assert!(initial.placement > 0.0);
    assert_eq!(initial.objects, 0.0);
    env.step(Action::Move {
        object: far,
        dx: -1,
        dy: 0,
    })
    .unwrap_or_else(|e| panic!("{e}"));
    assert!(env.score().placement > initial.placement);
}

#[test]
fn mult_tile_scene_capacity_checks_every_covered_tile() {
    let mut t = task();
    t.palette = vec![PaletteEntry {
        id: 10,
        kind: 10,
        size: [2, 1],
    }];
    t.target.objects.clear();
    let mut env = Env::reset(t, 1).unwrap_or_else(|e| panic!("{e}"));
    for orientation in 0..4 {
        let mut o = object();
        o.kind = 10;
        o.orientation = orientation;
        env.step(Action::Place { object: o })
            .unwrap_or_else(|e| panic!("{e}"));
    }
    // Different anchors reach the same occupied footprint tile; the sixth object is rejected.
    let mut o = object();
    o.kind = 10;
    o.x = 0;
    env.step(Action::Place { object: o.clone() })
        .unwrap_or_else(|e| panic!("{e}"));
    o.x = 1;
    o.y = 0;
    o.orientation = 1;
    let before = env.document().clone();
    assert!(env.step(Action::Place { object: o }).is_err());
    assert_eq!(env.document(), &before);
}

#[test]
fn path_lay_reconstructs_generated_paths_task_with_exact_match_and_reversible_undo() {
    let path_task = generated_paths(1).unwrap_or_else(|e| panic!("{e}"));
    let mut env = Env::reset(path_task, 1).unwrap_or_else(|e| panic!("{e}"));
    let initial_doc = env.document().clone();
    assert!(!env.score().exact);

    let step = env
        .step(Action::PathLay {
            plane: 0,
            points: vec![[2.5, 4.5], [9.5, 9.5], [17.5, 14.5]],
            width: 3.0,
            edge_weight: 1.5,
            smooth_curve: true,
            overlay_id: 4,
            companion_underlay: Some(10),
            smooth_heights: Some(PathSmoothSpec {
                passes: 2,
                blend: 1.5,
            }),
        })
        .unwrap_or_else(|e| panic!("{e}"));
    assert!(step.exact);
    assert!((step.score - 1.0).abs() < 1e-9);
    assert!((step.breakdown.terrain - 1.0).abs() < 1e-9);
    assert!((step.breakdown.height - 1.0).abs() < 1e-9);

    env.step(Action::Undo).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(*env.document(), initial_doc);
}

#[test]
fn height_brush_room_rect_corners_hip_roof_and_partial_terrain_credit_work() {
    let mut t = task();
    t.overlays = vec![4];
    t.underlays = vec![2, 10];
    t.palette = vec![
        PaletteEntry {
            id: 10,
            kind: 0,
            size: [1, 1],
        },
        PaletteEntry {
            id: 11,
            kind: 2,
            size: [1, 1],
        },
        PaletteEntry {
            id: 20,
            kind: 12,
            size: [1, 1],
        },
        PaletteEntry {
            id: 21,
            kind: 13,
            size: [1, 1],
        },
    ];
    t.target.tiles = vec![PrefabTile {
        x: 0,
        y: 0,
        plane: 0,
        underlay: Some(10),
        overlay: Some(PrefabOverlay {
            id: 4,
            shape: 2,
            rotation: 1,
        }),
        flags: 0,
    }];
    t.dont_care_tiles = vec![PrefabCell {
        x: 3,
        y: 3,
        plane: 0,
    }];
    let mut env = Env::reset(t, 1).unwrap_or_else(|e| panic!("{e}"));
    let baseline = env.score().terrain;

    // Painting matching underlay + overlay ID with wrong shape earns partial terrain credit.
    let res = env
        .step(Action::PaintRect {
            x: 0,
            y: 0,
            width: 1,
            height: 1,
            plane: 0,
            overlay: Some(4),
            underlay: Some(10),
            shape: 0,
            rotation: 0,
            flags: Some(0),
        })
        .unwrap_or_else(|e| panic!("{e}"));
    assert!(res.breakdown.terrain > baseline);
    assert!(res.breakdown.terrain < 1.0);

    // Painting inside a dont_care_tiles cell does not reduce terrain score.
    let before_masked = env.score().terrain;
    env.step(Action::PaintRect {
        x: 3,
        y: 3,
        width: 1,
        height: 1,
        plane: 0,
        overlay: Some(4),
        underlay: Some(2),
        shape: 0,
        rotation: 0,
        flags: Some(0),
    })
    .unwrap_or_else(|e| panic!("{e}"));
    assert!((env.score().terrain - before_masked).abs() < 1e-9);

    // RoomRect with corner_id places walls and 4 corners; HipRoof places gable roof; HeightBrush sculpts.
    env.step(Action::RoomRect {
        object: PrefabObject {
            id: 10,
            x: 0,
            y: 0,
            plane: 0,
            kind: 0,
            orientation: 0,
        },
        width: 3,
        height: 3,
        corner_id: Some(11),
    })
    .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(
        env.document()
            .object_entries()
            .iter()
            .filter(|o| o.kind.get() == 2)
            .count(),
        4
    );

    env.step(Action::HipRoof {
        plane: 0,
        x0: 0,
        y0: 0,
        x1: 2,
        y1: 2,
        slope_id: 20,
        edge_id: None,
        ridge_id: Some(21),
    })
    .unwrap_or_else(|e| panic!("{e}"));
    assert!(
        env.document()
            .object_entries()
            .iter()
            .any(|o| o.kind.get() == 13 && o.object_id.get() == 21)
    );

    env.step(Action::HeightBrush {
        plane: 0,
        points: vec![[2, 2]],
        radius: 1,
        circle: false,
        op: HeightOp::Raise,
        strength: 24,
        falloff: Falloff::Flat,
        flatten_to: None,
        seed: 1,
    })
    .unwrap_or_else(|e| panic!("{e}"));
}

#[test]
fn policy_validation_is_target_free_and_preserves_budget_history_and_author_locks() {
    let mut task = task();
    let object = task.target.objects[0].clone();
    task.initial.objects.push(object.clone());
    task.locks.push(object.clone());
    let mut env = Env::reset(task, 7).unwrap_or_else(|e| panic!("{e}"));
    let before = env.document().clone();
    let remaining = env.legal_mask().remaining;
    assert!(
        env.validate_action(&Action::Remove {
            object: object.clone()
        })
        .is_err()
    );
    assert_eq!(env.document(), &before);
    assert_eq!(env.legal_mask().remaining, remaining);
    assert!(env.trace().is_empty());
    assert!(env.step(Action::Remove { object }).is_err());
    assert_eq!(env.document(), &before);
    let json = serde_json::to_value(env.observation(ObservationMode::ScoreOnly))
        .unwrap_or_else(|e| panic!("{e}"));
    assert!(json.get("target").is_none());
    assert!(
        json["legal"]["locks"]
            .as_array()
            .is_some_and(|a| a.len() == 1)
    );
}
