//! Full-frame isolated prefab editing. The parent document and its history stay parked.
mod catalog;
mod lab;
pub(in crate::app) mod overlays;
mod save;
mod session;
use super::{
    EditorApp,
    prefab_gallery::stage::{CENTRE, Stage},
};
use eframe::egui;
use osrs_core::coords::{MapTile, SourcePlane};
use osrs_edit::{MapDocument, PlaceSpec, Prefab, plan_place_exact};
use session::{Choice, Destination, StudioSession};

pub(in crate::app) enum SceneCommand {
    Save,
    SaveAsNew,
    Close,
    Revert,
    Report,
}

pub(in crate::app) struct Studio {
    stage: Box<Stage>,
    lab: lab::LabPanel,
    file: String,
    original: Prefab,
    pad: Option<MapDocument>,
    origin: MapTile,
    session: StudioSession,
    problem: Option<String>,
    relations: bool,
    pub(super) auto_own_painted_floors: bool,
}
impl Studio {
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
            original,
            pad: None,
            session: StudioSession::default(),
            problem: None,
            relations: true,
            auto_own_painted_floors: true,
        })
    }
    fn initialize(&mut self) -> Result<(), String> {
        let viewer = self.stage.viewer();
        let pad = viewer.session.document().clone();
        let plane = SourcePlane::new(0).ok_or("plane")?;
        let plan = plan_place_exact(
            &pad,
            &self.original,
            PlaceSpec {
                origin: self.origin,
                base_plane: plane,
                quarter_turns: 0,
                terrain: true,
                heights: true,
                objects: true,
                replace_objects: false,
            },
            &|id, _| viewer.placer.object_size(id.get()),
        )
        .map_err(|e| e.to_string())?;
        if !plan.patch.is_empty()
            && !super::editing::apply_baseline_patch(viewer, plan.patch, "Prefab scene baseline")
        {
            return Err("Scene projection failed; retry after loading.".into());
        }
        viewer.interaction.clear_selection();
        viewer.interaction.drop_pending_reselect();
        viewer.settings.prefs.properties_panel = true;
        viewer.settings.prefs.rail_tab = crate::settings::RailTab::Inspect;
        viewer.settings.prefs.hover_tile = true;
        let side = f32::from(self.original.width.max(self.original.height)).max(6.0);
        let origin = viewer.renderer.render_origin();
        viewer.interaction.focus_camera(
            [
                (CENTRE.0 * 128 + 64 - origin.0) as f32,
                -(side * 40.0).min(900.0),
                (CENTRE.1 * 128 + 64 - origin.1) as f32,
            ],
            (side * 128.0 * 1.5).clamp(900.0, 9000.0),
        );
        self.session.baseline(viewer.session.edit_revision());
        self.pad = Some(pad);
        Ok(())
    }
}

pub(in crate::app) fn request(app: &mut EditorApp, file: String) {
    app.prefabs.studio_request = Some(file);
}

/// Returns true while Studio owns the frame. No parent input, streaming or project UI runs.
pub(super) fn frame(app: &mut EditorApp, ui: &mut egui::Ui, ctx: &egui::Context) -> bool {
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
            studio.stage.viewer().session.edit_revision(),
        );
    }
    studio.stage.step(ctx);
    if studio.pad.is_none() && studio.stage.ready() {
        match studio.initialize() {
            Ok(()) => {
                let viewer = studio.stage.viewer();
                app.script
                    .rebase_frames(app.diagnostics.frames(), viewer.diagnostics.frames());
                viewer.script = std::mem::take(&mut app.script);
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
    egui::Panel::top("prefab-studio-header").show(ui, |ui| {
        ui.horizontal_wrapped(|ui| {
            ui.strong(format!(
                "Prefab Studio · {} ({}×{}×{}){}",
                studio.original.name,
                studio.original.width,
                studio.original.height,
                studio.original.planes,
                if dirty { " · Unsaved" } else { "" }
            ));
            save = ui
                .add_enabled(ready && !studio.lab.active, egui::Button::new("Save"))
                .clicked();
            save_as = ui
                .add_enabled(
                    ready && !studio.lab.active,
                    egui::Button::new("Save as new"),
                )
                .clicked();
            expand_bounds = ui
                .add_enabled(
                    ready && !studio.lab.active,
                    egui::Button::new("Expand bounds"),
                )
                .on_hover_text(
                    "Grow prefab dimensions to encompass all placed objects and painted terrain",
                )
                .clicked();
            revert = ui
                .add_enabled(ready && !studio.lab.active, egui::Button::new("Revert"))
                .clicked();
            leave |= ui.button("Return to world").clicked();
            ui.checkbox(&mut studio.relations, "Relationships");
            if ui.selectable_label(studio.lab.active, "Lab").clicked() {
                studio.lab.active = true;
            }
            if ui.selectable_label(!studio.lab.active, "Edit").clicked()
                && let Err(e) = studio.lab.leave(studio.stage.viewer())
            {
                studio.problem = Some(e);
            }
        });
        if let Some(problem) = &studio.problem {
            ui.colored_label(ui.visuals().error_fg_color, problem);
        }
    });
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
            let viewer = studio.stage.viewer();
            if studio.lab.active {
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
        if studio.relations && !studio.lab.active {
            overlays::panel(&mut studio, ctx);
        }
        let viewer = studio.stage.viewer();
        match viewer.prefabs.studio_command.take() {
            Some(SceneCommand::Save) => save = true,
            Some(SceneCommand::SaveAsNew) => save_as = true,
            Some(SceneCommand::Close) => leave = true,
            Some(SceneCommand::Revert) => revert = true,
            Some(SceneCommand::Report) => eprintln!(
                "smoke: studio: {} objects, undo {}, dirty {}",
                viewer.session.document().object_entries().len(),
                viewer.can_undo(),
                dirty
            ),
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
                studio.problem =
                    Some("Accepted reconstruction saved as a new library prefab".into())
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
            ui.label("The world document remains parked while this scene is open.");
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
    if destination.is_some() {
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
            match Studio::new(app, file, ctx) {
                Ok(s) => app.prefabs.studio = Some(Box::new(s)),
                Err(e) => app.prefabs.say(e),
            }
        }
        Some(Destination::Switch(file)) => {
            drop(studio);
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
    app.diagnostics.finish_frame(ctx);
    ctx.request_repaint();
    true
}

#[cfg(test)]
mod tests;
