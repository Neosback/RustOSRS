//! Detached prefab authoring window with an independent semantic session and history.
mod author;
mod blueprint;
mod catalog;
mod initialize;
mod lab;
pub(in crate::app) mod overlays;
mod save;
mod session;
mod window;
pub(super) use window::show as window;
#[cfg(not(target_arch = "wasm32"))]
pub(super) use window::viewport_id;
mod commands;
pub(in crate::app) mod workspace;
use super::{
    EditorApp,
    prefab_gallery::stage::{CENTRE, Stage},
};
pub(in crate::app) use commands::{SceneCommand, open_library, request};
use eframe::egui;
use osrs_core::coords::{MapTile, SourcePlane};
use osrs_edit::{MapDocument, PlaceSpec, Prefab, plan_place_exact};
use session::{Choice, Destination, StudioSession};

pub(in crate::app) struct Studio {
    stage: Box<Stage>,
    lab: lab::LabPanel,
    file: String,
    original: Prefab,
    library_baseline: Prefab,
    pad: Option<MapDocument>,
    origin: MapTile,
    session: StudioSession,
    problem: Option<String>,
    relations: bool,
    pub(super) auto_own_painted_floors: bool,
    /// Smoke-test hook: save this window to a PNG (`RUSTOSRS_STUDIO_SCREENSHOT`).
    shot: Option<crate::screenshot::ScreenshotHook>,
}
impl Studio {
    /// Whether the scene is open on this library file.
    pub(in crate::app) fn is_editing(&self, file: &str) -> bool {
        self.file == file
    }

    fn new(app: &mut EditorApp, file: String, ctx: &egui::Context) -> Result<Self, String> {
        let original = app
            .prefabs
            .entry(&file)
            .ok_or("Prefab no longer exists")?
            .prefab
            .clone();
        let mut stage = match super::prefab_gallery::take_stage(app, &file) {
            Some(s) => s,
            None => Box::new(Stage::new(app, ctx)?),
        };
        stage.clear_preview();
        stage.viewer().prefabs.sync_entries_from(&app.prefabs);
        app.prefabs.gallery.open = false;
        Ok(Self {
            stage,
            lab: lab::LabPanel::default(),
            file,
            origin: MapTile::new(
                CENTRE.0 - i32::from(original.width) / 2,
                CENTRE.1 - i32::from(original.height) / 2,
            ),
            library_baseline: original.clone(),
            original,
            pad: None,
            session: StudioSession::default(),
            problem: None,
            relations: false,
            auto_own_painted_floors: true,
            shot: crate::screenshot::ScreenshotHook::for_studio(),
        })
    }
}

/// Draw only the child viewport. Parent-world processing continues in its own viewport.
fn frame(app: &mut EditorApp, ui: &mut egui::Ui, ctx: &egui::Context) -> bool {
    let requested = app.prefabs.studio_request.take();
    let mut studio = app.prefabs.studio.take();
    if studio.is_none()
        && let Some(file) = requested.clone()
    {
        match Studio::new(app, file, ctx) {
            Ok(s) => studio = Some(Box::new(s)),
            Err(e) => app.prefabs.say(e),
        }
    }
    let Some(mut studio) = studio else {
        return false;
    };
    let mut destination = None;
    if let Some(file) = requested
        && file != studio.file
    {
        destination = studio.session.request(
            Destination::Switch(file),
            studio.lab.author_revision(studio.stage.viewer()),
        );
    }
    studio.stage.step(ctx);
    if studio.pad.is_none() && studio.stage.ready() {
        match studio.initialize() {
            Ok(()) => {
                let viewer = studio.stage.viewer();
                if let Ok(script) = std::env::var("RUSTOSRS_STUDIO_SCRIPT") {
                    viewer.run_script(&script);
                } else {
                    app.script
                        .rebase_frames(app.diagnostics.frames(), viewer.diagnostics.frames());
                    viewer.script = std::mem::take(&mut app.script);
                }
            }
            Err(e) => studio.problem = Some(e),
        }
    }
    let ready = studio.pad.is_some();
    if studio.relations && !studio.lab.active {
        let mut live_prefab = studio.original.clone();
        if ready {
            let origin = studio.origin;
            let mut objs: Vec<_> = studio
                .stage
                .viewer()
                .session
                .document()
                .object_entries()
                .into_iter()
                .filter_map(|p| {
                    let lx = p.position.tile.x - origin.x;
                    let ly = p.position.tile.y - origin.y;
                    (lx >= 0 && ly >= 0).then(|| osrs_edit::PrefabObject {
                        id: p.object_id.get(),
                        x: lx as u16,
                        y: ly as u16,
                        plane: p.position.plane.index().get(),
                        kind: p.kind.get(),
                        orientation: p.orientation.get(),
                    })
                })
                .collect();
            objs.sort_by_key(|o| (o.plane, o.y, o.x, o.kind, o.id, o.orientation));
            live_prefab.objects = objs;
        }
        studio.stage.viewer().prefabs.studio_overlay = Some(overlays::Overlay {
            prefab: live_prefab,
            origin: studio.origin,
            selected: studio
                .stage
                .viewer()
                .prefabs
                .studio_overlay
                .as_ref()
                .and_then(|v| v.selected),
        });
    } else {
        studio.stage.viewer().prefabs.studio_overlay = None;
    }
    let revision = studio.lab.author_revision(studio.stage.viewer());
    let dirty = studio.session.dirty(revision);
    let mut save = false;
    let mut save_as = false;
    let mut expand_bounds = false;
    let mut leave = ctx.input(|i| i.viewport().close_requested());
    if leave {
        ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
    }
    let mut revert = false;
    // Which tab: the practice room is the lab being on.
    let mut tab = app.prefabs.workshop.tab;
    if studio.lab.active {
        tab = crate::app::workshop::Tab::Practice;
    } else if tab == crate::app::workshop::Tab::Practice && ready {
        studio.lab.active = true;
    }
    let mut picked: Option<crate::app::workshop::Tab> = None;
    egui::Panel::top("prefab-studio-header").show(ui, |ui| {
        ui.horizontal_wrapped(|ui| {
            ui.strong(format!(
                "Workshop · {} ({}×{}×{}){}",
                studio.original.name,
                studio.original.width,
                studio.original.height,
                studio.original.planes,
                if dirty { " · Unsaved" } else { "" }
            ));
            ui.separator();
            for (t, label, hint) in crate::app::workshop::Tab::ALL {
                if ui
                    .selectable_label(tab == t, label)
                    .on_hover_text(hint)
                    .clicked()
                {
                    picked = Some(t);
                }
            }
            ui.separator();
            leave |= ui.button("Close window").clicked();
        });
        if tab == crate::app::workshop::Tab::Scene {
            ui.horizontal_wrapped(|ui| {
                save = ui
                    .add_enabled(ready, egui::Button::new("Save"))
                    .clicked();
                save_as = ui
                    .add_enabled(ready, egui::Button::new("Save as new"))
                    .clicked();
                expand_bounds = ui
                    .add_enabled(ready, egui::Button::new("Expand bounds"))
                    .on_hover_text(
                        "Grow prefab dimensions to encompass all placed objects and painted terrain",
                    )
                    .clicked();
                revert = ui
                    .add_enabled(ready, egui::Button::new("Revert"))
                    .clicked();
                ui.checkbox(&mut studio.relations, "What belongs to this building")
                    .on_hover_text(
                        "Show which floor tiles and pieces are part of this building, and fix them",
                    );
                ui.checkbox(
                    &mut studio.stage.viewer().ui.prefab_workspace.open,
                    "Workspace tools",
                );
            });
        }
        if let Some(problem) = &studio.problem {
            ui.colored_label(ui.visuals().error_fg_color, problem);
        }
    });
    if let Some(t) = picked {
        if t == crate::app::workshop::Tab::Practice {
            studio.lab.active = true;
        } else if studio.lab.active
            && let Err(e) = studio.lab.leave(studio.stage.viewer())
        {
            studio.problem = Some(e);
        }
        tab = if studio.lab.active {
            crate::app::workshop::Tab::Practice
        } else {
            t
        };
    }
    if let Some(hook) = &mut studio.shot {
        hook.tick(ctx, app.diagnostics.frames());
    }
    app.prefabs.workshop.tab = tab;
    app.prefabs.workshop.editing = Some(studio.file.clone());
    crate::app::workshop::ask(app, ctx);
    if expand_bounds && let Err(e) = save::expand_bounds_to_fit(&mut studio) {
        studio.problem = Some(e);
    }
    if ready {
        if ctx.input_mut(|i| {
            i.consume_shortcut(&egui::KeyboardShortcut::new(
                egui::Modifiers::COMMAND,
                egui::Key::S,
            ))
        }) {
            save = true;
        }
        if studio.session.pending.is_none() {
            egui::Panel::bottom("workshop-runs")
                .default_size(100.0)
                .resizable(true)
                .show(ui, |ui| crate::app::workshop::policy::tray(app, ui));
            let viewer = studio.stage.viewer();
            if tab.is_page() {
                crate::app::workshop::task_view(viewer, app, ui, ctx);
            } else if studio.lab.active {
                super::script::lab_controls(viewer, ctx);
                viewer.settings.prefs.tool = crate::paint::Tool::Select;
                viewer.prefabs.disarm();
                viewer.placer.disarm();
                viewer.npcs.cancel_gesture();
                egui::Panel::left("studio-lab-controls")
                    .default_size(400.0)
                    .resizable(true)
                    .show(ui, |ui| {
                        egui::ScrollArea::vertical().show(ui, |ui| {
                            if let Some(pad) = &studio.pad {
                                studio.lab.show(viewer, pad, &studio.original, ui);
                            }
                        });
                    });
                viewer.toolbar(ui);
                super::viewport::read_only(viewer, ui, ctx);
            } else {
                let mut workspace = std::mem::take(&mut viewer.ui.prefab_workspace);
                if std::mem::take(&mut app.prefabs.workshop.want_builder) {
                    workspace.show_builder();
                }
                workspace.show(viewer, &studio.file, &studio.original, ui, &mut |ui| {
                    crate::app::workshop::library_page(app, ui)
                });
                viewer.ui.prefab_workspace = workspace;
                super::input::editor_actions(viewer, ctx, false);
                viewer.tool_strip(ui);
                super::prefab_panel::review_placement(viewer, ctx);
                viewer.toolbar(ui);
                viewer.status_bar(ui);
                viewer.tool_drawer(ui);
                viewer.dock(ui);
                super::viewport::frame(viewer, ui, ctx);
                viewer.quick_tools_hud(ctx);
            }
        }
        if studio.relations && tab == crate::app::workshop::Tab::Scene {
            overlays::panel(&mut studio, ctx);
        }
        let viewer = studio.stage.viewer();
        match viewer
            .prefabs
            .studio_command
            .take()
            .or_else(|| app.prefabs.studio_command.take())
        {
            Some(SceneCommand::Save) => save = true,
            Some(SceneCommand::SaveAsNew) => save_as = true,
            Some(SceneCommand::Close) => leave = true,
            Some(SceneCommand::Revert) => revert = true,
            Some(SceneCommand::Report) => {
                eprintln!(
                    "smoke: studio: {} objects, undo {}, dirty {}",
                    viewer.session.document().object_entries().len(),
                    viewer.can_undo(),
                    dirty
                );
                studio.lab.report();
                eprintln!(
                    "smoke: studio viewport {:?}, frames {}",
                    ctx.viewport_id(),
                    viewer.diagnostics.frames()
                );
            }
            Some(SceneCommand::Demonstrate) => {
                if let Some(pad) = &studio.pad
                    && let Err(e) = studio
                        .lab
                        .generate_demonstration(viewer, pad, &studio.original)
                {
                    studio.problem = Some(e);
                }
            }
            #[cfg(not(target_arch = "wasm32"))]
            Some(SceneCommand::PolicyReplay(replay)) => {
                if let Some(pad) = &studio.pad {
                    let (task, actions) = *replay;
                    match studio.lab.load_policy(viewer, pad, task, actions) {
                        Ok(()) => app.prefabs.workshop.tab = crate::app::workshop::Tab::Practice,
                        Err(e) => studio.problem = Some(e),
                    }
                }
            }
            Some(SceneCommand::PlayReplay) => studio.lab.play_replay(),
            Some(SceneCommand::Workshop(tab)) => app.prefabs.workshop.tab = tab,
            None => {}
        }
        if let Some(file) = viewer.prefabs.studio_request.take() {
            destination = studio.session.request(Destination::Switch(file), revision);
        }
        studio.stage.viewer().diagnostics.finish_frame(ctx);
    } else {
        egui::CentralPanel::default().show(ui, |ui| {
            ui.label("Preparing the isolated scene and its models…");
            if let Some(problem) = &studio.stage.problem {
                ui.label(problem);
            }
        });
    }
    if let Some(prefab) = studio.lab.accepted.take() {
        match app.prefabs.add(prefab) {
            Ok(_) => {
                studio
                    .stage
                    .viewer()
                    .prefabs
                    .sync_entries_from(&app.prefabs);
                studio.problem =
                    Some("Accepted reconstruction saved as a new library prefab".into());
            }
            Err(e) => studio.problem = Some(e),
        }
    }
    if (save || save_as)
        && !studio.lab.active
        && let Err(e) = save::save(&mut studio, app, save_as)
    {
        studio.problem = Some(e);
    }
    if (leave || revert)
        && studio.lab.active
        && let Err(e) = studio.lab.leave(studio.stage.viewer())
    {
        studio.problem = Some(e);
        leave = false;
        revert = false;
    }
    if leave {
        destination = studio.session.request(
            Destination::Close,
            studio.stage.viewer().session.edit_revision(),
        );
    }
    if revert {
        destination = studio.session.request(
            Destination::Revert,
            studio.stage.viewer().session.edit_revision(),
        );
    }
    if studio.session.pending.is_some() {
        let mut choice = None;
        egui::Modal::new(egui::Id::new("studio-unsaved")).show(ctx, |ui| {
            ui.heading("Save changes to this prefab?");
            ui.label("This prefab has its own history. Your world editor stays open.");
            ui.horizontal(|ui| {
                if ui.button("Save").clicked() {
                    choice = Some(Choice::Save)
                }
                if ui.button("Discard").clicked() {
                    choice = Some(Choice::Discard)
                }
                if ui.button("Cancel").clicked() {
                    choice = Some(Choice::Cancel)
                }
            });
        });
        if let Some(choice) = choice {
            #[cfg(not(target_arch = "wasm32"))]
            if choice == Choice::Cancel {
                app.ui.studio_quit_pending = false;
            }
            let saved = if choice == Choice::Save {
                match save::save(&mut studio, app, false) {
                    Ok(()) => Some(studio.stage.viewer().session.edit_revision()),
                    Err(e) => {
                        studio.problem = Some(e);
                        None
                    }
                }
            } else {
                None
            };
            destination = studio.session.choose(choice, saved);
        }
    }
    if destination.is_some() && std::env::var_os("RUSTOSRS_STUDIO_SCRIPT").is_none() {
        let viewer = studio.stage.viewer();
        viewer
            .script
            .rebase_frames(viewer.diagnostics.frames(), app.diagnostics.frames());
        app.script = std::mem::take(&mut viewer.script);
    }
    match destination {
        Some(Destination::Close) => {
            app.prefabs.load();
        }
        Some(Destination::Revert) => {
            let file = studio.file.clone();
            drop(studio);
            app.prefabs.load();
            match Studio::new(app, file, ctx) {
                Ok(s) => app.prefabs.studio = Some(Box::new(s)),
                Err(e) => app.prefabs.say(e),
            }
        }
        Some(Destination::Switch(file)) => {
            drop(studio);
            app.prefabs.load();
            match Studio::new(app, file, ctx) {
                Ok(s) => app.prefabs.studio = Some(Box::new(s)),
                Err(e) => app.prefabs.say(e),
            }
        }
        None => {
            app.prefabs.studio = Some(studio);
        }
    }
    if app.diagnostics.expired(app.started.elapsed().as_secs_f32()) {
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }
    ctx.request_repaint();
    true
}

#[cfg(test)]
mod tests;
