//! Review a frozen placement patch against current semantic, asset, gesture and definition inputs.
use crate::app::{EditorApp, prefab_runtime::Armed};
use eframe::egui;
use osrs_core::{coords::MapTile, ids::ObjectId};
use osrs_edit::{LocKind, MapOp, MapPatch, Prefab, PrefabError};
use std::collections::BTreeMap;

pub(super) type DefinitionInputs = BTreeMap<(ObjectId, LocKind), Option<(u8, u8)>>;

pub(super) struct PlacementInput {
    pub(super) armed: Armed,
    pub(super) gesture: Option<Armed>,
    pub(super) prefab: Prefab,
    pub(super) plane: u8,
    pub(super) origin: MapTile,
    pub(super) definitions: DefinitionInputs,
}

struct Current<'a> {
    revision: u64,
    generation: u64,
    armed: Option<&'a Armed>,
    prefab: Option<&'a Prefab>,
    plane: u8,
    size_of: &'a dyn Fn(ObjectId, LocKind) -> Option<(u8, u8)>,
}

pub(in crate::app) struct PlacementReview {
    patch: MapPatch,
    revision: u64,
    generation: u64,
    name: String,
    input: PlacementInput,
}

/// Known source dimensions win; captured dimensions support portable/headless placement.
/// No unchecked narrowing or one-tile guess may enter the fallible semantic planner.
pub(super) fn resolve_size(
    prefab: &Prefab,
    id: ObjectId,
    kind: LocKind,
    source: Option<(u8, u8)>,
) -> Result<(u8, u8), PrefabError> {
    source
        .or_else(|| {
            let &(x, y) = prefab.building.as_ref()?.object_sizes.get(&id.get())?;
            Some((u8::try_from(x).ok()?, u8::try_from(y).ok()?))
        })
        .ok_or(PrefabError::MissingFootprint { object: id, kind })
}

impl PlacementReview {
    pub(super) fn new(
        patch: MapPatch,
        revision: u64,
        generation: u64,
        name: String,
        input: PlacementInput,
    ) -> Self {
        Self {
            patch,
            revision,
            generation,
            name,
            input,
        }
    }

    fn is_current(&self, current: &Current<'_>) -> bool {
        self.revision == current.revision
            && self.generation == current.generation
            && current.armed == self.input.gesture.as_ref()
            && current.prefab == Some(&self.input.prefab)
            && current.plane == self.input.plane
            && self
                .input
                .definitions
                .iter()
                .all(|(&(id, kind), &size)| (current.size_of)(id, kind) == size)
    }

    fn approve(self, current: &Current<'_>) -> Result<(MapPatch, String), String> {
        if !self.is_current(current) {
            return Err("Placement inputs changed. Preview and review replacement again.".into());
        }
        Ok((self.patch, self.name))
    }
}

fn current<'a>(
    app: &'a EditorApp,
    input: &PlacementInput,
    size_of: &'a dyn Fn(ObjectId, LocKind) -> Option<(u8, u8)>,
) -> Current<'a> {
    let armed = app.prefabs.armed.as_ref();
    Current {
        revision: app.session.revision(),
        generation: app.project.generation(),
        armed,
        prefab: app
            .prefabs
            .entry(&input.armed.file)
            .map(|entry| &entry.prefab),
        plane: app.interaction.view_plane(),
        size_of,
    }
}

pub(in crate::app) fn show(app: &mut EditorApp, ctx: &egui::Context) {
    let Some(review) = &app.prefabs.placement_review else {
        return;
    };
    let ready = review.is_current(&current(app, &review.input, &|id, _| {
        app.placer.known_object_size(id.get())
    }));
    let mut apply = false;
    let mut cancel = false;
    egui::Modal::new(egui::Id::new("building-replacement-review")).show(ctx, |ui| {
        ui.heading("Review building replacement");
        ui.label(format!("{} at {},{} · plane {}", review.name, review.input.origin.x, review.input.origin.y, review.input.plane));
        ui.label("Only these conflicting placements will be removed. Foundation and floor changes are included in the same undo step.");
        if !ready { ui.label("Placement inputs changed. Preview and review replacement again."); }
        egui::ScrollArea::vertical().max_height(240.0).show(ui, |ui| {
            for op in review.patch.ops() {
                if let MapOp::RemoveObject { placement: p } = op {
                    ui.label(format!("{} · type {} · {},{} · plane {}", p.object_id.get(), p.kind.get(), p.position.tile.x, p.position.tile.y, p.position.plane.index().get()));
                }
            }
        });
        ui.horizontal(|ui| {
            apply = ui.add_enabled(ready, egui::Button::new("Replace reviewed conflicts")).clicked();
            cancel = ui.button("Cancel").clicked();
        });
    });
    if apply && let Some(review) = app.prefabs.placement_review.take() {
        let size_of = |id: ObjectId, _| app.placer.known_object_size(id.get());
        let context = current(app, &review.input, &size_of);
        match review.approve(&context) {
            Ok((patch, name)) => {
                crate::app::editing::commit_map_patch(app, patch, &format!("Placed {name}"));
            }
            Err(error) => app.prefabs.say(error),
        }
    } else if cancel {
        app.prefabs.placement_review = None;
    }
}

#[cfg(test)]
mod tests;
