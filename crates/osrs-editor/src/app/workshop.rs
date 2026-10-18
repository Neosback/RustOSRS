//! The Workshop: one window for everything about saved buildings. A list of buildings on the
//! left, and three things to do with them on the right: **Buildings** (look, rename, delete),
//! **Teach** (tell the editor what the objects in them are) and **Train** (let it learn their
//! style, then ask it to build new ones).
//!
//! Plain words everywhere: this window is for people who are not programmers. Nothing here edits
//! the map except the buttons that say so (placing a building); everything else changes the
//! library, the name book or the models.

mod buildings;
mod strip;
mod table;
mod teach;
mod train;

use crate::app::EditorApp;
use eframe::egui;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(in crate::app) enum Tab {
    #[default]
    Buildings,
    Teach,
    Train,
}

impl Tab {
    const ALL: [(Self, &'static str, &'static str); 3] = [
        (Self::Buildings, "Buildings", "Look at your saved buildings"),
        (
            Self::Teach,
            "Teach",
            "Tell the editor what things in a building are",
        ),
        (
            Self::Train,
            "Train",
            "Let the editor learn from your buildings and build new ones",
        ),
    ];
}

#[derive(Default)]
pub(in crate::app) struct Workshop {
    pub(in crate::app) open: bool,
    pub(in crate::app) tab: Tab,
    strip: strip::StripState,
    teach: teach::TeachState,
    train: train::TrainState,
}

const STRIP_WIDTH: f32 = 300.0;

/// Once per frame: the window when it is open, and the delete question whenever one is waiting.
pub(super) fn window(app: &mut EditorApp, ctx: &egui::Context) {
    strip::ask(app, ctx);
    train::poll(app);
    if !app.prefabs.workshop.open {
        return;
    }
    let mut open = true;
    egui::Window::new("Workshop")
        .id(egui::Id::new("workshop-window"))
        .open(&mut open)
        .default_size([1060.0, 700.0])
        .min_size([760.0, 440.0])
        .resizable(true)
        .show(ctx, |ui| {
            ui.horizontal_top(|ui| {
                ui.allocate_ui_with_layout(
                    egui::vec2(STRIP_WIDTH, ui.available_height()),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| strip::show(app, ui),
                );
                ui.separator();
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        for (tab, label, hint) in Tab::ALL {
                            ui.selectable_value(&mut app.prefabs.workshop.tab, tab, label)
                                .on_hover_text(hint);
                        }
                    });
                    let hint = Tab::ALL
                        .iter()
                        .find(|(tab, ..)| *tab == app.prefabs.workshop.tab)
                        .map_or("", |(_, _, hint)| *hint);
                    ui.weak(hint);
                    ui.separator();
                    match app.prefabs.workshop.tab {
                        Tab::Buildings => buildings::show(app, ui),
                        Tab::Teach => teach::show(app, ui),
                        Tab::Train => train::show(app, ui),
                    }
                });
            });
        });
    app.prefabs.workshop.open = open;
    if open {
        ctx.request_repaint_after(std::time::Duration::from_millis(100));
    }
}

/// `prefab workshop [buildings|teach|train] [train NAME [PREFAB...]]` for smoke scripts: open a
/// tab, or pick buildings (all when none are named) and start training a model.
pub(super) fn script(app: &mut EditorApp, words: &[&str]) -> Result<(), String> {
    app.prefabs.workshop.open = true;
    match words {
        [] | ["buildings"] => app.prefabs.workshop.tab = Tab::Buildings,
        ["teach"] => app.prefabs.workshop.tab = Tab::Teach,
        ["train"] => app.prefabs.workshop.tab = Tab::Train,
        ["train", name, picks @ ..] => {
            app.prefabs.workshop.tab = Tab::Train;
            train::begin(app, name, picks)?;
        }
        other => return Err(format!("unknown workshop command {other:?}")),
    }
    Ok(())
}
