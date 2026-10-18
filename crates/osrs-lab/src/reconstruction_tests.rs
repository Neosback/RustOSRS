//! Saved-building task boundaries and real tool reconstruction; no rendering/cache prerequisite.
use super::*;
use osrs_edit::{Prefab, PrefabHeight, PrefabObject, PrefabSource, PrefabTile};
fn prefab() -> Prefab {
    Prefab {
        format: osrs_edit::PREFAB_FORMAT.into(),
        version: osrs_edit::PREFAB_VERSION,
        name: "copy fixture".into(),
        tags: vec![],
        notes: String::new(),
        source: PrefabSource::Captured {
            tile_x: 3225,
            tile_y: 3235,
            plane: 0,
        },
        width: 4,
        height: 4,
        planes: 2,
        tiles: vec![PrefabTile {
            x: 0,
            y: 0,
            plane: 0,
            underlay: Some(3),
            overlay: None,
            flags: 0,
        }],
        heights: vec![PrefabHeight {
            x: 0,
            y: 0,
            plane: 0,
            value: -80,
        }],
        objects: vec![
            PrefabObject {
                id: 10,
                x: 0,
                y: 0,
                plane: 0,
                kind: 10,
                orientation: 0,
            },
            PrefabObject {
                id: 10,
                x: 2,
                y: 1,
                plane: 0,
                kind: 10,
                orientation: 1,
            },
        ],
        building: None,
    }
}
#[test]
fn reconstruction_preserves_sparse_tiles_occurrences_and_geographic_family() {
    let prefab = prefab();
    let task = reconstruction_task(&prefab, &|_, _| Some((2, 1))).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(task.target.tiles, prefab.tiles);
    assert_eq!(task.target.objects, prefab.objects);
    assert_eq!(task.palette.len(), 1);
    assert_eq!(task.palette[0].size, [2, 1]);
    assert!(task.initial.objects.is_empty());
    assert!(task.initial.tiles.is_empty());
    assert_eq!(task.group, "geography:6:6");
    let mut nearby = prefab.clone();
    nearby.name = "nearby copy rotated".into();
    nearby.source = PrefabSource::Captured {
        tile_x: 3250,
        tile_y: 3260,
        plane: 0,
    };
    assert_eq!(
        reconstruction_task(&nearby, &|_, _| Some((2, 1)))
            .unwrap_or_else(|e| panic!("{e}"))
            .group,
        task.group
    );
    let env = Env::reset(task, 0).unwrap_or_else(|e| panic!("{e}"));
    assert!(env.observation(ObservationMode::ScoreOnly).target.is_none());
}
#[test]
fn missing_dimensions_do_not_become_guessed_one_tile_targets() {
    let Err(error) = reconstruction_task(&prefab(), &|_, _| None) else {
        panic!("unresolved dimensions were accepted")
    };
    assert!(error.contains("Missing dimensions"));
    assert!(reconstruction_task(&prefab(), &|_, _| Some((0, 1))).is_err());
}
#[test]
fn copy_task_legal_actions_reach_exact_and_undo_redo() {
    let p = prefab();
    let task = reconstruction_task(&p, &|_, _| Some((2, 1))).unwrap_or_else(|e| panic!("{e}"));
    let mut env = Env::reset(task.clone(), 17).unwrap_or_else(|e| panic!("{e}"));
    let mut actions = vec![
        Action::PaintRect {
            x: 0,
            y: 0,
            width: 1,
            height: 1,
            plane: 0,
            underlay: Some(3),
            overlay: None,
            shape: 0,
            rotation: 0,
            flags: Some(0),
        },
        Action::HeightStroke {
            x: 0,
            y: 0,
            width: 1,
            height: 1,
            plane: 0,
            value: -80,
        },
    ];
    actions.extend(
        p.objects
            .iter()
            .cloned()
            .map(|object| Action::Place { object }),
    );
    for action in &actions {
        env.step(action.clone()).unwrap_or_else(|e| panic!("{e}"));
    }
    assert!(env.score().exact);
    let copied = env.document().clone();
    env.step(Action::Undo).unwrap_or_else(|e| panic!("{e}"));
    assert!(!env.score().exact);
    env.step(Action::Redo).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(env.document(), &copied);
    assert!(
        Env::replay(task, 17, &actions)
            .unwrap_or_else(|e| panic!("{e}"))
            .score()
            .exact
    );
}
