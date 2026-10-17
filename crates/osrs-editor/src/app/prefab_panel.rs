//! The Prefabs dock panel: the saved library, capturing from the map, and placing a prefab in the
//! viewport (armed gesture with a ghost, R to turn, click to place as one undoable step).
//!
//! Drawing and gestures return nothing that mutates the map except the placement click, which
//! plans a patch against the authoritative document and commits it through the edit gateway.

mod build;
mod library;
mod preview;
mod viewport;

use crate::app::{EditorApp, prefab_runtime::Page};
use eframe::egui;

pub(super) use viewport::{armed_input, draw_world, place};

/// Once per frame: finish file dialogs and keep the generator's preview current while it shows.
pub(super) fn update(app: &mut EditorApp) {
    viewport::poll(app);
    let showing = app.settings.prefs.properties_panel
        && app.settings.prefs.rail_tab == crate::settings::RailTab::Prefabs
        && app.prefabs.page == Page::Build;
    if showing {
        build::refresh(app);
    }
}

pub(super) fn show(app: &mut EditorApp, ui: &mut egui::Ui) {
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 4.0;
            ui.horizontal(|ui| {
                ui.selectable_value(&mut app.prefabs.page, Page::Library, "Library");
                ui.selectable_value(&mut app.prefabs.page, Page::Build, "Build");
            });
            ui.separator();
            match app.prefabs.page {
                Page::Library => library::show(app, ui),
                Page::Build => build::show(app, ui),
            }
        });
}
