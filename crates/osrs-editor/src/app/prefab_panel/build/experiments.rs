//! Small task controls; the selected candidate is preview-only until accepted by the edit gateway.
use super::*;
use crate::app::prefab_runtime::GenerationMode;
use osrs_edit::BuildingStage;

pub(super) fn controls(app: &mut EditorApp, ui: &mut egui::Ui) {
    let form = &mut app.prefabs.build.form;
    egui::ComboBox::from_label("Operation")
        .selected_text(format!("{:?}", form.mode))
        .show_ui(ui, |ui| {
            for (mode, label) in [
                (GenerationMode::Generate, "Generate building"),
                (GenerationMode::Complete, "Complete missing parts"),
                (GenerationMode::Furnish, "Furnish rooms"),
                (GenerationMode::ReplaceStyle, "Replace architectural style"),
                (GenerationMode::Repair, "Repair selected area"),
            ] {
                ui.selectable_value(&mut form.mode, mode, label);
            }
        });
    ui.add_enabled_ui(form.sketch.is_some(), |ui| {
        egui::ComboBox::from_label("Reroll stage")
            .selected_text(
                form.stage
                    .map_or_else(|| "All stages".into(), |s| format!("{s:?}")),
            )
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut form.stage, None, "All stages");
                for stage in [
                    BuildingStage::Foundation,
                    BuildingStage::Floors,
                    BuildingStage::Circulation,
                    BuildingStage::Shell,
                    BuildingStage::Roof,
                    BuildingStage::Furnishings,
                    BuildingStage::Decoration,
                ] {
                    ui.selectable_value(&mut form.stage, Some(stage), format!("{stage:?}"));
                }
            });
    });
    let mut chosen = None;
    ui.collapsing("Candidates · validated before acceptance", |ui| {
        for (i, p) in app
            .prefabs
            .build
            .candidates(
                app.project.generation(),
                app.session.revision(),
                &|id, _| app.placer.known_object_size(id.get()),
            )
            .iter()
            .enumerate()
        {
            ui.horizontal(|ui| {
                crate::app::prefab_panel::preview::show(ui, &p.thumbnail, 52.0);
                if ui
                    .selectable_label(
                        app.prefabs
                            .build
                            .current_preview(
                                app.project.generation(),
                                app.session.revision(),
                                &|id, _| app.placer.known_object_size(id.get()),
                            )
                            .is_some_and(|s| s.form.seed == p.form.seed),
                        format!(
                            "Seed {} · {} rooms · {} pieces",
                            p.form.seed, p.verdict.rooms, p.objects
                        ),
                    )
                    .clicked()
                {
                    chosen = Some(i);
                }
            });
        }
        for (seed, failure) in app.prefabs.build.failure_messages(
            app.project.generation(),
            app.session.revision(),
            &|id, _| app.placer.known_object_size(id.get()),
        ) {
            ui.horizontal_wrapped(|ui| {
                ui.label(format!("Rejected: {failure}"));
                #[cfg(not(target_arch = "wasm32"))]
                if ui
                    .add_enabled(
                        !app.prefabs.build.reproduction_running()
                            && app.prefabs.build.site_rect().is_some_and(|(origin, _, _)| {
                                app.world.source_target(origin).is_some()
                            }),
                        egui::Button::new("Save report…").small(),
                    )
                    .on_hover_text(format!("Save a reproduction of rejected seed {seed}"))
                    .clicked()
                    && let Some(path) = rfd::FileDialog::new()
                        .set_file_name(format!("building-failure-{seed}.json"))
                        .save_file()
                    && let Err(error) = super::export_rejected_reproduction(app, path, seed)
                {
                    app.ui.notify(error);
                }
                #[cfg(target_arch = "wasm32")]
                let _ = seed;
            });
        }
    });
    if let Some(index) = chosen {
        app.prefabs.build.select_candidate(
            index,
            app.project.generation(),
            app.session.revision(),
            &|id, _| app.placer.known_object_size(id.get()),
        );
    }
}
