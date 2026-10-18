//! Initialize a disposable scene baseline without author history.
use super::*;
impl Studio {
    pub(super) fn initialize(&mut self) -> Result<(), String> {
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
            && !crate::app::editing::apply_baseline_patch(
                viewer,
                plan.patch,
                "Prefab scene baseline",
            )
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
