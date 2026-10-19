//! The Buildings tab: the selected building, what it is, and what can be done with it.

use crate::app::{EditorApp, prefab_panel};
use eframe::egui;
use osrs_core::object_names::NameSource;
use osrs_edit::PrefabSource;
use std::collections::BTreeSet;

/// What is known about the selected building, and what can be done with it. Drawn under the
/// list in the Scene tab's Library page.
pub(super) fn show(app: &mut EditorApp, ui: &mut egui::Ui) {
    let Some(file) = app.prefabs.selected.clone() else {
        ui.weak("Pick a building above to see its details.");
        return;
    };
    let Some(prefab) = app.prefabs.entry(&file).map(|e| e.prefab.clone()) else {
        ui.weak("That building is no longer in the library.");
        return;
    };
    ui.label(egui::RichText::new(&prefab.name).strong().size(16.0));
    ui.weak(prefab.summary());
    ui.weak(source_sentence(&prefab.source));
    let editing = app.prefabs.workshop.editing.as_deref() == Some(file.as_str());
    ui.horizontal_wrapped(|ui| {
        if ui
            .add_enabled(!editing, egui::Button::new("Edit it in the scene"))
            .on_hover_text(if editing {
                "This building is open in the scene now"
            } else {
                "Open the building as its own scene, change it, and save it back (or double-click it in the list)"
            })
            .clicked()
        {
            app.prefabs.studio_request = Some(file.clone());
        }
        if ui
            .button("Place it on the map")
            .on_hover_text("Then click the map in the main window. R turns it, Esc stops")
            .clicked()
        {
            prefab_panel::arm(app, &file);
        }
    });
    ui.add_space(4.0);
    ui.label("Name");
    if let Some(name) = prefab_panel::text_field(ui, "ws-name", &prefab.name, false)
        && let Err(error) = app.prefabs.edit(&file, |p| p.name = name.trim().to_owned())
    {
        app.prefabs.say(error);
    }
    ui.label("Tags (separated by commas)");
    if let Some(tags) = prefab_panel::text_field(ui, "ws-tags", &prefab.tags.join(", "), false) {
        let tags: Vec<String> = tags
            .split(',')
            .map(|t| t.trim().to_lowercase())
            .filter(|t| !t.is_empty())
            .collect();
        if let Err(error) = app.prefabs.edit(&file, |p| p.tags = tags) {
            app.prefabs.say(error);
        }
    }
    ui.label("Notes");
    if let Some(notes) = prefab_panel::text_field(ui, "ws-notes", &prefab.notes, true)
        && let Err(error) = app.prefabs.edit(&file, |p| p.notes = notes)
    {
        app.prefabs.say(error);
    }
    ui.add_space(4.0);
    naming_meter(app, ui, &prefab);
    ui.add_space(4.0);
    ui.horizontal_wrapped(|ui| {
        if ui.button("Make a copy").clicked()
            && let Err(error) = app.prefabs.duplicate(&file)
        {
            app.prefabs.say(error);
        }
        if ui
            .button("Save to a file")
            .on_hover_text("Share the building as a .prefab.json file")
            .clicked()
        {
            app.prefabs.start_export(&file);
        }
    });
    if let Some(notice) = app.prefabs.notice().map(str::to_owned) {
        ui.horizontal_wrapped(|ui| {
            ui.label(notice);
            if ui.small_button("x").clicked() {
                app.prefabs.clear_notice();
            }
        });
    }
}

fn source_sentence(source: &PrefabSource) -> String {
    match source {
        PrefabSource::Captured {
            tile_x,
            tile_y,
            plane,
        } => format!("Copied from the map at {tile_x}, {tile_y} (floor {plane})."),
        PrefabSource::Generated {
            archetype,
            style,
            seed,
            floors,
        } => format!(
            "Built by the editor: a {archetype}, style \"{style}\", {floors} floor(s), plan number {seed}."
        ),
        PrefabSource::Authored => "Made by hand.".to_owned(),
    }
}

/// "3 of 12 kinds of objects in this building have a name", with a way to the Teach tab.
fn naming_meter(app: &mut EditorApp, ui: &mut egui::Ui, prefab: &osrs_edit::Prefab) {
    let ids: BTreeSet<u32> = prefab.objects.iter().map(|o| o.id).collect();
    if ids.is_empty() {
        return;
    }
    let named = ids
        .iter()
        .filter(|id| {
            let cache = app
                .placer
                .catalog_entry(**id)
                .map(|e| e.name.clone())
                .unwrap_or_default();
            app.names.resolve(**id, Some(&cache)).source != NameSource::Unnamed
        })
        .count();
    ui.horizontal(|ui| {
        ui.add(
            egui::ProgressBar::new(named as f32 / ids.len() as f32)
                .desired_width(160.0)
                .text(format!("{named} of {} named", ids.len())),
        );
        ui.weak("kinds of objects in this building have a name.");
        if named < ids.len() && ui.small_button("Name the rest").clicked() {
            app.prefabs.workshop.tab = super::Tab::Names;
        }
    });
}
