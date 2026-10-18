use super::*;
use osrs_edit::{PrefabHeight, PrefabObject};
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
