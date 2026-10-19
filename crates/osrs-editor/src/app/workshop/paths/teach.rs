//! Step 2: let the editor learn from the picked areas. A progress bar and a short log while it
//! works, then what it learned in sentences, with bars for how well it does on ground it did not
//! see and the reasons behind its drawing.

use super::{PathsState, job};
use crate::app::EditorApp;
use eframe::egui;
use osrs_lab::{PathMetrics, PathModel, PathScore};

#[derive(Default)]
pub(super) struct TeachState {
    name: String,
    thorough: bool,
    job: Option<job::Job>,
    pub(super) finished: Option<PathModel>,
    problem: Option<String>,
}

/// Pick every area and start learning (for scripts).
pub(super) fn begin(state: &mut PathsState, name: &str) -> Result<(), String> {
    state.picked = state
        .lessons
        .entries
        .iter()
        .map(|(f, _)| f.clone())
        .collect();
    if state.picked.is_empty() {
        return Err("no areas to learn from".into());
    }
    state.teach.name = name.replace('_', " ");
    let lessons = state
        .lessons
        .entries
        .iter()
        .map(|(_, l)| l.clone())
        .collect();
    let id = state.models.free_id(&state.teach.name, "paths");
    state.teach.job = Some(job::Job::start(job::Inputs {
        id,
        name: state.teach.name.clone(),
        created: crate::app::workshop::train_date(),
        lessons,
        hidden: 12,
        epochs: 60,
    }));
    Ok(())
}

/// Take what the trainer has said; keep the model when it is done.
pub(super) fn poll(state: &mut PathsState) {
    let Some(running) = &mut state.teach.job else {
        return;
    };
    let Some(result) = running.poll() else { return };
    state.teach.job = None;
    match result {
        Ok(model) => {
            let model = *model;
            match state.models.save(&model.id, &model) {
                Ok(file) => {
                    state.models.keep(file.clone(), model.clone());
                    state.try_it.model = Some(file);
                    state.teach.problem = None;
                }
                Err(error) => {
                    state.teach.problem = Some(format!("The model could not be saved ({error})."))
                }
            }
            state.teach.finished = Some(model);
        }
        Err(error) if error == "stopped" => {
            state.teach.problem = Some("Stopped. Nothing was kept.".into())
        }
        Err(error) => state.teach.problem = Some(error),
    }
}

pub(super) fn show(app: &mut EditorApp, ui: &mut egui::Ui) {
    super::heading(ui, &mut app.prefabs.workshop.paths.scroll_to, 2, "Teach it");
    let picked = app.prefabs.workshop.paths.picked.len();
    let state = &mut app.prefabs.workshop.paths;
    if let Some(running) = &state.teach.job {
        ui.add(
            egui::ProgressBar::new(f32::from(running.percent) / 100.0)
                .text(format!("{}  {}%", running.message, running.percent))
                .animate(true),
        );
        ui.collapsing("What it has done so far", |ui| {
            for line in &running.log {
                ui.label(format!("\u{2713} {line}"));
            }
        });
        if ui.button("Stop").clicked() {
            running.stop();
        }
        return;
    }
    ui.weak("Tick the areas above to learn from. Learning from several different places teaches it what paths have in common instead of one place's quirks.");
    ui.horizontal(|ui| {
        ui.label("Call the model");
        ui.add(
            egui::TextEdit::singleline(&mut state.teach.name)
                .hint_text("for example: Town roads")
                .desired_width(220.0),
        );
    });
    ui.horizontal(|ui| {
        ui.radio_value(&mut state.teach.thorough, false, "Quick (about 10 seconds)");
        ui.radio_value(&mut state.teach.thorough, true, "Thorough (about a minute)");
    });
    let ready = picked > 0 && !state.teach.name.trim().is_empty();
    if ui
        .add_enabled(
            ready,
            egui::Button::new(format!("Start learning from {picked} area(s)")),
        )
        .on_disabled_hover_text("Tick at least one area and give the model a name")
        .clicked()
    {
        let lessons = state
            .picked
            .iter()
            .filter_map(|f| state.lesson(f).cloned())
            .collect::<Vec<_>>();
        let name = state.teach.name.trim().to_owned();
        let id = state.models.free_id(&name, "paths");
        let (hidden, epochs) = if state.teach.thorough {
            (24, 150)
        } else {
            (12, 60)
        };
        state.teach.problem = None;
        state.teach.finished = None;
        state.teach.job = Some(job::Job::start(job::Inputs {
            id,
            name,
            created: crate::app::workshop::train_date(),
            lessons,
            hidden,
            epochs,
        }));
    }
    if let Some(problem) = &state.teach.problem {
        ui.colored_label(ui.visuals().error_fg_color, problem);
    }
    if let Some(model) = &state.teach.finished {
        results(ui, model);
    }
}

fn bar(ui: &mut egui::Ui, label: &str, value: f32) {
    ui.horizontal(|ui| {
        ui.add_sized([230.0, 18.0], egui::Label::new(label));
        ui.add(
            egui::ProgressBar::new(value.clamp(0.0, 1.0))
                .desired_width(200.0)
                .text(format!("{:.0}%", value * 100.0)),
        );
    });
}

/// What training found, in sentences, bars and the model's reasons.
pub(super) fn results(ui: &mut egui::Ui, model: &PathModel) {
    ui.add_space(6.0);
    ui.group(|ui| {
        ui.strong(format!(
            "\"{}\" is ready ({:.1} seconds)",
            model.name, model.metrics.seconds
        ));
        for line in &model.summary {
            ui.label(format!("\u{2022} {line}"));
        }
        ui.add_space(4.0);
        scores(ui, &model.metrics);
        ui.add_space(4.0);
        reasons(ui, model);
    });
}

fn scores(ui: &mut egui::Ui, m: &PathMetrics) {
    let pct = |s: &PathScore| s.f1;
    ui.label(
        egui::RichText::new(if m.checked_on_unseen {
            "How well it does on places it never saw"
        } else {
            "How well it does (on ground set aside, but near ground it learned from)"
        })
        .strong(),
    );
    bar(
        ui,
        "Told where each path starts and ends",
        pct(&m.given_model),
    );
    bar(ui, "   a straight line instead", pct(&m.given_straight));
    bar(ui, "   the shortest walk instead", pct(&m.given_shortest));
    bar(ui, "Left to find paths by itself", pct(&m.alone_tiles));
    bar(
        ui,
        "   painting every open tile instead",
        pct(&m.alone_everywhere),
    );
}

pub(super) fn reasons(ui: &mut egui::Ui, model: &PathModel) {
    ui.label(egui::RichText::new("Why it draws where it does").strong());
    ui.weak("Longer bar: the more it matters. These are habits it picked up from the real paths.");
    for reason in model.reasons.iter().filter(|r| r.strength > 0.03).take(8) {
        ui.horizontal(|ui| {
            ui.add(
                egui::ProgressBar::new(reason.strength.clamp(0.0, 1.0))
                    .desired_width(120.0)
                    .fill(if reason.toward {
                        egui::Color32::from_rgb(70, 150, 90)
                    } else {
                        egui::Color32::from_rgb(190, 90, 80)
                    }),
            );
            ui.label(format!("Paths {}", reason.text));
        });
    }
}
