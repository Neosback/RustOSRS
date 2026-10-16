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

use authoring::{place_section, playback_section, selection_section};
use scenario::{load_sidecar, scenario_section};
pub(super) use viewport::{armed_input, draw_world};

/// Spawn anchors farther than this many tiles from the camera are not drawn.
const DRAW_RADIUS_TILES: i32 = 64;
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
    let (camera, radius) = (
        app.camera_tile(),
        (app.settings.prefs.load_radius.max(1) + 1) * 64,
    );
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
        radius_tiles: radius,
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
            }
            ImportEvent::Installation(Err(error)) => {
                app.npcs.notice =
                    Some(format!("Could not read the OpenRune installation: {error}"));
            }
            ImportEvent::Preview(preview) => app.npcs.preview = Some(preview),
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

pub(super) fn show(app: &mut EditorApp, ui: &mut egui::Ui) {
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 4.0;
            if app.npcs.source().is_none() {
                ui.weak("Open a cache to author NPCs.");
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
            scenario_section(app, ui);
            if app.session.scenario().definition().is_some() {
                playback_section(app, ui);
                egui::CollapsingHeader::new("Edit spawns (advanced)")
                    .id_salt("npc-edit")
                    .show(ui, |ui| {
                        place_section(app, ui);
                        selection_section(app, ui);
                    });
            }
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
