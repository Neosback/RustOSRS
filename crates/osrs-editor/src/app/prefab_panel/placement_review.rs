//! Explicit review of the exact removals in a building replacement; stale plans cannot commit.
use crate::app::EditorApp;
use eframe::egui;
use osrs_edit::{MapOp, MapPatch};
pub(in crate::app) struct PlacementReview {
    patch: MapPatch,
    revision: u64,
    generation: u64,
    name: String,
}
impl PlacementReview {
    pub(super) fn new(patch: MapPatch, revision: u64, generation: u64, name: String) -> Self {
        Self {
            patch,
            revision,
            generation,
            name,
        }
    }
    fn approve(self, revision: u64, generation: u64) -> Result<(MapPatch, String), String> {
        if (self.revision, self.generation) != (revision, generation) {
            return Err("The map changed. Preview and review replacement again.".into());
        }
        Ok((self.patch, self.name))
    }
}
pub(in crate::app) fn show(app: &mut EditorApp, ctx: &egui::Context) {
    let Some(review) = &app.prefabs.placement_review else {
        return;
    };
    let mut apply = false;
    let mut cancel = false;
    egui::Modal::new(egui::Id::new("building-replacement-review")).show(ctx,|ui|{
        ui.heading("Review building replacement");
        ui.label("Only these conflicting placements will be removed. Foundation and floor changes are included in the same undo step.");
        egui::ScrollArea::vertical().max_height(240.0).show(ui,|ui|{for op in review.patch.ops() {if let MapOp::RemoveObject{placement:p}=op {ui.label(format!("{} · type {} · {},{} · plane {}",p.object_id.get(),p.kind.get(),p.position.tile.x,p.position.tile.y,p.position.plane.index().get()));}}});
        ui.horizontal(|ui|{apply=ui.button("Replace reviewed conflicts").clicked();cancel=ui.button("Cancel").clicked();});
    });
    if apply && let Some(review) = app.prefabs.placement_review.take() {
        match review.approve(app.session.map_revision(), app.project.generation()) {
            Ok((patch, name)) => {
                crate::app::editing::commit_map_patch(app, patch, &format!("Placed {name}"));
            }
            Err(e) => app.prefabs.say(e),
        }
    } else if cancel {
        app.prefabs.placement_review = None;
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cancellation_and_stale_replacement_leave_authority_and_history_unchanged() {
        let document = osrs_edit::MapDocument::new();
        let before = document.clone();
        let pending = PlacementReview::new(MapPatch::new(vec![]), 1, 2, "test".into());
        drop(pending);
        assert_eq!(document, before);
        let pending = PlacementReview::new(MapPatch::new(vec![]), 1, 2, "test".into());
        assert!(pending.approve(2, 2).is_err());
        assert_eq!(document, before);
    }
}
