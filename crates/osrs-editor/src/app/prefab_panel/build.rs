//! The Build page: choose a kit, size and seed, put the site on the map, look at the ghost, then
//! apply the building as one undoable step or keep it in the library.

use crate::app::{
    EditorApp,
    prefab_runtime::{ARCHETYPES, MAX_SITE, STYLES},
};
use eframe::egui;

pub(super) fn show(app: &mut EditorApp, ui: &mut egui::Ui) {
    let accent = ui.visuals().selection.stroke.color;
    ui.weak("Plans rooms, doors, windows and a roof for the site, using the chosen kit's objects. One floor for now.");
    kit_picker(app, ui);

    let form = &mut app.prefabs.build.form;
    egui::ComboBox::from_label("Kind")
        .selected_text(
            ARCHETYPES
                .iter()
                .find(|(a, _)| *a == form.archetype)
                .map_or("", |(_, label)| *label),
        )
        .show_ui(ui, |ui| {
            for (archetype, label) in ARCHETYPES {
                ui.selectable_value(&mut form.archetype, archetype, label);
            }
        });
    egui::ComboBox::from_label("Style")
        .selected_text(
            STYLES
                .iter()
                .find(|(s, _)| *s == form.style)
                .map_or("", |(_, label)| *label),
        )
        .show_ui(ui, |ui| {
            for (style, label) in STYLES {
                ui.selectable_value(&mut form.style, style, label);
            }
        });
    ui.horizontal(|ui| {
        ui.label("Site");
        ui.add(
            egui::DragValue::new(&mut form.width)
                .range(6..=MAX_SITE)
                .suffix(" wide"),
        );
        ui.add(
            egui::DragValue::new(&mut form.height)
                .range(6..=MAX_SITE)
                .suffix(" deep"),
        );
    });
    let mut reroll = false;
    ui.horizontal(|ui| {
        ui.label("Seed");
        ui.add(egui::DragValue::new(&mut form.seed));
        if ui
            .button(format!("{} Reroll", egui_phosphor::regular::DICE_FIVE))
            .on_hover_text("A new layout from the next seed in the sequence")
            .clicked()
        {
            reroll = true;
        }
    });
    if reroll {
        app.prefabs.build.reroll();
    }
    let form = &mut app.prefabs.build.form;
    ui.horizontal(|ui| {
        ui.label("Windows and doors");
        ui.add(egui::DragValue::new(&mut form.variation));
        if ui
            .button(egui_phosphor::regular::DICE_FIVE)
            .on_hover_text("Re-roll the windows and doors; the rooms stay as they are")
            .clicked()
        {
            form.variation = form.variation.wrapping_add(1);
        }
    });
    crate::theme::toggle(ui, &mut form.clear_site, "Clear objects on the site")
        .on_hover_text("Removes what stands on the site (and the plane above) when applying");

    ui.separator();
    site_controls(app, ui);

    ui.separator();
    if let Some(error) = app.prefabs.build.error.clone() {
        ui.colored_label(egui::Color32::LIGHT_RED, error);
    }
    let Some(preview) = app.prefabs.build.preview.clone() else {
        if app.prefabs.build.site.is_none() {
            ui.weak("Choose where the building goes.");
        }
        return;
    };
    let mut summary = format!("{} objects", preview.objects);
    if preview.removed > 0 {
        summary.push_str(&format!(", {} removed", preview.removed));
    }
    if preview.standing > 0 {
        summary.push_str(&format!(", {} already there", preview.standing));
    }
    ui.colored_label(accent, summary);
    let roof_plane = preview.base_plane.index().get() + 1;
    if roof_plane <= 3 {
        let viewing = app.interaction.view_plane() == roof_plane;
        ui.horizontal_wrapped(|ui| {
            ui.weak("The roof is on the plane above the walls.");
            if ui
                .selectable_label(
                    viewing,
                    format!("{} View plane {roof_plane}", egui_phosphor::regular::EYE),
                )
                .on_hover_text("Switch the view to the plane the roof is on, and back")
                .clicked()
            {
                app.interaction
                    .set_view_plane(if viewing { roof_plane - 1 } else { roof_plane });
            }
        });
    }
    ui.horizontal(|ui| {
        if ui
            .button(format!(
                "{} Apply to the map",
                egui_phosphor::regular::CHECK
            ))
            .on_hover_text("One undo step")
            .clicked()
            && crate::app::editing::commit_map_patch(
                app,
                preview.apply.clone(),
                "Generated building",
            )
        {
            // Placed: the next roll starts from a new seed.
            app.prefabs.build.reroll();
        }
    });
    let width = ui.available_width();
    ui.add_sized(
        [width, 22.0],
        egui::TextEdit::singleline(&mut app.prefabs.build.form.name)
            .hint_text("Name for the library"),
    );
    if ui
        .button(format!(
            "{} Save to library",
            egui_phosphor::regular::FLOPPY_DISK
        ))
        .clicked()
    {
        match app
            .prefabs
            .build
            .to_prefab()
            .and_then(|prefab| app.prefabs.add(prefab))
        {
            Ok(_) => {
                app.prefabs.build.form.name.clear();
                app.prefabs.say("Saved to the library.");
            }
            Err(error) => app.prefabs.say(format!("Not saved: {error}")),
        }
    }
}

fn kit_picker(app: &mut EditorApp, ui: &mut egui::Ui) {
    let build = &mut app.prefabs.build;
    let selected = build
        .kits()
        .get(build.form.kit)
        .map_or_else(|| "none".to_owned(), |kit| kit.name.clone());
    let names: Vec<String> = build.kits().iter().map(|kit| kit.name.clone()).collect();
    egui::ComboBox::from_label("Kit")
        .selected_text(selected)
        .show_ui(ui, |ui| {
            for (index, name) in names.iter().enumerate() {
                ui.selectable_value(&mut build.form.kit, index, name);
            }
        });
    for problem in build.kit_problems() {
        ui.colored_label(egui::Color32::LIGHT_RED, problem);
    }
}

fn site_controls(app: &mut EditorApp, ui: &mut egui::Ui) {
    ui.horizontal_wrapped(|ui| {
        let choosing = app.prefabs.build.choosing;
        let label = if choosing {
            "Click the map (Esc stops)"
        } else {
            "Choose site on the map"
        };
        if ui
            .selectable_label(
                choosing,
                format!("{} {label}", egui_phosphor::regular::CROSSHAIR),
            )
            .clicked()
        {
            app.prefabs.disarm();
            app.placer.disarm();
            app.npcs.cancel_gesture();
            app.prefabs.build.choosing = !choosing;
        }
        let tiles = app.interaction.selection_tiles();
        if ui
            .add_enabled(!tiles.is_empty(), egui::Button::new("Use selection"))
            .on_hover_text("Fit the site to the selected tiles")
            .clicked()
        {
            let min_x = tiles.iter().map(|t| t.0).min().unwrap_or(0);
            let min_y = tiles.iter().map(|t| t.1).min().unwrap_or(0);
            let max_x = tiles.iter().map(|t| t.0).max().unwrap_or(0);
            let max_y = tiles.iter().map(|t| t.1).max().unwrap_or(0);
            let clamp = |side: i32| u16::try_from(side).unwrap_or(MAX_SITE).clamp(6, MAX_SITE);
            let (width, height) = (clamp(max_x - min_x + 1), clamp(max_y - min_y + 1));
            app.prefabs.build.form.width = width;
            app.prefabs.build.form.height = height;
            app.prefabs.build.site = Some(osrs_core::coords::MapTile::new(min_x, min_y));
            app.prefabs.build.site_plane = app.interaction.view_plane();
        }
    });
    match app.prefabs.build.site {
        Some(origin) => {
            ui.label(format!(
                "Site: {} x {} tiles from {}, {} on plane {}",
                app.prefabs.build.form.width,
                app.prefabs.build.form.height,
                origin.x,
                origin.y,
                app.prefabs.build.site_plane
            ));
        }
        None => {
            ui.weak("No site yet.");
        }
    }
}

/// Refresh the preview from the current map and form (called every frame the page is showing).
pub(super) fn refresh(app: &mut EditorApp) {
    let revision = app.session.map_revision();
    app.prefabs.build.refresh(app.session.document(), revision);
}
