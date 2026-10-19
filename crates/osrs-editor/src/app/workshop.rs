//! The Workshop: the plain-language side of the one building window (the Studio). The window
//! has a tab for each thing you do with buildings: **Scene** (look at and change a building in
//! 3D, build new ones, your saved library), **Names** (tell the editor what objects in a building
//! are), **Train** (let it learn their style, then ask it to build new ones), **Paths** (teach it
//! to lay roads), **WFC** (see what the floor-pattern builder makes of a building) and the
//! **Practice room** (watch a robot rebuild a building step by step).
//!
//! This module draws every tab except Scene and the Practice room, which are the Studio's own.
//! Plain words everywhere: the window is for people who are not programmers. Nothing here edits
//! the map except the buttons that say so (placing a building, laying paths); everything else
//! changes the library, the name book or the models.

mod buildings;
pub(in crate::app) mod paths;
mod places;
mod strip;
mod table;
mod teach;
mod train;
mod wfc;

use crate::app::EditorApp;
use eframe::egui;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(in crate::app) enum Tab {
    /// The 3D scene with its library and builder (the Studio's own screen).
    #[default]
    Scene,
    Names,
    Train,
    Paths,
    Wfc,
    /// The robot replay (the Studio's own screen).
    Practice,
}

impl Tab {
    pub(in crate::app) const ALL: [(Self, &'static str, &'static str); 6] = [
        (
            Self::Scene,
            "Scene",
            "Look at and change a building in 3D, build new ones, and your saved library",
        ),
        (
            Self::Names,
            "Names",
            "Tell the editor what things in a building are",
        ),
        (
            Self::Train,
            "Train",
            "Let the editor learn from your buildings, then ask it to build new ones",
        ),
        (
            Self::Paths,
            "Paths",
            "Teach the editor to lay roads and trails from real ones",
        ),
        (
            Self::Wfc,
            "Floor patterns",
            "See what the floor-pattern builder (WFC) makes of a building, and test it",
        ),
        (
            Self::Practice,
            "Practice room",
            "Watch a robot rebuild a building step by step with the editor's own tools",
        ),
    ];

    /// Whether this tab is drawn here (the rest are the Studio's own screens).
    pub(in crate::app) fn is_page(self) -> bool {
        matches!(self, Self::Names | Self::Train | Self::Paths | Self::Wfc)
    }
}

#[derive(Default)]
pub(in crate::app) struct Workshop {
    pub(in crate::app) tab: Tab,
    /// The building open in the scene (set by the Studio every frame).
    pub(in crate::app) editing: Option<String>,
    /// Show the Scene tab's builder page next frame.
    pub(in crate::app) want_builder: bool,
    strip: strip::StripState,
    teach: teach::TeachState,
    train: train::TrainState,
    wfc: wfc::WfcState,
    places: places::Places,
    paths: paths::PathsState,
}

const STRIP_WIDTH: f32 = 300.0;

/// Once per frame in the main window: take what the background trainers have said.
pub(super) fn window(app: &mut EditorApp, _ctx: &egui::Context) {
    train::poll(app);
    paths::poll(app);
}

/// Open the building window on `tab` (the Studio opens on the selected building, or on a blank
/// scene when there is none). When it is already open it just changes tab.
pub(in crate::app) fn open(app: &mut EditorApp, tab: Tab) {
    app.prefabs.workshop.tab = tab;
    if app.prefabs.studio.is_none() {
        super::prefab_studio::open_library(app);
    }
}

/// Show the Scene tab's builder (where a trained style can be used on a site).
pub(in crate::app) fn open_builder(app: &mut EditorApp) {
    app.prefabs.workshop.want_builder = true;
    open(app, Tab::Scene);
}

/// The delete question, in whichever window shows the list. One at a time.
pub(in crate::app) fn ask(app: &mut EditorApp, ctx: &egui::Context) {
    strip::ask(app, ctx);
}

/// The saved buildings, with what you can do to the selected one: the Scene tab's Library page.
/// Double-clicking a building opens it in the scene.
pub(in crate::app) fn library_page(app: &mut EditorApp, ui: &mut egui::Ui) {
    if ui
        .button("New blank scene")
        .on_hover_text("Start a new building from an empty 12 by 12 floor")
        .clicked()
    {
        match app.prefabs.create_blank_scene() {
            Ok(file) => app.prefabs.studio_request = Some(file),
            Err(error) => app.prefabs.say(error),
        }
    }
    let height = ui.available_height();
    ui.allocate_ui_with_layout(
        egui::vec2(ui.available_width(), (height * 0.5).max(240.0)),
        egui::Layout::top_down(egui::Align::Min),
        |ui| strip::show(app, ui),
    );
    ui.separator();
    egui::ScrollArea::vertical()
        .id_salt("library-details")
        .auto_shrink([false, false])
        .show(ui, |ui| buildings::show(app, ui));
}

/// Draw the page of a tab that is drawn here (see [`Tab::is_page`]): the list of buildings on the
/// left where the tab works on one, then the tab itself.
pub(in crate::app) fn page(app: &mut EditorApp, ui: &mut egui::Ui) {
    let tab = app.prefabs.workshop.tab;
    ui.horizontal_top(|ui| {
        // The list of saved buildings is not needed to teach paths.
        if tab != Tab::Paths {
            ui.allocate_ui_with_layout(
                egui::vec2(STRIP_WIDTH, ui.available_height()),
                egui::Layout::top_down(egui::Align::Min),
                |ui| strip::show(app, ui),
            );
            ui.separator();
        }
        ui.vertical(|ui| {
            let hint = Tab::ALL
                .iter()
                .find(|(t, ..)| *t == tab)
                .map_or("", |(_, _, hint)| *hint);
            ui.weak(hint);
            ui.separator();
            match tab {
                Tab::Names => teach::show(app, ui),
                Tab::Train => train::show(app, ui),
                Tab::Paths => paths::show(app, ui),
                Tab::Wfc => wfc::show(app, ui),
                Tab::Scene | Tab::Practice => {}
            }
        });
    });
    ui.ctx()
        .request_repaint_after(std::time::Duration::from_millis(100));
}

/// `prefab workshop [buildings|teach|train] [train NAME [PREFAB...]]` for smoke scripts: open a
/// tab, or pick buildings (all when none are named) and start training a model.
pub(super) fn script(app: &mut EditorApp, words: &[&str]) -> Result<(), String> {
    open(app, Tab::Scene);
    match words {
        [] | ["scene"] => app.prefabs.workshop.tab = Tab::Scene,
        ["buildings", name @ ..] | ["scene", name @ ..] => {
            app.prefabs.workshop.tab = Tab::Scene;
            select(app, name)?;
        }
        ["practice"] => app.prefabs.workshop.tab = Tab::Practice,
        ["names"] | ["teach"] => app.prefabs.workshop.tab = Tab::Names,
        ["train"] => app.prefabs.workshop.tab = Tab::Train,
        ["wfc", name @ ..] => {
            app.prefabs.workshop.tab = Tab::Wfc;
            select(app, name)?;
            if name.get(1) == Some(&"run") {
                wfc::request_run(app);
            }
        }
        ["paths", rest @ ..] => {
            app.prefabs.workshop.tab = Tab::Paths;
            paths::script(app, rest)?;
        }
        ["train", name, picks @ ..] => {
            app.prefabs.workshop.tab = Tab::Train;
            train::begin(app, name, picks)?;
        }
        other => return Err(format!("unknown workshop command {other:?}")),
    }
    Ok(())
}

/// Today as `YYYY-MM-DD` for the files the Workshop writes.
pub(super) fn train_date() -> String {
    train::today()
}

/// Pick the saved building called `name` (underscores stand for spaces); nothing to do for no name.
fn select(app: &mut EditorApp, name: &[&str]) -> Result<(), String> {
    let Some(name) = name.first().map(|n| n.replace('_', " ")) else {
        return Ok(());
    };
    let file = app
        .prefabs
        .entries()
        .iter()
        .find(|e| e.prefab.name == name)
        .map(|e| e.file.clone())
        .ok_or_else(|| format!("no saved building named {name:?}"))?;
    app.prefabs.selected = Some(file);
    Ok(())
}

pub(in crate::app) use train::live::{best_seed, chosen_model, offer_kits};
