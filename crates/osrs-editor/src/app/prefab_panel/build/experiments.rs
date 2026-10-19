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
    ui.collapsing("Candidates · 8 seeds, validated before acceptance", |ui| {
        for (i, p) in app.prefabs.build.candidates.iter().enumerate() {
            ui.horizontal(|ui| {
                crate::app::prefab_panel::preview::show(ui, &p.thumbnail, 52.0);
                if ui
                    .selectable_label(
                        app.prefabs
                            .build
                            .preview
                            .as_ref()
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
        for failure in &app.prefabs.build.failures {
            ui.label(format!("Rejected: {failure}"));
        }
    });
    if let Some(index) = chosen {
        app.prefabs.build.preview = app.prefabs.build.candidates.get(index).cloned();
    }
}
