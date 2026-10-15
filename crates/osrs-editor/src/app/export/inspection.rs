//! Isolated read-only viewport. The authoring application remains parked in its owner untouched.
use super::*;
use crate::paint::Tool;
use crate::pick;

pub(super) struct ExportInspection {
    viewer: EditorApp,
    path: PathBuf,
    revision: u64,
}

impl ExportInspection {
    fn new(
        parent: &EditorApp,
        source: InspectionSource,
        ctx: &egui::Context,
    ) -> Result<Self, String> {
        let tile = parent.camera_tile();
        let mut viewer = EditorApp::from_source(
            ctx,
            parent.renderer.render_state(),
            source.source,
            source.path.clone(),
            tile.0 - 64,
            tile.1 - 64,
            false,
        )?;
        viewer.prefs = parent.prefs.clone();
        viewer.prefs.tool = Tool::Select;
        viewer.prefs.inspector_follow = false;
        viewer.interaction.camera = parent.interaction.camera;
        viewer.interaction.view_plane = parent.interaction.view_plane;
        viewer
            .renderer
            .set_render_origin(parent.renderer.render_origin());
        // Opening from File should start at authored content even when the creator is elsewhere.
        if let Some(first) = source.manifest.regions.first() {
            viewer.teleport_to(crate::hud::Destination::Region {
                x: first.region[0],
                y: first.region[1],
            });
        }
        Ok(Self {
            viewer,
            path: source.path,
            revision: source.manifest.edit_revision,
        })
    }

    fn ui(&mut self, ui: &mut egui::Ui) -> bool {
        let ctx = ui.ctx().clone();
        let viewer = &mut self.viewer;
        let mut close = false;
        egui::Panel::top("export-inspection-header").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.strong(format!("READ-ONLY EXPORT · revision {}", self.revision));
                if ui.button("Return to authoring").clicked() {
                    close = true;
                }
                ui.label(self.path.display().to_string());
            });
            ui.horizontal(|ui| {
                ui.label("Plane");
                for plane in 0..4 {
                    if ui
                        .selectable_label(viewer.interaction.view_plane == plane, plane.to_string())
                        .clicked()
                    {
                        viewer.interaction.view_plane = plane;
                        viewer.clear_selection();
                        viewer.world.mark_animation_dirty();
                    }
                }
                ui.separator();
                if ui.button("Focus selected object").clicked() {
                    viewer.focus_selection();
                }
                ui.label("Right drag to look/orbit · Shift + right drag to pan · WASD to fly");
            });
        });
        if close {
            return true;
        }
        viewer.rebase_if_needed();
        viewer.stream(&ctx);
        viewer.update_animations();
        viewer.poll_palette();
        viewer.poll_catalog();
        viewer.assets.poll(&ctx);
        egui::Panel::right("export-inspector")
            .default_size(300.0)
            .show(ui, |ui| {
                if let Some(error) = &viewer.cache_error {
                    ui.colored_label(ui.visuals().error_fg_color, error);
                }
                if let Some(status) = &viewer.status {
                    ui.label(status);
                }
                viewer.inspection_inspector(ui);
            });
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ui, |ui| {
                let (rect, response) =
                    ui.allocate_exact_size(ui.available_size(), egui::Sense::click_and_drag());
                let density = ctx.pixels_per_point() * viewer.prefs.render_scale;
                let width = (rect.width() * density).max(16.0) as u32;
                let height = (rect.height() * density).max(16.0) as u32;
                // Only camera navigation is dispatched. No authoring shortcuts, scripts, tools,
                // context menus, gizmos, save/export commands, or semantic edit routes run here.
                viewer.handle_input(&ctx, &response);
                if let Some(pointer) = response.hover_pos() {
                    let ray = pick::ray_through_pixel(
                        &viewer.interaction.camera,
                        viewer.renderer.render_origin(),
                        width as f32,
                        height as f32,
                        (pointer.x - rect.min.x) * density,
                        (pointer.y - rect.min.y) * density,
                    );
                    viewer.interaction.hover =
                        pick::pick(&viewer.infos, &ray, viewer.interaction.view_plane);
                    if let Some(tile) = viewer.interaction.hover.tile {
                        viewer.interaction.last_hover_tile = Some(tile.tile);
                    }
                    if response.clicked() {
                        viewer.clear_selection();
                        viewer.interaction.inspect_tile =
                            viewer.interaction.hover.tile.map(|p| p.tile);
                        if let Some(object) = viewer.interaction.hover.object.clone() {
                            viewer.select_only(object);
                        }
                    }
                } else {
                    viewer.interaction.hover = pick::Pick::default();
                }
                if let Some(object) = viewer.interaction.selected.clone() {
                    let key = (object.region, object.index);
                    if !viewer.outlines.contains_key(&key)
                        && viewer.outline_requested.insert(key)
                        && let Some(loc) = viewer
                            .infos
                            .get(&object.region)
                            .and_then(|i| i.locs.get(object.index))
                    {
                        viewer
                            .world
                            .request_outline(object.region, object.index, loc.clone());
                    }
                }
                viewer.render_viewport_image(ui, rect, width, height);
                viewer.draw_selection(ui, rect, density, width, height);
            });
        ctx.request_repaint();
        close
    }
}

impl EditorApp {
    pub(in crate::app) fn export_inspection_ui(&mut self, ui: &mut egui::Ui) -> bool {
        let ctx = ui.ctx().clone();
        if let Some(source) = self.exports.pending_inspection.take() {
            match ExportInspection::new(self, source, &ctx) {
                Ok(inspection) => {
                    self.exports.open = false;
                    self.exports.inspection = Some(Box::new(inspection));
                }
                Err(error) => self.exports.failure = Some(error),
            }
        }
        let Some(inspection) = &mut self.exports.inspection else {
            return false;
        };
        // Closing the app while reviewing must return to the authoring unsaved-work guard.
        if ctx.input(|i| i.viewport().close_requested()) {
            self.exports.inspection = None;
            self.exports.open = false;
            // The normal authoring frame now handles its own unsaved-work close guard.
            return false;
        }
        if inspection.ui(ui) {
            self.exports.inspection = None;
            self.exports.open = true;
            let build = self
                .cache_build
                .as_ref()
                .map(|(build, _)| *build)
                .unwrap_or(241);
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(format!(
                "RustOSRS: build {build}"
            )));
        }
        true
    }
}
