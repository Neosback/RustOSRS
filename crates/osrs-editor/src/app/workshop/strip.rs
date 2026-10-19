//! The list of saved buildings on the left of the Workshop: search, filters, a tick box on
//! every row, and the delete question (which moves buildings to a trash you can bring them back
//! from).

use crate::app::{EditorApp, prefab_panel::preview};
use eframe::egui;
use osrs_edit::{Prefab, PrefabSource};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Source {
    #[default]
    All,
    Captured,
    Generated,
    Authored,
}

impl Source {
    const ALL: [(Self, &'static str); 4] = [
        (Self::All, "All"),
        (Self::Captured, "From the map"),
        (Self::Generated, "Made by the editor"),
        (Self::Authored, "By hand"),
    ];

    fn matches(self, prefab: &Prefab) -> bool {
        matches!(
            (self, &prefab.source),
            (Self::All, _)
                | (Self::Captured, PrefabSource::Captured { .. })
                | (Self::Generated, PrefabSource::Generated { .. })
                | (Self::Authored, PrefabSource::Authored)
        )
    }
}

#[derive(Default)]
pub(super) struct StripState {
    source: Source,
    tag: Option<String>,
    /// Said by the last delete or bring-back, until dismissed.
    notice: Option<String>,
}

const THUMB: f32 = 40.0;

pub(super) fn show(app: &mut EditorApp, ui: &mut egui::Ui) {
    let width = ui.available_width();
    ui.add_sized(
        [width, 24.0],
        egui::TextEdit::singleline(&mut app.prefabs.filter)
            .hint_text("Search by name, tag or note"),
    );
    ui.horizontal_wrapped(|ui| {
        for (source, label) in Source::ALL {
            ui.selectable_value(&mut app.prefabs.workshop.strip.source, source, label);
        }
    });
    let tags: BTreeSet<String> = app
        .prefabs
        .entries()
        .iter()
        .flat_map(|entry| entry.prefab.tags.iter().cloned())
        .collect();
    if !tags.is_empty() {
        ui.horizontal_wrapped(|ui| {
            for tag in tags {
                let on = app.prefabs.workshop.strip.tag.as_deref() == Some(tag.as_str());
                if ui.selectable_label(on, &tag).clicked() {
                    app.prefabs.workshop.strip.tag = if on { None } else { Some(tag) };
                }
            }
        });
    }
    actions(app, ui);
    ui.separator();

    let (source, tag) = {
        let strip = &app.prefabs.workshop.strip;
        (strip.source, strip.tag.clone())
    };
    let rows: Vec<(String, Prefab)> = app
        .prefabs
        .visible()
        .into_iter()
        .filter(|entry| source.matches(&entry.prefab))
        .filter(|entry| {
            tag.as_ref()
                .is_none_or(|tag| entry.prefab.tags.contains(tag))
        })
        .map(|entry| (entry.file.clone(), entry.prefab.clone()))
        .collect();
    if rows.is_empty() {
        ui.weak(if app.prefabs.entries().is_empty() {
            "No saved buildings yet. Select a building on the map and capture it from the \
             Prefabs panel, or build one in the Train tab."
        } else {
            "Nothing matches."
        });
        return;
    }
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for (file, prefab) in rows {
                row(app, ui, &file, &prefab);
            }
        });
}

fn actions(app: &mut EditorApp, ui: &mut egui::Ui) {
    let ticked = app.prefabs.trash.checked.len();
    ui.horizontal_wrapped(|ui| {
        let label = if ticked > 0 {
            format!("Delete {ticked} ticked")
        } else {
            "Delete the selected one".to_owned()
        };
        let target: Vec<String> = if ticked > 0 {
            app.prefabs.trash.checked.iter().cloned().collect()
        } else {
            app.prefabs.selected.iter().cloned().collect()
        };
        if ui
            .add_enabled(!target.is_empty(), egui::Button::new(label))
            .on_hover_text("Tick buildings to delete several at once. Deleted buildings go to a trash; you can bring them back.")
            .clicked()
        {
            app.prefabs.ask_to_delete(target);
        }
        if app.prefabs.trash.can_restore()
            && ui
                .button("Bring back the last deleted")
                .on_hover_text("Undo the last delete")
                .clicked()
        {
            app.prefabs.workshop.strip.notice = Some(match app.prefabs.restore_last() {
                Ok(files) => format!("Brought back {} building(s).", files.len()),
                Err(error) => error,
            });
        }
    });
    ui.horizontal_wrapped(|ui| {
        if ui
            .small_button("Import")
            .on_hover_text("Add a building file (.prefab.json)")
            .clicked()
        {
            app.prefabs.start_import();
        }
        if ui
            .small_button("Reload")
            .on_hover_text("Read the folder again (after changing files by hand)")
            .clicked()
        {
            app.prefabs.load();
        }
        if let Some(folder) = app.prefabs.directory()
            && ui
                .small_button("Copy folder path")
                .on_hover_text(folder.display().to_string())
                .clicked()
        {
            ui.ctx().copy_text(folder.display().to_string());
        }
    });
    if let Some(notice) = app.prefabs.workshop.strip.notice.clone() {
        ui.horizontal_wrapped(|ui| {
            ui.label(notice);
            if ui.small_button("x").clicked() {
                app.prefabs.workshop.strip.notice = None;
            }
        });
    }
}

fn row(app: &mut EditorApp, ui: &mut egui::Ui, file: &str, prefab: &Prefab) {
    let selected = app.prefabs.selected.as_deref() == Some(file);
    let frame = egui::Frame::new()
        .inner_margin(egui::Margin::same(3))
        .corner_radius(4.0)
        .fill(if selected {
            ui.visuals().selection.bg_fill.gamma_multiply(0.45)
        } else {
            egui::Color32::TRANSPARENT
        });
    let response = frame
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                let mut ticked = app.prefabs.trash.checked.contains(file);
                if ui
                    .checkbox(&mut ticked, "")
                    .on_hover_text("Tick to delete several at once")
                    .changed()
                {
                    if ticked {
                        app.prefabs.trash.checked.insert(file.to_owned());
                    } else {
                        app.prefabs.trash.checked.remove(file);
                    }
                }
                let still = crate::app::prefab_gallery::still_of(app, file);
                match still {
                    Some(texture) => {
                        ui.add(
                            egui::Image::new((texture.id(), egui::vec2(THUMB * 1.33, THUMB)))
                                .corner_radius(3.0),
                        );
                    }
                    None => {
                        preview::show(ui, prefab, THUMB);
                    }
                }
                ui.vertical(|ui| {
                    ui.add(egui::Label::new(egui::RichText::new(&prefab.name).strong()).truncate());
                    ui.add(
                        egui::Label::new(egui::RichText::new(prefab.summary()).weak().small())
                            .truncate(),
                    );
                });
            });
        })
        .response
        .interact(egui::Sense::click());
    if response.clicked() {
        app.prefabs.selected = Some(file.to_owned());
        if app.prefabs.workshop.tab.is_page() {
            app.prefabs.studio_request = Some(file.to_owned());
        }
    }
    if response.double_clicked() {
        app.prefabs.studio_request = Some(file.to_owned());
    }
}

/// The delete question. One at a time, so an old answer can never apply to a new question.
pub(super) fn ask(app: &mut EditorApp, ctx: &egui::Context) {
    let Some(files) = app.prefabs.trash.asking.clone() else {
        return;
    };
    let names: Vec<String> = files
        .iter()
        .map(|file| {
            app.prefabs
                .entry(file)
                .map_or_else(|| file.clone(), |entry| entry.prefab.name.clone())
        })
        .collect();
    let open_in_scene = app
        .prefabs
        .studio
        .as_ref()
        .is_some_and(|studio| files.iter().any(|f| studio.is_editing(f)));
    let mut answer = None;
    egui::Window::new("Delete?")
        .id(egui::Id::new("workshop-delete"))
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.label(format!("Delete {} building(s)?", names.len()));
            for name in names.iter().take(8) {
                ui.label(format!("  \u{2022} {name}"));
            }
            if names.len() > 8 {
                ui.weak(format!("  and {} more", names.len() - 8));
            }
            if open_in_scene {
                ui.colored_label(
                    ui.visuals().warn_fg_color,
                    "One of these is open in the scene editor. Close it there first.",
                );
            }
            ui.weak("They move to a trash. \"Bring back the last deleted\" undoes this.");
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(!open_in_scene, egui::Button::new("Delete"))
                    .clicked()
                {
                    answer = Some(true);
                }
                if ui.button("Keep them").clicked() {
                    answer = Some(false);
                }
            });
        });
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        answer = Some(false);
    }
    match answer {
        Some(true) => {
            app.prefabs.trash.asking = None;
            app.prefabs.workshop.strip.notice = Some(match app.prefabs.move_to_trash(&files) {
                Ok(count) => format!("Deleted {count} building(s)."),
                Err(error) => error,
            });
        }
        Some(false) => app.prefabs.trash.asking = None,
        None => {}
    }
}
