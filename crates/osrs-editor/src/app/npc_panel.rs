//! The NPC dock panel and its viewport gestures: importing and authoring an offline scenario,
//! running the playtest, and showing anchors, footprints, routes and blocked-movement reasons.
//!
//! Drawing returns nothing that mutates the map. Scenario edits go through the session's
//! transaction gateway (so they share chronological undo/redo with map edits); playback commands
//! are queued on `NpcRuntime` and never enter history.

mod authoring;
mod scenario;
pub(in crate::app) mod script;
mod viewport;

use crate::app::{
    EditorApp,
    npc_runtime::{ImportEvent, NpcHost, OpenKind},
};
use eframe::egui;
use osrs_core::{
    coords::{MapTile, PlaneIndex},
    ids::VarpId,
    scenario::{ActorLocation, ScenarioDefinition},
};

use crate::app::npc_runtime::{AutoLoad, NpcDisplay};
use authoring::{place_section, playback_section, selection_section};
use scenario::advanced_section;
pub(in crate::app) use scenario::load_sidecar;
pub(super) use viewport::{armed_input, draw_world};

const SIDECAR_FORMAT: &str = "rustosrs-npc-scenario";
const FACINGS: [(&str, u16); 8] = [
    ("S", 0),
    ("SW", 256),
    ("W", 512),
    ("NW", 768),
    ("N", 1024),
    ("NE", 1280),
    ("E", 1536),
    ("SE", 1792),
];

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Sidecar {
    format: String,
    scenario: ScenarioDefinition,
}

fn actor_location(tile: (i32, i32), plane: u8) -> Option<ActorLocation> {
    Some(ActorLocation {
        tile: MapTile::new(tile.0, tile.1),
        plane: PlaneIndex::new(plane)?,
    })
}

/// Drive the owner once per frame (called from the frame loop before the chrome is drawn).
pub(super) fn update(app: &mut EditorApp, ctx: &egui::Context) {
    poll_tasks(app);
    start_loading_if_asked(app);
    let camera = app.camera_tile();
    let display = NpcDisplay {
        enabled: app.settings.prefs.npc.spawns,
        distance_tiles: i32::from(app.settings.prefs.npc.distance),
        max_drawn: usize::from(app.settings.prefs.npc.max_drawn),
        models: app.settings.prefs.npc.models,
    };
    let varps = app
        .settings
        .prefs
        .player
        .varps
        .iter()
        .map(|(id, value)| (VarpId::new(*id), *value))
        .collect();
    let mut host = NpcHost {
        session: &app.session,
        world: &app.world,
        renderer: &mut app.renderer,
        dt: ctx.input(|i| i.stable_dt),
        camera,
        display,
        varps,
    };
    if app.npcs.update(&mut host) {
        ctx.request_repaint();
    }
}

fn poll_tasks(app: &mut EditorApp) {
    if let Some(job) = &mut app.npcs.import_job
        && let Some(event) = job.poll()
    {
        app.npcs.import_job = None;
        match event {
            ImportEvent::Installation(Ok(data)) => {
                app.npcs.file_selection = vec![false; data.spawn_files.len()];
                app.npcs.preview = None;
                app.npcs.notice = Some(format!(
                    "Read OpenRune revision {}: {} spawn files",
                    data.revision,
                    data.spawn_files.len()
                ));
                app.npcs.installation = Some(data);
                if app.npcs.auto == AutoLoad::Reading {
                    // The switch loads every spawn file without asking which ones.
                    #[cfg(not(target_arch = "wasm32"))]
                    if let Some(data) = &app.npcs.installation {
                        app.npcs.import_job =
                            Some(crate::app::npc_runtime::preview_import(data, None));
                        app.npcs.auto = AutoLoad::Parsing;
                    }
                }
            }
            ImportEvent::Installation(Err(error)) => {
                let message = format!("Could not read the OpenRune installation: {error}");
                if app.npcs.auto != AutoLoad::Idle {
                    app.npcs.auto = AutoLoad::Failed(message.clone());
                }
                app.npcs.notice = Some(message);
            }
            ImportEvent::Preview(preview) => {
                app.npcs.preview = Some(preview);
                if app.npcs.auto == AutoLoad::Parsing {
                    scenario::apply_import(app);
                    app.npcs.auto =
                        if app.session.scenario().definition().is_some() {
                            AutoLoad::Loaded
                        } else {
                            AutoLoad::Failed(app.npcs.notice.clone().unwrap_or_else(|| {
                                "The OpenRune spawns could not be loaded".into()
                            }))
                        };
                }
            }
        }
    }
    if let Some((kind, task)) = &mut app.npcs.open_task
        && let Some(result) = task.poll()
    {
        let kind = *kind;
        app.npcs.open_task = None;
        match result {
            Ok(Some(picked)) => match kind {
                OpenKind::Scenario => load_sidecar(app, &picked.name, &picked.text),
            },
            Ok(None) => {}
            Err(error) => app.npcs.notice = Some(error),
        }
    }
    if let Some(task) = &mut app.npcs.save_task
        && let Some(result) = task.poll()
    {
        app.npcs.save_task = None;
        match result {
            Ok(Some(name)) => app.npcs.notice = Some(format!("Saved {name}")),
            Ok(None) => {}
            Err(error) => app.npcs.notice = Some(error),
        }
    }
}

/// The switch asked for spawns and none are loaded: read them from the installation.
fn start_loading_if_asked(app: &mut EditorApp) {
    if !app.settings.prefs.npc.spawns
        || app.npcs.auto != AutoLoad::Idle
        || app.npcs.import_job.is_some()
        || app.npcs.source().is_none()
        || app.session.scenario().definition().is_some()
    {
        return;
    }
    begin_loading(app);
}

/// Read the spawns of the OpenRune installation the open cache belongs to.
pub(super) fn begin_loading(app: &mut EditorApp) {
    #[cfg(not(target_arch = "wasm32"))]
    match crate::app::names::installation_of(app.project.cache_label()) {
        Some(root) => {
            app.npcs.auto = AutoLoad::Reading;
            app.npcs.import_job = Some(crate::app::npc_runtime::read_installation(root));
        }
        None => {
            app.npcs.auto = AutoLoad::Failed(
                "The open cache is not an OpenRune LIVE cache. Choose the OpenRune folder under Advanced."
                    .into(),
            );
        }
    }
    #[cfg(target_arch = "wasm32")]
    {
        app.npcs.auto = AutoLoad::Failed("Loading OpenRune spawns needs the native editor".into());
    }
}

/// One plain sentence on where the spawns stand.
fn status_line(app: &EditorApp) -> String {
    let count = app
        .session
        .scenario()
        .definition()
        .map_or(0, |d| d.spawns.len());
    match &app.npcs.auto {
        AutoLoad::Reading => "Reading the OpenRune installation...".into(),
        AutoLoad::Parsing => "Reading the spawn files...".into(),
        AutoLoad::Failed(reason) => reason.clone(),
        _ if count > 0 => format!("{count} spawns loaded"),
        _ => "No spawns loaded".into(),
    }
}

pub(super) fn show(app: &mut EditorApp, ui: &mut egui::Ui) {
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 4.0;
            if app.npcs.source().is_none() {
                ui.weak("Open a cache to see NPCs.");
                return;
            }
            if let Some(notice) = app.npcs.notice.clone() {
                ui.horizontal_wrapped(|ui| {
                    ui.label(notice);
                    if ui.small_button(egui_phosphor::regular::X).clicked() {
                        app.npcs.notice = None;
                    }
                });
            }
            crate::theme::toggle(
                ui,
                &mut app.settings.prefs.npc.spawns,
                "Show OpenRune spawns",
            )
            .on_hover_text("Loads every NPC the OpenRune server places and draws the ones near the camera. Off hides them and stops them moving.");
            let failed = matches!(app.npcs.auto, AutoLoad::Failed(_));
            let status = status_line(app);
            if failed {
                ui.colored_label(egui::Color32::LIGHT_RED, status);
            } else {
                ui.weak(status);
            }
            if app.settings.prefs.npc.spawns && app.session.scenario().definition().is_some() {
                playback_section(app, ui);
                selection_section(app, ui);
                place_section(app, ui);
            }
            advanced_section(app, ui);
            diagnostics_section(app, ui);
        });
}

fn diagnostics_section(app: &mut EditorApp, ui: &mut egui::Ui) {
    if app.npcs.diagnostics().is_empty() {
        return;
    }
    egui::CollapsingHeader::new(format!("Diagnostics ({})", app.npcs.diagnostics().len()))
        .id_salt("npc-diagnostics")
        .show(ui, |ui| {
            for line in app.npcs.diagnostics() {
                ui.weak(line);
            }
        });
}
