//! "Name this object": the inspector's box for giving an object a name that every building
//! and prefab using it will show, and saying what it is (a door, a bed...).

use crate::app::EditorApp;
use eframe::egui;
use osrs_core::object_names::{NameSource, ROLES, Resolved, Witness};

/// What the header shows for an object: its name, or a plain "no name yet".
pub(super) fn title(resolved: &Resolved) -> String {
    if resolved.text.is_empty() {
        "Unnamed object".to_owned()
    } else {
        resolved.text.clone()
    }
}

pub(super) fn card(
    app: &mut EditorApp,
    ui: &mut egui::Ui,
    id: u32,
    cache_name: &str,
    resolved: &Resolved,
) {
    let symbol = app
        .names
        .book()
        .and_then(|b| b.object(id))
        .unwrap_or_default()
        .to_owned();
    let witness = Witness {
        cache_name: cache_name.to_owned(),
        symbol,
    };
    ui.weak(resolved.source.label());
    egui::CollapsingHeader::new("Name this object")
        .id_salt(("name-object", id))
        .default_open(resolved.source == NameSource::Unnamed)
        .show(ui, |ui| {
            ui.weak(
                "A name you give is remembered for every building. It helps the editor tell \
                 doors from walls and beds from tables, and what to put in each room.",
            );
            let key = ui.id().with(("name-draft", id));
            let current = app
                .names
                .custom()
                .book()
                .get(id)
                .and_then(|e| e.name.clone())
                .unwrap_or_default();
            let mut text: String = ui
                .data_mut(|d| d.get_temp::<(String, String)>(key))
                .filter(|(origin, _)| *origin == current)
                .map_or_else(|| current.clone(), |(_, text)| text);
            let width = ui.available_width();
            let response = ui.add_sized(
                [width, 22.0],
                egui::TextEdit::singleline(&mut text).hint_text("A name, like \"Brick wall\""),
            );
            if response.changed() {
                ui.data_mut(|d| d.insert_temp(key, (current.clone(), text.clone())));
            }
            if response.lost_focus() {
                ui.data_mut(|d| d.remove::<(String, String)>(key));
                if text != current {
                    app.names
                        .custom_mut()
                        .name_object(id, &text, witness.clone());
                }
            }
            let role = app
                .names
                .custom()
                .book()
                .get(id)
                .and_then(|e| e.role.clone())
                .unwrap_or_default();
            ui.horizontal(|ui| {
                ui.label("It is a");
                let mut chosen = role.clone();
                egui::ComboBox::from_id_salt(("object-role", id))
                    .selected_text(if role.is_empty() {
                        "(not set)"
                    } else {
                        role.as_str()
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut chosen, String::new(), "(not set)");
                        for option in ROLES {
                            ui.selectable_value(&mut chosen, (*option).to_owned(), *option);
                        }
                    });
                if chosen != role {
                    app.names
                        .custom_mut()
                        .change(|book| book.set_role(id, &chosen, witness.clone()));
                }
            });
            if app.names.custom().book().get(id).is_some()
                && ui
                    .small_button("Forget what I said about this object")
                    .clicked()
            {
                app.names.custom_mut().change(|book| book.remove(id));
            }
            if let Some(problem) = app.names.custom().problem() {
                ui.colored_label(ui.visuals().error_fg_color, problem);
            }
        });
}
