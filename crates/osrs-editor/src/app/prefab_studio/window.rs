//! A native workspace viewport. The world owns its usual frame and never shares input/history.
use super::*;
pub(in crate::app) fn viewport_id() -> egui::ViewportId {
    egui::ViewportId::from_hash_of("rustosrs-prefab-workspace")
}
pub(in crate::app) fn show(app: &mut EditorApp, ctx: &egui::Context) {
    if app.prefabs.studio.is_none() && app.prefabs.studio_request.is_none() {
        return;
    }
    let builder = egui::ViewportBuilder::default()
        .with_title("RustOSRS · Workshop")
        .with_active(true)
        .with_inner_size([1440.0, 900.0])
        .with_min_inner_size([960.0, 640.0]);
    let opening = app.prefabs.studio.is_none();
    ctx.show_viewport_immediate(viewport_id(), builder, |ui, class| {
        if opening {
            eprintln!(
                "workspace: native={} viewport {:?}",
                matches!(class, egui::ViewportClass::Immediate),
                ui.ctx().viewport_id()
            );
        }
        let child_ctx = ui.ctx().clone();
        frame(app, ui, &child_ctx);
    });
}
