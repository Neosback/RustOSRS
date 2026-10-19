use super::*;
use crate::{Env, ObservationMode};
use std::collections::BTreeMap;

const ROAD: u16 = 10;
const FLOWERBED: u16 = 77;

fn entry(name: &str, blocks: Option<bool>) -> ObjectEntry {
    ObjectEntry {
        name: name.into(),
        symbol: String::new(),
        actions: vec![],
        size: [1, 1],
        blocks,
    }
}

/// A 48 by 40 lawn with a street across it and three houses on the north side, each with its
/// door on the south wall and a short road from the door to the street.
fn town(seed: u32, with_path: bool) -> PathLesson {
    let (w, h) = (48_usize, 40_usize);
    let mut lesson = PathLesson {
        format: LESSON_FORMAT.into(),
        version: 1,
        name: format!("town {seed}"),
        origin: [0, 0],
        group: format!("g{seed}"),
        width: w as u16,
        height: h as u16,
        overlay: vec![0; w * h],
        shape: vec![0; w * h],
        rotation: vec![0; w * h],
        underlay: vec![5; w * h],
        flags: vec![0; w * h],
        heights: vec![0; w * h],
        objects: vec![],
        names: BTreeMap::from([
            (1, entry("wall", Some(true))),
            (2, entry("door", Some(false))),
            (3, entry("tree", Some(true))),
        ]),
        places: vec![],
        path_overlays: vec![ROAD],
    };
    let set = |lesson: &mut PathLesson, x: usize, y: usize, id: u16| lesson.overlay[y * w + x] = id;
    // The street.
    for x in 0..w {
        for y in 8..11 {
            if with_path {
                set(&mut lesson, x, y, ROAD);
            }
        }
    }
    let shift = (seed % 3) as usize;
    for house in 0..3 {
        let x0 = 4 + house * 14 + shift;
        let y0 = 18;
        let door = x0 + 2;
        // Walls: south (with the door), north, west, east.
        for x in x0 + 1..x0 + 5 {
            lesson
                .objects
                .push([if x == door { 2 } else { 1 }, 0, 3, x as u32, y0 as u32]);
            lesson.objects.push([1, 0, 1, x as u32, (y0 + 4) as u32]);
        }
        for y in y0 + 1..y0 + 4 {
            lesson.objects.push([1, 0, 0, x0 as u32, y as u32]);
            lesson.objects.push([1, 0, 2, (x0 + 5) as u32, y as u32]);
        }
        // Corners are single pieces that own two edges.
        for (cx, cy, o) in [
            (x0, y0, 3),
            (x0, y0 + 4, 0),
            (x0 + 5, y0 + 4, 1),
            (x0 + 5, y0, 2),
        ] {
            lesson.objects.push([1, 2, o, cx as u32, cy as u32]);
        }
        // A floor inside, and the road from the door front down to the street.
        for y in y0..y0 + 5 {
            for x in x0..x0 + 6 {
                set(&mut lesson, x, y, FLOWERBED);
            }
        }
        if with_path {
            for y in 11..y0 {
                set(&mut lesson, door, y, ROAD);
            }
        }
        lesson.objects.push([3, 10, 0, (x0 + 8) as u32, 25]);
    }
    lesson
}

#[test]
fn a_lesson_round_trips_and_is_checked() {
    let lesson = town(0, true);
    let text = lesson.to_json().unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(
        PathLesson::from_json(&text).unwrap_or_else(|e| panic!("{e}")),
        lesson
    );
    let mut bad = lesson.clone();
    bad.heights.pop();
    assert!(bad.validate().is_err());
}

#[test]
fn thin_overlays_are_taken_for_paths_and_blobs_are_not() {
    let mut lesson = town(0, true);
    lesson.path_overlays.clear();
    let scene = Scene::of(&lesson);
    let inside: std::collections::BTreeSet<usize> =
        (0..scene.tiles()).filter(|a| scene.inside[*a]).collect();
    let guessed = lesson.guess_paths(&inside);
    assert!(guessed.iter().any(|g| g.overlay == ROAD), "{guessed:?}");
    assert!(
        !guessed.iter().any(|g| g.overlay == FLOWERBED),
        "house floors are inside buildings, not paths"
    );
    // A big square of one overlay is not a path.
    let mut lake = town(1, false);
    lake.path_overlays.clear();
    for y in 2..30 {
        for x in 2..30 {
            lake.overlay[y * 48 + x] = 99;
        }
    }
    let inside = std::collections::BTreeSet::new();
    assert!(lake.guess_paths(&inside).iter().all(|g| g.overlay != 99));
}

#[test]
fn the_scene_finds_doors_and_never_sees_the_paths() {
    let with = Scene::of(&town(0, true));
    let without = Scene::of(&town(0, false));
    assert_eq!(with.door_fronts.len(), 3, "{:?}", with.door_fronts);
    assert!(
        with.door_fronts.iter().all(|(_, y)| *y == 17),
        "just outside the south wall"
    );
    assert!(with.inside.iter().filter(|b| **b).count() >= 3 * 20);
    assert_eq!(
        with.features, without.features,
        "taking the paths off changes nothing the learner sees"
    );
    assert!(with.mask.iter().filter(|m| **m).count() > 100);
    assert_eq!(without.mask.iter().filter(|m| **m).count(), 0);
    let door = with.door_fronts[0];
    let at = door.1 * 48 + door.0;
    assert!(with.features[at][5] > 0.5, "the door-front number is set");
    assert!(!with.candidate[20 * 48 + 6], "buildings are not candidates");
}

#[test]
fn training_learns_where_paths_go_and_beats_guessing_on_unseen_places() {
    let lessons: Vec<PathLesson> = (0..12).map(|i| town(i, true)).collect();
    let mut steps = 0;
    let model = train_paths(
        &PathTrainRequest {
            id: "t",
            name: "t",
            created: "2026-10-18",
            lessons: &lessons,
            hidden: 6,
            epochs: 60,
            seed: 3,
            cancelled: &|| false,
        },
        &|| 0.0,
        &mut |PathProgress::Step { percent, .. }| {
            assert!(percent <= 100);
            steps += 1;
        },
    )
    .unwrap_or_else(|e| panic!("{e}"));
    assert!(steps > 3, "progress is reported");
    assert!(
        model.metrics.checked_on_unseen,
        "whole places were set aside"
    );
    assert!(model.metrics.auc > 0.85, "{:?}", model.metrics);
    assert!(
        model.metrics.given_model.f1 > model.metrics.given_shortest.f1,
        "{:?}",
        model.metrics
    );
    assert!(
        model.metrics.alone_tiles.f1 > model.metrics.alone_everywhere.f1,
        "{:?}",
        model.metrics
    );
    assert!(model.validate().is_ok());
    assert!(!model.summary.is_empty() && !model.reasons.is_empty());
    let again = train_paths(
        &PathTrainRequest {
            id: "t",
            name: "t",
            created: "2026-10-18",
            lessons: &lessons,
            hidden: 6,
            epochs: 60,
            seed: 3,
            cancelled: &|| false,
        },
        &|| 0.0,
        &mut |_| {},
    )
    .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(
        model.net, again.net,
        "the same lessons and settings give the same model"
    );
    let text = model.to_json().unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(
        PathModel::from_json(&text).unwrap_or_else(|e| panic!("{e}")),
        model
    );
    let damaged = text.replacen(&format!("\"id\": \"{}\"", model.id), "\"id\": \"u\"", 1);
    assert!(
        PathModel::from_json(&damaged).is_err(),
        "a changed model is refused"
    );
}

#[test]
fn training_can_be_stopped_and_needs_something_to_learn_from() {
    let lessons = vec![town(0, true)];
    let stopped = train_paths(
        &PathTrainRequest {
            id: "t",
            name: "t",
            created: "",
            lessons: &lessons,
            hidden: 0,
            epochs: 10,
            seed: 1,
            cancelled: &|| true,
        },
        &|| 0.0,
        &mut |_| {},
    );
    assert!(stopped.is_err_and(|e| e.contains("stopped")));
    let bare = vec![town(0, false)];
    let nothing = train_paths(
        &PathTrainRequest {
            id: "t",
            name: "t",
            created: "",
            lessons: &bare,
            hidden: 0,
            epochs: 10,
            seed: 1,
            cancelled: &|| false,
        },
        &|| 0.0,
        &mut |_| {},
    );
    assert!(
        nothing.is_err_and(|e| e.contains("no")),
        "a place with no path teaches nothing"
    );
}

#[test]
fn the_shipped_model_loads_and_draws_between_places_through_the_lab_tool() {
    let model = PathModel::starter().unwrap_or_else(|e| panic!("{e}"));
    let lesson = town(2, true);
    let scene = Scene::of(&lesson);
    let groups = endpoints_of(&scene.mask, scene.width, scene.height, 12);
    assert!(!groups.is_empty());
    let drawn = model.draw_between(&scene, &groups);
    assert!(!drawn.routes.is_empty());
    // The same model, the same place and the same ends give the same drawing.
    assert_eq!(drawn.routes, model.draw_between(&scene, &groups).routes);
    // Through the lab: paths are laid on the blank canvas with the path tool and scored against
    // the hidden real place, and they raise the score.
    let task = path_task(&lesson).unwrap_or_else(|e| panic!("{e}"));
    let mut env = Env::reset(task, 1).unwrap_or_else(|e| panic!("{e}"));
    assert!(
        env.observation(ObservationMode::ScoreOnly).target.is_none(),
        "the real paths stay hidden"
    );
    let painted_tiles = |env: &Env| -> Vec<bool> {
        let snapshot = crate::Snapshot::capture(env.document());
        let mut mask = vec![false; 48 * 40];
        for t in &snapshot.tiles {
            if t.overlay.as_ref().is_some_and(|o| o.id == u32::from(ROAD)) {
                mask[usize::from(t.y) * 48 + usize::from(t.x)] = true;
            }
        }
        mask
    };
    assert_eq!(
        painted_tiles(&env).iter().filter(|m| **m).count(),
        0,
        "the canvas starts blank"
    );
    let mut painter = model.clone();
    painter.overlay = ROAD;
    painter.width = 3.0;
    for action in lay_actions(&painter, &drawn.routes) {
        env.step(action).unwrap_or_else(|e| panic!("{e}"));
    }
    let laid = painted_tiles(&env);
    assert!(
        laid.iter().filter(|m| **m).count() > 50,
        "paths were painted"
    );
    let matched = super::eval::score(&scene.mask, &laid, 48, 40, 2);
    assert!(
        matched.f1 > 0.3,
        "the paths laid through the lab tool land on the real ones: {matched:?}"
    );
}
