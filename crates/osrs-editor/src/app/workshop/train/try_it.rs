//! Step 4: ask a model for a new building. No example is involved: a brief (what kind, how big)
//! goes in, the model builds several candidates and keeps the one its judge likes best.

use crate::app::EditorApp;
use eframe::egui;
use osrs_lab::{Brief, Built, build_with_model, generated_prefab};

const KINDS: [(&str, &str); 6] = [
    ("house", "House"),
    ("shop", "Shop"),
    ("tavern", "Tavern"),
    ("bank", "Bank"),
    ("church", "Church"),
    ("warehouse", "Warehouse"),
];

pub(super) struct TryState {
    /// The file of the model chosen.
    model: Option<String>,
    kind: usize,
    width: u16,
    height: u16,
    floors: u8,
    seed: u64,
    tries: u32,
    result: Option<Result<Built, String>>,
    /// What was said about the last result.
    notice: Option<String>,
}

impl Default for TryState {
    fn default() -> Self {
        Self {
            model: None,
            kind: 0,
            width: 12,
            height: 10,
            floors: 1,
            seed: 1,
            tries: 8,
            result: None,
            notice: None,
        }
    }
}

impl TryState {
    pub(super) fn choose(&mut self, file: &str) {
        self.model = Some(file.to_owned());
        self.result = None;
    }
}

pub(super) fn show(app: &mut EditorApp, ui: &mut egui::Ui) {
    super::heading(ui, 4, "Ask a model to build something new");
    let train = &mut app.prefabs.workshop.train;
    if train.models.entries.is_empty() {
        ui.weak("Train a model first (step 3). It will appear here.");
        problems(app, ui);
        return;
    }
    models_list(app, ui);
    ui.group(|ui| {
        ui.label("To use a model on the map: open the Builder, pick the kit called \"... (trained model)\", choose a site on the map and apply. It is also in the main window: Prefabs panel, Build page, Kit.");
        if ui.button("Open the Builder").clicked() {
            crate::app::workshop::open_builder(app);
        }
    });
    let train = &mut app.prefabs.workshop.train;
    let Some(file) = train
        .try_it
        .model
        .clone()
        .or_else(|| train.models.entries.first().map(|(f, _)| f.clone()))
    else {
        return;
    };
    train.try_it.model.get_or_insert_with(|| file.clone());
    ui.add_space(4.0);
    let try_it = &mut train.try_it;
    ui.horizontal_wrapped(|ui| {
        ui.label("Build a");
        egui::ComboBox::from_id_salt("try-kind")
            .selected_text(KINDS[try_it.kind.min(KINDS.len() - 1)].1)
            .show_ui(ui, |ui| {
                for (index, (_, label)) in KINDS.iter().enumerate() {
                    ui.selectable_value(&mut try_it.kind, index, *label);
                }
            });
        ui.label("this wide and deep:");
        ui.add(
            egui::DragValue::new(&mut try_it.width)
                .range(8..=24)
                .suffix(" tiles"),
        );
        ui.label("\u{d7}");
        ui.add(
            egui::DragValue::new(&mut try_it.height)
                .range(8..=24)
                .suffix(" tiles"),
        );
        ui.label("floors");
        ui.add(egui::DragValue::new(&mut try_it.floors).range(1..=2));
    });
    ui.horizontal_wrapped(|ui| {
        ui.label("Number");
        ui.add(egui::DragValue::new(&mut try_it.seed).range(0..=999_999))
            .on_hover_text("Different numbers give different buildings; the same number always gives the same one");
        if ui.small_button("Another").clicked() {
            try_it.seed += 1;
        }
        ui.label("Tries");
        ui.add(egui::Slider::new(&mut try_it.tries, 1..=32))
            .on_hover_text("The model builds this many and keeps the one that looks most real");
    });
    if ui.button("Build it").clicked() {
        build(app, &file);
    }
    problems(app, ui);
    result(app, ui, &file);
}

fn problems(app: &mut EditorApp, ui: &mut egui::Ui) {
    let models = &app.prefabs.workshop.train.models;
    for problem in &models.problems {
        ui.colored_label(ui.visuals().warn_fg_color, problem);
    }
}

fn models_list(app: &mut EditorApp, ui: &mut egui::Ui) {
    let mut delete: Option<String> = None;
    let train = &mut app.prefabs.workshop.train;
    ui.label("Your models");
    for (file, model) in &train.models.entries {
        ui.horizontal(|ui| {
            let selected = train.try_it.model.as_deref() == Some(file.as_str());
            if ui.radio(selected, "").clicked() {
                train.try_it.choose(file);
            }
            ui.vertical(|ui| {
                ui.strong(&model.name);
                ui.weak(format!(
                    "{} \u{b7} learned from {} building(s) \u{b7} ranking score {:.3} (best of 16) \u{b7} {:.0}% usable",
                    model.created,
                    model.trained_on.len(),
                    model.metrics.realism_best_of_16,
                    model.metrics.usable_rate * 100.0
                ));
            });
            if ui.small_button("Delete").on_hover_text("Moves the file to the models/.trash folder").clicked() {
                delete = Some(file.clone());
            }
        });
    }
    if let Some(file) = delete {
        let train = &mut app.prefabs.workshop.train;
        if let Err(error) = train.models.trash(&file) {
            train.problem = Some(error);
        }
        if train.try_it.model.as_deref() == Some(file.as_str()) {
            train.try_it.model = None;
            train.try_it.result = None;
        }
    }
}

fn build(app: &mut EditorApp, file: &str) {
    let train = &mut app.prefabs.workshop.train;
    let Some((_, model)) = train.models.entries.iter().find(|(f, _)| f == file) else {
        return;
    };
    let t = &train.try_it;
    let brief = Brief {
        sketch: None,
        kind: KINDS[t.kind.min(KINDS.len() - 1)].0.to_owned(),
        width: t.width,
        height: t.height,
        floors: t.floors,
        seed: t.seed,
        variation: 0,
        rooms: None,
    };
    let result = build_with_model(model, &brief, t.tries);
    let train = &mut app.prefabs.workshop.train;
    train.try_it.result = Some(result);
    train.try_it.notice = None;
}

fn result(app: &mut EditorApp, ui: &mut egui::Ui, file: &str) {
    let Some(result) = app.prefabs.workshop.train.try_it.result.clone() else {
        return;
    };
    let built = match result {
        Ok(built) => built,
        Err(error) => {
            ui.colored_label(
                ui.visuals().error_fg_color,
                format!("It could not build that: {error}"),
            );
            ui.weak("Try a different size or number.");
            return;
        }
    };
    let Some(model) = app
        .prefabs
        .workshop
        .train
        .models
        .entries
        .iter()
        .find(|(f, _)| f == file)
        .map(|(_, m)| m.clone())
    else {
        return;
    };
    let verdict = &built.generated.verdict;
    let name = format!(
        "{} {} #{}",
        model.name,
        KINDS
            .iter()
            .find(|(k, _)| *k == built.generated.brief.kind)
            .map_or("building", |(_, l)| *l),
        built.generated.brief.seed
    );
    let prefab = generated_prefab(
        &built.generated,
        &name,
        &model.name,
        &format!("Built by the model \"{}\" (no example).", model.name),
    );
    ui.add_space(4.0);
    ui.group(|ui| {
        ui.horizontal_top(|ui| {
            crate::app::prefab_panel::preview::show(ui, &prefab, 200.0);
            ui.vertical(|ui| {
                ui.strong(&name);
                ui.label(format!(
                    "{} room(s), {} way(s) in. Ranking score {:.3} (uncalibrated).",
                    verdict.rooms, verdict.entrances, built.realism
                ));
                ui.weak(format!(
                    "It tried {} seed(s) and kept the best; {} could not be used.",
                    built.tried.len() + built.rejected,
                    built.rejected
                ));
                ui.horizontal_wrapped(|ui| {
                    if ui.button("Keep it as a building").clicked() {
                        keep(app, &prefab, false);
                    }
                    if ui
                        .button("Keep it and open it in the scene editor")
                        .clicked()
                    {
                        keep(app, &prefab, true);
                    }
                });
                ui.horizontal_wrapped(|ui| {
                    ui.label("Is this a good building?");
                    if ui.small_button("Yes").clicked() {
                        rate(app, file, &built, "good");
                    }
                    if ui.small_button("Not really").clicked() {
                        rate(app, file, &built, "poor");
                    }
                });
            });
        });
        if let Some(notice) = app.prefabs.workshop.train.try_it.notice.clone() {
            ui.label(notice);
        }
    });
}

fn keep(app: &mut EditorApp, prefab: &osrs_edit::Prefab, open: bool) {
    let notice = match app.prefabs.add(prefab.clone()) {
        Ok(file) => {
            if open {
                app.prefabs.studio_request = Some(file);
            }
            "Saved in your buildings.".to_owned()
        }
        Err(error) => error,
    };
    app.prefabs.workshop.train.try_it.notice = Some(notice);
}

fn rate(app: &mut EditorApp, file: &str, built: &Built, rating: &str) {
    let id = file.strip_suffix(".model.json").unwrap_or(file);
    let line = serde_json::json!({
        "model": id,
        "brief": built.generated.brief,
        "realism": built.realism,
        "rating": rating,
    })
    .to_string();
    let train = &mut app.prefabs.workshop.train;
    train.try_it.notice = Some(match train.models.rate(id, &line) {
        Ok(()) => "Thanks. Your answer is saved with the model.".to_owned(),
        Err(error) => error,
    });
}
