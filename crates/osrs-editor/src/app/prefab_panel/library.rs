//! The library page: search, the list, the selected prefab's details, and capturing from the map.

use super::preview;
use crate::app::EditorApp;
use eframe::egui;
use osrs_core::coords::{MapTile, SourcePlane};
use osrs_edit::{CaptureSpec, Prefab, PrefabSource};

const THUMB: f32 = 44.0;

pub(super) fn show(app: &mut EditorApp, ui: &mut egui::Ui) {
    if app.session.document().terrain_entry_count() == 0
        && app.session.document().object_count() == 0
    {
        ui.weak("Open a cache and a map to capture and place prefabs.");
    }
    if let Some(notice) = app.prefabs.notice().map(str::to_owned) {
        ui.horizontal_wrapped(|ui| {
            ui.label(notice);
            if ui.small_button(egui_phosphor::regular::X).clicked() {
                app.prefabs.clear_notice();
            }
        });
    }
    ui.horizontal(|ui| {
        let width = (ui.available_width() - 90.0).max(80.0);
        ui.add_sized(
            [width, 22.0],
            egui::TextEdit::singleline(&mut app.prefabs.filter)
                .hint_text("Search name, tag or note"),
        );
        if ui
            .button(format!(
                "{} Import",
                egui_phosphor::regular::DOWNLOAD_SIMPLE
            ))
            .on_hover_text("Add a prefab file (.prefab.json) to the library")
            .clicked()
        {
            app.prefabs.start_import();
        }
    });
    ui.horizontal_wrapped(|ui| {
        if ui
            .button(format!("{} Open the Workshop", egui_phosphor::regular::CUBE))
            .on_hover_text(
                "One window to look at and change buildings in 3D, build new ones, name things, teach the AI, lay paths and see what the floor-pattern builder learns",
            )
            .clicked()
        {
            crate::app::workshop::open(app, crate::app::workshop::Tab::Scene);
        }
        if ui
            .button(format!("{} New scene", egui_phosphor::regular::PLUS))
            .on_hover_text("Create a blank 12×12 scene prefab and open it in the Workshop")
            .clicked()
        {
            match app.prefabs.create_blank_scene() {
                Ok(file) => crate::app::prefab_studio::request(app, file),
                Err(error) => app.prefabs.say(error),
            }
        }
    });
    capture_section(app, ui);
    ui.separator();
    list(app, ui);
    if let Some(file) = app.prefabs.selected.clone() {
        ui.separator();
        details(app, ui, &file);
    }
    if let Some(directory) = app.prefabs.directory() {
        ui.add_space(4.0);
        ui.add(
            egui::Label::new(
                egui::RichText::new(format!("Saved in {}", directory.display())).weak(),
            )
            .truncate(),
        )
        .on_hover_text(directory.display().to_string());
    }
}

/// The selected tiles' bounding box on the viewed plane: origin and size.
fn selection_box(app: &EditorApp) -> Option<(MapTile, u16, u16)> {
    let tiles = app.interaction.selection_tiles();
    let min_x = tiles.iter().map(|t| t.0).min()?;
    let max_x = tiles.iter().map(|t| t.0).max()?;
    let min_y = tiles.iter().map(|t| t.1).min()?;
    let max_y = tiles.iter().map(|t| t.1).max()?;
    let width = u16::try_from(max_x - min_x + 1).ok()?;
    let height = u16::try_from(max_y - min_y + 1).ok()?;
    Some((MapTile::new(min_x, min_y), width, height))
}

fn capture_section(app: &mut EditorApp, ui: &mut egui::Ui) {
    egui::CollapsingHeader::new("Capture from the map")
        .id_salt("prefab-capture")
        .default_open(true)
        .show(ui, |ui| {
            ui.checkbox(&mut app.prefabs.form.building_only,
                "Ignore ground outside the building's outer walls")
                .on_hover_text("Keep walls and attached decoration even when their anchors are outside. Review rooms, floors and pieces before capture. Uncheck for a rectangular copy.");
            let building_only = app.prefabs.form.building_only;
            let selection = selection_box(app);
            match selection {
                Some((origin, w, h)) => ui.label(format!(
                    "Selection: {w} x {h} tiles from {}, {} on plane {}",
                    origin.x,
                    origin.y,
                    app.interaction.view_plane()
                )),
                None => ui.weak(
                    "Select tiles (Select tool, tile mode, drag a box) around a building, then capture it.",
                ),
            };
            let width = ui.available_width();
            ui.add_sized(
                [width, 22.0],
                egui::TextEdit::singleline(&mut app.prefabs.form.name).hint_text("Name"),
            );
            if !building_only { ui.horizontal_wrapped(|ui| {
                ui.label("Planes");
                ui.add(egui::DragValue::new(&mut app.prefabs.form.planes).range(1..=4))
                    .on_hover_text("How many planes up from the viewed one to copy");
                crate::theme::toggle(ui, &mut app.prefabs.form.terrain, "Ground");
                crate::theme::toggle(ui, &mut app.prefabs.form.heights, "Heights");
                crate::theme::toggle(ui, &mut app.prefabs.form.objects, "Objects");
            });
            }
            let capture = ui.add_enabled(
                selection.is_some() || (building_only && app.interaction.inspect_tile().is_some()),
                egui::Button::new(if building_only { "Review building capture" } else { "Capture rectangle" }),
            );
            if capture.clicked() && building_only
                && let Some(seed) = super::capture::seed(app) {
                    super::capture::start(app, seed);
            }
            if capture.clicked() && !building_only
                && let Some((origin, width, height)) = selection
                && let Some(base_plane) = SourcePlane::new(app.interaction.view_plane())
            {
                let form = &app.prefabs.form;
                let spec = CaptureSpec {
                    origin,
                    width,
                    height,
                    base_plane,
                    planes: form.planes.clamp(1, 4),
                    terrain: form.terrain,
                    heights: form.heights,
                    objects: form.objects,
                };
                let name = form.name.clone();
                match app.prefabs.capture(app.session.document(), spec, &name) {
                    Ok(_) => {
                        app.prefabs.form.name.clear();
                        app.prefabs.say("Captured. It is saved in the library.");
                    }
                    Err(error) => app.prefabs.say(format!("Nothing captured: {error}")),
                }
            }
            if building_only {
                ui.weak("Pick an interior floor tile, or select tiles around one building. Outside wall anchors do not own the ground beneath them. Courtyards and holes stay empty.");
            }
            super::capture::show(app, ui);
        });
}

fn list(app: &mut EditorApp, ui: &mut egui::Ui) {
    let shown: Vec<_> = app
        .prefabs
        .visible()
        .into_iter()
        .map(|e| e.file.clone())
        .collect();
    if shown.is_empty() {
        ui.weak(if app.prefabs.entries().is_empty() {
            "The library is empty. Capture a building from the map, or import a file."
        } else {
            "Nothing matches the search."
        });
        return;
    }
    for file in shown {
        let Some(entry) = app.prefabs.entry(&file) else {
            continue;
        };
        let selected = app.prefabs.selected.as_deref() == Some(file.as_str());
        let armed = app.prefabs.armed.as_ref().is_some_and(|a| a.file == file);
        let frame = egui::Frame::new()
            .inner_margin(egui::Margin::same(3))
            .corner_radius(4.0)
            .fill(if selected {
                ui.visuals().selection.bg_fill.gamma_multiply(0.5)
            } else {
                egui::Color32::TRANSPARENT
            });
        let response = frame
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let still = crate::app::prefab_gallery::still_of(app, &file);
                    match still {
                        Some(texture) => {
                            ui.add(
                                egui::Image::new((texture.id(), egui::vec2(THUMB * 1.33, THUMB)))
                                    .corner_radius(3.0),
                            );
                        }
                        None => {
                            preview::show(ui, &entry.prefab, THUMB);
                        }
                    }
                    ui.vertical(|ui| {
                        ui.label(egui::RichText::new(&entry.prefab.name).strong());
                        ui.weak(entry.prefab.summary());
                        if !entry.prefab.tags.is_empty() {
                            ui.weak(entry.prefab.tags.join(", "));
                        }
                        if armed {
                            ui.colored_label(ui.visuals().selection.stroke.color, "Placing");
                        }
                    });
                });
            })
            .response
            .interact(egui::Sense::click());
        if response.clicked() {
            app.prefabs.selected = Some(file.clone());
        }
        if response.double_clicked() {
            crate::app::prefab_studio::request(app, file.clone());
        }
    }
}

/// Start placing `file`: it owns the next viewport click.
pub(in crate::app) fn arm(app: &mut EditorApp, file: &str) {
    app.placer.disarm();
    app.npcs.cancel_gesture();
    app.prefabs.selected = Some(file.to_owned());
    app.prefabs.arm(file);
}

/// A text field that reports the new value once, when the user leaves it changed.
pub(in crate::app) fn text_field(
    ui: &mut egui::Ui,
    key: &str,
    current: &str,
    multiline: bool,
) -> Option<String> {
    let id = ui.id().with(key);
    let mut text = ui
        .data_mut(|d| d.get_temp::<(String, String)>(id))
        .filter(|(origin, _)| origin == current)
        .map_or_else(|| current.to_owned(), |(_, text)| text);
    let width = ui.available_width();
    let response = if multiline {
        ui.add_sized([width, 52.0], egui::TextEdit::multiline(&mut text))
    } else {
        ui.add_sized([width, 22.0], egui::TextEdit::singleline(&mut text))
    };
    if response.changed() {
        ui.data_mut(|d| d.insert_temp(id, (current.to_owned(), text.clone())));
    }
    if response.lost_focus() {
        ui.data_mut(|d| d.remove::<(String, String)>(id));
        if text != current {
            return Some(text);
        }
    }
    None
}

fn details(app: &mut EditorApp, ui: &mut egui::Ui, file: &str) {
    let Some(entry) = app.prefabs.entry(file) else {
        return;
    };
    let prefab: Prefab = entry.prefab.clone();
    if let Some(name) = text_field(ui, "name", &prefab.name, false)
        && let Err(error) = app.prefabs.edit(file, |p| p.name = name.trim().to_owned())
    {
        app.prefabs.say(error);
    }
    ui.weak(match &prefab.source {
        PrefabSource::Captured {
            tile_x,
            tile_y,
            plane,
        } => {
            format!("Captured from {tile_x}, {tile_y} plane {plane}")
        }
        PrefabSource::Generated {
            archetype,
            style,
            seed,
            floors,
        } => {
            format!("Generated: {archetype}, {style}, seed {seed}, {floors} floor(s)")
        }
        PrefabSource::Authored => "Made by hand".to_owned(),
    });
    ui.label("Tags (comma separated)");
    if let Some(tags) = text_field(ui, "tags", &prefab.tags.join(", "), false) {
        let tags: Vec<String> = tags
            .split(',')
            .map(|t| t.trim().to_lowercase())
            .filter(|t| !t.is_empty())
            .collect();
        if let Err(error) = app.prefabs.edit(file, |p| p.tags = tags) {
            app.prefabs.say(error);
        }
    }
    ui.label("Notes");
    if let Some(notes) = text_field(ui, "notes", &prefab.notes, true)
        && let Err(error) = app.prefabs.edit(file, |p| p.notes = notes)
    {
        app.prefabs.say(error);
    }
    if ui.button("Edit this prefab").clicked() {
        crate::app::prefab_studio::request(app, file.to_owned());
    }
    place_controls(app, ui, file);
    ui.horizontal(|ui| {
        if ui
            .button(format!("{} Duplicate", egui_phosphor::regular::COPY))
            .clicked()
        {
            match app.prefabs.duplicate(file) {
                Ok(_) => {}
                Err(error) => app.prefabs.say(error),
            }
        }
        if ui
            .button(format!("{} Export", egui_phosphor::regular::UPLOAD_SIMPLE))
            .clicked()
        {
            app.prefabs.start_export(file);
        }
        let confirm_id = ui.id().with(("delete", file));
        let confirming = ui.data(|d| d.get_temp::<bool>(confirm_id)).unwrap_or(false);
        let label = if confirming {
            "Really delete?"
        } else {
            "Delete"
        };
        let button = ui.button(format!("{} {label}", egui_phosphor::regular::TRASH));
        if button.clicked() {
            if confirming {
                match app.prefabs.remove(file) {
                    Ok(()) => {}
                    Err(error) => app.prefabs.say(error),
                }
                ui.data_mut(|d| d.remove::<bool>(confirm_id));
            } else {
                ui.data_mut(|d| d.insert_temp(confirm_id, true));
            }
        } else if confirming && !button.hovered() && ui.input(|i| i.pointer.any_click()) {
            ui.data_mut(|d| d.remove::<bool>(confirm_id));
        }
    });
}

fn place_controls(app: &mut EditorApp, ui: &mut egui::Ui, file: &str) {
    let armed = app.prefabs.armed.as_ref().is_some_and(|a| a.file == file);
    if !armed {
        if ui
            .button(format!("{} Place on the map", egui_phosphor::regular::CURSOR_CLICK))
            .on_hover_text("Then click the map. R turns it, Esc stops. Double-click opens the prefab in Studio")
            .clicked()
        {
            arm(app, file);
        }
        return;
    }
    let Some(armed) = app.prefabs.armed.as_mut() else {
        return;
    };
    ui.horizontal(|ui| {
        if ui
            .button(egui_phosphor::regular::ARROW_COUNTER_CLOCKWISE)
            .on_hover_text("Turn left (Shift+R)")
            .clicked()
        {
            armed.turns = (armed.turns + 3) & 3;
        }
        ui.label(format!("{} degrees", u32::from(armed.turns) * 90));
        if ui
            .button(egui_phosphor::regular::ARROW_CLOCKWISE)
            .on_hover_text("Turn right (R)")
            .clicked()
        {
            armed.turns = (armed.turns + 1) & 3;
        }
    });
    ui.horizontal_wrapped(|ui| {
        crate::theme::toggle(ui, &mut armed.terrain, "Ground");
        crate::theme::toggle(ui, &mut armed.heights, "Heights");
        crate::theme::toggle(ui, &mut armed.objects, "Objects");
    });
    crate::theme::toggle(
        ui,
        &mut armed.replace_objects,
        "Remove objects already there",
    )
    .on_hover_text("Clears the area first so the prefab replaces what stands there");
    ui.horizontal(|ui| {
        ui.colored_label(
            ui.visuals().selection.stroke.color,
            "Click the map to place.",
        );
        if ui.button("Stop").clicked() {
            app.prefabs.disarm();
        }
    });
}
