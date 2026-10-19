//! Step 3: give a model a blank copy of a place and see what it draws, next to the real paths.

use super::canvas::{self, Layers};
use super::teach::reasons;
use crate::app::EditorApp;
use eframe::egui;
use osrs_lab::{PathAttempt, PathMode};

pub(super) struct TryState {
    pub(super) model: Option<String>,
    lesson: Option<String>,
    mode: PathMode,
    show_real: bool,
    show_chances: bool,
    result: Option<(String, String, PathAttempt)>,
    /// The floor type to lay with (0: the one the area's own paths are made of).
    overlay: u16,
    notice: Option<String>,
}

impl Default for TryState {
    fn default() -> Self {
        Self {
            model: None,
            lesson: None,
            mode: PathMode::Given,
            show_real: true,
            show_chances: false,
            result: None,
            overlay: 0,
            notice: None,
        }
    }
}

/// Run the first model on the selected (or first) area (for scripts).
pub(super) fn run(state: &mut super::PathsState, mode: PathMode) -> Result<(), String> {
    let models = state.all_models();
    let (key, model, _) = models.first().ok_or("no model")?.clone();
    let lesson = state
        .selected
        .clone()
        .or_else(|| state.lessons.entries.first().map(|(f, _)| f.clone()))
        .ok_or("no area")?;
    let scene = state.scene(&lesson).ok_or("no picture")?;
    let attempt = model.attempt(&scene, mode);
    if let Some(score) = &attempt.score {
        eprintln!(
            "smoke: paths try {:?}: matches {:.0}%",
            mode,
            score.f1 * 100.0
        );
    }
    state.try_it.mode = mode;
    state.try_it.model = Some(key.clone());
    state.try_it.lesson = Some(lesson.clone());
    state.try_it.result = Some((key, lesson, attempt));
    Ok(())
}

pub(super) fn show(app: &mut EditorApp, ui: &mut egui::Ui) {
    super::heading(
        ui,
        &mut app.prefabs.workshop.paths.scroll_to,
        3,
        "Watch it draw",
    );
    let state = &mut app.prefabs.workshop.paths;
    let models = state.all_models();
    let lessons: Vec<(String, String)> = state
        .lessons
        .entries
        .iter()
        .map(|(f, l)| (f.clone(), l.name.clone()))
        .collect();
    if lessons.is_empty() {
        ui.weak("Import an area first (step 1). The editor hides its paths and asks the model to draw them back.");
        return;
    }
    let model_key = state
        .try_it
        .model
        .clone()
        .filter(|k| models.iter().any(|(f, ..)| f == k))
        .or_else(|| models.first().map(|(f, ..)| f.clone()));
    let lesson_key = state
        .try_it
        .lesson
        .clone()
        .filter(|k| lessons.iter().any(|(f, _)| f == k))
        .or_else(|| {
            state
                .selected
                .clone()
                .filter(|k| lessons.iter().any(|(f, _)| f == k))
        })
        .or_else(|| lessons.first().map(|(f, _)| f.clone()));
    let (Some(model_key), Some(lesson_key)) = (model_key, lesson_key) else {
        return;
    };
    state.try_it.model = Some(model_key.clone());
    state.try_it.lesson = Some(lesson_key.clone());
    ui.label("Model");
    for (file, model, starter) in &models {
        ui.horizontal(|ui| {
            if ui.radio(model_key == *file, "").clicked() {
                state.try_it.model = Some(file.clone());
                state.try_it.result = None;
            }
            ui.vertical(|ui| {
                ui.strong(if *starter {
                    format!("{} (comes with the editor)", model.name)
                } else {
                    model.name.clone()
                });
                ui.weak(format!(
                    "learned from {} area(s) \u{b7} {} path tiles \u{b7} {}",
                    model.metrics.lessons, model.metrics.path_tiles, model.created
                ));
            });
            if !starter
                && ui
                    .small_button("Delete")
                    .on_hover_text("Moves the file to the models/.trash folder")
                    .clicked()
            {
                let file = file.clone();
                if let Err(error) = state.models.trash(&file) {
                    state.problem = Some(error);
                }
                state.try_it.model = None;
                state.try_it.result = None;
            }
        });
    }
    ui.horizontal(|ui| {
        ui.label("Blank copy of");
        egui::ComboBox::from_id_salt("paths-try-lesson")
            .selected_text(
                lessons
                    .iter()
                    .find(|(f, _)| *f == lesson_key)
                    .map_or("", |(_, n)| n.as_str()),
            )
            .show_ui(ui, |ui| {
                for (file, name) in &lessons {
                    if ui.selectable_label(*file == lesson_key, name).clicked() {
                        state.try_it.lesson = Some(file.clone());
                        state.try_it.result = None;
                    }
                }
            });
    });
    ui.horizontal_wrapped(|ui| {
        ui.radio_value(
            &mut state.try_it.mode,
            PathMode::Given,
            "Tell it where each path starts and ends",
        )
        .on_hover_text("You give the two ends of every real path; it decides how to join them");
        ui.radio_value(
            &mut state.try_it.mode,
            PathMode::Alone,
            "Let it find the paths by itself",
        )
        .on_hover_text("Nothing is given: it paints the tiles it is sure are path");
    });
    if ui.button("Draw").clicked() {
        let scene = state.scene(&lesson_key);
        if let (Some(scene), Some((_, model, _))) =
            (scene, models.iter().find(|(f, ..)| *f == model_key))
        {
            let attempt = model.attempt(&scene, state.try_it.mode);
            state.try_it.result = Some((model_key.clone(), lesson_key.clone(), attempt));
        }
    }
    let Some((rm, rl, attempt)) = state.try_it.result.clone() else {
        return;
    };
    if rm != model_key || rl != lesson_key {
        return;
    }
    let (Some(scene), Some((_, model, _))) = (
        state.scene(&lesson_key),
        models.iter().find(|(f, ..)| *f == model_key),
    ) else {
        return;
    };
    ui.horizontal(|ui| {
        ui.checkbox(&mut state.try_it.show_real, "Show the real paths");
        ui.checkbox(&mut state.try_it.show_chances, "Show how sure it is");
    });
    ui.horizontal_top(|ui| {
        let layers = Layers {
            real: state.try_it.show_real,
            chances: state
                .try_it
                .show_chances
                .then_some(attempt.chances.as_slice()),
            drawn: Some(&attempt.drawn),
        };
        let hovered = canvas::show(ui, &scene, 400.0, &layers);
        ui.vertical(|ui| {
            canvas::legend(ui, state.try_it.show_real, true, state.try_it.show_chances);
            ui.add_space(6.0);
            verdict(ui, &attempt);
            ui.add_space(6.0);
            match hovered {
                Some((x, y)) => why_here(ui, model, &scene, x, y),
                None => {
                    ui.weak(
                        "Point at a tile to see why the model would or would not put a path there.",
                    );
                }
            }
        });
    });
    ui.add_space(4.0);
    // Laying on the map: the area it came from, one undoable change.
    let lesson = state.lesson(&lesson_key).cloned();
    let auto_overlay = lesson
        .as_ref()
        .and_then(|l| {
            let mut counts: std::collections::BTreeMap<u16, usize> = Default::default();
            for (a, on) in scene.mask.iter().enumerate() {
                if *on {
                    *counts.entry(l.overlay[a]).or_default() += 1;
                }
            }
            counts.into_iter().max_by_key(|(_, n)| *n).map(|(o, _)| o)
        })
        .unwrap_or(model.overlay);
    ui.horizontal_wrapped(|ui| {
        ui.label("Lay it with floor type");
        ui.add(egui::DragValue::new(&mut state.try_it.overlay).range(0..=999))
            .on_hover_text("0 means the floor type this area's own paths are made of");
        ui.weak(format!(
            "(now: {})",
            if state.try_it.overlay == 0 {
                auto_overlay
            } else {
                state.try_it.overlay
            }
        ));
    });
    let overlay = if state.try_it.overlay == 0 {
        auto_overlay
    } else {
        state.try_it.overlay
    };
    let width = model.width;
    let lay_now = ui
        .button("Lay these paths on the map")
        .on_hover_text("Puts the drawn paths on the map where this area came from. One undo takes them off again")
        .clicked();
    if let Some(notice) = &state.try_it.notice {
        ui.label(notice);
    }
    let reasons_model = model.clone();
    if lay_now && let Some(lesson) = lesson {
        let tiles = super::lay::tiles_for(
            lesson.origin,
            (scene.width, scene.height),
            overlay,
            width,
            &attempt,
        );
        let count = app.apply_terrain_path(&tiles);
        app.prefabs.workshop.paths.try_it.notice = Some(if count == 0 {
            "Nothing was laid: that part of the map is not loaded. Fly there first.".to_owned()
        } else {
            format!("Laid {count} path tiles. Edit, Undo takes them off.")
        });
    }
    reasons(ui, &reasons_model);
}

fn verdict(ui: &mut egui::Ui, attempt: &PathAttempt) {
    let Some(score) = &attempt.score else {
        ui.label("This area has no real paths to compare with.");
        return;
    };
    ui.label(
        egui::RichText::new(format!(
            "It matches the real paths {:.0}%",
            score.f1 * 100.0
        ))
        .strong(),
    );
    for (name, baseline) in &attempt.baselines {
        ui.label(format!("   {name}: {:.0}%", baseline.f1 * 100.0));
    }
    ui.weak(
        "Of what it drew, how much is on a real path, and of the real paths, how much it covers.",
    );
}

fn why_here(
    ui: &mut egui::Ui,
    model: &osrs_lab::PathModel,
    scene: &osrs_lab::PathScene,
    x: usize,
    y: usize,
) {
    let a = y * scene.width + x;
    ui.label(egui::RichText::new(format!("Tile {x}, {y}")).strong());
    if scene.inside[a] {
        ui.label("Inside a building: paths do not go here.");
        return;
    }
    if !scene.candidate[a] {
        ui.label("Already another kind of floor, so a path is not needed here.");
        return;
    }
    let chance = model.net.prob(&scene.features[a]);
    ui.label(format!(
        "The model thinks there is a {:.0}% chance of a path here.",
        chance * 100.0
    ));
    for why in model.why(scene, x, y, 4) {
        ui.label(format!(
            "{} paths {}",
            if why.toward { "\u{2795}" } else { "\u{2796}" },
            why.phrase
        ));
    }
}
