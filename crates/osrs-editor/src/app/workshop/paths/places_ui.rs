//! Place names in the Paths tab: bring in a `map_labels.json`, name imported areas after the
//! nearest town, and import "the area around Lumbridge" in one click (the camera goes there, the
//! map loads, and the area is taken).

use super::{PathsState, Pending, import};
use crate::app::EditorApp;
use eframe::egui;
use osrs_core::coords::MapTile;
use osrs_lab::PathLesson;

/// How long to wait for the map to load before giving up, in frames.
const PATIENCE: u32 = 900;
/// Half the side of the area imported around a place, in tiles.
const HALF: i32 = 32;

/// Once per frame: take a place names file, and finish an import that waits for the map.
pub(super) fn poll(app: &mut EditorApp) {
    if let Some(task) = &mut app.prefabs.workshop.paths.import_places
        && let Some(result) = task.poll()
    {
        app.prefabs.workshop.paths.import_places = None;
        let places = &mut app.prefabs.workshop.places;
        match result {
            Ok(Some(file)) => {
                if let Err(error) = places.import(&file.text) {
                    places.problem = Some(format!("{}: {error}", file.name));
                }
            }
            Ok(None) => {}
            Err(error) => places.problem = Some(error),
        }
    }
    let Some(pending) = &mut app.prefabs.workshop.paths.pending else {
        return;
    };
    pending.waited += 1;
    let (name, x, y, waited) = (pending.name.clone(), pending.x, pending.y, pending.waited);
    let regions: std::collections::BTreeSet<_> =
        [(-HALF, -HALF), (HALF, -HALF), (-HALF, HALF), (HALF, HALF)]
            .iter()
            .map(|(dx, dy)| MapTile::new(x + dx, y + dy).split_region().0)
            .collect();
    let loaded: std::collections::BTreeSet<_> = app.session.available_regions().collect();
    if regions.iter().all(|r| loaded.contains(r)) {
        app.prefabs.workshop.paths.pending = None;
        let result = import::around_camera(app, 64, &name);
        super::keep(app, result);
    } else if waited > PATIENCE {
        let state = &mut app.prefabs.workshop.paths;
        state.pending = None;
        state.problem = Some(format!(
            "The map around {name} did not load. Try again, or fly there in the main window first."
        ));
    }
}

/// The place names row: import the file, or search and jump.
pub(super) fn section(app: &mut EditorApp, ui: &mut egui::Ui) {
    app.prefabs.workshop.places.ensure_loaded();
    if app.prefabs.workshop.places.items.is_empty() {
        ui.weak("Place names (towns, mines, dungeon mouths) name the areas you import and let you jump to a place. Bring in a map_labels.json file once and the editor keeps a copy.");
        if ui.button("Import place names...").clicked() {
            app.prefabs.workshop.paths.import_places = Some(crate::app::npc_runtime::open_text(
                "Load place names (map_labels.json)",
                "json",
            ));
        }
        if let Some(problem) = &app.prefabs.workshop.places.problem {
            ui.colored_label(ui.visuals().error_fg_color, problem);
        }
        return;
    }
    let count = app.prefabs.workshop.places.items.len();
    let query = app.prefabs.workshop.paths.place_query.clone();
    let hits: Vec<(String, i32, i32, u8)> = app
        .prefabs
        .workshop
        .places
        .search(&query, 6)
        .into_iter()
        .map(|p| (p.name.clone(), p.x, p.y, p.size))
        .collect();
    ui.horizontal_wrapped(|ui| {
        ui.label(format!("{count} place names. Find a place:"));
        ui.add(
            egui::TextEdit::singleline(&mut app.prefabs.workshop.paths.place_query)
                .hint_text("Lumbridge, Varrock...")
                .desired_width(160.0),
        );
    });
    let waiting = app.prefabs.workshop.paths.pending.is_some();
    for (name, x, y, size) in hits {
        ui.horizontal(|ui| {
            let kind = match size {
                2 => "region",
                1 => "town",
                _ => "site",
            };
            ui.label(format!("{name}  ({kind})"));
            if ui
                .add_enabled(!waiting, egui::Button::new("Import the area around it"))
                .on_hover_text(
                    "Moves the main map there, waits for it to load, and takes a 64 by 64 area",
                )
                .clicked()
            {
                app.teleport_to(crate::hud::Destination::Tile { x, y, plane: 0 });
                app.prefabs.workshop.paths.pending = Some(Pending {
                    name: format!("{name} ({x},{y})"),
                    x,
                    y,
                    waited: 0,
                });
            }
        });
    }
    if waiting {
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label("Loading the map there...");
        });
    }
}

/// The name for a new area: what was typed, else the nearest town and where the camera is.
pub(super) fn name_for(app: &mut EditorApp, typed: &str) -> String {
    if !typed.trim().is_empty() {
        return typed.trim().to_owned();
    }
    let (x, y) = app.camera_tile();
    app.prefabs.workshop.places.ensure_loaded();
    match app.prefabs.workshop.places.nearest(x, y, 1, 120) {
        Some(place) => format!("{} ({x},{y})", place.name),
        None => format!("Area near {x},{y}"),
    }
}

/// Give a lesson the place names around it (kept in its file).
pub(super) fn attach(state_places: &super::super::places::Places, lesson: &mut PathLesson) {
    lesson.places = state_places.marks_near(
        lesson.origin,
        (i32::from(lesson.width), i32::from(lesson.height)),
        200,
    );
}

/// Silence the unused-field lint on builds with no place names.
#[allow(dead_code)]
fn _state(_: &PathsState) {}
