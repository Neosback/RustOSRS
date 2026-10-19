//! `train-paths`: teach a path model from a folder of lessons (see the `path_lessons` example).

use osrs_lab::{PathLesson, PathProgress, PathScene, PathTrainRequest, train_paths};

/// train-paths LESSON_DIR OUT_MODEL [HIDDEN] [EPOCHS] [MAX_LESSONS] [MIN_PATH_TILES] [ID] [NAME]
pub fn train_paths_command(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let usage = "train-paths LESSON_DIR OUT_MODEL [HIDDEN] [EPOCHS] [MAX_LESSONS] [MIN_PATH_TILES] [ID] [NAME]";
    let dir = std::path::PathBuf::from(args.first().ok_or(usage)?);
    let out = args.get(1).ok_or(usage)?;
    let number = |i: usize, default: usize| args.get(i).map_or(Ok(default), |s| s.parse::<usize>());
    let (hidden, epochs, max_lessons, min_tiles) = (
        number(2, 12)?,
        number(3, 80)?,
        number(4, 400)?,
        number(5, 40)?,
    );
    let mut files: Vec<_> = std::fs::read_dir(&dir)?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.to_string_lossy().ends_with(".lesson.json"))
        .collect();
    files.sort();
    let mut lessons = Vec::new();
    for file in files {
        let lesson = PathLesson::from_json(&std::fs::read_to_string(&file)?)?;
        let tiles = PathScene::of(&lesson).mask.iter().filter(|m| **m).count();
        if tiles >= min_tiles {
            lessons.push(lesson);
        }
    }
    // Spread the pick over the whole world rather than taking the first ones.
    if lessons.len() > max_lessons {
        let step = lessons.len() as f32 / max_lessons as f32;
        lessons = (0..max_lessons)
            .map(|i| lessons[(i as f32 * step) as usize].clone())
            .collect();
    }
    eprintln!(
        "{} lessons with at least {min_tiles} path tiles",
        lessons.len()
    );
    let started = std::time::Instant::now();
    let model = train_paths(
        &PathTrainRequest {
            id: args.get(6).map_or("cli", String::as_str),
            name: args.get(7).map_or("cli", String::as_str),
            created: "2026-10-18",
            lessons: &lessons,
            hidden,
            epochs,
            seed: 1,
            cancelled: &|| false,
        },
        &|| started.elapsed().as_secs_f32(),
        &mut |PathProgress::Step { percent, message }| eprintln!("[{percent:3}%] {message}"),
    )?;
    for line in &model.summary {
        println!("{line}");
    }
    for reason in model.reasons.iter().take(10) {
        println!(
            "  {:>4.0}%  {} {}",
            reason.strength * 100.0,
            if reason.toward { "toward" } else { "away  " },
            reason.text
        );
    }
    println!("{:#?}", model.metrics);
    std::fs::write(out, model.to_json()?)?;
    Ok(())
}

/// paths-census LESSON_DIR [STRIDE]: which overlays the path guesser takes for paths, over many lessons.
pub fn census_command(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let dir = std::path::PathBuf::from(args.first().ok_or("paths-census LESSON_DIR [STRIDE]")?);
    let stride = args.get(1).map_or(Ok(10), |s| s.parse::<usize>())?;
    let mut files: Vec<_> = std::fs::read_dir(&dir)?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.to_string_lossy().ends_with(".lesson.json"))
        .collect();
    files.sort();
    let mut totals: std::collections::BTreeMap<u16, (u64, u64, u32)> = Default::default();
    for file in files.iter().step_by(stride.max(1)) {
        let lesson = PathLesson::from_json(&std::fs::read_to_string(file)?)?;
        let scene = PathScene::of(&lesson);
        let inside: std::collections::BTreeSet<usize> =
            (0..scene.tiles()).filter(|a| scene.inside[*a]).collect();
        for overlay in &lesson.overlay {
            if *overlay != 0 {
                totals.entry(*overlay).or_default().0 += 1;
            }
        }
        for guess in lesson.guess_paths(&inside) {
            let entry = totals.entry(guess.overlay).or_default();
            entry.1 += u64::from(guess.tiles);
            entry.2 += 1;
        }
    }
    let mut rows: Vec<_> = totals.into_iter().collect();
    rows.sort_by_key(|(_, t)| std::cmp::Reverse(t.1));
    println!("overlay  tiles seen  tiles taken as path  lessons");
    for (overlay, (seen, taken, lessons)) in rows.iter().take(40) {
        println!("{overlay:>7} {seen:>11} {taken:>20} {lessons:>8}");
    }
    Ok(())
}

/// paths-show LESSON_FILE: the lesson as letters. `#` path, `.` open ground, `B` building,
/// `o` other floor, `X` blocked, `D` just outside a door.
pub fn show_command(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let file = args.first().ok_or("paths-show LESSON_FILE")?;
    let lesson = PathLesson::from_json(&std::fs::read_to_string(file)?)?;
    let scene = PathScene::of(&lesson);
    for y in (0..scene.height).rev() {
        let line: String = (0..scene.width)
            .map(|x| {
                let a = y * scene.width + x;
                if scene.door_fronts.contains(&(x, y)) {
                    'D'
                } else if scene.mask[a] {
                    '#'
                } else if scene.inside[a] {
                    'B'
                } else if !scene.candidate[a] {
                    'o'
                } else if !scene.walkable[a] {
                    'X'
                } else {
                    '.'
                }
            })
            .collect();
        println!("{line}");
    }
    Ok(())
}

/// paths-stats LESSON_DIR [STRIDE]: how paths relate to doors and the edge of the area.
pub fn stats_command(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let dir = std::path::PathBuf::from(args.first().ok_or("paths-stats LESSON_DIR [STRIDE]")?);
    let stride = args.get(1).map_or(Ok(10), |s| s.parse::<usize>())?;
    let mut files: Vec<_> = std::fs::read_dir(&dir)?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.to_string_lossy().ends_with(".lesson.json"))
        .collect();
    files.sort();
    let (mut doors, mut doors_on, mut pieces, mut pieces_door, mut pieces_edge, mut pieces_all) =
        (0, 0, 0, 0, 0, 0);
    for file in files.iter().step_by(stride.max(1)) {
        let lesson = PathLesson::from_json(&std::fs::read_to_string(file)?)?;
        let scene = PathScene::of(&lesson);
        if scene.mask.iter().filter(|m| **m).count() < 40 {
            continue;
        }
        let near = |x: usize, y: usize, r: usize| {
            (y.saturating_sub(r)..(y + r + 1).min(scene.height)).any(|yy| {
                (x.saturating_sub(r)..(x + r + 1).min(scene.width))
                    .any(|xx| scene.mask[yy * scene.width + xx])
            })
        };
        for &(x, y) in &scene.door_fronts {
            doors += 1;
            doors_on += usize::from(near(x, y, 2));
        }
        for piece in osrs_lab_pieces(&scene) {
            pieces += 1;
            pieces_all += piece.len();
            pieces_door += usize::from(piece.iter().any(|&(x, y)| {
                scene
                    .door_fronts
                    .iter()
                    .any(|&(dx, dy)| dx.abs_diff(x) <= 2 && dy.abs_diff(y) <= 2)
            }));
            pieces_edge +=
                usize::from(piece.iter().any(|&(x, y)| {
                    x == 0 || y == 0 || x + 1 == scene.width || y + 1 == scene.height
                }));
        }
    }
    println!(
        "door fronts: {doors}; with path within 2 tiles: {doors_on} ({:.0}%)",
        100.0 * doors_on as f32 / doors.max(1) as f32
    );
    println!(
        "path pieces: {pieces} (avg {:.0} tiles); touching a door: {pieces_door} ({:.0}%); touching the area edge: {pieces_edge} ({:.0}%)",
        pieces_all as f32 / pieces.max(1) as f32,
        100.0 * pieces_door as f32 / pieces.max(1) as f32,
        100.0 * pieces_edge as f32 / pieces.max(1) as f32
    );
    Ok(())
}

fn osrs_lab_pieces(scene: &PathScene) -> Vec<Vec<(usize, usize)>> {
    osrs_lab::path_pieces(&scene.mask, scene.width, scene.height, 12)
}
