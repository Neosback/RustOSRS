//! The Prefabs dock panel: the saved library, capturing from the map, and placing a prefab in the
//! viewport (armed gesture with a ghost, R to turn, click to place as one undoable step).
//!
//! Drawing and gestures return nothing that mutates the map except the placement click, which
//! plans a patch against the authoritative document and commits it through the edit gateway.

mod build;
pub(in crate::app) mod capture;
mod library;
pub(in crate::app) mod placement_review;
pub(in crate::app) mod preview;
mod viewport;
pub(super) fn review_placement(app: &mut EditorApp, ctx: &egui::Context) {
    capture::refine::window(app, ctx);
    placement_review::show(app, ctx);
}

use crate::app::{EditorApp, prefab_runtime::Page};
use eframe::egui;

pub(in crate::app) use library::{arm, text_field};
pub(super) use viewport::{armed_input, draw_world, place};

/// The same generator controls inside the isolated authoring workspace.
pub(in crate::app) fn builder(app: &mut EditorApp, ui: &mut egui::Ui) {
    build::refresh(app);
    build::show(app, ui);
}

/// Once per frame: finish file dialogs and keep the generator's preview current while it shows.
pub(super) fn update(app: &mut EditorApp) {
    viewport::poll(app);
    capture::poll(app);
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
