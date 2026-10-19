//! Scenario lifecycle: create, import, load, save and export, all through the edit gateway.

use super::{SIDECAR_FORMAT, Sidecar};
use crate::app::{
    EditorApp,
    edit_runtime::EditExecution,
    npc_runtime::{ExportPreview, OpenKind, open_text, save_text},
};
use eframe::egui::{self, Color32};
use osrs_core::scenario::ScenarioDefinition;
use osrs_edit::{ScenarioPatch, plan_replace_scenario};
use std::convert::Infallible;
// ----------------------------------------------------------------------------- commits

/// Commit one scenario patch through the session gateway. Returns whether history advanced.
pub(super) fn commit(app: &mut EditorApp, patch: ScenarioPatch) -> bool {
    match app
        .session
        .execute_scenario(patch, |_| Ok::<(), Infallible>(()))
    {
        Ok(EditExecution::Committed { .. }) => true,
        Ok(EditExecution::NoChange) => false,
        Err(error) => {
            app.npcs.notice = Some(format!("Spawn change rejected: {error:?}"));
            false
        }
    }
}

pub(super) fn plan_and_commit<E: std::fmt::Display>(
    app: &mut EditorApp,
    plan: impl FnOnce(&osrs_edit::ScenarioDocument) -> Result<ScenarioPatch, E>,
) -> bool {
    match plan(app.session.scenario()) {
        Ok(patch) => commit(app, patch),
        Err(error) => {
            app.npcs.notice = Some(error.to_string());
            false
        }
    }
}

pub(super) fn new_scenario(app: &mut EditorApp) {
    let Some(source) = app.npcs.source().cloned() else {
        app.npcs.notice = Some("Open a cache first".into());
        return;
    };
    let scenario = ScenarioDefinition {
        version: 1,
        build: source.build,
        client_fingerprint: source.fingerprint.clone(),
        seed: seed_for(&source.fingerprint),
        spawns: Vec::new(),
    };
    plan_and_commit(app, |doc| plan_replace_scenario(doc, Some(scenario)));
}

pub(super) fn seed_for(fingerprint: &str) -> u64 {
    u64::from_str_radix(fingerprint.get(..16).unwrap_or("0"), 16).unwrap_or(0)
}

pub(in crate::app) fn load_sidecar(app: &mut EditorApp, name: &str, text: &str) {
    let parsed: Result<Sidecar, _> = serde_json::from_str(text);
    let sidecar = match parsed {
        Ok(sidecar) if sidecar.format == SIDECAR_FORMAT => sidecar,
        Ok(sidecar) => {
            app.npcs.notice = Some(format!(
                "Not a spawn-edits file (format {:?})",
                sidecar.format
            ));
            return;
        }
        Err(error) => {
            app.npcs.notice = Some(format!("{name} rejected: {error}"));
            return;
        }
    };
    install_scenario(app, sidecar.scenario, &format!("Loaded {name}"));
}

/// Validate against the open cache, then replace the active scenario in one undoable step.
fn install_scenario(app: &mut EditorApp, scenario: ScenarioDefinition, label: &str) {
    let Some(source) = app.npcs.source().cloned() else {
        app.npcs.notice = Some("Open a cache first".into());
        return;
    };
    if scenario.client_fingerprint != source.fingerprint {
        app.npcs.notice = Some(
            "These spawn edits were made for a different cache and cannot be used with the open one"
                .into(),
        );
        return;
    }
    if let Err(error) = scenario.validate() {
        app.npcs.notice = Some(format!("Spawn edits rejected: {error}"));
        return;
    }
    let count = scenario.spawns.len();
    if plan_and_commit(app, |doc| plan_replace_scenario(doc, Some(scenario))) {
        app.npcs.selection = None;
        app.npcs.notice = Some(format!("{label}: {count} spawns"));
    }
}

pub(super) fn apply_import(app: &mut EditorApp) {
    let (Some(source), Some(preview), Some(data)) = (
        app.npcs.source().cloned(),
        app.npcs.preview.take(),
        app.npcs.installation.as_ref(),
    ) else {
        return;
    };
    if data.cache_fingerprint != source.fingerprint {
        app.npcs.notice = Some(
            "The open cache is not this OpenRune installation's LIVE cache; open that cache first"
                .into(),
        );
        return;
    }
    if data.revision != source.build {
        app.npcs.notice = Some(format!(
            "Installation revision {} does not match cache build {}",
            data.revision, source.build
        ));
        return;
    }
    let seed = seed_for(&source.fingerprint);
    match preview.into_scenario(source.build, source.fingerprint.clone(), seed) {
        Ok(scenario) => install_scenario(app, scenario, "Loaded OpenRune spawns"),
        Err(errors) => {
            app.npcs.notice = Some(format!(
                "Import has {} errors; nothing was replaced",
                errors.len()
            ));
        }
    }
}

fn save_scenario(app: &mut EditorApp) {
    let Some(definition) = app.session.scenario().definition() else {
        return;
    };
    let sidecar = Sidecar {
        format: SIDECAR_FORMAT.into(),
        scenario: definition.clone(),
    };
    match serde_json::to_string_pretty(&sidecar) {
        Ok(text) => app.npcs.save_task = Some(save_text("spawn-edits.rustosrs-npc.json", text)),
        Err(error) => app.npcs.notice = Some(error.to_string()),
    }
}

pub(super) fn preview_export(app: &mut EditorApp) {
    let Some(definition) = app.session.scenario().definition() else {
        return;
    };
    app.npcs.export_preview = Some(match osrs_cache::openrune::export_scenario(definition) {
        Ok(export) => ExportPreview {
            spawns_toml: export.spawns_toml,
            behaviors_toml: export.behaviors_toml,
            error: None,
        },
        Err(error) => ExportPreview {
            spawns_toml: String::new(),
            behaviors_toml: String::new(),
            error: Some(error),
        },
    });
}

/// Everything beyond the switch: where spawns come from, how much to draw, and files.
pub(super) fn advanced_section(app: &mut EditorApp, ui: &mut egui::Ui) {
    egui::CollapsingHeader::new("Advanced")
        .id_salt("npc-advanced")
        .show(ui, |ui| {
            let count = app.session.scenario().definition().map(|d| d.spawns.len());
            #[cfg(not(target_arch = "wasm32"))]
            {
                let detected =
                    crate::app::names::installation_of(app.project.cache_label()).is_some();
                ui.horizontal_wrapped(|ui| {
                    if ui
                        .add_enabled(
                            detected && app.npcs.import_job.is_none(),
                            egui::Button::new("Reload from OpenRune"),
                        )
                        .on_hover_text(
                            "Read every spawn again from the installation. Replaces spawns you changed.",
                        )
                        .clicked()
                    {
                        super::begin_loading(app);
                    }
                    if count.is_some() && ui.button("Remove all spawns").clicked() {
                        plan_and_commit(app, |doc| plan_replace_scenario(doc, None));
                        app.npcs.selection = None;
                        app.npcs.auto = crate::app::npc_runtime::AutoLoad::Idle;
                    }
                });
                if !detected {
                    ui.weak("The open cache is not an OpenRune LIVE cache; choose the OpenRune folder below.");
                }
            }
            crate::theme::section_heading(ui, "What to draw");
            let prefs = &mut app.settings.prefs;
            let mut distance = i32::from(prefs.npc.distance);
            ui.add(egui::Slider::new(&mut distance, 8..=184).text("distance (tiles)"));
            prefs.npc.distance = distance as u16;
            let mut most = i32::from(prefs.npc.max_drawn);
            ui.add(
                egui::Slider::new(&mut most, 10..=2000)
                    .logarithmic(true)
                    .text("most NPCs"),
            );
            prefs.npc.max_drawn = most as u16;
            ui.checkbox(&mut prefs.npc.footprints, "Outline each spawn's tiles");
            ui.checkbox(&mut prefs.npc.models, "Draw the NPC models");
            #[cfg(not(target_arch = "wasm32"))]
            import_ui(app, ui);
            egui::CollapsingHeader::new("Spawn edits and export")
                .id_salt("npc-files")
                .show(ui, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        if count.is_none() && ui.button("New empty list").clicked() {
                            new_scenario(app);
                        }
                        if ui.button("Open spawn edits...").clicked() && app.npcs.open_task.is_none() {
                            app.npcs.open_task = Some((
                                OpenKind::Scenario,
                                open_text("Open spawn edits", "json"),
                            ));
                        }
                        if count.is_some() {
                            if ui.button("Save spawn edits...").clicked() {
                                save_scenario(app);
                            }
                            if ui.button("Export to OpenRune...").clicked() {
                                preview_export(app);
                            }
                        }
                    });
                    export_preview_ui(app, ui);
                });
        });
}

fn export_preview_ui(app: &mut EditorApp, ui: &mut egui::Ui) {
    let Some(preview) = &app.npcs.export_preview else {
        return;
    };
    let mut close = false;
    let mut write = None;
    egui::CollapsingHeader::new("OpenRune export preview")
        .default_open(true)
        .show(ui, |ui| {
            if let Some(error) = &preview.error {
                ui.colored_label(Color32::LIGHT_RED, format!("Cannot export: {error}"));
            } else {
                ui.weak("New files only; the OpenRune checkout is never modified.");
                for (title, text) in [
                    ("spawns.toml", &preview.spawns_toml),
                    ("npc-behaviors.toml", &preview.behaviors_toml),
                ] {
                    ui.label(egui::RichText::new(title).strong());
                    let shown: String = text.lines().take(12).collect::<Vec<_>>().join("\n");
                    ui.add(egui::Label::new(
                        egui::RichText::new(shown).monospace().small(),
                    ));
                    if ui.button(format!("Save {title}...")).clicked() {
                        write = Some((title, text.clone()));
                    }
                }
            }
            if ui.button("Close preview").clicked() {
                close = true;
            }
        });
    if let Some((name, text)) = write {
        app.npcs.save_task = Some(save_text(name, text));
    }
    if close {
        app.npcs.export_preview = None;
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn import_ui(app: &mut EditorApp, ui: &mut egui::Ui) {
    use crate::app::npc_runtime::{preview_import, read_installation};
    egui::CollapsingHeader::new("Choose spawn files")
        .id_salt("npc-import")
        .default_open(true)
        .show(ui, |ui| {
            let busy = app.npcs.import_job.is_some();
            if busy {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label("Working...");
                });
            }
            // The open cache usually is the installation's own LIVE cache: one click reads it.
            let detected = crate::app::names::installation_of(app.project.cache_label());
            if let Some(root) = detected.filter(|_| app.npcs.installation.is_none())
                && ui
                    .add_enabled(
                        !busy,
                        egui::Button::new(format!(
                            "Read spawns from {}",
                            root.file_name().map_or_else(String::new, |n| n
                                .to_string_lossy()
                                .into_owned())
                        )),
                    )
                    .on_hover_text(root.display().to_string())
                    .clicked()
            {
                app.npcs.import_job = Some(read_installation(root));
            }
            if ui
                .add_enabled(!busy, egui::Button::new("Choose another folder..."))
                .clicked()
                && let Some(folder) = rfd::FileDialog::new()
                    .set_title("OpenRune-Server folder")
                    .pick_folder()
            {
                app.npcs.import_job = Some(read_installation(folder));
            }
            let Some(data) = &app.npcs.installation else {
                return;
            };
            let matches_open = app
                .npcs
                .source()
                .is_some_and(|s| s.fingerprint == data.cache_fingerprint);
            ui.label(format!(
                "{} \u{b7} revision {}{}",
                data.root.file_name().map_or_else(String::new, |n| n.to_string_lossy().into_owned()),
                data.revision,
                data.commit
                    .as_deref()
                    .map(|c| format!(" \u{b7} {}", c.get(..10).unwrap_or(c)))
                    .unwrap_or_default()
            ));
            if matches_open {
                ui.colored_label(Color32::LIGHT_GREEN, "The open cache is this installation's LIVE cache");
            } else {
                ui.colored_label(
                    Color32::LIGHT_RED,
                    "The open cache is not this installation's LIVE cache; import cannot be applied",
                );
            }
            ui.add(
                egui::TextEdit::singleline(&mut app.npcs.file_filter)
                    .hint_text("Filter spawn files")
                    .desired_width(f32::INFINITY),
            );
            let filter = app.npcs.file_filter.to_ascii_lowercase();
            let visible: Vec<usize> = data
                .spawn_files
                .iter()
                .enumerate()
                .filter(|(_, f)| filter.is_empty() || f.path.to_ascii_lowercase().contains(&filter))
                .map(|(i, _)| i)
                .collect();
            ui.horizontal_wrapped(|ui| {
                if ui.small_button("Select shown").clicked() {
                    for &i in &visible {
                        app.npcs.file_selection[i] = true;
                    }
                }
                if ui.small_button("Clear").clicked() {
                    app.npcs.file_selection.iter_mut().for_each(|s| *s = false);
                }
            });
            egui::ScrollArea::vertical()
                .max_height(140.0)
                .id_salt("npc-import-files")
                .show(ui, |ui| {
                    for &i in &visible {
                        let label = data.spawn_files[i]
                            .path
                            .rsplit('/')
                            .next()
                            .unwrap_or(&data.spawn_files[i].path);
                        ui.checkbox(&mut app.npcs.file_selection[i], label);
                    }
                });
            let chosen: Vec<usize> = app
                .npcs
                .file_selection
                .iter()
                .enumerate()
                .filter(|(_, s)| **s)
                .map(|(i, _)| i)
                .collect();
            ui.horizontal_wrapped(|ui| {
                if ui
                    .add_enabled(
                        !busy && !chosen.is_empty(),
                        egui::Button::new(format!("Preview {} files", chosen.len())),
                    )
                    .clicked()
                {
                    app.npcs.import_job = Some(preview_import(data, Some(chosen)));
                }
                if ui
                    .add_enabled(!busy, egui::Button::new("Preview all"))
                    .clicked()
                {
                    app.npcs.import_job = Some(preview_import(data, None));
                }
            });
            let missing: usize;
            let mut apply = false;
            let mut discard = false;
            if let Some(preview) = &app.npcs.preview {
                let catalog = app.npcs.catalog();
                missing = preview
                    .report
                    .spawns
                    .iter()
                    .filter(|s| catalog.binary_search_by_key(&s.npc, |e| e.id).is_err())
                    .count();
                ui.label(format!(
                    "{} files \u{b7} {} spawns \u{b7} {} warnings \u{b7} {} errors",
                    preview.files,
                    preview.report.spawns.len(),
                    preview.report.warnings.len(),
                    preview.report.errors.len()
                ));
                if missing > 0 {
                    ui.colored_label(
                        Color32::LIGHT_RED,
                        format!("{missing} spawns use NPC definitions missing from the open cache"),
                    );
                }
                for error in preview.report.errors.iter().take(5) {
                    ui.colored_label(Color32::LIGHT_RED, error);
                }
                for warning in preview.report.warnings.iter().take(3) {
                    ui.weak(warning);
                }
                ui.horizontal_wrapped(|ui| {
                    let ready = preview.report.errors.is_empty() && matches_open && missing == 0;
                    apply = ui
                        .add_enabled(ready, egui::Button::new("Load these spawns"))
                        .on_hover_text("One undoable step; resolve errors first")
                        .clicked();
                    discard = ui.button("Discard preview").clicked();
                });
            }
            if apply {
                apply_import(app);
            } else if discard {
                app.npcs.preview = None;
            }
        });
}
