//! The Paths tab: teach the editor to lay paths. Take areas of the map that have paths in them,
//! let the editor learn what makes a path go where it goes, then give it a blank copy of a place
//! and watch it draw the paths back, with the real ones to compare.

mod canvas;
mod import;
mod job;
mod lay;
pub(in crate::app) mod live;
mod places_ui;
mod store;
mod teach;
mod tryit;

use crate::app::EditorApp;
use eframe::egui;
use osrs_lab::{PathLesson, PathModel, PathScene};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use store::FileStore;

const LESSON_SUFFIX: &str = ".lesson.json";
const MODEL_SUFFIX: &str = ".pathmodel.json";

/// "Take me to Lumbridge and import what is there": waiting for the map to load.
struct Pending {
    name: String,
    x: i32,
    y: i32,
    waited: u32,
}

pub(super) struct PathsState {
    lessons: FileStore<PathLesson>,
    models: FileStore<PathModel>,
    starter: Option<Result<PathModel, String>>,
    scenes: BTreeMap<String, Arc<PathScene>>,
    selected: Option<String>,
    /// The model the Path tool uses (its file name, or `starter`).
    live_model: Option<String>,
    picked: BTreeSet<String>,
    size: usize,
    name: String,
    problem: Option<String>,
    /// A section to scroll to on the next frame (for scripts).
    scroll_to: Option<u8>,
    /// A place names file being picked.
    import_places: Option<crate::app::npc_runtime::OpenTask>,
    place_query: String,
    /// Importing the area around a place once the map there has loaded.
    pending: Option<Pending>,
    teach: teach::TeachState,
    try_it: tryit::TryState,
}

fn lesson_store() -> FileStore<PathLesson> {
    FileStore::new(
        "paths",
        LESSON_SUFFIX,
        PathLesson::from_json,
        |l| l.to_json(),
        |l| l.name.clone(),
    )
}

fn model_store() -> FileStore<PathModel> {
    FileStore::new(
        "models",
        MODEL_SUFFIX,
        PathModel::from_json,
        |m| m.to_json(),
        |m| m.created.clone(),
    )
}

impl Default for PathsState {
    fn default() -> Self {
        Self::with(lesson_store(), model_store())
    }
}

impl PathsState {
    fn with(lessons: FileStore<PathLesson>, models: FileStore<PathModel>) -> Self {
        Self {
            lessons,
            models,
            starter: None,
            scenes: BTreeMap::new(),
            selected: None,
            live_model: None,
            picked: BTreeSet::new(),
            size: 2,
            name: String::new(),
            problem: None,
            scroll_to: None,
            import_places: None,
            place_query: String::new(),
            pending: None,
            teach: teach::TeachState::default(),
            try_it: tryit::TryState::default(),
        }
    }

    /// The picture of a lesson (computed once, then remembered).
    fn scene(&mut self, file: &str) -> Option<Arc<PathScene>> {
        if let Some(scene) = self.scenes.get(file) {
            return Some(Arc::clone(scene));
        }
        let lesson = self.lessons.entries.iter().find(|(f, _)| f == file)?;
        let scene = Arc::new(PathScene::of(&lesson.1));
        self.scenes.insert(file.to_owned(), Arc::clone(&scene));
        Some(scene)
    }

    fn lesson(&self, file: &str) -> Option<&PathLesson> {
        self.lessons
            .entries
            .iter()
            .find(|(f, _)| f == file)
            .map(|(_, l)| l)
    }

    /// The model that comes with the editor, loaded once.
    fn starter(&mut self) -> Option<&PathModel> {
        self.starter
            .get_or_insert_with(PathModel::starter)
            .as_ref()
            .ok()
    }

    /// Every model that can be tried: yours first, then the one that comes with the editor.
    fn all_models(&mut self) -> Vec<(String, PathModel, bool)> {
        let mut out: Vec<(String, PathModel, bool)> = self
            .models
            .entries
            .iter()
            .map(|(f, m)| (f.clone(), m.clone(), false))
            .collect();
        if let Some(starter) = self.starter().cloned() {
            out.push(("starter".to_owned(), starter, true));
        }
        out
    }
}

/// Script commands for smoke tests: `import [SIZE]` the area around the camera, `train NAME` on all
/// areas, `try [alone]` the first model on the selected area.
pub(super) fn script(app: &mut EditorApp, words: &[&str]) -> Result<(), String> {
    app.prefabs.workshop.paths.lessons.ensure_loaded();
    app.prefabs.workshop.paths.models.ensure_loaded();
    match words {
        [] => {}
        ["import", rest @ ..] => {
            let size = rest
                .first()
                .map_or(Ok(64), |s| s.parse::<u16>().map_err(|e| e.to_string()))?;
            let name = default_name(app, "");
            let result = import::around_camera(app, size, &name);
            keep(app, result);
            if let Some(problem) = &app.prefabs.workshop.paths.problem {
                return Err(problem.clone());
            }
        }
        ["scroll", n] => app.prefabs.workshop.paths.scroll_to = n.parse().ok(),
        ["train", name] => teach::begin(&mut app.prefabs.workshop.paths, name)?,
        ["try", rest @ ..] => {
            let mode = if rest.first() == Some(&"alone") {
                osrs_lab::PathMode::Alone
            } else {
                osrs_lab::PathMode::Given
            };
            tryit::run(&mut app.prefabs.workshop.paths, mode)?;
        }
        other => return Err(format!("unknown paths command {other:?}")),
    }
    Ok(())
}

/// Once per frame: take what the trainer has said, and keep the model when it finishes.
pub(super) fn poll(app: &mut EditorApp) {
    teach::poll(&mut app.prefabs.workshop.paths);
    places_ui::poll(app);
}

pub(super) fn show(app: &mut EditorApp, ui: &mut egui::Ui) {
    app.prefabs.workshop.paths.lessons.ensure_loaded();
    app.prefabs.workshop.paths.models.ensure_loaded();
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.weak(
            "Roads and trails are not random: they join doors, skirt buildings, avoid water and \
             steep ground. Here you show the editor real paths from the map, it learns what they \
             have in common, and then it draws paths on a blank copy of a place it has never seen, \
             so you can check it against the real ones. To use a model on the real map, pick the Path tool and press \"Let the AI route it\" in its AI helper.",
        );
            lessons_section(app, ui);
            teach::show(app, ui);
            tryit::show(app, ui);
        });
}

pub(super) fn heading(ui: &mut egui::Ui, scroll: &mut Option<u8>, number: u8, text: &str) {
    ui.add_space(8.0);
    if *scroll == Some(number) {
        ui.scroll_to_cursor(Some(egui::Align::TOP));
        *scroll = None;
    }
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(format!("{number}")).strong().size(18.0));
        ui.label(egui::RichText::new(text).strong().size(16.0));
    });
}

fn lessons_section(app: &mut EditorApp, ui: &mut egui::Ui) {
    heading(
        ui,
        &mut app.prefabs.workshop.paths.scroll_to,
        1,
        "Real paths to learn from",
    );
    ui.weak(
        "Pick an area of the map that has roads or trails, ideally with buildings and doors. The \
         editor saves a copy of the ground, the heights and the objects there, and notes which \
         floor types are paths.",
    );
    places_ui::section(app, ui);
    ui.horizontal_wrapped(|ui| {
        let state = &mut app.prefabs.workshop.paths;
        ui.label("Take the area around the camera,");
        egui::ComboBox::from_id_salt("paths-size")
            .selected_text(format!("{0} by {0} tiles", import::SIZES[state.size]))
            .show_ui(ui, |ui| {
                for (i, size) in import::SIZES.iter().enumerate() {
                    ui.selectable_value(&mut state.size, i, format!("{size} by {size} tiles"));
                }
            });
        ui.add(
            egui::TextEdit::singleline(&mut state.name)
                .hint_text("a name (optional)")
                .desired_width(160.0),
        );
    });
    ui.horizontal_wrapped(|ui| {
        if ui.button("Import the area around the camera").clicked() {
            let (size, name) = {
                let state = &app.prefabs.workshop.paths;
                (import::SIZES[state.size], state.name.clone())
            };
            let name = default_name(app, &name);
            let result = import::around_camera(app, size, &name);
            keep(app, result);
        }
        let has_selection = !app.interaction.selection_tiles().is_empty();
        if ui
            .add_enabled(
                has_selection,
                egui::Button::new("Import the tiles I selected"),
            )
            .on_disabled_hover_text("Select tiles on the map first (Select tool, tile mode)")
            .clicked()
        {
            let name = default_name(app, &app.prefabs.workshop.paths.name.clone());
            let result = import::around_selection(app, &name);
            keep(app, result);
        }
    });
    if let Some(problem) = &app.prefabs.workshop.paths.problem {
        ui.colored_label(ui.visuals().error_fg_color, problem);
    }
    let rows: Vec<(String, String, usize, usize, Option<String>)> = {
        let state = &mut app.prefabs.workshop.paths;
        let files: Vec<String> = state
            .lessons
            .entries
            .iter()
            .map(|(f, _)| f.clone())
            .collect();
        files
            .into_iter()
            .filter_map(|file| {
                let scene = state.scene(&file)?;
                let lesson = state.lesson(&file)?;
                let tiles = scene.mask.iter().filter(|m| **m).count();
                Some((
                    file,
                    lesson.name.clone(),
                    tiles,
                    scene.door_fronts.len(),
                    import::caveat(lesson, tiles),
                ))
            })
            .collect()
    };
    if rows.is_empty() {
        ui.weak("No areas yet. Fly to a town or a road on the map and press the button above.");
    }
    let mut delete: Option<String> = None;
    for (file, name, tiles, doors, caveat) in &rows {
        let state = &mut app.prefabs.workshop.paths;
        ui.horizontal(|ui| {
            let mut on = state.picked.contains(file);
            if ui
                .checkbox(&mut on, "")
                .on_hover_text("Learn from this area")
                .changed()
            {
                if on {
                    state.picked.insert(file.clone())
                } else {
                    state.picked.remove(file)
                };
            }
            let selected = state.selected.as_deref() == Some(file.as_str());
            if ui
                .selectable_label(
                    selected,
                    format!("{name}  \u{b7}  {tiles} path tiles  \u{b7}  {doors} doors"),
                )
                .clicked()
            {
                state.selected = if selected { None } else { Some(file.clone()) };
            }
            if ui
                .small_button("Delete")
                .on_hover_text("Moves the file to the paths/.trash folder")
                .clicked()
            {
                delete = Some(file.clone());
            }
        });
        if let Some(caveat) = caveat {
            ui.weak(format!("      {caveat}"));
        }
    }
    if let Some(file) = delete {
        let state = &mut app.prefabs.workshop.paths;
        if let Err(error) = state.lessons.trash(&file) {
            state.problem = Some(error);
        }
        state.picked.remove(&file);
        state.scenes.remove(&file);
        if state.selected.as_deref() == Some(file.as_str()) {
            state.selected = None;
        }
    }
    selected_detail(app, ui);
}

fn default_name(app: &mut EditorApp, typed: &str) -> String {
    places_ui::name_for(app, typed)
}

fn keep(app: &mut EditorApp, result: Result<PathLesson, String>) {
    match result {
        Ok(mut lesson) => {
            places_ui::attach(&app.prefabs.workshop.places, &mut lesson);
            let state = &mut app.prefabs.workshop.paths;
            let id = state.lessons.free_id(&lesson.name, "area");
            match state.lessons.save(&id, &lesson) {
                Ok(file) => {
                    state.lessons.keep(file.clone(), lesson);
                    state.picked.insert(file.clone());
                    state.selected = Some(file);
                    state.problem = None;
                    state.name.clear();
                }
                Err(error) => state.problem = Some(error),
            }
        }
        Err(error) => app.prefabs.workshop.paths.problem = Some(error),
    }
}

/// The picture of the selected area, and which of its floor types count as paths.
fn selected_detail(app: &mut EditorApp, ui: &mut egui::Ui) {
    let state = &mut app.prefabs.workshop.paths;
    let Some(file) = state.selected.clone() else {
        return;
    };
    let (Some(scene), Some(lesson)) = (state.scene(&file), state.lesson(&file).cloned()) else {
        return;
    };
    ui.add_space(4.0);
    ui.group(|ui| {
        ui.strong(&lesson.name);
        ui.horizontal_top(|ui| {
            canvas::show(ui, &scene, 360.0, &canvas::Layers { real: true, ..Default::default() });
            ui.vertical(|ui| {
                canvas::legend(ui, true, false, false);
                ui.add_space(6.0);
                ui.label(egui::RichText::new("Which floors are paths?").strong());
                ui.weak("The editor guessed from their shape (long and thin). Change it if it is wrong.");
                path_chips(ui, state, &file, &lesson, &scene);
            });
        });
    });
}

fn path_chips(
    ui: &mut egui::Ui,
    state: &mut PathsState,
    file: &str,
    lesson: &PathLesson,
    scene: &PathScene,
) {
    let inside: BTreeSet<usize> = (0..scene.tiles()).filter(|a| scene.inside[*a]).collect();
    let chosen = lesson.path_overlay_set(&inside);
    let guessed: BTreeSet<u16> = lesson
        .guess_paths(&inside)
        .into_iter()
        .map(|g| g.overlay)
        .collect();
    let mut counts: BTreeMap<u16, usize> = BTreeMap::new();
    for overlay in lesson.overlay.iter().filter(|o| **o != 0) {
        *counts.entry(*overlay).or_default() += 1;
    }
    let mut rows: Vec<(u16, usize)> = counts.into_iter().collect();
    rows.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
    let mut next = chosen.clone();
    for (overlay, tiles) in rows.into_iter().take(12) {
        let mut on = chosen.contains(&overlay);
        let label = format!(
            "floor type {overlay}  ({tiles} tiles){}",
            if guessed.contains(&overlay) {
                "  \u{2013} looks like a path"
            } else {
                ""
            }
        );
        if ui.checkbox(&mut on, label).changed() {
            if on {
                next.insert(overlay)
            } else {
                next.remove(&overlay)
            };
        }
    }
    if next != chosen {
        let list: Vec<u16> = next.into_iter().collect();
        match state.lessons.edit(file, |l| {
            l.path_overlays = if list.is_empty() { vec![0] } else { list }
        }) {
            Ok(()) => {
                state.scenes.remove(file);
            }
            Err(error) => state.problem = Some(error),
        }
    }
}

#[cfg(test)]
mod tests;
