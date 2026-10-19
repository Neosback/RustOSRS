//! The Train tab: pick buildings, let the editor learn their style, then ask it to build new
//! ones. Four numbered steps, in plain words; what the trainer does is in `osrs-lab`.

mod job;
pub(super) mod live;
mod models;
mod try_it;

use crate::app::{EditorApp, prefab_runtime::starter_kit};
use eframe::egui;
use osrs_core::object_names::NameSource;
use osrs_lab::ModelMetrics;
use std::collections::BTreeSet;

/// Starting styles to learn on top of: what the picked buildings do not show stays as it is.
const STARTS: [&str; 2] = [
    "Stone and brick (Lumbridge)",
    "Wood and thatch (timber town)",
];

pub(super) struct Finished {
    name: String,
    summary: Vec<String>,
    metrics: ModelMetrics,
    seconds: f32,
}

pub(in crate::app) struct TrainState {
    picked: BTreeSet<String>,
    start: usize,
    name: String,
    job: Option<job::Job>,
    finished: Option<Finished>,
    problem: Option<String>,
    models: models::ModelList,
    /// Models whose style is already in the Build page's kit list.
    offered: BTreeSet<String>,
    try_it: try_it::TryState,
}

impl Default for TrainState {
    fn default() -> Self {
        Self {
            picked: BTreeSet::new(),
            start: 0,
            name: String::new(),
            job: None,
            finished: None,
            problem: None,
            models: models::ModelList::beside_settings(),
            offered: BTreeSet::new(),
            try_it: try_it::TryState::default(),
        }
    }
}

/// Today as `YYYY-MM-DD`.
pub(super) fn today() -> String {
    job::today()
}

/// Once per frame: take what the trainer has said, and save the model when it finishes.
pub(super) fn poll(app: &mut EditorApp) {
    let train = &mut app.prefabs.workshop.train;
    let Some(job) = &mut train.job else {
        return;
    };
    let Some(result) = job.poll() else {
        return;
    };
    let name = job.name.clone();
    train.job = None;
    match result {
        Ok(model) => {
            train.problem = None;
            train.finished = Some(Finished {
                name: name.clone(),
                summary: model.summary.clone(),
                metrics: model.metrics.clone(),
                seconds: model.metrics.seconds,
            });
            match train.models.save(&model) {
                Ok(file) => {
                    train.try_it.choose(&file);
                }
                Err(error) => {
                    train.problem = Some(format!("The model could not be saved ({error})."))
                }
            }
        }
        Err(error) => train.problem = Some(error),
    }
}

pub(super) fn show(app: &mut EditorApp, ui: &mut egui::Ui) {
    app.prefabs.workshop.train.models.ensure_loaded();
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            super::policy::show(app, ui);
            ui.separator();
            egui::CollapsingHeader::new("Learn style tables and use the procedural generator")
                .show(ui, |ui| {
                    intro(ui);
                    pick(app, ui);
                    names(app, ui);
                    teach(app, ui);
                    try_it::show(app, ui);
                });
        });
}

fn intro(ui: &mut egui::Ui) {
    ui.weak(
        "Learn style extracts asset and placement tables and fits a ranking critic. It uses the procedural planner; this is separate from training a tool-action policy.",
    );
    ui.add_space(6.0);
}

fn heading(ui: &mut egui::Ui, number: u8, text: &str) {
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(format!("{number}")).strong().size(18.0));
        ui.label(egui::RichText::new(text).strong().size(16.0));
    });
}

fn pick(app: &mut EditorApp, ui: &mut egui::Ui) {
    heading(ui, 1, "Pick the buildings to learn from");
    ui.weak(
        "One building teaches the editor that building's style. Several teach it what they have \
         in common, which is better for building new ones.",
    );
    let rows: Vec<(String, String, String)> = app
        .prefabs
        .entries()
        .iter()
        .map(|e| (e.file.clone(), e.prefab.name.clone(), e.prefab.summary()))
        .collect();
    if rows.is_empty() {
        ui.weak("You have no saved buildings yet. Capture one from the map first.");
        return;
    }
    let known: BTreeSet<String> = rows.iter().map(|(file, ..)| file.clone()).collect();
    let train = &mut app.prefabs.workshop.train;
    train.picked.retain(|f| known.contains(f));
    for (file, name, summary) in rows {
        let mut on = train.picked.contains(&file);
        if ui
            .checkbox(&mut on, format!("{name}  \u{b7}  {summary}"))
            .changed()
        {
            if on {
                train.picked.insert(file);
            } else {
                train.picked.remove(&file);
            }
        }
    }
}

/// How many kinds of object in the picked buildings have no name.
fn unnamed_among(app: &EditorApp) -> (usize, usize) {
    let prefabs: Vec<&osrs_edit::Prefab> = app
        .prefabs
        .workshop
        .train
        .picked
        .iter()
        .filter_map(|f| app.prefabs.entry(f).map(|e| &e.prefab))
        .collect();
    let ids: BTreeSet<u32> = prefabs
        .iter()
        .flat_map(|p| p.objects.iter().map(|o| o.id))
        .collect();
    let unnamed = ids
        .iter()
        .filter(|id| {
            let cache = app
                .placer
                .catalog_entry(**id)
                .map(|e| e.name.clone())
                .unwrap_or_default();
            app.names.resolve(**id, Some(&cache)).source == NameSource::Unnamed
        })
        .count();
    (unnamed, ids.len())
}

fn names(app: &mut EditorApp, ui: &mut egui::Ui) {
    heading(ui, 2, "Name what the editor does not know (optional)");
    let (unnamed, total) = unnamed_among(app);
    if total == 0 {
        ui.weak("Pick a building above to see how many of its objects have names.");
        return;
    }
    if unnamed == 0 {
        ui.label("Every kind of object in these buildings has a name.");
        return;
    }
    ui.label(format!(
        "{unnamed} of {total} kinds of objects in these buildings have no name."
    ));
    ui.weak(
        "Training works without names, but it learns better rules (which furniture goes in which \
         room) when it knows what things are.",
    );
    if ui.button("Name them now").clicked() {
        app.prefabs.workshop.tab = super::Tab::Names;
    }
}

fn teach(app: &mut EditorApp, ui: &mut egui::Ui) {
    heading(ui, 3, "Teach it");
    let picked = app.prefabs.workshop.train.picked.len();
    let train = &mut app.prefabs.workshop.train;
    if let Some(job) = &train.job {
        ui.add(
            egui::ProgressBar::new(f32::from(job.percent) / 100.0)
                .text(format!("{}  {}%", job.message, job.percent))
                .animate(true),
        );
        ui.weak(format!(
            "Learning from {} building(s)...",
            job.building_count
        ));
        ui.collapsing("What it has done so far", |ui| {
            for line in &job.log {
                ui.label(format!("\u{2713} {line}"));
            }
        });
        if ui.button("Stop").clicked() {
            job.stop();
        }
        return;
    }
    ui.horizontal(|ui| {
        ui.label("Start from");
        egui::ComboBox::from_id_salt("train-start")
            .selected_text(STARTS[train.start.min(STARTS.len() - 1)])
            .show_ui(ui, |ui| {
                for (index, label) in STARTS.iter().enumerate() {
                    ui.selectable_value(&mut train.start, index, *label);
                }
            });
    });
    ui.weak("What your buildings do not show (like the roof shape) comes from this style.");
    ui.horizontal(|ui| {
        ui.label("Call the model");
        ui.add(
            egui::TextEdit::singleline(&mut train.name)
                .hint_text("for example: Lumbridge houses")
                .desired_width(240.0),
        );
    });
    let ready = picked > 0 && !train.name.trim().is_empty();
    if ui
        .add_enabled(
            ready,
            egui::Button::new(format!("Start training on {picked} building(s)")),
        )
        .on_disabled_hover_text("Pick at least one building and give the model a name")
        .clicked()
    {
        start(app);
    }
    if let Some(problem) = &app.prefabs.workshop.train.problem {
        ui.colored_label(ui.visuals().error_fg_color, problem);
    }
    if let Some(done) = &app.prefabs.workshop.train.finished {
        results(ui, done);
    }
}

/// Pick the named buildings (all of them when none are named) and start training.
pub(super) fn begin(app: &mut EditorApp, name: &str, picks: &[&str]) -> Result<(), String> {
    let files: BTreeSet<String> = app
        .prefabs
        .entries()
        .iter()
        .filter(|e| picks.is_empty() || picks.iter().any(|p| *p == e.prefab.name))
        .map(|e| e.file.clone())
        .collect();
    if files.is_empty() {
        return Err("no saved building matches".into());
    }
    let train = &mut app.prefabs.workshop.train;
    train.picked = files;
    train.name = name.replace('_', " ");
    start(app);
    match &app.prefabs.workshop.train.problem {
        Some(problem) => Err(problem.clone()),
        None => Ok(()),
    }
}

fn start(app: &mut EditorApp) {
    let train = &app.prefabs.workshop.train;
    let prefabs: Vec<(String, osrs_edit::Prefab)> = train
        .picked
        .iter()
        .filter_map(|f| app.prefabs.entry(f))
        .map(|e| (e.prefab.name.clone(), e.prefab.clone()))
        .collect();
    let refs: Vec<&osrs_edit::Prefab> = prefabs.iter().map(|(_, p)| p).collect();
    let (objects, _) = super::table::table_for(app, &refs);
    let base_kit = match starter_kit(train.start) {
        Ok(kit) => kit,
        Err(error) => {
            app.prefabs.workshop.train.problem = Some(error);
            return;
        }
    };
    let name = train.name.trim().to_owned();
    let train = &mut app.prefabs.workshop.train;
    let id = train.models.free_id(&name);
    let seed = prefabs
        .iter()
        .map(|(_, p)| p.objects.len() as u64)
        .sum::<u64>()
        + 1;
    train.problem = None;
    train.finished = None;
    train.job = Some(job::Job::start(job::Inputs {
        id,
        name,
        created: job::today(),
        prefabs,
        objects,
        base_kit,
        seed,
    }));
}

/// What training found, in sentences and two bars.
fn results(ui: &mut egui::Ui, done: &Finished) {
    ui.add_space(6.0);
    ui.group(|ui| {
        ui.strong(format!(
            "\"{}\" is ready ({:.1} seconds)",
            done.name, done.seconds
        ));
        for line in &done.summary {
            ui.label(format!("\u{2022} {line}"));
        }
        ui.add_space(4.0);
        ui.label("Procedural generation checks and ranking scores");
        ui.label(format!(
            "First-try ranking score: {:.3}",
            done.metrics.realism_single
        ));
        ui.label(format!(
            "Best-of-16 ranking score: {:.3}",
            done.metrics.realism_best_of_16
        ));
        bar(ui, "built a usable building", done.metrics.usable_rate);
        ui.weak(
            "\"Look real\" is a score from a small judge trained on real buildings of the game. \
             It is a guide, not a measure of taste.",
        );
    });
}

fn bar(ui: &mut egui::Ui, label: &str, value: f32) {
    ui.horizontal(|ui| {
        ui.add_sized([150.0, 18.0], egui::Label::new(label));
        ui.add(
            egui::ProgressBar::new(value.clamp(0.0, 1.0))
                .desired_width(220.0)
                .text(format!("{:.0}%", value * 100.0)),
        );
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use osrs_lab::{Brief, generate, generated_prefab, kit_objects};

    /// A model trained on one generated building, for tests of what uses models.
    pub(super) fn quick_model() -> osrs_lab::Model {
        let kit = starter_kit(0).unwrap_or_else(|e| panic!("{e}"));
        let objects = kit_objects(&kit);
        let brief = Brief {
            sketch: None,
            kind: "tavern".to_owned(),
            width: 16,
            height: 12,
            floors: 1,
            seed: 3,
            variation: 0,
            rooms: None,
        };
        let made = generate(&kit, &brief).unwrap_or_else(|e| panic!("{e}"));
        let prefab = generated_prefab(&made, "Tavern", "test", "");
        let mut running = job::Job::start(job::Inputs {
            id: "quick".to_owned(),
            name: "Quick houses".to_owned(),
            created: job::today(),
            prefabs: vec![("Tavern".to_owned(), prefab)],
            objects,
            base_kit: kit,
            seed: 1,
        });
        loop {
            if let Some(result) = running.poll() {
                return *result.unwrap_or_else(|e| panic!("training failed: {e}"));
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
    }

    /// The whole Train path without a window: a building goes in, the background trainer reports
    /// steps and finishes, the model is saved, listed, and builds a new building from a brief
    /// with no example (the same brief and number give the same building).
    #[test]
    fn training_in_the_background_saves_a_model_that_builds_from_a_brief() {
        let kit = starter_kit(0).unwrap_or_else(|e| panic!("{e}"));
        let objects = kit_objects(&kit);
        let brief = Brief {
            sketch: None,
            kind: "tavern".to_owned(),
            width: 16,
            height: 12,
            floors: 1,
            seed: 3,
            variation: 0,
            rooms: None,
        };
        let made = generate(&kit, &brief).unwrap_or_else(|e| panic!("{e}"));
        let prefab = generated_prefab(&made, "Tavern", "test", "");
        let dir = std::env::temp_dir().join(format!("rustosrs-train-ui-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut models = models::ModelList::in_directory(dir.clone());
        let id = models.free_id("Test houses");
        let mut running = job::Job::start(job::Inputs {
            id,
            name: "Test houses".to_owned(),
            created: job::today(),
            prefabs: vec![("Tavern".to_owned(), prefab)],
            objects,
            base_kit: kit,
            seed: 1,
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
        let file = models.save(&model).unwrap_or_else(|e| panic!("{e}"));
        models.reload();
        assert_eq!(models.entries.len(), 1);
        assert!(models.problems.is_empty(), "{:?}", models.problems);
        let ask = Brief {
            sketch: None,
            kind: "house".to_owned(),
            width: 12,
            height: 10,
            floors: 1,
            seed: 5,
            variation: 0,
            rooms: None,
        };
        let loaded = &models.entries[0].1;
        let first = osrs_lab::build_with_model(loaded, &ask, 4).unwrap_or_else(|e| panic!("{e}"));
        let again = osrs_lab::build_with_model(loaded, &ask, 4).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(
            first.generated.snapshot.objects,
            again.generated.snapshot.objects
        );
        assert!(first.generated.verdict.usable);
        models.trash(&file).unwrap_or_else(|e| panic!("{e}"));
        assert!(models.entries.is_empty());
        let _ = std::fs::remove_dir_all(dir);
    }
}
