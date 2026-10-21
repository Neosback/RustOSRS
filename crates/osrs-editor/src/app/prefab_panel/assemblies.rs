//! Saved assembly assets share the ordinary library, preview, placement and history path.
use crate::app::EditorApp;
use eframe::egui;

pub(super) fn show(app: &mut EditorApp, ui: &mut egui::Ui, file: &str, source: &osrs_edit::Prefab) {
    if source.building.is_none() {
        return;
    }
    match source.reviewed_assemblies() {
        Ok(groups) if !groups.is_empty() => {
            egui::CollapsingHeader::new("Reusable assemblies").id_salt(("assemblies", file)).show(ui, |ui| {
                ui.weak("These groups come from the saved prefab. Save Studio changes before extracting.");
                for group in groups {
                    let first = group[0];
                    let name = format!("{} assembly {}", source.name, first + 1);
                    if ui.button(format!("Save assembly {} · {} pieces", first + 1, group.len())).clicked() {
                        match app.prefabs.extract_assembly(file, first, &name) {
                            Ok(_) => app.prefabs.say("Assembly saved. Place or export it from the library."),
                            Err(error) => app.prefabs.say(error),
                        }
                    }
                }
            });
        }
        Err(error) => {
            ui.weak(format!("Assembly extraction needs review: {error}"));
        }
        _ => {
            ui.weak("To save reusable assemblies, group pieces in Studio's Relationships panel.");
        }
    }
}
