use super::*;
use osrs_lab::PathMode;

/// A 40 by 40 lawn with a road three tiles wide across it.
fn lawn(seed: u32) -> PathLesson {
    let (w, h) = (40_usize, 40_usize);
    let mut lesson = PathLesson {
        format: osrs_lab::LESSON_FORMAT.into(),
        version: 1,
        name: format!("Lawn {seed}"),
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
        names: BTreeMap::new(),
        path_overlays: vec![10],
    };
    for x in 0..w {
        for y in 18 + (seed as usize % 3)..21 + (seed as usize % 3) {
            lesson.overlay[y * w + x] = 10;
        }
    }
    lesson
}

/// The whole Paths flow without a window: areas are saved, the background trainer reports
/// steps and finishes, the model is saved, listed, tried on a blank copy, and deleted.
#[test]
fn teaching_in_the_background_saves_a_model_that_can_be_tried() {
    let dir = std::env::temp_dir().join(format!("rustosrs-paths-ui-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut state = PathsState::with(
        lesson_store().in_directory(dir.join("paths")),
        model_store().in_directory(dir.join("models")),
    );
    for seed in 0..5 {
        let lesson = lawn(seed);
        let id = state.lessons.free_id(&lesson.name, "area");
        state
            .lessons
            .save(&id, &lesson)
            .unwrap_or_else(|e| panic!("{e}"));
    }
    assert_eq!(state.lessons.entries.len(), 5);
    let file = state.lessons.entries[0].0.clone();
    let scene = state.scene(&file).unwrap_or_else(|| panic!("scene"));
    assert!(scene.mask.iter().filter(|m| **m).count() >= 100);
    // Change which floors are paths: the picture is rebuilt.
    state
        .lessons
        .edit(&file, |l| l.path_overlays = vec![0])
        .unwrap_or_else(|e| panic!("{e}"));
    state.scenes.remove(&file);
    assert_eq!(
        state
            .scene(&file)
            .map(|s| s.mask.iter().filter(|m| **m).count()),
        Some(0)
    );
    state
        .lessons
        .edit(&file, |l| l.path_overlays = vec![10])
        .unwrap_or_else(|e| panic!("{e}"));
    state.scenes.remove(&file);
    let lessons: Vec<PathLesson> = state
        .lessons
        .entries
        .iter()
        .map(|(_, l)| l.clone())
        .collect();
    let mut running = job::Job::start(job::Inputs {
        id: state.models.free_id("Lawn roads", "paths"),
        name: "Lawn roads".into(),
        created: "2026-10-18".into(),
        lessons,
        hidden: 4,
        epochs: 40,
    });
    let started = std::time::Instant::now();
    let model = loop {
        if let Some(result) = running.poll() {
            break result.unwrap_or_else(|e| panic!("training failed: {e}"));
        }
        assert!(started.elapsed().as_secs() < 300, "training took too long");
        std::thread::sleep(std::time::Duration::from_millis(20));
    };
    assert!(
        running.percent > 0 && !running.log.is_empty(),
        "progress was reported"
    );
    state
        .models
        .save(&model.id, &model)
        .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(state.models.entries.len(), 1);
    assert!(
        state.models.problems.is_empty(),
        "{:?}",
        state.models.problems
    );
    let all = state.all_models();
    assert_eq!(all.len(), 2, "yours and the one that comes with the editor");
    assert!(all.last().is_some_and(|(_, _, starter)| *starter));
    let scene = state.scene(&file).unwrap_or_else(|| panic!("scene"));
    for mode in [PathMode::Given, PathMode::Alone] {
        let attempt = all[0].1.attempt(&scene, mode);
        assert!(attempt.drawn.iter().any(|d| *d), "it drew something");
        assert!(attempt.score.is_some_and(|s| s.f1 > 0.3));
        assert_eq!(
            attempt,
            all[0].1.attempt(&scene, mode),
            "the same model and place give the same drawing"
        );
    }
    state
        .models
        .trash(&all[0].0)
        .unwrap_or_else(|e| panic!("{e}"));
    assert!(state.models.entries.is_empty());
    let _ = std::fs::remove_dir_all(dir);
}
