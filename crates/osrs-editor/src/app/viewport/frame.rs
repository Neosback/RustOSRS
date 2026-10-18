//! Shared main-world and isolated-scene viewport composition.
use super::*;

/// Shared viewport composition; callers own lifecycle and chrome.
pub(in crate::app) fn draw(app: &mut EditorApp, ui: &mut egui::Ui, ctx: &egui::Context) {
    egui::CentralPanel::default()
        .frame(egui::Frame::NONE)
        .show(ui, |ui| {
            let available = ui.available_size();
            let (rect, response) = ui.allocate_exact_size(available, egui::Sense::click_and_drag());
            // The viewport renders at the window's pixel density times the resolution scale; every
            // pick and overlay maps target pixels back to points through the same factor.
            let pixels_per_point = ctx.pixels_per_point() * app.settings.prefs.render_scale;
            let width = ((rect.width() * pixels_per_point) as u32).max(16);
            let height = ((rect.height() * pixels_per_point) as u32).max(16);

            let phase = Instant::now();
            app.handle_input(ctx, &response);
            app.diagnostics.phase("input", phase);
            let phase = Instant::now();
            app.update_picking(&response, rect, pixels_per_point, width, height);
            app.diagnostics.phase("pick", phase);

            app.render_viewport_image(ui, rect, width, height);
            let phase = Instant::now();
            app.draw_selection(ui, rect, pixels_per_point, width, height);
            app.draw_selection_extras(
                &ui.painter().with_clip_rect(rect),
                rect,
                pixels_per_point,
                width,
                height,
            );
            app.draw_place_ghost(
                &ui.painter().with_clip_rect(rect),
                rect,
                pixels_per_point,
                width,
                height,
            );
            app.draw_map_icons_3d(
                &ui.painter().with_clip_rect(rect),
                rect,
                pixels_per_point,
                width,
                height,
            );
            app.draw_object_drag(
                &ui.painter().with_clip_rect(rect),
                rect,
                pixels_per_point,
                width,
                height,
            );
            app.draw_spline(
                &ui.painter().with_clip_rect(rect),
                rect,
                pixels_per_point,
                width,
                height,
            );
            sound_panel::draw_world(
                app,
                &ui.painter().with_clip_rect(rect),
                rect,
                pixels_per_point,
                width,
                height,
            );
            prefab_panel::draw_world(
                app,
                &ui.painter().with_clip_rect(rect),
                rect,
                pixels_per_point,
                width,
                height,
            );
            npc_panel::draw_world(
                app,
                &ui.painter().with_clip_rect(rect),
                rect,
                pixels_per_point,
                width,
                height,
            );
            player_panel::draw_world(
                app,
                &ui.painter().with_clip_rect(rect),
                rect,
                pixels_per_point,
                width,
                height,
            );
            super::super::prefab_studio::overlays::draw(
                app,
                &ui.painter().with_clip_rect(rect),
                rect,
                pixels_per_point,
                width,
                height,
            );
            app.object_gizmo(ui, rect, &response);
            app.diagnostics.phase("overlays", phase);
            let phase = Instant::now();
            app.viewport_overlays(ui, rect);
            app.floating_toolbar(ctx, rect);
            app.context_menu(ctx);
            app.diagnostics.phase("viewport_ui", phase);
        });
}

/// Camera-only replay viewport. Lab actions own semantics; scene input cannot edit this projection.
pub(in crate::app) fn read_only(app: &mut EditorApp, ui: &mut egui::Ui, ctx: &egui::Context) {
    egui::CentralPanel::default()
        .frame(egui::Frame::NONE)
        .show(ui, |ui| {
            let (rect, response) =
                ui.allocate_exact_size(ui.available_size(), egui::Sense::click_and_drag());
            let scale = ctx.pixels_per_point() * app.settings.prefs.render_scale;
            app.handle_input(ctx, &response);
            app.render_viewport_image(
                ui,
                rect,
                ((rect.width() * scale) as u32).max(16),
                ((rect.height() * scale) as u32).max(16),
            );
        });
}
