//! The Teach tab: tell the editor what the objects in a building are.
//!
//! Most objects in the game have no name. A name (and what the object is for) is remembered for
//! every building and prefab, and it changes what the editor can understand: a door is a way
//! through a wall, a bed makes a room a bedroom, and the AI can learn what goes where.

use crate::app::EditorApp;
use eframe::egui;
use osrs_core::object_names::{NameSource, ROLES, Resolved, Witness};
use osrs_scene::building_census::{Guess, guess_objects};
use std::collections::BTreeMap;

#[derive(Default)]
pub(super) struct TeachState {
    /// What the rows were made from: the building and the name-book generation.
    made_from: Option<(String, usize, u64)>,
    rows: Vec<Row>,
    show_named: bool,
    limit: usize,
}

struct Row {
    id: u32,
    count: usize,
    kind: u8,
    cache_name: String,
    symbol: String,
    guess: Option<Guess>,
}

/// What a kind of placement is, in plain words.
fn kind_words(kind: u8) -> &'static str {
    match kind {
        0 => "wall piece",
        1 | 3 => "pillar",
        2 => "wall corner",
        4..=8 => "wall decoration",
        9 => "diagonal wall",
        10 => "standing object",
        11 => "diagonal object",
        12..=21 => "roof piece",
        22 => "floor decoration",
        _ => "object",
    }
}

pub(super) fn show(app: &mut EditorApp, ui: &mut egui::Ui) {
    ui.group(|ui| {
        ui.strong("Why name things?");
        ui.label(
            "Most objects in the game have no name. When you say what an object is, the editor \
             understands buildings better: a door becomes a way through a wall, a bed makes a room \
             a bedroom, and the AI can learn which things go where. A name you give is remembered \
             for every building that uses that object.",
        );
    });
    ui.add_space(6.0);
    let Some(file) = app.prefabs.selected.clone() else {
        ui.weak("Pick a building on the left to see its objects.");
        return;
    };
    refresh(app, &file);
    let named = app
        .prefabs
        .workshop
        .teach
        .rows
        .iter()
        .filter(|row| resolved(app, row).source != NameSource::Unnamed)
        .count();
    let total = app.prefabs.workshop.teach.rows.len();
    ui.horizontal(|ui| {
        ui.add(
            egui::ProgressBar::new(named as f32 / total.max(1) as f32)
                .desired_width(180.0)
                .text(format!("{named} of {total} named")),
        );
        ui.weak("kinds of objects in this building");
    });
    ui.horizontal_wrapped(|ui| {
        let teach = &mut app.prefabs.workshop.teach;
        ui.checkbox(
            &mut teach.show_named,
            "Also show the ones that already have a name",
        );
        let sure: Vec<(u32, Guess)> = teach
            .rows
            .iter()
            .filter_map(|r| r.guess.clone().map(|g| (r.id, g)))
            .filter(|(_, g)| g.confidence >= 60)
            .collect();
        if ui
            .add_enabled(
                !sure.is_empty(),
                egui::Button::new(format!("Accept {} sure guess(es)", sure.len())),
            )
            .on_hover_text("Accepts every guess the editor is at least 60% sure about")
            .clicked()
        {
            for (id, guess) in sure {
                accept(app, id, &guess);
            }
        }
    });
    if let Some(problem) = app.names.custom().problem() {
        ui.colored_label(ui.visuals().error_fg_color, problem);
    }
    ui.separator();
    rows(app, ui);
    ui.add_space(8.0);
    explainer(ui);
}

/// Rebuild the rows when the building or the names changed.
fn refresh(app: &mut EditorApp, file: &str) {
    let generation = app.names.custom().generation();
    let Some(prefab) = app.prefabs.entry(file).map(|e| e.prefab.clone()) else {
        return;
    };
    let key = (file.to_owned(), prefab.objects.len(), generation);
    if app.prefabs.workshop.teach.made_from.as_ref() == Some(&key) {
        return;
    }
    let mut by_id: BTreeMap<u32, (usize, u8)> = BTreeMap::new();
    for object in &prefab.objects {
        let entry = by_id.entry(object.id).or_insert((0, object.kind));
        entry.0 += 1;
    }
    // The names the census reads: the game's, plus anything you said.
    let (table, raw) = super::table::table_for(app, &[&prefab]);
    let mut cache: BTreeMap<u32, (String, String)> = raw
        .into_iter()
        .map(|(id, name, symbol)| (id, (name, symbol)))
        .collect();
    let buildings = osrs_lab::prefab_buildings(std::slice::from_ref(&prefab), &table);
    let guesses: BTreeMap<u32, Guess> = guess_objects(&buildings, &table)
        .into_iter()
        .map(|g| (g.object.get(), g))
        .collect();
    let mut rows: Vec<Row> = by_id
        .into_iter()
        .map(|(id, (count, kind))| {
            let (cache_name, symbol) = cache.remove(&id).unwrap_or_default();
            Row {
                id,
                count,
                kind,
                cache_name,
                symbol,
                guess: guesses.get(&id).cloned(),
            }
        })
        .collect();
    rows.sort_by(|a, b| b.count.cmp(&a.count).then(a.id.cmp(&b.id)));
    let teach = &mut app.prefabs.workshop.teach;
    teach.rows = rows;
    teach.made_from = Some(key);
    if teach.limit == 0 {
        teach.limit = 40;
    }
}

fn resolved(app: &EditorApp, row: &Row) -> Resolved {
    app.names.resolve(row.id, Some(&row.cache_name))
}

fn witness(row_cache: &str, symbol: &str) -> Witness {
    Witness {
        cache_name: row_cache.to_owned(),
        symbol: symbol.to_owned(),
    }
}

fn accept(app: &mut EditorApp, id: u32, guess: &Guess) {
    let (cache, symbol) = app
        .prefabs
        .workshop
        .teach
        .rows
        .iter()
        .find(|r| r.id == id)
        .map(|r| (r.cache_name.clone(), r.symbol.clone()))
        .unwrap_or_default();
    app.names.custom_mut().change(|book| {
        book.accept_guess(
            id,
            &guess.label,
            Some(&guess.role),
            guess.confidence,
            witness(&cache, &symbol),
        );
    });
}

fn rows(app: &mut EditorApp, ui: &mut egui::Ui) {
    let show_named = app.prefabs.workshop.teach.show_named;
    let limit = app.prefabs.workshop.teach.limit;
    let ids: Vec<u32> = app
        .prefabs
        .workshop
        .teach
        .rows
        .iter()
        .filter(|row| show_named || resolved(app, row).source == NameSource::Unnamed)
        .take(limit)
        .map(|row| row.id)
        .collect();
    if ids.is_empty() {
        ui.weak("Every kind of object in this building has a name.");
        return;
    }
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .max_height((ui.available_height() - 150.0).max(160.0))
        .show(ui, |ui| {
            for id in ids {
                row(app, ui, id);
                ui.separator();
            }
            let total = app.prefabs.workshop.teach.rows.len();
            if limit < total && ui.button("Show more").clicked() {
                app.prefabs.workshop.teach.limit += 40;
            }
        });
}

fn row(app: &mut EditorApp, ui: &mut egui::Ui, id: u32) {
    let Some(row) = app.prefabs.workshop.teach.rows.iter().find(|r| r.id == id) else {
        return;
    };
    let (count, kind, cache, symbol, guess) = (
        row.count,
        row.kind,
        row.cache_name.clone(),
        row.symbol.clone(),
        row.guess.clone(),
    );
    let now = app.names.resolve(id, Some(&cache));
    ui.horizontal(|ui| {
        ui.strong(if now.text.is_empty() {
            format!("Object {id}")
        } else {
            now.text.clone()
        });
        ui.weak(format!(
            "{} \u{b7} {count} in this building \u{b7} {}",
            kind_words(kind),
            now.source.label()
        ));
    });
    if let Some(guess) = guess.filter(|_| now.source == NameSource::Unnamed) {
        ui.horizontal_wrapped(|ui| {
            ui.label(format!(
                "A guess ({}% sure): {}.",
                guess.confidence, guess.label
            ));
            if ui.small_button("Accept").clicked() {
                accept(app, id, &guess);
            }
        });
        ui.weak(&guess.why);
    }
    ui.horizontal(|ui| {
        let key = ui.id().with(("teach-name", id));
        let own = app
            .names
            .custom()
            .book()
            .get(id)
            .and_then(|e| e.name.clone())
            .unwrap_or_default();
        let mut text = ui
            .data_mut(|d| d.get_temp::<(String, String)>(key))
            .filter(|(origin, _)| *origin == own)
            .map_or_else(|| own.clone(), |(_, text)| text);
        let response = ui.add(
            egui::TextEdit::singleline(&mut text)
                .hint_text("Type a name")
                .desired_width(200.0),
        );
        if response.changed() {
            ui.data_mut(|d| d.insert_temp(key, (own.clone(), text.clone())));
        }
        if response.lost_focus() {
            ui.data_mut(|d| d.remove::<(String, String)>(key));
            if text != own {
                app.names
                    .custom_mut()
                    .name_object(id, &text, witness(&cache, &symbol));
            }
        }
        let role = app
            .names
            .custom()
            .book()
            .get(id)
            .and_then(|e| e.role.clone())
            .unwrap_or_default();
        let mut chosen = role.clone();
        egui::ComboBox::from_id_salt(("teach-role", id))
            .selected_text(if role.is_empty() {
                "what is it?"
            } else {
                role.as_str()
            })
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut chosen, String::new(), "(not set)");
                for option in ROLES {
                    ui.selectable_value(&mut chosen, (*option).to_owned(), *option);
                }
            });
        if chosen != role {
            app.names
                .custom_mut()
                .change(|book| book.set_role(id, &chosen, witness(&cache, &symbol)));
        }
    });
}

/// What the floor-pattern solver (WFC) can and cannot see, in plain words.
fn explainer(ui: &mut egui::Ui) {
    egui::CollapsingHeader::new("What can the editor's pattern builder see?")
        .id_salt("workshop-wfc")
        .show(ui, |ui| {
            ui.label(
                "When the editor lays out floors from patterns it learned, it works with tiles \
                 that have four labelled edges (what is on the north, east, south and west side) \
                 and a weight (how common that tile was). It only places two tiles side by side \
                 when it saw that happen in your buildings.",
            );
            ui.add_space(4.0);
            ui.strong("It cannot see");
            ui.label(
                "\u{2022} what objects are (a bed and a barrel are the same to it)\n\
                 \u{2022} which room is a kitchen or a bank\n\
                 \u{2022} the difference between a door and a window\n\
                 \u{2022} the outline of the building it is filling",
            );
            ui.add_space(4.0);
            ui.weak(
                "That is why names matter: names and roles are how this knowledge reaches the \
                 editor, and the Train tab uses them.",
            );
        });
}
